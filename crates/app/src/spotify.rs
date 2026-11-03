//! Where the Spotify token file lives: the service user's state dir
//! (`$XDG_STATE_HOME/rackscreen/spotify.json`, default `~/.local/state`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The token file under a state dir.
pub fn token_path_in(state: &Path) -> PathBuf {
    rackscreen_sources::spotify::token_path_in(state)
}

/// Our own token file (the service reads it).
pub fn token_path() -> Option<PathBuf> {
    crate::play::state_dir().map(|d| token_path_in(&d))
}

/// The token file of the service user and who should own it: under sudo the
/// invoking user's `~/.local/state` (setup writes it for them), else ours.
pub fn token_path_for_service_user() -> Result<(PathBuf, Option<(u32, u32)>)> {
    match crate::config::sudo_user() {
        Some(u) => Ok((
            token_path_in(&u.dir.join(".local").join("state")),
            Some((u.uid.as_raw(), u.gid.as_raw())),
        )),
        None => Ok((token_path().context("no home directory")?, None)),
    }
}

/// A login exists for the service: the token file is there and was not
/// refused. The music role needs this (or a Jellyfin URL).
pub fn logged_in(path: &Path) -> bool {
    rackscreen_sources::spotify::read_token(path).is_some_and(|t| t.error.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rackscreen_sources::spotify::{write_token, TokenFile};

    #[test]
    fn token_path_is_under_the_state_dir() {
        assert_eq!(
            token_path_in(Path::new("/home/u/.local/state")),
            PathBuf::from("/home/u/.local/state/rackscreen/spotify.json")
        );
    }

    #[test]
    fn logged_in_needs_a_good_token_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = token_path_in(dir.path());
        assert!(!logged_in(&path));
        let tok = TokenFile {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_at: 1,
            display_name: None,
            error: None,
        };
        write_token(&path, &tok, None).unwrap();
        assert!(logged_in(&path));
        write_token(
            &path,
            &TokenFile {
                error: Some("invalid_grant".into()),
                ..tok
            },
            None,
        )
        .unwrap();
        assert!(!logged_in(&path), "a refused login does not count");
    }
}
