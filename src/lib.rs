//! herdmates — shared library backing the `herdmates` binary and the
//! `teammux` shim binary (issue #85 commit 3).
//!
//! Split out of what was a single-binary crate so `teammux` (src/bin/
//! teammux.rs) can reuse `idmap`/`tmuxargs`/`teammux` without duplicating
//! source. `src/main.rs` re-imports everything below via `use herdmates::*;`
//! so its existing subcommand dispatch is unchanged.

pub mod attention;
pub mod audit;
pub mod dateutil;
pub mod doctor;
pub mod focus_pane;
pub mod focusfile;
pub mod gather;
pub mod herdr;
pub mod idmap;
pub mod inbox_write;
pub mod jump;
pub mod metadata;
pub mod pane_board;
pub mod paths;
pub mod pump;
pub mod recorder;
pub mod signal_engine;
pub mod team_hook;
pub mod teamfiles;
pub mod teammux;
pub mod teammux_launch;
pub mod tmuxargs;
pub mod tokens;
