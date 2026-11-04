//! `/api/tailscale`: Tailscale on the Pi, managed from the web UI.
//!
//! Everything here runs as the service user. The one root step (install,
//! start `tailscaled`, make the service user its operator) is
//! `sudo -n /usr/local/bin/rackscreen tailscale-setup`, allowed by the
//! sudoers drop-in. After that the `tailscale` CLI accepts `up`, `down`,
//! `login`, `logout`, `set` and `serve` from the operator without root.
//!
//! Commands never go through a shell: fixed argument lists, with the few
//! user-provided values (device name) validated and passed as their own
//! arguments. The auth key never goes on a command line (where `ps` would
//! show it): it is handed over in a private file, `--auth-key=file:<path>`.
//! Anything that looks like a key is scrubbed from every message and log.

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};

use crate::update::{JobState, BINARY, SUDO_CHECK_EVERY};
use crate::{ApiError, ApiResult, AppState};

pub const TIMEOUT: Duration = Duration::from_secs(10);
/// `up`, `down`, `logout` and `serve` may talk to the coordination server.
pub const SLOW_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a browser login may take to offer its link.
pub const AUTH_URL_WAIT: Duration = Duration::from_secs(10);
/// How long a browser login's `tailscale up` may wait for the user before
/// it is killed.
pub const LOGIN_TTL: Duration = Duration::from_secs(5 * 60);

/// Why every Tailscale action is refused while `web.password` is empty.
pub const NEEDS_PASSWORD: &str =
    "Set a web password first (System → Web access): Tailscale can give remote access to this Pi.";

/// The first release whose `--auth-key` reads `file:<path>`. Older ones
/// (long out of support) get the key on the command line, with a warning.
pub const AUTH_KEY_FILE_SINCE: (u32, u32) = (1, 32);

pub const NEEDS_SETUP: &str =
    "RackScreen may not manage Tailscale yet: finish the setup first (it makes the service user Tailscale's operator)";

// ---------------------------------------------------------------------------
// Running commands.

/// What a command left behind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// The exit code; None when it was killed (timeout) or died of a signal.
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Output {
    pub fn ok(stdout: &str) -> Output {
        Output {
            status: Some(0),
            stdout: stdout.into(),
            ..Output::default()
        }
    }
    pub fn fail(stderr: &str) -> Output {
        Output {
            status: Some(1),
            stderr: stderr.into(),
            ..Output::default()
        }
    }
    pub fn success(&self) -> bool {
        self.status == Some(0) && !self.timed_out
    }
    fn text(&self) -> String {
        format!("{}\n{}", self.stderr, self.stdout)
    }
}

/// A command started in the background (the browser login). Dropping it
/// kills the command.
pub struct Background {
    /// Everything it printed so far, stdout and stderr together.
    pub output: Arc<Mutex<String>>,
    /// Set once it exited: its exit code (None: killed by a signal).
    pub exited: Arc<Mutex<Option<Option<i32>>>>,
    _kill: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Background {
    /// A handle with fixed output (tests). Dropping it closes `kill`'s
    /// receiver, so a test can tell whether it was let go.
    pub fn fixed(
        output: &str,
        exited: Option<Option<i32>>,
        kill: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> Background {
        Background {
            output: Arc::new(Mutex::new(output.into())),
            exited: Arc::new(Mutex::new(exited)),
            _kill: kill,
        }
    }
}

pub type LineSink = Arc<dyn Fn(String) + Send + Sync>;

/// How commands are run: the real processes, or a fake in tests.
pub trait Runner: Send + Sync {
    /// Run to the end or `timeout`. `Err` only when it cannot start
    /// (`NotFound`: no such program).
    fn run(
        &self,
        prog: &str,
        args: &[String],
        timeout: Duration,
    ) -> BoxFuture<'static, io::Result<Output>>;
    /// Run to the end, every output line to `line`; true on exit 0.
    fn stream(
        &self,
        prog: &str,
        args: &[String],
        line: LineSink,
    ) -> BoxFuture<'static, io::Result<bool>>;
    /// Start in the background (see `Background`).
    fn spawn(&self, prog: &str, args: &[String]) -> io::Result<Background>;
}

pub struct RealRunner;

/// Hand every line of `r` to `f`, without the line ending; bytes that are
/// not UTF-8 become U+FFFD instead of ending the read and stalling the pipe.
async fn lossy_lines(r: impl AsyncRead + Unpin, mut f: impl FnMut(String)) {
    let mut r = BufReader::new(r);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match r.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if buf.ends_with(b"\n") {
                    buf.pop();
                    if buf.ends_with(b"\r") {
                        buf.pop();
                    }
                }
                f(String::from_utf8_lossy(&buf).into_owned());
            }
        }
    }
}

fn command(prog: &str, args: &[String]) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(prog);
    c.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    c
}

impl Runner for RealRunner {
    fn run(
        &self,
        prog: &str,
        args: &[String],
        timeout: Duration,
    ) -> BoxFuture<'static, io::Result<Output>> {
        let mut cmd = command(prog, args);
        Box::pin(async move {
            let mut child = cmd.spawn()?;
            let mut so = child.stdout.take().expect("piped");
            let mut se = child.stderr.take().expect("piped");
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let done = tokio::time::timeout(timeout, async {
                let (_, _, status) = tokio::join!(
                    so.read_to_end(&mut out),
                    se.read_to_end(&mut err),
                    child.wait()
                );
                status
            })
            .await;
            let (status, timed_out) = match done {
                Ok(s) => (s?.code(), false),
                Err(_) => {
                    let _ = child.kill().await;
                    (None, true)
                }
            };
            Ok(Output {
                status,
                stdout: String::from_utf8_lossy(&out).into_owned(),
                stderr: String::from_utf8_lossy(&err).into_owned(),
                timed_out,
            })
        })
    }

    fn stream(
        &self,
        prog: &str,
        args: &[String],
        line: LineSink,
    ) -> BoxFuture<'static, io::Result<bool>> {
        let mut cmd = command(prog, args);
        Box::pin(async move {
            let mut child = cmd.spawn()?;
            let so = child.stdout.take().expect("piped");
            let se = child.stderr.take().expect("piped");
            let l2 = line.clone();
            let out = tokio::spawn(lossy_lines(so, move |l| line(l)));
            let err = tokio::spawn(lossy_lines(se, move |l| l2(l)));
            let status = child.wait().await?;
            let _ = out.await;
            let _ = err.await;
            Ok(status.success())
        })
    }

    fn spawn(&self, prog: &str, args: &[String]) -> io::Result<Background> {
        let mut child = command(prog, args).spawn()?;
        let output = Arc::new(Mutex::new(String::new()));
        let exited = Arc::new(Mutex::new(None));
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        for pipe in [
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn tokio::io::AsyncRead + Send + Unpin>),
        ]
        .into_iter()
        .flatten()
        {
            let output = output.clone();
            tokio::spawn(lossy_lines(pipe, move |l| {
                let mut o = output.lock().unwrap();
                if o.len() < 16 * 1024 {
                    o.push_str(&l);
                    o.push('\n');
                }
            }));
        }
        let done = exited.clone();
        tokio::spawn(async move {
            tokio::select! {
                status = child.wait() => {
                    *done.lock().unwrap() = Some(status.ok().and_then(|s| s.code()));
                }
                // the handle was dropped (or a new login replaced it)
                _ = rx => {
                    let _ = child.kill().await;
                    *done.lock().unwrap() = Some(None);
                }
            }
        });
        Ok(Background {
            output,
            exited,
            _kill: Some(tx),
        })
    }
}

