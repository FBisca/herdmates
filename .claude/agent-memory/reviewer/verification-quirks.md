---
name: verification-quirks
description: How to verify herdmates diffs — gate command output shape, and where to check workers' "live-verified" claims independently
metadata:
  type: project
---

Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings &&
cargo test && cargo build --bins` in the worktree. As of 2026-08-20 a clean
run reports ~286 passed / 0 failed / 1 ignored (the `#[ignore]`d live
socket probe in `src/lead_post.rs`).

**Why:** workers paste stale output; live claims here are independently
checkable, so there is no reason to take them on faith.

**How to apply:** verify `[live]` claims by grepping the session transcripts
under `~/.claude/projects/<slug>/*.jsonl` (probe text, `verifiedPeerPid`,
`origin.kind`) and the hook spool under
`~/.local/state/herdmates/hook-spool/`. Verify `[source]` claims about
Claude Code internals with `grep -a` against
`~/.local/share/claude/versions/<version>` (the shipped bundle).

Related: [[hook-payload-session-identity]]
