//! Where the Bambu Lab login's token file lives: next to Spotify's, in the
//! service user's state dir (`$XDG_STATE_HOME/rackscreen/bambu.json`).

use std::path::PathBuf;

/// Our own token file (the service reads it, the web UI writes it).
pub fn token_path() -> Option<PathBuf> {
    crate::play::state_dir().map(|d| rackscreen_sources::bambu_cloud::token_path_in(&d))
}