// ---------------------------------------------------------------------------
// State.

#[derive(Clone, Debug, Default, Serialize)]
pub struct SetupJob {
    pub state: JobState,
    pub log: Vec<String>,
}

pub struct TsState {
    pub runner: Arc<dyn Runner>,
    /// The user this process runs as (who must be Tailscale's operator).
    pub user: String,
    /// `web.bind`: `tailscale serve` can only proxy to 127.0.0.1.
    pub bind: Mutex<String>,
    can_setup: Mutex<Option<(Instant, bool)>>,
    pub job: Arc<Mutex<SetupJob>>,
    /// The `tailscale up` of a browser login that is waiting for the user,
    /// tagged with the generation that started it.
    login: Mutex<Option<(u64, Background)>>,
    login_gen: AtomicU64,
    /// One login at a time, from the first check to the answer.
    login_lock: tokio::sync::Mutex<()>,
    /// How long a waiting browser login lives (`LOGIN_TTL`; tests shorten it).
    pub login_ttl: Mutex<Duration>,
}

impl TsState {
    pub fn new(runner: Arc<dyn Runner>, user: String) -> TsState {
        TsState {
            runner,
            user,
            bind: Mutex::new("0.0.0.0".into()),
            can_setup: Mutex::new(None),
            job: Arc::default(),
            login: Mutex::new(None),
            login_gen: AtomicU64::new(0),
            login_lock: tokio::sync::Mutex::new(()),
            login_ttl: Mutex::new(LOGIN_TTL),
        }
    }

    /// Keep `bg` as the waiting login (killing any older one); its generation.
    pub fn store_login(&self, bg: Background) -> u64 {
        let generation = self.login_gen.fetch_add(1, Ordering::Relaxed) + 1;
        let old = self.login.lock().unwrap().replace((generation, bg));
        drop(old);
        generation
    }

    /// Kill the waiting login if it is still the one of `generation`;
    /// whether it was.
    pub fn drop_login(&self, generation: u64) -> bool {
        let mut slot = self.login.lock().unwrap();
        if slot.as_ref().is_some_and(|(g, _)| *g == generation) {
            let bg = slot.take();
            drop(slot);
            drop(bg);
            true
        } else {
            false
        }
    }

    /// Kill whichever login is waiting.
    pub fn drop_any_login(&self) {
        let bg = self.login.lock().unwrap().take();
        drop(bg);
    }

    pub fn real() -> TsState {
        let user = nix::unistd::User::from_uid(nix::unistd::geteuid())
            .ok()
            .flatten()
            .map(|u| u.name)
            .or_else(|| std::env::var("USER").ok())
            .unwrap_or_default();
        TsState::new(Arc::new(RealRunner), user)
    }

    async fn ts(&self, args: &[&str], timeout: Duration) -> io::Result<Output> {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        self.runner.run("tailscale", &args, timeout).await
    }

    /// `sudo -n -l <binary> tailscale-setup` succeeds when the sudoers
    /// drop-in allows the one-click setup (checked at most once a minute).
    async fn can_setup(&self) -> bool {
        if let Some((t, ok)) = *self.can_setup.lock().unwrap() {
            if t.elapsed() < SUDO_CHECK_EVERY {
                return ok;
            }
        }
        let args = ["-n", "-l", BINARY, "tailscale-setup"].map(String::from);
        let ok = self
            .runner
            .run("sudo", &args, TIMEOUT)
            .await
            .is_ok_and(|o| o.success());
        *self.can_setup.lock().unwrap() = Some((Instant::now(), ok));
        ok
    }
}

