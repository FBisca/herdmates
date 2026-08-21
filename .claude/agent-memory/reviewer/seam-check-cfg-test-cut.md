---
name: seam-check-cfg-test-cut
description: check-boundary-seams.sh's cfg(test) skip — fixed 2026-08-21 by brace-depth tracking; residual failure mode is a braceless #[cfg(test)] item, which silently blinds the rest of that file
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

**Residual, mutation-proven 2026-08-21.** The skip only ends once a brace
has opened and closed. A *braceless* `#[cfg(test)]` item —
`#[cfg(test)] use x;`, `#[cfg(test)] const …;`, or a one-line
`#[cfg(test)] fn f() { … }` — never sets `started`, so `skip` stays 1 to
EOF and the rest of that file is silently unscanned. Proof: inserting
`#[cfg(test)] use std::fmt as _;` at pump.rs:40 plus a real violation at
pump.rs:50 → check exits **0**. No such item exists in the tree today.

**How to apply:** when reviewing any `src/*.rs`-sweeping shell check
here, mutation-prove it in the file it most needs to cover, and probe the
skip/exclusion logic's boundary conditions, not just the happy path.

Related: [[verification-quirks]], [[silent-noop-checks]]
