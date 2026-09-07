//! Self-update: read the GitHub releases API, download the aarch64 binary and swap it in.
//!
//! The pure halves (`parse_release`, `is_newer`, `parse_sha256_file`, `platform_ok`,
//! `decide_check`) are tested without a network; the effects go through `reqwest::blocking`
//! and the `Shell`/`Paths` seams the rest of the setup crate uses.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::ops::paths::{service_user, Paths};
use crate::ops::shell::{RealShell, Shell};
use crate::ops::systemd::Systemd;

pub const LATEST_URL: &str = "https://api.github.com/repos/silkepilon/RackScreen/releases/latest";
pub const ASSET: &str = "rackscreen-aarch64";
pub const SHA_ASSET: &str = "rackscreen-aarch64.sha256";

/// What the background check on the menu knows about the latest release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub latest: String,
    pub newer: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: (u64, u64, u64),
    pub asset_url: String,
    pub asset_size: u64,
    pub sha_url: Option<String>,
}

/// `v1.2.3`, `1.2.3` and `1.2.3-rc1` all parse; anything else is None.
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// True only when `latest` is strictly greater; unparseable input is never "newer".
pub fn is_newer(current: &str, latest: &str) -> bool {
    match (parse_version(current), parse_version(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

pub fn parse_release(json: &str) -> Result<Release> {
    let v: serde_json::Value = serde_json::from_str(json).context("parse release json")?;
    let tag = v["tag_name"]
        .as_str()
        .context("release json has no tag_name")?
        .to_string();
    let version = parse_version(&tag).with_context(|| format!("tag {tag} is not a version"))?;
    let empty = Vec::new();
    let assets = v["assets"].as_array().unwrap_or(&empty);
    let find = |name: &str| assets.iter().find(|a| a["name"].as_str() == Some(name));
    let url = |a: &serde_json::Value| {
        a["browser_download_url"]
            .as_str()
            .map(std::string::ToString::to_string)
    };
    let bin = find(ASSET).with_context(|| format!("release {tag} has no {ASSET} asset"))?;
    Ok(Release {
        tag,
        version,
        asset_url: url(bin).context("asset has no browser_download_url")?,
        asset_size: bin["size"].as_u64().unwrap_or(0),
        sha_url: find(SHA_ASSET).and_then(url),
    })
}

/// The first whitespace-separated token of a `sha256sum` file, lowercased.
pub fn parse_sha256_file(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_ascii_lowercase())
    } else {
        None
    }
}

/// The lowercase hex sha256 of a file's contents.
pub fn sha256_hex(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>())
}

pub fn verify_sha256(path: &Path, hex: &str) -> Result<bool> {
    Ok(sha256_hex(path)? == hex.trim().to_ascii_lowercase())
}

/// Only aarch64 machines with the binary already in place can self-update; everything else
/// gets pointed at the install one-liner.
pub fn platform_ok(arch: &str, binary_present: bool) -> Result<String> {
    if arch != "aarch64" {
        bail!("releases only ship an aarch64 binary, this machine is {arch}; build from source");
    }
    if !binary_present {
        bail!("/usr/local/bin/rackscreen is not installed; run the install.sh one-liner first");
    }
    Ok("aarch64, /usr/local/bin/rackscreen".to_string())
}

/// The line and exit code for `rackscreen update --check`: 0 up to date, 1 update, 2 unknown.
pub fn decide_check(current: &str, latest: Option<&str>) -> (String, i32) {
    match latest.and_then(|l| parse_version(l).map(|_| l)) {
        None => (format!("current v{current}, latest unknown, unknown"), 2),
        Some(l) => {
            let l = format!("v{}", l.trim().trim_start_matches(['v', 'V']));
            if is_newer(current, &l) {
                (
                    format!("current v{current}, latest {l}, update available"),
                    1,
                )
            } else {
                (format!("current v{current}, latest {l}, up to date"), 0)
            }
        }
    }
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.0} kB", bytes as f64 / 1024.0)
    }
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client> {
    // Same one-shot provider install as `rackscreen_sources::http`; a second call is a no-op.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("rackscreen/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("build http client")
}

