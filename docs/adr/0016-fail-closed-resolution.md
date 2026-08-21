# ADR-0016: Fail-closed resolution — refuse and name the fix, never guess

Status: proposed (2026-08-21, drafted from issue #129; awaiting Caio)

## Context

Several herdmates code paths resolve "which team / which lead / which
state does this invocation belong to" from ambient facts:

- `brain.rs` resolves the invoking session's team by matching
  `CLAUDE_CODE_SESSION_ID` against each team's `leadSessionId` (#122).
- `team_hook.rs::resolve_team_bucket` maps a hook payload to a spool
  bucket via `team_name`, then session-id lookup, then a fallback bucket.
- Any future board/focus surface and the teammux shim will resolve state
  from a pane's cwd — and multiple teammates sharing one repo root makes
  cwd genuinely ambiguous.

Prior art (issue #129): OthmanAdi/planning-with-files
(`scripts/inject-plan.sh:34-54, 232-280`) hit the same class — nested or
shared plan roots — and settled on: when resolution is ambiguous, inject
NOTHING, print one line naming the ambiguity and the escape hatches.
Their escape hatch is an explicit pin (`PWF_PLAN_ROOT`).

We have already paid for the guessing alternative once: the #123 review
found in-process teammates carrying the LEAD's `transcript_path`, where
trusting the ambient value produced exactly the wrong-reason class
ADR-0013 forbids ("honesty over completeness"). The fix there —
`transcript_is_teammates_own`, unknown ownership → no fact — is this
doctrine applied locally, before it had a name.

## Decision

1. **Doctrine: wherever herdmates resolves an owner (team, lead, plan,
   focus, pane state) from ambient facts and the answer is ambiguous,
   it refuses and names the fix.** Refusing means: no injection, no
   token publish, no post for that target — plus exactly one diagnostic
   line stating what was ambiguous and every escape hatch by name.
   Guessing among candidates is forbidden; ADR-0012's "degrade to a
   skipped pass" stays for *errors*, but ambiguity must additionally
   name the fix (a silent skip hides a resolvable configuration
   problem).
2. **The escape hatch is an explicit pin: `HERDMATES_TEAM_PIN`.** When
   set, its team name wins over all inference in every resolver. A pin
   naming a nonexistent team is itself an error worth one diagnostic
   line — not a fallback to inference (a stale pin silently reverting
   to guessing would reintroduce the bug class).
3. **Unambiguous resolution stays untouched.** Zero candidates =
   existing degrade policy (skip; diagnostics where a channel exists).
   One candidate = proceed. Only two-plus candidates trigger the
   refuse-and-name path.
4. **Governed resolvers now**: `brain.rs` team resolution,
   `resolve_team_bucket`'s session-id arm (the `team_name` arm is
   payload-explicit, not inference). **Governed at birth**: any
   cwd-based state resolution in future board/focus surfaces and the
   shim — these must ship fail-closed, not retrofit it.

## Consequences

- A pane in an ambiguous spot gets no sidebar tokens / no enrichment
  until pinned — visible, explained, and fixable in one env var,
  instead of plausibly-wrong output (ADR-0013's honesty rule extended
  from facts to *addressing*).
- One new env var to document in the doctor output; `doctor` should
  report the pin's value and whether it resolves.
- `resolve_team_bucket`'s fallback bucket (`_unknown`) survives: the
  spool is a black-box recorder and must not drop observations; the
  doctrine governs *interpretation* (enrichment, tokens), not capture.
- Implementation splits from #129 once this ADR is accepted.
