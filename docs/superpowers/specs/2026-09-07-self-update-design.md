# Self-update from GitHub releases

Date: 2026-09-07. Status: approved by Silke.

## Behaviour

- **Check**: `GET https://api.github.com/repos/silkepilon/RackScreen/releases/latest` (headers `User-Agent: rackscreen/<version>`, `Accept: application/vnd.github+json`, 10 s timeout, no token). Parse `tag_name` (`vX.Y.Z`), `assets[].{name, browser_download_url, size}`. Compare semver against `CARGO_PKG_VERSION`; "newer" only when strictly greater. Offline or rate-limited: treated as "unknown", never an error dialog.
- **TUI menu**: at start a background thread runs the check once; the footer line shows `● update: v0.3.1 available` (amber) when newer, `● up to date` (grey) when equal, nothing when unknown. New menu item **Update** between Status and Uninstall ("download and install the latest release").
- **Update screen** (step list like Install):
  1. Check release (shows current → latest; if not newer: "already up to date", Enter returns).
  2. Platform check: `uname -m` must be `aarch64` and `/usr/local/bin/rackscreen` must exist (installed), else fail with a hint to use the one-liner.
  3. Download `rackscreen-aarch64` to `/usr/local/bin/rackscreen.new` (streamed, progress bar from `size`).
  4. Verify: download `rackscreen-aarch64.sha256` (text `<hex>  rackscreen-aarch64`) and compare SHA-256; missing checksum asset → Warn (older release) and continue; mismatch → Failed, temp file removed.
  5. Install: `chmod 755`, atomic `rename` over `/usr/local/bin/rackscreen`.
  6. Restart service if active (`systemctl restart rackscreen@<user>`).
  7. Relaunch: message "installed vX.Y.Z, press Enter to restart the setup UI"; Enter restores the terminal and `exec`s `/usr/local/bin/rackscreen setup` (same args as the current process minus nothing), so the user lands in the new version. Esc returns to the menu of the old binary.
  Config is never touched.
- **CLI**: `rackscreen update` runs steps 1–6 non-interactively with plain stdout lines and exits 0 (updated or up to date) / 1 (failed); `rackscreen update --check` prints `current vA, latest vB, update available|up to date|unknown` and exits 0 when up to date, 1 when an update is available, 2 when unknown. Both need root unless `--check`; the existing `ensure_root` applies.
- **Release workflow**: publish `rackscreen-aarch64.sha256` next to the binary (`sha256sum rackscreen-aarch64 > rackscreen-aarch64.sha256`).
- **Status screen**: shows `binary vX.Y.Z` (from `rackscreen --version` of the installed binary is overkill; use `CARGO_PKG_VERSION` of the running process) and the update line.

## Implementation notes

- `crates/setup/src/ops/update.rs`: pure `parse_release(json) -> Release { tag, version: (u64,u64,u64), asset_url, asset_size, sha_url: Option }`, `is_newer(current: &str, latest: &str) -> bool`, `parse_sha256_file(text) -> Option<String>`, `verify_sha256(path, hex) -> Result<bool>`; effects: `fetch_latest() -> Result<Release>`, `download(url, dest, progress: &mut dyn FnMut(u64, u64)) -> Result<()>`, `install_new_binary(tmp, dest) -> Result<()>`. HTTP through `reqwest::blocking` (features `rustls-no-provider`, `blocking`, `json`) with the ring provider installed like `rackscreen_sources::http`.
- `crates/setup/src/screens/update.rs`: worker thread + step events like Install; the relaunch uses `std::os::unix::process::CommandExt::exec` after `ratatui::restore()`.
- `crates/setup/src/lib.rs`: `Shared.update: Option<UpdateInfo>` filled by the check thread (a `Receiver` polled in `App::run`); menu footer and Status read it.
- `src/main.rs`: `Cmd::Update { check: bool }`.

## Tests

- `parse_release` on a fixture with both assets; `is_newer` cases (`0.3.0` vs `v0.3.1` true, equal false, `0.3.0` vs `0.2.9` false, garbage false); `parse_sha256_file`; `verify_sha256` on a temp file; Update screen step list snapshot (TestBackend) in the "already up to date" and "downloading 40 %" states; `rackscreen update --check` exit codes via a unit test on the pure decision function.
