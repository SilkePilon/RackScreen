//! A Bambu Lab printer over MQTT: its own broker (port 8883, TLS, user
//! `bblp`, the access code as the password), which needs LAN only mode with
//! Developer Mode on, or Bambu's cloud broker with an account login (see
//! `bambu_cloud`), which needs neither.
//!
//! The printer reports on `device/<serial>/report`: one full state after a
//! `pushall` request, partial updates after that (the P1 and A1 series send
//! only what changed), so the reports are merged before they are read.
//!
//! The printer's certificate is signed by Bambu's own CA for the printer's
//! serial, not for its address, so on the LAN it is accepted without any
//! check: keep the printer on a LAN you trust, as the access code crosses
//! it. The cloud broker's certificate is checked like any other.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use rackscreen_core::event::{Event, PrintStage, PrinterReport, PrinterStatus};
use rackscreen_core::theme::Color;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::SourceCtx;

pub const PORT: u16 = 8883;
pub const USER: &str = "bblp";
const KEEPALIVE_SECS: u16 = 30;
const PING_EVERY: Duration = Duration::from_secs(20);
/// No packet at all (not even a ping response) for this long: the link is dead.
const SILENT_LIMIT: Duration = Duration::from_secs(60);
/// Ask for the full state again now and then, in case a delta was missed.
const PUSHALL_EVERY: Duration = Duration::from_secs(600);
/// A full report is a few kilobytes; refuse anything absurd.
const MAX_PACKET: usize = 1 << 20;

#[derive(Clone, Debug)]
pub struct BambuConfig {
    pub host: String,
    /// `PORT` on every printer; tests point it elsewhere.
    pub port: u16,
    /// The broker user, `bblp` on every printer.
    pub user: String,
    pub serial: String,
    pub access_code: String,
    pub tls: Tls,
    /// Cloud mode: the login's token file. Each connection reads it for
    /// the user and password, which replace `user` and `access_code`.
    pub token_file: Option<PathBuf>,
}

impl BambuConfig {
    /// The printer's own broker.
    pub fn lan(host: &str, serial: &str, access_code: &str) -> Self {
        Self {
            host: host.trim().to_string(),
            port: PORT,
            user: USER.to_string(),
            serial: serial.trim().to_string(),
            access_code: access_code.trim().to_string(),
            tls: Tls::AnyCert,
            token_file: None,
        }
    }

    /// Bambu's broker, with the login in `token_file`; the login's region
    /// picks the broker.
    pub fn cloud(serial: &str, token_file: PathBuf) -> Self {
        Self {
            host: String::new(),
            port: PORT,
            user: String::new(),
            serial: serial.trim().to_string(),
            access_code: String::new(),
            tls: Tls::Verified,
            token_file: Some(token_file),
        }
    }
}

/// How the broker's certificate is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tls {
    /// The printer's own: accepted as it is (see `AnyCert`).
    AnyCert,
    /// Against the system's roots and the host name.
    Verified,
}

/// How often a cloud source without a usable login looks for one, and a
/// connected one checks that its login is still the current one.
const LOGIN_RECHECK: Duration = Duration::from_secs(5);

/// Where and as whom one connection logs in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Creds {
    pub host: String,
    pub user: String,
    pub pass: String,
}

// ---- MQTT 3.1.1, the four packets this needs -------------------------------

fn put_len(out: &mut Vec<u8>, mut n: usize) {
    loop {
        let mut b = (n % 128) as u8;
        n /= 128;
        if n > 0 {
            b |= 0x80;
        }
        out.push(b);
        if n == 0 {
            break;
        }
    }
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(&(s.len() as u16).to_be_bytes());
    out.extend_from_slice(s.as_bytes());
}

fn packet(first: u8, body: Vec<u8>) -> Vec<u8> {
    let mut out = vec![first];
    put_len(&mut out, body.len());
    out.extend(body);
    out
}

