---
name: hook-path-must-not-block
description: src/team_hook.rs's "must never block" invariant — #125's synchronous pump was fixed in 9b582a0 by a detached pump-board child; that spawn is now an unguarded subcommand-name seam
metadata:
  type: project
---

`src/team_hook.rs` documents a hard "must never block" invariant. Stdin is
bounded (`STDIN_READ_TIMEOUT` / `STDIN_READ_MAX_BYTES`), but
`HerdrClient::invoke` (src/herdr.rs) still uses `Command::output()` with
**no timeout** — a wedged `herdr` blocks forever.

Issue #125 (5192aa6) put the whole pump pass synchronously on that path.
**Fixed in 9b582a0 (2026-08-21):** `pump::auto_pump` (src/pump.rs:63) runs
only the marker-file debounce in-process, then detaches
`std::env::current_exe() pump-board` with all stdio nulled. `maybe_pump_at`
became `#[cfg(test)]`; `debounce_admit` is the shared production core.
Marker is now written on ADMIT (before the work), which is the only
possible semantics once the work is in another process — a dead child
costs one missed tick (≤2 s), and the debounce tests' assertions are
unchanged by the move.

**New seam this created (mutation-proven 2026-08-21):** `auto_pump`'s
literal `.arg("pump-board")` is a *fourth* producer of the subcommand
name, and `scripts/check-subcommand-seams.sh` only cross-compares
main.rs arms / usage string / `herdr-plugin.toml`. Renaming the arm to
`board-pump` leaves both seam scripts and `cargo test` green while the
board silently stops ticking (child stderr is nulled, spawn result
discarded). No test covers `auto_pump`.

**How to apply:** any new work in `hook_command` must be timeout-bounded
or detached — debounce bounds frequency, not duration. And any *in-code*
invocation of our own subcommands is a seam: add it to
`docs/agents/boundary-pairs.md` and the subcommand check, or it drifts
silently.

Related: [[verification-quirks]], [[hook-payload-session-identity]],
[[seam-check-cfg-test-cut]]