// ---------------------------------------------------------------------------
// Parsing `tailscale status --json`, `debug prefs` and `serve status --json`.

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawStatus {
    #[serde(rename = "BackendState")]
    pub backend_state: String,
    #[serde(rename = "AuthURL")]
    pub auth_url: String,
    #[serde(rename = "Self")]
    pub self_: Option<RawPeer>,
    #[serde(rename = "Health")]
    pub health: Option<Vec<String>>,
    #[serde(rename = "CurrentTailnet")]
    pub current_tailnet: Option<RawTailnet>,
    #[serde(rename = "Peer")]
    pub peer: Option<HashMap<String, RawPeer>>,
    #[serde(rename = "User")]
    pub user: Option<HashMap<String, RawUser>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawPeer {
    #[serde(rename = "HostName")]
    pub host_name: String,
    #[serde(rename = "DNSName")]
    pub dns_name: String,
    #[serde(rename = "OS")]
    pub os: String,
    #[serde(rename = "UserID")]
    pub user_id: i64,
    #[serde(rename = "TailscaleIPs")]
    pub ips: Option<Vec<String>>,
    #[serde(rename = "Online")]
    pub online: bool,
    #[serde(rename = "LastSeen")]
    pub last_seen: Option<String>,
    #[serde(rename = "ExitNode")]
    pub exit_node: bool,
    #[serde(rename = "ExitNodeOption")]
    pub exit_node_option: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawTailnet {
    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawUser {
    #[serde(rename = "LoginName")]
    pub login_name: String,
    #[serde(rename = "DisplayName")]
    pub display_name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct RawPrefs {
    #[serde(rename = "RunSSH")]
    pub run_ssh: bool,
    #[serde(rename = "OperatorUser")]
    pub operator_user: String,
    #[serde(rename = "Hostname")]
    pub hostname: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawServe {
    #[serde(rename = "TCP")]
    tcp: Option<HashMap<String, RawTcp>>,
    #[serde(rename = "Web")]
    web: Option<HashMap<String, RawWeb>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawTcp {
    #[serde(rename = "TCPForward")]
    tcp_forward: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawWeb {
    #[serde(rename = "Handlers")]
    handlers: Option<HashMap<String, RawHandler>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RawHandler {
    #[serde(rename = "Proxy")]
    proxy: String,
    #[serde(rename = "Path")]
    path: String,
    #[serde(rename = "Text")]
    text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Device {
    pub hostname: String,
    /// The MagicDNS name, without the trailing dot ("" when there is none).
    pub dns_name: String,
    pub ips: Vec<String>,
    pub os: String,
    pub online: bool,
    /// RFC 3339; None when never seen or online now.
    pub last_seen: Option<String>,
    /// It offers itself as an exit node.
    pub exit_node: bool,
}

impl Device {
    fn from_raw(p: &RawPeer) -> Device {
        Device {
            hostname: p.host_name.clone(),
            dns_name: p.dns_name.trim_end_matches('.').to_string(),
            ips: p.ips.clone().unwrap_or_default(),
            os: p.os.clone(),
            online: p.online,
            last_seen: p
                .last_seen
                .clone()
                .filter(|t| !t.is_empty() && !t.starts_with("0001-")),
            exit_node: p.exit_node || p.exit_node_option,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UserView {
    pub login: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ServeView {
    pub on: bool,
    pub url: Option<String>,
}

/// What the page shows, taken from `tailscale status --json`.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct StatusView {
    pub backend_state: Option<String>,
    pub auth_url: Option<String>,
    #[serde(rename = "self")]
    pub self_: Option<Device>,
    pub tailnet: Option<String>,
    pub user: Option<UserView>,
    pub peers: Vec<Device>,
    pub health: Vec<String>,
}

pub fn parse_status(text: &str) -> Result<StatusView, serde_json::Error> {
    let raw: RawStatus = serde_json::from_str(text)?;
    let nonempty = |s: &str| (!s.is_empty()).then(|| s.to_string());
    let running = raw.backend_state == "Running";
    // Before login `Self` is a placeholder without a name on the tailnet.
    let self_ = raw
        .self_
        .as_ref()
        .filter(|s| !s.dns_name.is_empty() || s.ips.as_ref().is_some_and(|i| !i.is_empty()))
        .map(Device::from_raw);
    let user = raw.self_.as_ref().and_then(|s| {
        raw.user
            .as_ref()?
            .get(&s.user_id.to_string())
            .map(|u| UserView {
                login: u.login_name.clone(),
                display_name: u.display_name.clone(),
            })
    });
    let mut peers: Vec<Device> = raw
        .peer
        .as_ref()
        .map(|p| p.values().map(Device::from_raw).collect())
        .unwrap_or_default();
    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
            .then_with(|| a.dns_name.cmp(&b.dns_name))
    });
    Ok(StatusView {
        backend_state: nonempty(&raw.backend_state),
        auth_url: nonempty(&raw.auth_url).filter(|_| !running),
        self_,
        tailnet: raw.current_tailnet.as_ref().and_then(|t| nonempty(&t.name)),
        user,
        peers,
        health: raw.health.unwrap_or_default(),
    })
}

pub fn parse_prefs(text: &str) -> Option<RawPrefs> {
    serde_json::from_str(text).ok()
}

fn proxies_to(proxy: &str, port: u16) -> bool {
    let p = proxy.trim_end_matches('/');
    p == format!("http://127.0.0.1:{port}") || p == format!("http://localhost:{port}")
}

/// Who has `https:443` in `tailscale serve`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Https443 {
    /// Nothing is served there.
    Free,
    /// Only RackScreen: `/` proxied to 127.0.0.1:<our port>, at `url`.
    Ours { url: String },
    /// Something else (alone or next to RackScreen), described for a message.
    Other(String),
}

/// Read `tailscale serve status --json` for what holds https:443.
pub fn https_443(text: &str, port: u16) -> Result<Https443, serde_json::Error> {
    if text.trim().is_empty() {
        return Ok(Https443::Free);
    }
    let raw: RawServe = serde_json::from_str(text)?;
    if let Some(fwd) = raw
        .tcp
        .as_ref()
        .and_then(|t| t.get("443"))
        .map(|t| t.tcp_forward.as_str())
        .filter(|f| !f.is_empty())
    {
        return Ok(Https443::Other(format!("a TCP forward to {fwd}")));
    }
    let mut ours = None;
    let mut webs: Vec<_> = raw.web.iter().flatten().collect();
    webs.sort_by(|a, b| a.0.cmp(b.0));
    for (hostport, web) in webs {
        let Some(host) = hostport.strip_suffix(":443") else {
            continue;
        };
        let mut handlers: Vec<_> = web.handlers.iter().flatten().collect();
        handlers.sort_by(|a, b| a.0.cmp(b.0));
        for (mount, h) in handlers {
            if mount == "/" && proxies_to(&h.proxy, port) {
                ours = Some(format!("https://{host}"));
                continue;
            }
            let what = if !h.proxy.is_empty() {
                format!("a proxy to {}", h.proxy)
            } else if !h.path.is_empty() {
                format!("the files in {}", h.path)
            } else if !h.text.is_empty() {
                "a fixed text".to_string()
            } else {
                "a handler".to_string()
            };
            return Ok(Https443::Other(format!("{what} at {mount}")));
        }
    }
    Ok(ours.map_or(Https443::Free, |url| Https443::Ours { url }))
}

/// Whether `tailscale serve` shares this UI (port `port`) over HTTPS on 443,
/// and at which address.
pub fn parse_serve(text: &str, port: u16) -> ServeView {
    let Ok(raw) = serde_json::from_str::<RawServe>(text) else {
        return ServeView::default();
    };
    let ours = |p: &str| proxies_to(p, port);
    raw.web
        .unwrap_or_default()
        .iter()
        .filter_map(|(host, web)| Some((host.strip_suffix(":443")?, web)))
        .find(|(_, web)| {
            web.handlers
                .as_ref()
                .is_some_and(|h| h.get("/").is_some_and(|h| ours(&h.proxy)))
        })
        .map(|(host, _)| ServeView {
            on: true,
            url: Some(format!("https://{host}")),
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Input and errors.

pub fn valid_hostname(h: &str) -> bool {
    (1..=63).contains(&h.len())
        && h.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn valid_auth_key(k: &str) -> bool {
    k.strip_prefix("tskey-").is_some_and(|rest| {
        !rest.is_empty()
            && rest.len() < 512
            && rest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

/// The first `https://` link in `text` that `pick` accepts.
pub fn find_url(text: &str, pick: impl Fn(&str) -> bool) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>')
        .map(|w| w.trim_end_matches(['.', ',', ')']))
        .find(|w| w.starts_with("https://") && w.len() > 8 && pick(w))
        .map(str::to_string)
}

/// `text` with every `tskey-[A-Za-z0-9_-]+` replaced by `tskey-***`: an
/// auth key must never reach a log line or the browser.
pub fn scrub_keys(text: &str) -> String {
    const PREFIX: &str = "tskey-";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(PREFIX) {
        out.push_str(&rest[..i + PREFIX.len()]);
        let after = &rest[i + PREFIX.len()..];
        let n = after
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
            .count();
        if n > 0 {
            out.push_str("***");
        }
        rest = &after[n..];
    }
    out.push_str(rest);
    out
}

/// `major.minor` of a `tailscale version` first line ("1.90.6").
pub fn parse_version(v: &str) -> Option<(u32, u32)> {
    let mut parts = v.trim().split(['.', '-']);
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Whether this `tailscale` reads `--auth-key=file:<path>`. An unknown
/// version is a current one (the official installer installs the latest).
pub fn auth_key_file_supported(version: Option<&str>) -> bool {
    version
        .and_then(parse_version)
        .is_none_or(|v| v >= AUTH_KEY_FILE_SINCE)
}

/// The auth key in a file only this user can read, in a directory only
/// this user can enter; both go when this is dropped (also on errors).
pub struct KeyFile {
    dir: tempfile::TempDir,
}

impl KeyFile {
    pub fn new(key: &str) -> io::Result<KeyFile> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let dir = tempfile::Builder::new()
            .prefix("rackscreen-tailscale-")
            .tempdir()?;
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(dir.path().join("auth-key"))?;
        f.write_all(key.as_bytes())?;
        f.sync_all()?;
        Ok(KeyFile { dir })
    }
    pub fn path(&self) -> std::path::PathBuf {
        self.dir.path().join("auth-key")
    }
}

/// A short readable message for a failed `tailscale` command, without keys.
pub fn friendly(out: &Output) -> String {
    scrub_keys(&friendly_raw(out))
}

fn friendly_raw(out: &Output) -> String {
    if out.timed_out {
        return "Tailscale did not answer in time".into();
    }
    let text = out.text();
    let lower = text.to_lowercase();
    if lower.contains("failed to connect to local tailscale")
        || lower.contains("doesn't appear to be running")
        || lower.contains("is tailscaled running")
    {
        return "tailscaled is not running on the Pi: finish the setup to start it".into();
    }
    if lower.contains("requires mentioning all non-default flags") {
        let hint = text
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("tailscale up"))
            .map(|l| format!(" Over SSH, the full command would be: {l}"))
            .unwrap_or_default();
        return format!(
            "Tailscale on this Pi has other settings this would undo; log in with the browser instead.{hint}"
        );
    }
    if lower.contains("access denied") || lower.contains("permission denied") {
        return NEEDS_SETUP.into();
    }
    if lower.contains("invalid key") || lower.contains("auth key") || lower.contains("authkey") {
        return "Tailscale did not accept that auth key (expired, used up or mistyped?)".into();
    }
    if lower.contains("no such host")
        || lower.contains("network is unreachable")
        || lower.contains("i/o timeout")
    {
        return "Tailscale cannot reach its servers; is the Pi online?".into();
    }
    let last = text
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("tailscale failed");
    let mut msg: String = last
        .trim_start_matches("Error: ")
        .chars()
        .take(300)
        .collect();
    if msg.is_empty() {
        msg = "tailscale failed".into();
    }
    msg
}

/// Tailscale can open the Pi to a whole tailnet (and SSH, possibly as
/// root), so nothing here changes without a password guarding the UI. The
/// auth guard already demands the session whenever one is set.
pub fn require_password(state: &AppState) -> Result<(), ApiError> {
    if state.login_required() {
        Ok(())
    } else {
        Err(ApiError(StatusCode::FORBIDDEN, NEEDS_PASSWORD.into()))
    }
}

fn spawn_error(e: io::Error) -> ApiError {
    if e.kind() == io::ErrorKind::NotFound {
        ApiError::bad_request("Tailscale is not installed on the Pi")
    } else {
        ApiError::internal(scrub_keys(&format!("cannot run tailscale: {e}")))
    }
}

/// Run `tailscale <args>` and turn any failure into an API error.
async fn ts_ok(ts: &TsState, args: &[&str], timeout: Duration) -> Result<Output, ApiError> {
    let out = ts.ts(args, timeout).await.map_err(spawn_error)?;
    if !out.success() {
        return Err(ApiError::bad_request(friendly(&out)));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Handlers.

#[derive(Clone, Debug, Default, Serialize)]
pub struct TailscaleView {
    pub installed: bool,
    pub version: Option<String>,
    pub operator_ok: bool,
    pub can_setup: bool,
    /// `web.password` is set; every action below needs it.
    pub password_set: bool,
    #[serde(flatten)]
    pub status: StatusView,
    pub ssh: bool,
    pub serve: ServeView,
    /// The Pi's own hostname (the default device name).
    pub hostname: String,
    /// Why the status could not be read (tailscaled not running, ...).
    pub error: Option<String>,
}

pub async fn view(state: &AppState) -> TailscaleView {
    let ts = &state.tailscale;
    let mut v = TailscaleView {
        hostname: crate::host::hostname().to_lowercase(),
        password_set: state.login_required(),
        ..TailscaleView::default()
    };
    let (version, can_setup) = tokio::join!(ts.ts(&["version"], TIMEOUT), ts.can_setup());
    v.can_setup = can_setup;
    match version {
        Err(_) => return v,
        Ok(o) => {
            v.installed = true;
            v.version = o
                .stdout
                .lines()
                .next()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty());
        }
    }
    let (status, prefs, serve) = tokio::join!(
        ts.ts(&["status", "--json"], TIMEOUT),
        ts.ts(&["debug", "prefs"], TIMEOUT),
        ts.ts(&["serve", "status", "--json"], TIMEOUT),
    );
    match status {
        Ok(o) if o.success() => match parse_status(&o.stdout) {
            Ok(s) => v.status = s,
            Err(e) => v.error = Some(format!("unexpected tailscale status: {e}")),
        },
        Ok(o) => v.error = Some(friendly(&o)),
        Err(e) => v.error = Some(scrub_keys(&e.to_string())),
    }
    v.status.health = v.status.health.iter().map(|h| scrub_keys(h)).collect();
    let prefs = prefs
        .ok()
        .filter(Output::success)
        .and_then(|o| parse_prefs(&o.stdout));
    v.operator_ok = nix::unistd::geteuid().is_root()
        || prefs
            .as_ref()
            .is_some_and(|p| !ts.user.is_empty() && p.operator_user == ts.user);
    v.ssh = prefs.as_ref().is_some_and(|p| p.run_ssh);
    if let Ok(o) = serve {
        if o.success() {
            v.serve = parse_serve(&o.stdout, state.port);
        }
    }
    v
}

pub async fn get_tailscale(State(state): State<Arc<AppState>>) -> ApiResult<TailscaleView> {
    Ok(Json(view(&state).await))
}

pub async fn get_setup(State(state): State<Arc<AppState>>) -> ApiResult<SetupJob> {
    Ok(Json(state.tailscale.job.lock().unwrap().clone()))
}

const MAX_LOG: usize = 400;

fn push_log(job: &Mutex<SetupJob>, line: String) {
    let mut j = job.lock().unwrap();
    j.log.push(scrub_keys(&line));
    if j.log.len() > MAX_LOG {
        let extra = j.log.len() - MAX_LOG;
        j.log.drain(..extra);
    }
}

pub async fn post_setup(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    require_password(&state)?;
    let ts = &state.tailscale;
    {
        let mut j = ts.job.lock().unwrap();
        if j.state == JobState::Running {
            return Err(ApiError::bad_request(
                "the Tailscale setup is already running",
            ));
        }
        *j = SetupJob {
            state: JobState::Running,
            log: vec![format!("$ sudo -n {BINARY} tailscale-setup")],
        };
    }
    let job = ts.job.clone();
    let runner = ts.runner.clone();
    let sink_job = job.clone();
    let sink: LineSink = Arc::new(move |l: String| {
        tracing::info!("tailscale-setup: {}", scrub_keys(&l));
        push_log(&sink_job, l);
    });
    // the sudo answer may change with this run (it does not, but the
    // cached "no" should not outlive a fresh install of the drop-in)
    *ts.can_setup.lock().unwrap() = None;
    tokio::spawn(async move {
        let args = ["-n", BINARY, "tailscale-setup"].map(String::from);
        let result = runner.stream("sudo", &args, sink).await;
        let state = match result {
            Ok(true) => JobState::Done,
            Ok(false) => {
                let needs_install = job
                    .lock()
                    .unwrap()
                    .log
                    .iter()
                    .any(|l| l.contains("password is required") || l.contains("not allowed"));
                if needs_install {
                    push_log(
                        &job,
                        "sudo refused: run `sudo rackscreen install` once (or `sudo rackscreen tailscale-setup`) on the Pi".into(),
                    );
                }
                JobState::Failed
            }
            Err(e) => {
                push_log(&job, format!("error: cannot run sudo: {e}"));
                JobState::Failed
            }
        };
        job.lock().unwrap().state = state;
    });
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct LoginBody {
    pub hostname: Option<String>,
    pub auth_key: Option<String>,
    pub ssh: Option<bool>,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Value> {
    require_password(&state)?;
    let ts = &state.tailscale;
    // Two logins at once would fight over the stored `tailscale up`.
    let _one_at_a_time = ts.login_lock.lock().await;
    let hostname = body
        .hostname
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty());
    if let Some(h) = &hostname {
        if !valid_hostname(h) {
            return Err(ApiError::bad_request(
                "device name: 1 to 63 lowercase letters, digits and dashes",
            ));
        }
    }
    let auth_key = body
        .auth_key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    if let Some(k) = &auth_key {
        if !valid_auth_key(k) {
            return Err(ApiError::bad_request(
                "an auth key starts with tskey- and has only letters, digits, - and _",
            ));
        }
    }
    // Settings first, with `set` (no "mention every flag" rule there).
    let mut set: Vec<String> = Vec::new();
    if let Some(h) = &hostname {
        set.push(format!("--hostname={h}"));
    }
    if let Some(s) = body.ssh {
        set.push(format!("--ssh={s}"));
    }
    if !set.is_empty() {
        let mut args = vec!["set"];
        args.extend(set.iter().map(String::as_str));
        ts_ok(ts, &args, TIMEOUT).await?;
    }
    // A new login replaces one still waiting.
    ts.drop_any_login();

    if let Some(key) = auth_key {
        // `up` with any flag must mention the non-default ones it keeps.
        let prefs = ts
            .ts(&["debug", "prefs"], TIMEOUT)
            .await
            .ok()
            .and_then(|o| parse_prefs(&o.stdout))
            .unwrap_or_default();
        let version = ts.ts(&["version"], TIMEOUT).await.ok().and_then(|o| {
            o.stdout
                .lines()
                .next()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
        });
        // kept until `up` is done, then removed (on every path out)
        let key_file;
        let mut args = vec!["up".to_string()];
        if auth_key_file_supported(version.as_deref()) {
            key_file = KeyFile::new(&key)
                .map_err(|e| ApiError::internal(format!("cannot hand over the auth key: {e}")))?;
            args.push(format!("--auth-key=file:{}", key_file.path().display()));
        } else {
            tracing::warn!(
                "tailscale {} cannot read the auth key from a file; it goes on the command line (update Tailscale)",
                version.as_deref().unwrap_or("?")
            );
            args.push(format!("--auth-key={key}"));
        }
        if let Some(h) = hostname.or((!prefs.hostname.is_empty()).then_some(prefs.hostname)) {
            args.push(format!("--hostname={h}"));
        }
        args.push(format!("--ssh={}", body.ssh.unwrap_or(prefs.run_ssh)));
        tracing::info!("tailscale: logging in with an auth key");
        let out = ts
            .runner
            .run("tailscale", &args, SLOW_TIMEOUT)
            .await
            .map_err(spawn_error)?;
        if !out.success() {
            let msg = friendly(&out);
            tracing::warn!("tailscale: auth key login failed: {msg}");
            return Err(ApiError::bad_request(msg));
        }
        return Ok(Json(json!({ "auth_url": null, "running": true })));
    }

    tracing::info!("tailscale: starting a browser login");
    let bg = ts
        .runner
        .spawn("tailscale", &["up".to_string()])
        .map_err(spawn_error)?;
    let (output, exited) = (bg.output.clone(), bg.exited.clone());
    let generation = ts.store_login(bg);
    // Nobody finished it in time: stop waiting (unless a newer login took over).
    let ttl = *ts.login_ttl.lock().unwrap();
    let st = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(ttl).await;
        if st.tailscale.drop_login(generation) {
            tracing::info!("tailscale: the browser login expired");
        }
    });
    let deadline = Instant::now() + AUTH_URL_WAIT;
    loop {
        let printed = output.lock().unwrap().clone();
        if let Some(url) = find_url(&printed, |u| !u.contains(".ts.net")) {
            return Ok(Json(json!({ "auth_url": url, "running": false })));
        }
        if let Ok(o) = ts.ts(&["status", "--json"], TIMEOUT).await {
            if let Ok(s) = parse_status(&o.stdout) {
                if let Some(url) = s.auth_url {
                    return Ok(Json(json!({ "auth_url": url, "running": false })));
                }
                if s.backend_state.as_deref() == Some("Running") {
                    return Ok(Json(json!({ "auth_url": null, "running": true })));
                }
            }
        }
        let code = *exited.lock().unwrap();
        if let Some(code) = code {
            ts.drop_login(generation);
            if code == Some(0) {
                return Ok(Json(json!({ "auth_url": null, "running": true })));
            }
            let out = Output {
                status: code,
                stderr: printed,
                ..Output::default()
            };
            return Err(ApiError::bad_request(friendly(&out)));
        }
        if Instant::now() >= deadline {
            ts.drop_login(generation);
            return Err(ApiError::unavailable(
                "Tailscale did not offer a login link in time; try again",
            ));
        }
        tokio::time::sleep(Duration::from_millis(400)).await;
    }
}

pub async fn up(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    require_password(&state)?;
    ts_ok(&state.tailscale, &["up"], SLOW_TIMEOUT).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn down(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    require_password(&state)?;
    ts_ok(&state.tailscale, &["down"], SLOW_TIMEOUT).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn logout(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    require_password(&state)?;
    state.tailscale.drop_any_login();
    ts_ok(&state.tailscale, &["logout"], SLOW_TIMEOUT).await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct OnBody {
    pub on: bool,
}

pub async fn ssh(State(state): State<Arc<AppState>>, Json(b): Json<OnBody>) -> ApiResult<Value> {
    require_password(&state)?;
    let args: &[&str] = if b.on {
        &["set", "--ssh=true"]
    } else {
        // turning it off while someone is connected over it asks first
        &["set", "--ssh=false", "--accept-risk=lose-ssh"]
    };
    ts_ok(&state.tailscale, args, TIMEOUT).await?;
    Ok(Json(json!({ "ok": true, "ssh": b.on })))
}

/// Whether `tailscale serve` can reach the UI: it only proxies to 127.0.0.1.
pub fn bind_reaches_loopback(bind: &str) -> bool {
    match bind.trim().parse::<std::net::IpAddr>() {
        Ok(ip) => ip.is_unspecified() || ip.is_loopback(),
        Err(_) => false,
    }
}

pub async fn serve(State(state): State<Arc<AppState>>, Json(b): Json<OnBody>) -> ApiResult<Value> {
    require_password(&state)?;
    let ts = &state.tailscale;
    // Only ever add or remove our own handler: https:443 may serve
    // something someone set up by hand.
    let status = ts_ok(ts, &["serve", "status", "--json"], TIMEOUT).await?;
    let current = https_443(&status.stdout, state.port).map_err(|e| {
        ApiError::internal(format!(
            "cannot tell what Tailscale serves on https:443: {e}"
        ))
    })?;
    if !b.on {
        return match current {
            Https443::Free => Ok(Json(json!({ "on": false, "url": null }))),
            Https443::Other(what) => Err(ApiError(
                StatusCode::CONFLICT,
                format!(
                    "Something else uses https:443 on this Pi's Tailscale ({what}), so RackScreen leaves it alone. Change it over SSH with `tailscale serve`."
                ),
            )),
            Https443::Ours { .. } => {
                ts_ok(ts, &["serve", "--https=443", "off"], SLOW_TIMEOUT).await?;
                Ok(Json(json!({ "on": false, "url": null })))
            }
        };
    }
    match current {
        Https443::Free => {}
        Https443::Ours { url } => return Ok(Json(json!({ "on": true, "url": url }))),
        Https443::Other(what) => {
            return Err(ApiError(
                StatusCode::CONFLICT,
                format!(
                    "Something else already uses https:443 on this Pi's Tailscale ({what}); RackScreen will not replace it. Remove it over SSH (`tailscale serve --https=443 off`) to share this page instead."
                ),
            ))
        }
    }
    let bind = ts.bind.lock().unwrap().clone();
    if !bind_reaches_loopback(&bind) {
        return Err(ApiError::bad_request(format!(
            "web.bind is {bind}; Tailscale can only share 127.0.0.1, so set web.bind to 0.0.0.0 first"
        )));
    }
    let target = format!("http://127.0.0.1:{}", state.port);
    let out = ts
        .ts(&["serve", "--bg", "--https=443", &target], SLOW_TIMEOUT)
        .await
        .map_err(spawn_error)?;
    if !out.success() {
        // HTTPS (or Serve) is not enabled on the tailnet yet: it prints a
        // link to the admin console and waits.
        if let Some(url) = find_url(&out.text(), |u| u.contains("login.tailscale.com")) {
            return Ok(Json(json!({
                "on": false,
                "url": null,
                "enable_url": url,
                "error": "Turn on HTTPS for your tailnet first, then try again",
            })));
        }
        return Err(ApiError::bad_request(friendly(&out)));
    }
    let url = match ts.ts(&["serve", "status", "--json"], TIMEOUT).await {
        Ok(o) if o.success() => parse_serve(&o.stdout, state.port).url,
        _ => None,
    }
    .or_else(|| {
        find_url(&out.text(), |u| u.contains(".ts.net"))
            .map(|u| u.trim_end_matches('/').to_string())
    });
    Ok(Json(json!({ "on": true, "url": url })))
}

// ---------------------------------------------------------------------------
// A scripted runner for tests.

/// Answers by the longest matching prefix of `"prog arg arg ..."`, records
/// every call (with auth keys masked), and can pretend `tailscale` is
/// missing.
#[derive(Default)]
pub struct FakeRunner {
    pub calls: Mutex<Vec<String>>,
    pub responses: Mutex<HashMap<String, Output>>,
    /// Lines `stream` prints, and whether it succeeds.
    pub stream_lines: Mutex<(Vec<String>, bool)>,
    /// What a spawned background command printed.
    pub spawn_output: Mutex<String>,
    pub missing: Mutex<bool>,
    /// Every `--auth-key=file:` seen: its path, what it held and the modes
    /// of the file and its directory, read while the command "ran".
    pub key_files: Mutex<Vec<KeyFileSeen>>,
    /// One receiver per spawned command: closed once its handle is dropped.
    spawned: Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyFileSeen {
    pub path: std::path::PathBuf,
    pub content: String,
    pub file_mode: u32,
    pub dir_mode: u32,
}

impl FakeRunner {
    pub fn respond(&self, prefix: &str, out: Output) {
        self.responses
            .lock()
            .unwrap()
            .insert(prefix.to_string(), out);
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    /// For every spawned command, in order: whether it is still kept (not killed).
    pub fn spawns_alive(&self) -> Vec<bool> {
        use tokio::sync::oneshot::error::TryRecvError;
        self.spawned
            .lock()
            .unwrap()
            .iter_mut()
            .map(|rx| matches!(rx.try_recv(), Err(TryRecvError::Empty)))
            .collect()
    }
    fn record(&self, prog: &str, args: &[String]) -> io::Result<String> {
        let line = std::iter::once(prog.to_string())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        let shown = args
            .iter()
            .map(|w| match w.strip_prefix("--auth-key=") {
                Some(file) if file.starts_with("file:") => {
                    use std::os::unix::fs::PermissionsExt;
                    let path = std::path::PathBuf::from(&file[5..]);
                    let mode = |p: &std::path::Path| {
                        std::fs::metadata(p).map_or(0, |m| m.permissions().mode() & 0o777)
                    };
                    self.key_files.lock().unwrap().push(KeyFileSeen {
                        content: std::fs::read_to_string(&path).unwrap_or_default(),
                        file_mode: mode(&path),
                        dir_mode: path.parent().map_or(0, mode),
                        path,
                    });
                    "--auth-key=file:<tmp>"
                }
                Some(_) => "--auth-key=***",
                None => w.as_str(),
            })
            .fold(prog.to_string(), |acc, w| format!("{acc} {w}"));
        self.calls.lock().unwrap().push(shown);
        if prog == "tailscale" && *self.missing.lock().unwrap() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no tailscale"));
        }
        Ok(line)
    }
}

impl Runner for FakeRunner {
    fn run(
        &self,
        prog: &str,
        args: &[String],
        _timeout: Duration,
    ) -> BoxFuture<'static, io::Result<Output>> {
        let res = self.record(prog, args).map(|line| {
            let responses = self.responses.lock().unwrap();
            responses
                .iter()
                .filter(|(k, _)| line.starts_with(k.as_str()))
                .max_by_key(|(k, _)| k.len())
                .map(|(_, o)| o.clone())
                .unwrap_or_else(|| Output::ok(""))
        });
        Box::pin(async move { res })
    }

    fn stream(
        &self,
        prog: &str,
        args: &[String],
        line: LineSink,
    ) -> BoxFuture<'static, io::Result<bool>> {
        let res = self.record(prog, args).map(|_| {
            let (lines, ok) = self.stream_lines.lock().unwrap().clone();
            lines.into_iter().for_each(|l| line(l));
            ok
        });
        Box::pin(async move { res })
    }

    fn spawn(&self, prog: &str, args: &[String]) -> io::Result<Background> {
        self.record(prog, args)?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.spawned.lock().unwrap().push(rx);
        Ok(Background::fixed(
            &self.spawn_output.lock().unwrap(),
            None,
            Some(tx),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING: &str = include_str!("../tests/fixtures/tailscale-status-running.json");
    const NEEDS_LOGIN: &str = include_str!("../tests/fixtures/tailscale-status-needslogin.json");
    const SERVE: &str = include_str!("../tests/fixtures/tailscale-serve-status.json");
    const PREFS: &str = include_str!("../tests/fixtures/tailscale-prefs.json");

    #[test]
    fn a_running_status_has_the_device_tailnet_user_and_peers_online_first() {
        let s = parse_status(RUNNING).unwrap();
        assert_eq!(s.backend_state.as_deref(), Some("Running"));
        assert_eq!(s.auth_url, None);
        let me = s.self_.unwrap();
        assert_eq!(me.hostname, "rackscreen");
        assert_eq!(me.dns_name, "rackscreen.tail1234.ts.net");
        assert_eq!(me.ips[0], "100.101.102.103");
        assert!(me.online);
        assert_eq!(me.last_seen, None, "the zero time is no time");
        assert_eq!(s.tailnet.as_deref(), Some("silke@example.com"));
        assert_eq!(
            s.user,
            Some(UserView {
                login: "silke@example.com".into(),
                display_name: "Silke".into()
            })
        );
        let names: Vec<_> = s.peers.iter().map(|p| p.hostname.as_str()).collect();
        assert_eq!(names, ["home-router", "laptop", "Silke's Pixel"]);
        assert!(s.peers[0].exit_node);
        assert_eq!(
            s.peers[2].last_seen.as_deref(),
            Some("2026-11-03T21:14:00Z")
        );
        assert_eq!(s.health.len(), 1);
    }

    #[test]
    fn a_logged_out_status_has_the_auth_url_and_no_device() {
        let s = parse_status(NEEDS_LOGIN).unwrap();
        assert_eq!(s.backend_state.as_deref(), Some("NeedsLogin"));
        assert_eq!(
            s.auth_url.as_deref(),
            Some("https://login.tailscale.com/a/1a2b3c4d5e6f")
        );
        assert!(s.self_.is_none() && s.user.is_none() && s.tailnet.is_none());
        assert!(s.peers.is_empty());
        // a stopped daemon's minimal answer parses too
        let s = parse_status(r#"{"BackendState":"Stopped"}"#).unwrap();
        assert_eq!(s.backend_state.as_deref(), Some("Stopped"));
        assert!(parse_status("not json").is_err());
    }

    #[test]
    fn serve_is_on_only_when_it_proxies_this_port() {
        assert_eq!(
            parse_serve(SERVE, 8740),
            ServeView {
                on: true,
                url: Some("https://rackscreen.tail1234.ts.net".into())
            }
        );
        assert_eq!(parse_serve(SERVE, 9000), ServeView::default());
        assert_eq!(parse_serve("{}", 8740), ServeView::default());
        assert_eq!(parse_serve("", 8740), ServeView::default());
    }

    #[test]
    fn https_443_is_ours_only_with_our_proxy_alone() {
        assert_eq!(
            https_443(SERVE, 8740).unwrap(),
            Https443::Ours {
                url: "https://rackscreen.tail1234.ts.net".into()
            }
        );
        assert_eq!(https_443("{}", 8740).unwrap(), Https443::Free);
        assert_eq!(https_443("", 8740).unwrap(), Https443::Free);
        assert!(https_443("nope", 8740).is_err());
        // another port's UI is not ours
        assert_eq!(
            https_443(SERVE, 9000).unwrap(),
            Https443::Other("a proxy to http://127.0.0.1:8740 at /".into())
        );
        // ours plus something else at another path: not ours to remove
        let mixed = r#"{"Web":{"pi.ts.net:443":{"Handlers":{
            "/":{"Proxy":"http://127.0.0.1:8740"},
            "/grafana":{"Proxy":"http://127.0.0.1:3000"}}}}}"#;
        assert_eq!(
            https_443(mixed, 8740).unwrap(),
            Https443::Other("a proxy to http://127.0.0.1:3000 at /grafana".into())
        );
        let files = r#"{"Web":{"pi.ts.net:443":{"Handlers":{"/":{"Path":"/srv/www"}}}}}"#;
        assert_eq!(
            https_443(files, 8740).unwrap(),
            Https443::Other("the files in /srv/www at /".into())
        );
        let tcp = r#"{"TCP":{"443":{"TCPForward":"127.0.0.1:8443"}}}"#;
        assert_eq!(
            https_443(tcp, 8740).unwrap(),
            Https443::Other("a TCP forward to 127.0.0.1:8443".into())
        );
        // other ports do not matter
        let other_port =
            r#"{"Web":{"pi.ts.net:8443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3000"}}}}}"#;
        assert_eq!(https_443(other_port, 8740).unwrap(), Https443::Free);
    }

    #[test]
    fn prefs_tell_the_operator_and_ssh() {
        let p = parse_prefs(PREFS).unwrap();
        assert!(p.run_ssh);
        assert_eq!(p.operator_user, "silke");
        assert_eq!(p.hostname, "rackscreen");
        let p = parse_prefs(r#"{"RunSSH":false}"#).unwrap();
        assert_eq!(p.operator_user, "");
    }

    #[test]
    fn user_input_is_validated() {
        assert!(valid_hostname("rackscreen") && valid_hostname("pi-4") && valid_hostname("a"));
        assert!(!valid_hostname("") && !valid_hostname("Pi") && !valid_hostname("a b"));
        assert!(!valid_hostname(&"a".repeat(64)) && !valid_hostname("x;rm"));
        assert!(valid_auth_key("tskey-auth-kAbC123CNTRL-xyz_9"));
        assert!(!valid_auth_key("tskey-") && !valid_auth_key("key-abc"));
        assert!(!valid_auth_key("tskey-abc def") && !valid_auth_key("tskey-abc\n--ssh"));
    }

    #[test]
    fn auth_keys_are_scrubbed_everywhere() {
        assert_eq!(
            scrub_keys("bad key tskey-auth-k123CNTRL-s3cr_et, try again"),
            "bad key tskey-***, try again"
        );
        assert_eq!(
            scrub_keys("tskey-a tskey- x tskey-b\n"),
            "tskey-*** tskey- x tskey-***\n"
        );
        assert_eq!(scrub_keys("nothing here"), "nothing here");
        assert_eq!(scrub_keys("ümlaut tskey-ä"), "ümlaut tskey-ä");
        let out = Output::fail("Error: invalid --auth-key=tskey-auth-kXyZ-123 given");
        assert!(!friendly(&out).contains("kXyZ"), "{}", friendly(&out));
        let job = Mutex::new(SetupJob::default());
        push_log(&job, "+ tailscale up --authkey=tskey-auth-zzz".into());
        assert_eq!(
            job.lock().unwrap().log[0],
            "+ tailscale up --authkey=tskey-***"
        );
    }

    #[test]
    fn the_key_file_is_private_and_goes_away() {
        use std::os::unix::fs::PermissionsExt;
        let kf = KeyFile::new("tskey-auth-abc").unwrap();
        let path = kf.path();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "tskey-auth-abc");
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), 0o600);
        assert_eq!(mode(path.parent().unwrap()), 0o700);
        drop(kf);
        assert!(!path.exists() && !path.parent().unwrap().exists());
    }

    #[test]
    fn old_tailscale_gets_the_key_on_the_command_line() {
        assert_eq!(parse_version("1.90.6"), Some((1, 90)));
        assert_eq!(parse_version("1.32.0-dev2022"), Some((1, 32)));
        assert_eq!(parse_version("garbage"), None);
        assert!(auth_key_file_supported(Some("1.90.6")));
        assert!(auth_key_file_supported(Some("1.32.0")));
        assert!(!auth_key_file_supported(Some("1.30.2")));
        assert!(auth_key_file_supported(None) && auth_key_file_supported(Some("?")));
    }

    #[test]
    fn failures_read_well() {
        let o = |e: &str| friendly(&Output::fail(e));
        assert_eq!(
            o("Access denied: prefs write access denied\n\nUse 'sudo tailscale up' or 'tailscale set --operator=$USER'"),
            NEEDS_SETUP
        );
        assert!(
            o("failed to connect to local tailscaled; it doesn't appear to be running")
                .contains("not running")
        );
        assert!(o("backend error: invalid key: unable to validate API key").contains("auth key"));
        assert!(o("Error: changing settings via 'tailscale up' requires mentioning all non-default flags. To proceed, either re-run your command with --reset or use the command below to explicitly mention the current value of all non-default settings:\n\n\ttailscale up --hostname=pi --ssh --operator=silke\n").contains("tailscale up --hostname=pi --ssh --operator=silke"));
        assert_eq!(o("Error: something odd\n"), "something odd");
        let t = Output {
            timed_out: true,
            ..Output::default()
        };
        assert!(friendly(&t).contains("in time"));
    }

    #[test]
    fn urls_are_found_in_cli_output() {
        let up = "\nTo authenticate, visit:\n\n\thttps://login.tailscale.com/a/abc123\n\n";
        assert_eq!(
            find_url(up, |_| true).as_deref(),
            Some("https://login.tailscale.com/a/abc123")
        );
        let serve = "Serve is not enabled on your tailnet.\nTo enable, visit:\n\n         https://login.tailscale.com/f/serve?node=nXyZ\n";
        assert_eq!(
            find_url(serve, |u| u.contains("login.tailscale.com")).as_deref(),
            Some("https://login.tailscale.com/f/serve?node=nXyZ")
        );
        assert_eq!(find_url("http://127.0.0.1:8740", |_| true), None);
    }

    #[test]
    fn serve_needs_a_bind_that_reaches_loopback() {
        assert!(bind_reaches_loopback("0.0.0.0") && bind_reaches_loopback("::"));
        assert!(bind_reaches_loopback("127.0.0.1"));
        assert!(!bind_reaches_loopback("192.168.1.172"));
    }

    #[tokio::test]
    async fn the_real_runner_times_out_and_reports_missing_programs() {
        let r = RealRunner;
        let o = r
            .run(
                "sh",
                &["-c".into(), "echo hi; sleep 5".into()],
                Duration::from_millis(300),
            )
            .await
            .unwrap();
        assert!(o.timed_out && !o.success());
        assert_eq!(o.stdout, "hi\n");
        let o = r
            .run("sh", &["-c".into(), "echo err >&2; exit 2".into()], TIMEOUT)
            .await
            .unwrap();
        assert_eq!((o.status, o.stderr.as_str()), (Some(2), "err\n"));
        let e = r
            .run("rackscreen-no-such-program", &[], TIMEOUT)
            .await
            .unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
        // a background command is killed when its handle goes
        let bg = r
            .spawn(
                "sh",
                &[
                    "-c".into(),
                    "echo https://login.example.com/a/1; sleep 30".into(),
                ],
            )
            .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(bg
            .output
            .lock()
            .unwrap()
            .contains("https://login.example.com/a/1"));
        assert!(bg.exited.lock().unwrap().is_none());
        // killing it marks it exited
        let exited = bg.exited.clone();
        drop(bg);
        for _ in 0..50 {
            if exited.lock().unwrap().is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(*exited.lock().unwrap(), Some(None));
    }

    #[tokio::test]
    async fn streamed_output_that_is_not_utf8_keeps_coming() {
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = lines.clone();
        let ok = RealRunner
            .stream(
                "sh",
                &[
                    "-c".into(),
                    r"printf 'a\377b\nnext\n'; printf 'e\376\n' >&2".into(),
                ],
                Arc::new(move |l| sink.lock().unwrap().push(l)),
            )
            .await
            .unwrap();
        assert!(ok);
        let mut got = lines.lock().unwrap().clone();
        got.sort();
        assert_eq!(got, ["a\u{FFFD}b", "e\u{FFFD}", "next"]);
    }

    #[tokio::test]
    async fn only_the_login_of_a_generation_is_killed_by_it() {
        let fake = Arc::new(FakeRunner::default());
        let ts = TsState::new(fake.clone(), "silke".into());
        let a = ts.store_login(fake.spawn("tailscale", &["up".into()]).unwrap());
        assert_eq!(fake.spawns_alive(), [true]);
        let b = ts.store_login(fake.spawn("tailscale", &["up".into()]).unwrap());
        assert_ne!(a, b);
        assert_eq!(fake.spawns_alive(), [false, true], "a newer login replaces");
        assert!(!ts.drop_login(a), "a stale generation leaves the newer one");
        assert_eq!(fake.spawns_alive(), [false, true]);
        assert!(ts.drop_login(b));
        assert_eq!(fake.spawns_alive(), [false, false]);
        assert!(!ts.drop_login(b));
    }
}