pub fn fetch_latest() -> Result<Release> {
    let resp = client(Duration::from_secs(10))?
        .get(LATEST_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .context("github releases api")?;
    let status = resp.status();
    let body = resp.text().context("read github response")?;
    if !status.is_success() {
        bail!("github api returned {status}");
    }
    parse_release(&body)
}

pub fn fetch_text(url: &str) -> Result<String> {
    let resp = client(Duration::from_secs(10))?
        .get(url)
        .send()
        .with_context(|| format!("get {url}"))?;
    let status = resp.status();
    let body = resp.text().with_context(|| format!("read {url}"))?;
    if !status.is_success() {
        bail!("{url} returned {status}");
    }
    Ok(body)
}

/// Stream `url` into `dest`, reporting `(downloaded, total)` as it goes.
pub fn download(url: &str, dest: &Path, progress: &mut dyn FnMut(u64, u64)) -> Result<()> {
    let mut resp = client(Duration::from_secs(600))?
        .get(url)
        .send()
        .with_context(|| format!("get {url}"))?;
    if !resp.status().is_success() {
        bail!("{url} returned {}", resp.status());
    }
    let total = resp.content_length().unwrap_or(0);
    let mut file =
        std::fs::File::create(dest).with_context(|| format!("create {}", dest.display()))?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let n = resp.read(&mut buf).context("read download")?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .with_context(|| format!("write {}", dest.display()))?;
        done += n as u64;
        progress(done, total);
    }
    file.sync_all()
        .with_context(|| format!("flush {}", dest.display()))?;
    Ok(())
}

/// `chmod 755` then an atomic rename over the running binary (allowed on Linux).
pub fn install_new_binary(tmp: &Path, dest: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(tmp, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("chmod {}", tmp.display()))?;
    std::fs::rename(tmp, dest)
        .with_context(|| format!("rename {} to {}", tmp.display(), dest.display()))?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Check,
    Platform,
    Download,
    Verify,
    Install,
    Restart,
}

impl StepId {
    pub const ALL: [StepId; 6] = [
        StepId::Check,
        StepId::Platform,
        StepId::Download,
        StepId::Verify,
        StepId::Install,
        StepId::Restart,
    ];

