---
name: seam-check-cfg-test-cut
description: check-boundary-seams.sh's cfg(test) skip — brace-depth (9b582a0) + semicolon-clear (4792349); residual is the ONE-LINE #[cfg(test)] fn, which over-skips the next few production lines
metadata:
  type: project
---

**History.** `scripts/check-boundary-seams.sh` (#127) originally stripped
each `src/*.rs` from the first `^#\[cfg(test)\]` to EOF, which cut 977
lines of `src/herdr.rs` (the mid-file `test_support` mod at 719 hid
production command-construction at 1171–1440), and reported post-strip
line numbers.

**Fixed in 9b582a0 (2026-08-21)** — single awk pass, skip each cfg(test)
item by brace depth, `FNR` for real line numbers. Re-verified by mutation
2026-08-21: `format!("cd {} && …")` at herdr.rs:1170 → exit 1 reporting
`src/herdr.rs:1170`; at herdr.rs:1440 → exit 1; at pump.rs:50 → reported
as `src/pump.rs:50`; inside teammux.rs's test mod (line 700) → exit 0
(fixtures still exempt). All 22 computed skip ranges in the tree land
exactly on the true block end — no string/comment brace miscount today.

**Braceless case fixed in 4792349 (2026-08-21)** —
`if (!started && /;[[:space:]]*$/) skip = 0` clears the latch at the
item's semicolon. A/B mutation-proven 2026-08-21: `#[cfg(test)] use
std::fmt as _;` + a `cd … &&` violation two lines later → pre-fix script
exits **0**, post-fix exits **1** reporting the real line.

**Residual, mutation-proven 2026-08-21.** A *one-line* item
(`#[cfg(test)] fn f() { let _ = 1; }`) still blinds: the attribute rule
`next`s before braces are counted, so `started` stays 0 and the skip runs
on until the next line ending in `;`. Proof: that one-liner plus a
violation on the following line → exit **0**. Bounded (not to EOF) but
non-zero. No one-line `#[cfg(test)]` item exists in the tree today
(`grep -c '#\[cfg(test)\] ' src/*.rs` → only a comment in team_hook.rs).

**How to apply:** when reviewing any `src/*.rs`-sweeping shell check
here, mutation-prove it in the file it most needs to cover, and probe the
skip/exclusion logic's boundary conditions, not just the happy path.

Related: [[verification-quirks]], [[silent-noop-checks]]