pub fn connect_packet(client_id: &str, user: &str, pass: &str, keepalive: u16) -> Vec<u8> {
    let mut b = Vec::new();
    put_str(&mut b, "MQTT");
    b.push(4); // 3.1.1
    b.push(0b1100_0010); // username, password, clean session
    b.extend_from_slice(&keepalive.to_be_bytes());
    put_str(&mut b, client_id);
    put_str(&mut b, user);
    put_str(&mut b, pass);
    packet(0x10, b)
}

pub fn subscribe_packet(id: u16, topic: &str) -> Vec<u8> {
    let mut b = id.to_be_bytes().to_vec();
    put_str(&mut b, topic);
    b.push(0); // QoS 0
    packet(0x82, b)
}

pub fn publish_packet(topic: &str, payload: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    put_str(&mut b, topic);
    b.extend_from_slice(payload);
    packet(0x30, b)
}

const PINGREQ: [u8; 2] = [0xC0, 0x00];

/// One packet: its first byte and body.
pub async fn read_packet<R: AsyncRead + Unpin>(r: &mut R) -> Result<(u8, Vec<u8>)> {
    let first = r.read_u8().await.context("read packet")?;
    let mut len = 0usize;
    let mut shift = 0;
    loop {
        let b = r.read_u8().await.context("read length")?;
        len |= ((b & 0x7f) as usize) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift > 21 {
            bail!("bad packet length");
        }
    }
    if len > MAX_PACKET {
        bail!("packet of {len} bytes is too large");
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).await.context("read body")?;
    Ok((first, body))
}

/// The payload of a PUBLISH body (QoS 0 or with a packet id).
pub fn publish_payload(first: u8, body: &[u8]) -> Option<&[u8]> {
    let tl = u16::from_be_bytes([*body.first()?, *body.get(1)?]) as usize;
    let mut at = 2 + tl;
    if (first >> 1) & 3 > 0 {
        at += 2;
    }
    body.get(at..)
}

// ---- the report ------------------------------------------------------------

/// Fold a (partial) report into the merged one: objects merge key by key,
/// anything else replaces.
pub fn merge(into: &mut Value, delta: Value) {
    match (into, delta) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                match a.get_mut(&k) {
                    Some(slot) => merge(slot, v),
                    None => {
                        a.insert(k, v);
                    }
                }
            }
        }
        (slot, v) => *slot = v,
    }
}

