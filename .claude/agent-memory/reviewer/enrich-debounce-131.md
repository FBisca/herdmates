---
name: enrich-debounce-131
description: The #131 enrichment debounce (b9d7c81) — key shape, what its regression test does and does NOT pin, and the costs that stay on the hook path
metadata:
  type: project
---

`push_enrichment` (src/team_hook.rs) debounces the lead post to one per
`(team, event_name, teammate_name)` per `ENRICH_DEBOUNCE_MS = 30_000`, via
`pump::debounce_admit_marker` on
`$XDG_STATE_HOME/herdmates/enrich-debounce/<sanitized-key>`. The spool
append stays unfiltered (black-box recorder).

**Mutation-proven at b9d7c81, re-run at b9db112 (2026-08-21):**
- `ENRICH_DEBOUNCE_MS = 0` → the regression test
  `a_repeated_event_within_the_debounce_window_posts_once` FAILS fast
  (panic at the "second event must not post" arm). Good at both commits.
- Key narrowed to `format!("{team}--{teammate}")`: at b9d7c81 it passed
  322/322 (granularity unguarded). b9db112 added a third event
  (`TaskCompleted`, same team+teammate) to pin granularity — but it
  calls `listener.set_nonblocking(false)` before that `accept()`, so the
  same mutation **HANGS FOREVER instead of failing** (proven:
  `timeout 90 cargo test <name>` → terminated). A regression guard whose
  failure mode is a wedged CI job is worse than a failing one. Fix: keep
  the listener non-blocking and retry `accept()` a bounded number of
  times, panicking on exhaustion.

**Costs that remain on the hook critical path:** the debounce sits AFTER
`enrich::enrichment_for_event`, which calls
`gather::team_task_dependencies` — so a ~1/s TeammateIdle storm still
pays a full task-tree read per event; only the socket post is suppressed.
Deliberate (a suppressed no-text event must not burn the window).

**Design trade-off:** the key is identity-based, not content-based — a
teammate going idle twice in 30 s with a genuinely CHANGED reason loses
the second signal.

**How to apply:** any future test for this debounce must vary one key
component at a time; otherwise the granularity stays unguarded.

Related: [[hook-path-must-not-block]], [[hook-payload-session-identity]],
[[verification-quirks]]