    pub fn title(self) -> &'static str {
        match self {
            StepId::Check => "Check the latest release",
            StepId::Platform => "Check platform",
            StepId::Download => "Download rackscreen-aarch64",
            StepId::Verify => "Verify checksum",
            StepId::Install => "Install to /usr/local/bin/rackscreen",
            StepId::Restart => "Restart the service",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Done(String),
    Skipped(String),
    Warn(String),
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum Event {
    Started(StepId),
    Progress {
        done: u64,
        total: u64,
    },
    Finished(StepId, Outcome),
    /// `installed: None` with `ok: true` means "was already up to date".
    Complete {
        installed: Option<String>,
        ok: bool,
    },
}

pub struct Updater {
    pub sh: Arc<dyn Shell>,
    pub paths: Paths,
    pub user: String,
    pub current: String,
}

impl Updater {
    pub fn system(current: &str) -> Updater {
        Updater {
            sh: Arc::new(RealShell),
            paths: Paths::system(),
            user: service_user(),
            current: current.to_string(),
        }
    }

    /// `uname -m`, falling back to the arch this binary was built for.
    pub fn arch(&self) -> String {
        match self.sh.run("uname", &["-m"]) {
            Ok(o) if o.success() && !o.stdout.trim().is_empty() => o.stdout.trim().to_string(),
            _ => std::env::consts::ARCH.to_string(),
        }
    }

    /// Run every step in order, reporting on `events`. Blocks; call from a worker thread.
    pub fn run(&self, events: Sender<Event>) {
        match self.try_run(&events) {
            Ok(installed) => {
                let _ = events.send(Event::Complete {
                    installed,
                    ok: true,
                });
            }
            Err(_) => {
                let _ = events.send(Event::Complete {
                    installed: None,
                    ok: false,
                });
            }
        }
    }

    fn try_run(&self, events: &Sender<Event>) -> Result<Option<String>> {
        let _ = events.send(Event::Started(StepId::Check));
        let release = report(events, StepId::Check, fetch_latest().map(|r| (r, None)))?;
        if !is_newer(&self.current, &release.tag) {
            let _ = events.send(Event::Finished(
                StepId::Check,
                Outcome::Skipped(format!("already up to date (v{})", self.current)),
            ));
            return Ok(None);
        }
        let _ = events.send(Event::Finished(
            StepId::Check,
            Outcome::Done(format!("v{} → {}", self.current, release.tag)),
        ));

        let _ = events.send(Event::Started(StepId::Platform));
        let arch = self.arch();
        report(
            events,
            StepId::Platform,
            platform_ok(&arch, self.paths.binary().exists()).map(|n| ((), Some(n))),
        )?;

        let dest = self.paths.binary();
        let tmp = dest.with_file_name("rackscreen.new");
        let _ = events.send(Event::Started(StepId::Download));
        let size = release.asset_size;
        let mut progress = |done: u64, total: u64| {
            let _ = events.send(Event::Progress {
                done,
                total: if total > 0 { total } else { size },
            });
        };
        let downloaded = download(&release.asset_url, &tmp, &mut progress).map(|()| {
            let n = std::fs::metadata(&tmp).map_or(size, |m| m.len());
            ((), Some(human_size(n)))
        });
        self.report_or_clean(events, StepId::Download, &tmp, downloaded)?;

        let _ = events.send(Event::Started(StepId::Verify));
        match release.sha_url.as_deref() {
            // Releases from before the checksum asset existed: say so and carry on.
            None => {
                let _ = events.send(Event::Finished(
                    StepId::Verify,
                    Outcome::Warn(format!("release has no {SHA_ASSET}")),
                ));
            }
            Some(url) => {
                let actual_hex = self.report_or_clean(
                    events,
                    StepId::Verify,
                    &tmp,
                    sha256_hex(&tmp).map(|h| (h, None)),
                )?;
                let sha_result = fetch_text(url).map(|text| parse_sha256_file(&text));
                let outcome = verify_outcome(sha_result, &actual_hex);
                let failed = matches!(outcome, Outcome::Failed(_));
                let _ = events.send(Event::Finished(StepId::Verify, outcome.clone()));
                if failed {
                    let _ = std::fs::remove_file(&tmp);
                    let Outcome::Failed(msg) = outcome else {
                        unreachable!()
                    };
                    bail!("{msg}");
                }
            }
        }

        let _ = events.send(Event::Started(StepId::Install));
        self.report_or_clean(
            events,
            StepId::Install,
            &tmp,
            install_new_binary(&tmp, &dest).map(|()| ((), Some(dest.display().to_string()))),
        )?;

        let _ = events.send(Event::Started(StepId::Restart));
        let sd = Systemd::new(self.sh.as_ref(), &self.user);
        let outcome = if sd.is_active().unwrap_or(false) {
            match sd.restart() {
                Ok(()) => Outcome::Done(format!("{} restarted", sd.unit())),
                // The new binary is already in place, so a failed restart is not fatal.
                Err(e) => Outcome::Warn(format!("restart failed: {e:#}")),
            }
        } else {
            Outcome::Skipped("service not running".into())
        };
        let _ = events.send(Event::Finished(StepId::Restart, outcome));
        Ok(Some(release.tag))
    }

    /// Report a step and, when it failed, drop the half-written temp file.
    fn report_or_clean<T>(
        &self,
        events: &Sender<Event>,
        id: StepId,
        tmp: &Path,
        r: Result<(T, Option<String>)>,
    ) -> Result<T> {
        if r.is_err() {
            let _ = std::fs::remove_file(tmp);
        }
        report(events, id, r)
    }
}

/// Decide the Verify step's outcome from the `.sha256` fetch and the actual hash of the
/// downloaded file. `sha_result` is `Err` when *fetching* the checksum asset failed
/// (network error) and `Ok(None)` when it fetched but had no parseable digest; both are
/// treated the same as a release with no checksum asset at all — warn and keep the binary.
/// Only an actual digest mismatch is a real failure.
fn verify_outcome(sha_result: Result<Option<String>>, actual_hex: &str) -> Outcome {
    match sha_result {
        Err(_) | Ok(None) => Outcome::Warn("checksum unavailable, installed unverified".into()),
        Ok(Some(hex)) if hex.eq_ignore_ascii_case(actual_hex) => {
            Outcome::Done(format!("sha256 {}…", &hex[..12]))
        }
        Ok(Some(_)) => Outcome::Failed("checksum mismatch, download discarded".into()),
    }
}

/// Send `Finished` for a step; `None` as the note means the caller reports it itself.
fn report<T>(events: &Sender<Event>, id: StepId, r: Result<(T, Option<String>)>) -> Result<T> {
    match r {
        Ok((v, note)) => {
            if let Some(n) = note {
                let _ = events.send(Event::Finished(id, Outcome::Done(n)));
            }
            Ok(v)
        }
        Err(e) => {
            let _ = events.send(Event::Finished(id, Outcome::Failed(format!("{e:#}"))));
            Err(e)
        }
    }
}

/// `rackscreen update --check`: print one line, return the exit code.
pub fn check_and_print(current: &str) -> i32 {
    let latest = fetch_latest().ok().map(|r| r.tag);
    let (line, code) = decide_check(current, latest.as_deref());
    println!("{line}");
    code
}

/// `rackscreen update`: steps 1-6 with plain stdout lines. Returns the exit code.
pub fn run_cli(current: &str) -> i32 {
    let (tx, rx) = mpsc::channel();
    let updater = Updater::system(current);
    let worker = std::thread::Builder::new()
        .name("update".into())
        .spawn(move || updater.run(tx))
        .expect("spawn updater");
    let mut code = 1;
    let mut last_pct = u64::MAX;
    for ev in rx {
        match ev {
            Event::Started(id) => println!(".. {}", id.title()),
            Event::Progress { done, total } => {
                let pct = (done * 100).checked_div(total).unwrap_or(0);
                if pct / 10 != last_pct / 10 {
                    last_pct = pct;
                    println!("   {pct} %");
                }
            }
            Event::Finished(_, Outcome::Done(n)) => println!("ok   {n}"),
            Event::Finished(_, Outcome::Skipped(n)) => println!("skip {n}"),
            Event::Finished(_, Outcome::Warn(n)) => println!("warn {n}"),
            Event::Finished(_, Outcome::Failed(n)) => println!("FAIL {n}"),
            Event::Complete { installed, ok } => {
                code = match (ok, &installed) {
                    (true, Some(tag)) => {
                        println!("updated to {tag}");
                        0
                    }
                    (true, None) => 0,
                    (false, _) => {
                        println!("update failed");
                        1
                    }
                };
            }
        }
    }
    let _ = worker.join();
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "tag_name": "v0.3.1",
      "name": "v0.3.1",
      "assets": [
        {"name": "install.sh", "browser_download_url": "https://example/install.sh", "size": 100},
        {"name": "rackscreen-aarch64", "browser_download_url": "https://example/rackscreen-aarch64", "size": 12345678},
        {"name": "rackscreen-aarch64.sha256", "browser_download_url": "https://example/rackscreen-aarch64.sha256", "size": 90}
      ]
    }"#;

    #[test]
    fn parses_a_release_with_both_assets() {
        let r = parse_release(FIXTURE).unwrap();
        assert_eq!(r.tag, "v0.3.1");
        assert_eq!(r.version, (0, 3, 1));
        assert_eq!(r.asset_url, "https://example/rackscreen-aarch64");
        assert_eq!(r.asset_size, 12_345_678);
        assert_eq!(
            r.sha_url.as_deref(),
            Some("https://example/rackscreen-aarch64.sha256")
        );
    }

    #[test]
    fn missing_checksum_asset_is_not_an_error() {
        let json = FIXTURE.replace("rackscreen-aarch64.sha256", "notes.txt");
        let r = parse_release(&json).unwrap();
        assert_eq!(r.sha_url, None);
    }

    #[test]
    fn release_without_the_binary_fails() {
        let json = r#"{"tag_name":"v9.9.9","assets":[]}"#;
        assert!(parse_release(json).is_err());
        assert!(parse_release("not json").is_err());
        assert!(parse_release(r#"{"tag_name":"nightly","assets":[]}"#).is_err());
    }

    #[test]
    fn is_newer_only_for_strictly_greater_versions() {
        assert!(is_newer("0.3.0", "v0.3.1"));
        assert!(is_newer("0.3.0", "1.0.0"));
        assert!(!is_newer("0.3.0", "v0.3.0"));
        assert!(!is_newer("0.3.0", "0.2.9"));
        assert!(!is_newer("0.3.0", "banana"));
        assert!(!is_newer("banana", "0.4.0"));
        assert_eq!(parse_version("v1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
    }

    #[test]
    fn parses_a_sha256sum_file() {
        let hex = "a".repeat(64);
        assert_eq!(
            parse_sha256_file(&format!("{hex}  rackscreen-aarch64\n")).as_deref(),
            Some(hex.as_str())
        );
        assert_eq!(
            parse_sha256_file(&format!("{}  x\n", hex.to_uppercase())).as_deref(),
            Some(hex.as_str())
        );
        assert_eq!(parse_sha256_file("garbage  file\n"), None);
        assert_eq!(parse_sha256_file(""), None);
    }

    #[test]
    fn verifies_a_file_hash() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("blob");
        std::fs::write(&f, b"hello").unwrap();
        let hex = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";
        assert!(verify_sha256(&f, hex).unwrap());
        assert!(!verify_sha256(&f, &"0".repeat(64)).unwrap());
        assert!(verify_sha256(&dir.path().join("missing"), hex).is_err());
    }

    #[test]
    fn platform_check_rejects_other_arches_and_missing_installs() {
        assert!(platform_ok("aarch64", true).is_ok());
        let e = platform_ok("x86_64", true).unwrap_err().to_string();
        assert!(e.contains("aarch64 binary"), "{e}");
        assert!(e.contains("build from source"), "{e}");
        let e = platform_ok("aarch64", false).unwrap_err().to_string();
        assert!(e.contains("not installed"), "{e}");
        assert!(e.contains("install.sh"), "{e}");
    }

    #[test]
    fn check_decision_lines_and_exit_codes() {
        assert_eq!(
            decide_check("0.3.0", Some("v0.3.1")),
            ("current v0.3.0, latest v0.3.1, update available".into(), 1)
        );
        assert_eq!(
            decide_check("0.3.0", Some("0.3.0")),
            ("current v0.3.0, latest v0.3.0, up to date".into(), 0)
        );
        assert_eq!(
            decide_check("0.3.0", Some("0.2.0")),
            ("current v0.3.0, latest v0.2.0, up to date".into(), 0)
        );
        assert_eq!(
            decide_check("0.3.0", None),
            ("current v0.3.0, latest unknown, unknown".into(), 2)
        );
        assert_eq!(decide_check("0.3.0", Some("nightly")).1, 2);
    }

    #[test]
    fn install_new_binary_sets_the_mode_and_renames() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tmp = dir.path().join("rackscreen.new");
        let dest = dir.path().join("rackscreen");
        std::fs::write(&tmp, b"binary").unwrap();
        install_new_binary(&tmp, &dest).unwrap();
        assert!(!tmp.exists());
        let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[test]
    fn verify_outcome_only_fails_on_a_real_mismatch() {
        let hex = "a".repeat(64);
        // A matching digest is a plain success.
        assert_eq!(
            verify_outcome(Ok(Some(hex.clone())), &hex),
            Outcome::Done(format!("sha256 {}…", &hex[..12]))
        );
        // A real mismatch is the only case that fails the step.
        assert_eq!(
            verify_outcome(Ok(Some("b".repeat(64))), &hex),
            Outcome::Failed("checksum mismatch, download discarded".into())
        );
        // A network error fetching the checksum asset just warns and keeps the binary.
        assert_eq!(
            verify_outcome(Err(anyhow::anyhow!("network error")), &hex),
            Outcome::Warn("checksum unavailable, installed unverified".into())
        );
        // A fetched-but-unparseable checksum file is treated the same way.
        assert_eq!(
            verify_outcome(Ok(None), &hex),
            Outcome::Warn("checksum unavailable, installed unverified".into())
        );
    }

    #[test]
    fn human_sizes_read_sensibly() {
        assert_eq!(human_size(12_345_678), "11.8 MB");
        assert_eq!(human_size(2048), "2 kB");
    }
}
