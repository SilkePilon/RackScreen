//! Play-now requests: `rackscreen play <id|random>` writes a one-line file in
//! the service user's state dir; the run loop polls it once a second.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rackscreen_core::shows::schedule::Request;

/// `$XDG_STATE_HOME`, else `$HOME/.local/state`.
pub fn state_dir_from(xdg: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    match xdg.filter(|x| !x.is_empty()) {
        Some(x) => Some(PathBuf::from(x)),
        None => home.map(|h| h.join(".local").join("state")),
    }
}

/// Our own state dir (the service user's, for the run loop).
pub fn state_dir() -> Option<PathBuf> {
    state_dir_from(std::env::var_os("XDG_STATE_HOME"), dirs::home_dir())
}

/// The request file under a state dir.
pub fn request_path(state: &Path) -> PathBuf {
    state.join("rackscreen").join("play")
}

/// `random` or a known show id; whitespace around it is ignored.
pub fn parse(text: &str) -> Result<Request> {
    let arg = text.trim();
    if arg == "random" {
        return Ok(Request::Random);
    }
    rackscreen_core::shows::by_id(arg)
        .map(Request::Id)
        .with_context(|| {
            let known: Vec<&str> = rackscreen_core::shows::SHOWS.iter().map(|s| s.id).collect();
            format!("unknown show '{arg}' (known: random, {})", known.join(", "))
        })
}

/// Read and delete the request at `path`: `None` when there is none, else
/// the parsed request (an unknown id is an error, and still deleted).
pub fn take(path: &Path) -> Option<Result<Request>> {
    let text = std::fs::read_to_string(path).ok()?;
    if let Err(e) = std::fs::remove_file(path) {
        tracing::warn!("play request {}: {e}", path.display());
    }
    Some(parse(&text))
}

/// Write `arg` (validated) as a request under `state`; returns the file.
pub fn write_in(state: &Path, arg: &str) -> Result<PathBuf> {
    write_owned(state, arg, None)
}

/// Write the request for the service user: under sudo into the invoking
/// user's state dir (`~/.local/state`, owned by them), else into ours.
/// Returns the file and a warning for the user, when there is one.
pub fn write_for_service_user(arg: &str) -> Result<(PathBuf, Option<&'static str>)> {
    match crate::config::sudo_user() {
        Some(u) => Ok((
            write_owned(
                &u.dir.join(".local").join("state"),
                arg,
                Some((u.uid.as_raw(), u.gid.as_raw())),
            )?,
            None,
        )),
        None => {
            let root = nix::unistd::geteuid().is_root();
            let sudo_set = std::env::var_os("SUDO_USER").is_some_and(|u| !u.is_empty());
            let path = write_in(&state_dir().context("no home directory")?, arg)?;
            Ok((path, no_sudo_warning(root, sudo_set)))
        }
    }
}

/// As root without `SUDO_USER` the request lands in root's own state dir,
/// which the service (running as its user) never reads.
fn no_sudo_warning(root: bool, sudo_user_set: bool) -> Option<&'static str> {
    (root && !sudo_user_set).then_some(
        "running as root without sudo: the request went to /root/.local/state; \
         run it with sudo from the service user's login, or as that user",
    )
}

/// Validate, then write via a temp file and rename, so the run loop never
/// reads half a request. With `owner`, every directory created on the way
/// and the file are chowned to it. Run as root into a user's directory, so
/// it never follows a link: the temp file is created fresh (a stale regular
/// one is removed first, anything else is an error), chowned through its
/// handle, and a link at the final path is refused.
fn write_owned(state: &Path, arg: &str, owner: Option<(u32, u32)>) -> Result<PathBuf> {
    use rackscreen_sources::owned_fs::{create_dir_all_owned, write_new_owned};

    parse(arg)?;
    let path = request_path(state);
    let dir = path.parent().expect("request path has a parent");
    create_dir_all_owned(dir, owner)?;
    let line = format!("{}\n", arg.trim());
    write_new_owned(
        &path,
        &path.with_extension("tmp"),
        line.as_bytes(),
        0o644,
        owner,
    )?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_dir_prefers_xdg_then_home() {
        assert_eq!(
            state_dir_from(Some("/x/state".into()), Some("/home/u".into())),
            Some(PathBuf::from("/x/state"))
        );
        assert_eq!(
            state_dir_from(Some("".into()), Some("/home/u".into())),
            Some(PathBuf::from("/home/u/.local/state")),
            "an empty XDG_STATE_HOME is unset"
        );
        assert_eq!(
            state_dir_from(None, Some("/home/u".into())),
            Some(PathBuf::from("/home/u/.local/state"))
        );
        assert_eq!(state_dir_from(None, None), None);
        assert_eq!(
            request_path(Path::new("/s")),
            PathBuf::from("/s/rackscreen/play")
        );
    }

    #[test]
    fn parses_random_and_ids() {
        assert!(matches!(parse("random\n"), Ok(Request::Random)));
        assert!(matches!(parse("  freight "), Ok(Request::Id(d)) if d.id == "freight"));
        let e = parse("nope").unwrap_err().to_string();
        assert!(e.contains("nope") && e.contains("freight"), "{e}");
        assert!(parse("").is_err());
    }

    #[test]
    fn write_then_take_consumes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = request_path(dir.path());
        assert!(take(&path).is_none());
        assert_eq!(write_in(dir.path(), "freight").unwrap(), path);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "freight\n");
        assert!(matches!(take(&path), Some(Ok(Request::Id(d))) if d.id == "freight"));
        assert!(!path.exists(), "deleted once used");
        assert!(take(&path).is_none());
        assert!(
            write_in(dir.path(), "bogus").is_err(),
            "validated before writing"
        );
        assert!(!path.exists());
        std::fs::write(&path, "bogus").unwrap();
        assert!(matches!(take(&path), Some(Err(_))));
        assert!(!path.exists(), "an unknown id is deleted too");
    }

    #[test]
    fn a_symlink_at_the_temp_path_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = request_path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let target = dir.path().join("victim");
        std::fs::write(&target, "keep me").unwrap();
        let tmp = path.with_extension("tmp");
        std::os::unix::fs::symlink(&target, &tmp).unwrap();
        assert!(write_in(dir.path(), "freight").is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        assert!(!path.exists(), "no request written");
        std::fs::remove_file(&tmp).unwrap();
        // a stale regular temp file is replaced
        std::fs::write(&tmp, "stale").unwrap();
        write_in(dir.path(), "freight").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "freight\n");
        // a symlink at the final path is refused too
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(write_in(dir.path(), "freight").is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "keep me");
        assert!(std::fs::symlink_metadata(&path).unwrap().is_symlink());
    }

    #[test]
    fn root_without_sudo_user_warns() {
        assert!(no_sudo_warning(true, false)
            .unwrap()
            .contains("running as root without sudo"));
        assert!(no_sudo_warning(true, true).is_none());
        assert!(no_sudo_warning(false, false).is_none());
    }
}
