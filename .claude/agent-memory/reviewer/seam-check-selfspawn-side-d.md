---
name: seam-check-selfspawn-side-d
description: check-subcommand-seams.sh Side D (in-code current_exe self-spawn) works for today's single spawn but has three mutation-proven blind spots — arg >10 lines away, .args([...]), and a second .arg after a live one
metadata:
  type: project
---

Side D (added 4792349, 2026-08-21) awk-scans `src/*.rs` for
`/current_exe/`, opens a 10-line window, and (since b9db112) cross-checks EVERY
`.arg("literal")` in that window against main.rs's live arms.

**Works, mutation-proven 2026-08-21:** renaming the `pump-board` arm to
`board-pump` → `[SEAM-SELFSPAWN-DEAD]`, exit 1. Same-line
`current_exe(...).arg("ghost")` is caught. Two separate self-spawns in one
file are both caught (`/current_exe/` resets the window).

**Blind spots, all mutation-proven at 4792349 (probe file in src/, script
exits 0):**
1. `.arg("x")` more than 10 lines below the `current_exe` line. STILL OPEN.
2. `.args(["x"])` / any non-`.arg("literal")` shape (const, variable).
   STILL OPEN.
3. A second `.arg("literal")` after the first — **FIXED in b9db112**
   (the awk now loops every literal on the line); re-mutation-proven:
   `.arg("pump-board").arg("ghost-second")` → exit 1.

**False-positive surface (pre-existing, not a regression), proven
2026-08-21:** the trigger is the bare string `current_exe` on ANY line,
comments and parameter names included. A doc comment mentioning
`current_exe` followed within 10 lines by an unrelated
`Command::new("git").arg("status")` → `[SEAM-SELFSPAWN-DEAD] Code spawns
'herdmates status'`, exit 1. Clean on the real tree today.

Only one real self-spawn exists today (pump.rs `auto_pump`), matching
shape 0, so the check is honest about the tree it guards — but note the
limits before citing it as coverage for a NEW self-spawn.

**How to apply:** if a diff adds a second in-code self-invocation, check
its shape against the three blind spots before accepting "the seam script
covers it".

Related: [[seam-check-cfg-test-cut]], [[hook-path-must-not-block]],
[[silent-noop-checks]]