fn num(v: &Value, key: &str) -> Option<f64> {
    match v.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// `FF6A13FF` (RRGGBBAA) as a colour; fully transparent or empty is none.
pub fn tray_color(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.len() < 6 || !s.is_char_boundary(6) {
        return None;
    }
    let rgb = u32::from_str_radix(&s[..6], 16).ok()?;
    if s.len() >= 8 && s.get(6..8) == Some("00") {
        return None;
    }
    Some(Color::hex(rgb))
}

fn tray(t: &Value) -> Option<(Color, String)> {
    let color = tray_color(t.get("tray_color")?.as_str()?)?;
    let kind = t
        .get("tray_type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((color, kind))
}

/// The loaded filament: the AMS tray in `tray_now` (`254` is the external
/// spool, `255` none), else the external spool when there is no AMS.
fn filament(print: &Value) -> Option<(Color, String)> {
    let ams = print.get("ams");
    let now = ams
        .and_then(|a| a.get("tray_now"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<u32>().ok());
    match now {
        Some(254) | None => print.get("vt_tray").and_then(tray),
        Some(255) => None,
        Some(i) => {
            let (unit, slot) = ((i / 4).to_string(), (i % 4).to_string());
            ams?.get("ams")?
                .as_array()?
                .iter()
                .find(|u| u.get("id").and_then(Value::as_str) == Some(unit.as_str()))?
                .get("tray")?
                .as_array()?
                .iter()
                .find(|t| t.get("id").and_then(Value::as_str) == Some(slot.as_str()))
                .and_then(tray)
        }
    }
}

pub fn stage(gcode_state: &str) -> PrintStage {
    match gcode_state {
        "PREPARE" | "SLICING" => PrintStage::Preparing,
        "RUNNING" => PrintStage::Printing,
        "PAUSE" => PrintStage::Paused,
        "FINISH" => PrintStage::Finished,
        "FAILED" => PrintStage::Failed,
        _ => PrintStage::Idle,
    }
}

/// The screen's view of the merged `{"print": {...}}` object; `None` until
/// the printer has said what it is doing.
pub fn to_report(merged: &Value) -> Option<PrinterReport> {
    let p = merged.get("print")?;
    let state = p.get("gcode_state")?.as_str()?;
    let layer = |k| num(p, k).filter(|v| *v >= 0.0).map(|v| v as u32);
    Some(PrinterReport {
        stage: stage(state),
        percent: num(p, "mc_percent").unwrap_or(0.0).clamp(0.0, 100.0) as f32,
        remaining_mins: num(p, "mc_remaining_time")
            .filter(|v| *v >= 0.0)
            .map(|v| v as u32),
        layer: layer("layer_num"),
        total_layers: layer("total_layer_num"),
        // whole degrees: the tenths change every report and would redraw
        nozzle_c: num(p, "nozzle_temper").map(|v| v.round() as f32),
        bed_c: num(p, "bed_temper").map(|v| v.round() as f32),
        job: p
            .get("subtask_name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim_end_matches(".gcode.3mf")
            .trim_end_matches(".3mf")
            .to_string(),
        filament: filament(p),
    })
}

// ---- TLS -------------------------------------------------------------------

/// Accepts the printer's certificate as it is. It is signed by Bambu's own
/// CA for the serial, not for the address, and some are old X.509 v1
/// certificates webpki cannot read; checking only the handshake signature
/// would add nothing once any certificate is accepted.
#[derive(Debug)]
struct AnyCert(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }
    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn tls_connector(tls: Tls) -> Result<tokio_rustls::TlsConnector> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default versions");
    let config = match tls {
        Tls::AnyCert => builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCert(provider))),
        Tls::Verified => {
            let v = rustls_platform_verifier::Verifier::new(provider)
                .context("load the system's certificate roots")?;
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(v))
        }
    }
    .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

// ---- the session -----------------------------------------------------------

enum Ended {
    Shutdown,
    Refused,
    /// Cloud mode: the token file changed (a new login, a log out).
    LoginChanged,
    Lost(anyhow::Error),
}

async fn send<W: AsyncWrite + Unpin>(w: &mut W, bytes: &[u8]) -> Result<()> {
    w.write_all(bytes).await.context("write")?;
    w.flush().await.context("flush")
}

fn pushall() -> Vec<u8> {
    br#"{"pushing":{"sequence_id":"0","command":"pushall","version":1,"push_target":1}}"#.to_vec()
}

async fn session(cfg: &BambuConfig, creds: &Creds, ctx: &SourceCtx, connected: &mut bool) -> Ended {
    match session_inner(cfg, creds, ctx, connected).await {
        Ok(e) => e,
        Err(e) => Ended::Lost(e),
    }
}

async fn session_inner(
    cfg: &BambuConfig,
    creds: &Creds,
    ctx: &SourceCtx,
    connected: &mut bool,
) -> Result<Ended> {
    let host = creds.host.trim();
    let tcp = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect((host, cfg.port)),
    )
    .await
    .context("connect timed out")?
    .with_context(|| format!("connect {host}:{}", cfg.port))?;
    let name = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => rustls::pki_types::ServerName::IpAddress(ip.into()),
        Err(_) => {
            rustls::pki_types::ServerName::try_from(host.to_string()).context("printer host")?
        }
    };
    let tls = tokio::time::timeout(
        Duration::from_secs(10),
        tls_connector(cfg.tls)?.connect(name, tcp),
    )
    .await
    .context("TLS handshake timed out")?
    .context("TLS handshake")?;
    let (mut rd, mut wr) = tokio::io::split(tls);
    let id = format!("rackscreen-{:08x}", fastrand::u32(..));
    send(
        &mut wr,
        &connect_packet(&id, &creds.user, &creds.pass, KEEPALIVE_SECS),
    )
    .await?;
    let (first, body) = tokio::time::timeout(Duration::from_secs(10), read_packet(&mut rd))
        .await
        .context("no CONNACK")??;
    if first != 0x20 || body.len() < 2 {
        bail!("expected CONNACK, got packet {first:#04x}");
    }
    match body[1] {
        0 => {}
        4 | 5 => return Ok(Ended::Refused),
        rc => bail!("broker refused the connection (code {rc})"),
    }
    let serial = cfg.serial.trim();
    send(
        &mut wr,
        &subscribe_packet(1, &format!("device/{serial}/report")),
    )
    .await?;
    let request = format!("device/{serial}/request");
    send(&mut wr, &publish_packet(&request, &pushall())).await?;
    tracing::info!("bambu: connected to {host} for {serial}");
    *connected = true;

    // Reading runs on its own task so a half-read packet is never dropped
    // by the select below.
    let (ptx, mut prx) = tokio::sync::mpsc::channel::<Result<(u8, Vec<u8>)>>(16);
    let reader = tokio::spawn(async move {
        loop {
            let p = read_packet(&mut rd).await;
            let stop = p.is_err();
            if ptx.send(p).await.is_err() || stop {
                return;
            }
        }
    });
    let _abort = AbortOnDrop(reader);

    let mut merged = Value::Object(Default::default());
    let mut last: Option<PrinterReport> = None;
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    let mut resync = tokio::time::interval(PUSHALL_EVERY);
    resync.tick().await;
    let mut recheck = tokio::time::interval(LOGIN_RECHECK);
    recheck.tick().await;
    loop {
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return Ok(Ended::Shutdown),
            _ = ping.tick() => send(&mut wr, &PINGREQ).await?,
            _ = recheck.tick(), if cfg.token_file.is_some() => {
                if credentials(cfg, unix_now()).as_ref() != Some(creds) {
                    return Ok(Ended::LoginChanged);
                }
            }
            _ = resync.tick() => send(&mut wr, &publish_packet(&request, &pushall())).await?,
            p = tokio::time::timeout(SILENT_LIMIT, prx.recv()) => {
                let (first, body) = match p {
                    Err(_) => bail!("printer went quiet"),
                    Ok(None) => bail!("connection closed"),
                    Ok(Some(p)) => p?,
                };
                if first >> 4 != 3 {
                    continue; // SUBACK, PINGRESP
                }
                let Some(payload) = publish_payload(first, &body) else { continue };
                let Ok(delta) = serde_json::from_slice::<Value>(payload) else {
                    tracing::debug!("bambu: report is not JSON");
                    continue;
                };
                merge(&mut merged, delta);
                let report = to_report(&merged);
                if report.is_some() && report != last {
                    ctx.emit(Event::Printer { status: PrinterStatus::Ok, report: report.clone() });
                    last = report;
                }
            }
        }
    }
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The broker, user and password for the next connection: the config's on
/// the LAN; in cloud mode the token file's, with the broker of the region
/// the login was made in (`None` while there is no usable login).
pub fn credentials(cfg: &BambuConfig, now: i64) -> Option<Creds> {
    match &cfg.token_file {
        None => Some(Creds {
            host: cfg.host.clone(),
            user: cfg.user.clone(),
            pass: cfg.access_code.clone(),
        }),
        Some(path) => crate::bambu_cloud::read_token(path)
            .filter(|t| t.usable(now))
            .map(|t| Creds {
                host: t.region.mqtt_host().to_string(),
                user: t.username,
                pass: t.token,
            }),
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub async fn run_bambu(cfg: BambuConfig, ctx: SourceCtx) {
    let cloud = cfg.token_file.is_some();
    let mut backoff = 5u64;
    // a printer that is switched off is normal: say so once, not every retry
    let mut said = false;
    let mut said_logged_out = false;
    loop {
        if ctx.shutdown.is_cancelled() {
            return;
        }
        let Some(creds) = credentials(&cfg, unix_now()) else {
            // cloud mode before a login (or after a refused one): look
            // again soon, so a login in the web UI needs no restart
            if !said_logged_out {
                tracing::warn!("bambu: no Bambu Lab login; log in on the Home & printer page");
                said_logged_out = true;
            }
            ctx.emit(Event::Printer {
                status: PrinterStatus::LoggedOut,
                report: None,
            });
            tokio::select! {
                _ = ctx.shutdown.cancelled() => return,
                _ = tokio::time::sleep(LOGIN_RECHECK) => {}
            }
            continue;
        };
        said_logged_out = false;
        let mut connected = false;
        let ended = session(&cfg, &creds, &ctx, &mut connected).await;
        if connected {
            backoff = 5;
            said = false;
        }
        let wait = match ended {
            Ended::Shutdown => return,
            Ended::LoginChanged => {
                tracing::info!("bambu: the login changed; connecting again");
                0
            }
            Ended::Refused if cloud => {
                tracing::warn!("bambu: Bambu Lab refused the login; log in again");
                let marked = cfg
                    .token_file
                    .as_deref()
                    .map(|path| crate::bambu_cloud::mark_refused(path, &creds.pass));
                match marked {
                    Some(Err(e)) => {
                        // the file still looks usable: do not hammer the broker
                        tracing::warn!("bambu: {e:#}");
                        ctx.emit(Event::Printer {
                            status: PrinterStatus::LoggedOut,
                            report: None,
                        });
                        300
                    }
                    // the next turn finds no usable login and waits for one
                    _ => 0,
                }
            }
            Ended::Refused => {
                tracing::warn!("bambu: access code refused; check it on the printer's screen");
                ctx.emit(Event::Printer {
                    status: PrinterStatus::Unauthorized,
                    report: None,
                });
                300
            }
            Ended::Lost(e) => {
                if said {
                    tracing::debug!("bambu: {e:#}");
                } else {
                    tracing::warn!("bambu: {e:#}; retrying quietly");
                    said = true;
                }
                ctx.emit(Event::Printer {
                    status: PrinterStatus::Offline,
                    report: None,
                });
                let w = backoff;
                backoff = (backoff * 2).min(60);
                w
            }
        };
        tokio::select! {
            _ = ctx.shutdown.cancelled() => return,
            _ = tokio::time::sleep(Duration::from_secs(wait)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = include_str!("../tests/fixtures/bambu-pushall.json");

    #[test]
    fn connect_packet_matches_the_spec() {
        let p = connect_packet("c", "bblp", "12345678", 30);
        assert_eq!(p[0], 0x10);
        assert_eq!(p[1] as usize, p.len() - 2);
        assert_eq!(&p[2..8], b"\x00\x04MQTT");
        assert_eq!(p[8], 4);
        assert_eq!(p[9], 0xC2);
        assert_eq!(&p[10..12], &[0, 30]);
        assert_eq!(&p[12..15], b"\x00\x01c");
        assert!(p.ends_with(b"\x00\x0812345678"));
    }

    #[test]
    fn subscribe_and_publish_packets() {
        let s = subscribe_packet(1, "device/X/report");
        assert_eq!(&s[..4], &[0x82, 20, 0, 1]);
        assert_eq!(*s.last().unwrap(), 0);
        let p = publish_packet("t", b"{}");
        assert_eq!(p, vec![0x30, 5, 0, 1, b't', b'{', b'}']);
        assert_eq!(publish_payload(0x30, &p[2..]), Some(&b"{}"[..]));
        // QoS 1 carries a packet id before the payload
        assert_eq!(
            publish_payload(0x32, &[0, 1, b't', 0, 9, b'x']),
            Some(&b"x"[..])
        );
    }

    #[test]
    fn long_lengths_use_continuation_bytes() {
        let mut out = Vec::new();
        put_len(&mut out, 321);
        assert_eq!(out, vec![0xC1, 0x02]);
    }

    #[tokio::test]
    async fn packets_read_back() {
        let body = vec![7u8; 200];
        let mut bytes = vec![0x30];
        put_len(&mut bytes, body.len());
        bytes.extend(&body);
        bytes.extend(PINGREQ);
        let mut r = &bytes[..];
        assert_eq!(read_packet(&mut r).await.unwrap(), (0x30, body));
        assert_eq!(read_packet(&mut r).await.unwrap(), (0xC0, vec![]));
        assert!(read_packet(&mut r).await.is_err(), "end of stream");
    }

    #[test]
    fn a_full_report_reads_and_deltas_merge() {
        let mut m = Value::Object(Default::default());
        merge(&mut m, serde_json::from_str(FULL).unwrap());
        let r = to_report(&m).unwrap();
        assert_eq!(r.stage, PrintStage::Printing);
        assert_eq!(r.percent, 42.0);
        assert_eq!(r.remaining_mins, Some(65));
        assert_eq!((r.layer, r.total_layers), (Some(120), Some(300)));
        assert_eq!((r.nozzle_c, r.bed_c), (Some(220.0), Some(60.0)));
        assert_eq!(r.job, "Benchy");
        assert_eq!(r.filament, Some((Color::hex(0x00AE42), "PETG".into())));

        merge(
            &mut m,
            serde_json::json!({"print": {"mc_percent": 43, "layer_num": 121, "command": "push_status"}}),
        );
        let r = to_report(&m).unwrap();
        assert_eq!((r.percent, r.layer), (43.0, Some(121)));
        assert_eq!(r.job, "Benchy", "untouched keys stay");

        merge(
            &mut m,
            serde_json::json!({"print": {"gcode_state": "FINISH"}}),
        );
        assert_eq!(to_report(&m).unwrap().stage, PrintStage::Finished);
    }

    #[test]
    fn no_state_no_report() {
        let v = serde_json::json!({"print": {"mc_percent": 3}});
        assert_eq!(to_report(&v), None);
        assert_eq!(to_report(&serde_json::json!({"info": {}})), None);
    }

    #[test]
    fn filament_from_the_external_spool_and_none() {
        let v = serde_json::json!({"print": {
            "gcode_state": "IDLE",
            "ams": {"tray_now": "254", "ams": []},
            "vt_tray": {"tray_color": "FFFFFFFF", "tray_type": "PLA"}
        }});
        assert_eq!(
            to_report(&v).unwrap().filament,
            Some((Color::hex(0xFFFFFF), "PLA".into()))
        );
        let none =
            serde_json::json!({"print": {"gcode_state": "IDLE", "ams": {"tray_now": "255"}}});
        assert_eq!(to_report(&none).unwrap().filament, None);
        assert_eq!(tray_color("00000000"), None, "transparent: empty slot");
        assert_eq!(tray_color(""), None);
        assert_eq!(tray_color("ZZ"), None);
    }

    /// The whole client against a public broker: TLS, CONNECT, SUBSCRIBE,
    /// and a report published from a second connection arriving as an
    /// event. `cargo test -p rackscreen-sources -- --ignored live_broker`
    #[tokio::test]
    #[ignore = "needs broker.emqx.io"]
    async fn live_broker() {
        let serial = format!("RS{:08X}", fastrand::u32(..));
        let cfg = BambuConfig {
            host: "broker.emqx.io".into(),
            port: 8883,
            user: "emqx".into(),
            serial: serial.clone(),
            access_code: "public".into(),
            tls: Tls::Verified,
            token_file: None,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = SourceCtx {
            tx,
            shutdown: tokio_util::sync::CancellationToken::new(),
        };
        let task = tokio::spawn(run_bambu(cfg, ctx.clone()));
        tokio::time::sleep(Duration::from_secs(4)).await;
        if let Ok(e) = rx.try_recv() {
            panic!("the source should be connected and quiet, got {e:?}");
        }
        // a second client plays the printer
        let tcp = tokio::net::TcpStream::connect(("broker.emqx.io", 8883))
            .await
            .unwrap();
        let name = rustls::pki_types::ServerName::try_from("broker.emqx.io").unwrap();
        let mut tls = tls_connector(Tls::Verified)
            .unwrap()
            .connect(name, tcp)
            .await
            .unwrap();
        send(
            &mut tls,
            &connect_packet("rs-printer", "emqx", "public", 30),
        )
        .await
        .unwrap();
        let (first, body) = read_packet(&mut tls).await.unwrap();
        assert_eq!((first, body[1]), (0x20, 0));
        let report = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/bambu-pushall.json"
        ))
        .unwrap();
        send(
            &mut tls,
            &publish_packet(&format!("device/{serial}/report"), &report),
        )
        .await
        .unwrap();
        let got = tokio::task::spawn_blocking(move || rx.recv_timeout(Duration::from_secs(10)))
            .await
            .unwrap()
            .expect("an event");
        ctx.shutdown.cancel();
        task.await.unwrap();
        match got {
            Event::Printer {
                status: PrinterStatus::Ok,
                report: Some(r),
            } => {
                assert_eq!(r.percent, 42.0);
                assert_eq!(r.job, "Benchy");
            }
            e => panic!("{e:?}"),
        }
    }

    #[test]
    fn credentials_by_mode() {
        let lan = BambuConfig::lan(" 192.168.1.5 ", "S1", "12345678");
        assert_eq!((lan.host.as_str(), lan.tls), ("192.168.1.5", Tls::AnyCert));
        let c = credentials(&lan, 0).unwrap();
        assert_eq!(
            (c.host.as_str(), c.user.as_str(), c.pass.as_str()),
            ("192.168.1.5", "bblp", "12345678")
        );

        let dir = tempfile::tempdir().unwrap();
        let path = crate::bambu_cloud::token_path_in(dir.path());
        let cloud = BambuConfig::cloud("S1", path.clone());
        assert_eq!(cloud.tls, Tls::Verified);
        assert_eq!(credentials(&cloud, 0), None, "no login yet");
        let tok = crate::bambu_cloud::TokenFile {
            token: "tok".into(),
            username: "u_9".into(),
            account: "a@b".into(),
            region: crate::bambu_cloud::Region::China,
            expires_at: 100,
            error: None,
        };
        crate::bambu_cloud::write_token(&path, &tok, None).unwrap();
        assert_eq!(
            credentials(&cloud, 50),
            Some(Creds {
                host: "cn.mqtt.bambulab.com".into(),
                user: "u_9".into(),
                pass: "tok".into()
            }),
            "the login's region picks the broker"
        );
        assert_eq!(credentials(&cloud, 100), None, "expired");
        crate::bambu_cloud::mark_refused(&path, "an older token").unwrap();
        assert!(
            credentials(&cloud, 50).is_some(),
            "a newer login is left alone"
        );
        crate::bambu_cloud::mark_refused(&path, "tok").unwrap();
        assert_eq!(credentials(&cloud, 50), None, "refused");
    }

    #[tokio::test]
    async fn verified_tls_loads_the_system_roots() {
        assert!(tls_connector(Tls::Verified).is_ok());
    }

    #[test]
    fn stages() {
        assert_eq!(stage("PREPARE"), PrintStage::Preparing);
        assert_eq!(stage("PAUSE"), PrintStage::Paused);
        assert_eq!(stage("FAILED"), PrintStage::Failed);
        assert_eq!(stage("IDLE"), PrintStage::Idle);
        assert_eq!(stage("whatever"), PrintStage::Idle);
    }
}
