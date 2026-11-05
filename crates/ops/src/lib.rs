//! Non-interactive system operations: install, uninstall, update, boot files,
//! systemd, paths, shell and small config edits. Every effect goes through a
//! `Shell` and a `Paths` seam so the steps are testable without a Pi.

pub mod boot;
pub mod config_file;
pub mod install;
pub mod paths;
pub mod shell;
pub mod spi_tune;
pub mod systemd;
pub mod tailscale;
pub mod uninstall;
pub mod update;
