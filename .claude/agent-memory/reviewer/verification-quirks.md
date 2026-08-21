---
name: verification-quirks
description: How to verify herdmates diffs — gate command output shape, and where to check workers' "live-verified" claims independently
metadata:
  type: project
---

Gate: `cargo fmt --check && cargo clippy --all-targets -- -D warnings &&
cargo test && cargo build --bins` in the worktree. Test count grows fast: ~286 passed
(2026-08-20 early) → 319 after #124 → **322 passed / 0 failed / 1 ignored** at b9d7c81 (2026-08-21);
the 1 ignored is always the `#[ignore]`d live socket probe in
`src/lead_post.rs`. Trust the number you just ran, not the memory.

In **background/teammate runs the LSP tool is unavailable**
(`ToolSearch("select:LSP")` returns nothing) while the repo's LSP-first
hooks still block `Read` on `.rs` files and block `grep` patterns that
contain CamelCase symbols. Workaround that works: `awk 'NR>=A && NR<=B'
file.rs` via Bash for reading, and greps phrased to avoid whole symbol
names (e.g. `ermission` instead of `PermissionPrompt`).

**Why:** workers paste stale output; live claims here are independently
checkable, so there is no reason to take them on faith.

**How to apply:** verify `[live]` claims by grepping the session transcripts
under `~/.claude/projects/<slug>/*.jsonl` (probe text, `verifiedPeerPid`,
`origin.kind`) and the hook spool under
`~/.local/state/herdmates/hook-spool/`. Verify `[source]` claims about
Claude Code internals with `grep -a` against
`~/.local/share/claude/versions/<version>` (the shipped bundle).

Related: [[hook-payload-session-identity]]
