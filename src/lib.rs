//! herdmates — shared library backing the `herdmates` binary and the
//! `teammux` shim binary (issue #85 commit 3).
//!
//! Split out of what was a single-binary crate so `teammux` (src/bin/
//! teammux.rs) can reuse `idmap`/`tmuxargs`/`teammux` without duplicating
//! source. `src/main.rs` re-imports everything below via `use herdmates::*;`
//! so its existing subcommand dispatch is unchanged.

#[cfg(feature = "legacy-v1")]
pub mod adopt;
#[cfg(feature = "legacy-v1")]
pub mod agents_md;
pub mod attention;
pub mod audit;
#[cfg(feature = "legacy-v1")]
pub mod board;
pub mod dateutil;
pub mod doctor;
pub mod focus_pane;
pub mod focusfile;
pub mod gather;
#[cfg(feature = "legacy-v1")]
pub mod god_cli;
pub mod herdr;
#[cfg(feature = "legacy-v1")]
pub mod hook;
pub mod idmap;
pub mod inbox_write;
pub mod jump;
#[cfg(feature = "legacy-v1")]
pub mod launcher;
pub mod metadata;
#[cfg(feature = "legacy-v1")]
pub mod msg;
pub mod pane_board;
pub mod paths;
pub mod pump;
#[cfg(feature = "legacy-v1")]
pub mod reconcile;
pub mod recorder;
#[cfg(feature = "legacy-v1")]
pub mod run;
pub mod signal_engine;
#[cfg(feature = "legacy-v1")]
pub mod socket;
#[cfg(all(unix, feature = "legacy-v1"))]
pub mod socket_backend;
#[cfg(feature = "legacy-v1")]
pub mod spawn;
#[cfg(feature = "legacy-v1")]
pub mod spec;
#[cfg(feature = "legacy-v1")]
pub mod status_kill;
pub mod team_hook;
pub mod teamfiles;
pub mod teammux;
pub mod teammux_launch;
pub mod tmuxargs;
pub mod tokens;
#[cfg(feature = "legacy-v1")]
pub mod types;
