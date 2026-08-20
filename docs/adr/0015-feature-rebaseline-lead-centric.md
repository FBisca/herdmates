# ADR-0015 — Feature re-baseline: the lead is mission control

Date: 2026-08-20
Status: accepted
Supersedes the surface list of ADR-0013 (pillars §2.2 board/focus) while
keeping its doctrine (honesty, risk layering, evidence hierarchy).
Builds on ADR-0012 (pivot) and ADR-0014 (legacy fence). Caio drove every
verdict interactively on 2026-08-20; full evidence and per-option
reasoning in `docs/research/feature-rebaseline-brainstorm-2026-08-20.md`.

## Context

Dogfooding bug #118 (pane-board dies when >1 team dir exists) exposed a
structural flaw, not a bug: every human-facing aggregate surface we
built beside the native team (TUI board, focus pane) must first answer
"which team?", and with team dirs keyed per session and every wrapped
claude session a lead, that question is genuinely ambiguous at open
time. Caio called for a review of all base features.

Research (2026-08-20: prior-art web survey, official agent-teams /
cross-session-messaging / subagent docs, AMQ-stack read, upstream
issue + changelog checks) established:

- The native lead session already ships most of mission control: agent
  panel, shared task list, idle notifications, native SendMessage to
  every teammate, plan-approval, permission relay.
- Team files are the canonical identity map; every serious external
  tool reads agent state from files/hooks, never pane scraping.
- A documented injection path into the lead exists: a session's own
  child process (hooks included) may post a message into the session's
  inbox socket (`CLAUDE_CODE_MESSAGING_SOCKET`/`_TOKEN`). No
  general external-process API exists (upstream #88331/#88332 stand,
  zero traction).
- Team config dirs are removed on clean session end; task dirs
  persist. In-process is the default teammate mode; panes are opt-in.
- AMQ-stack doctrine worth adopting: terminals are runtime surfaces,
  not the source of truth; directives go to the live surface when up,
  the durable mailbox when down; preview-first confirm-gated controls.

## Decision

Herdmates feeds the native lead instead of building parallel
human-facing team UIs. The base becomes:

| Layer | Modules | Fate |
|---|---|---|
| Host | shim (`teammux`, `teammux-launch`), doctor | keep (#117 fix pending) |
| Facts | signal engine, team-file gather, hook companion/spool | keep |
| Record | recorder (engine observations only) | keep — "black box" framing: the only durable trace after a lead crash |
| Ambient (human) | sidebar tokens via `pane report-metadata` | keep; add priority-differentiated states (permission-prompt distinct from working/idle) |
| Brain (lead) | hook enrichment + lead-facing skill verbs | new (v3.1+) |
| Break-glass | inbox-file nudge | demoted: fallback channel only, when the lead is down (amq-noc direct-lead shape) |
| Deleted | pane-board TUI, focus pane, jump, legacy v1 (15 fenced modules) | delete outright; git is the archive |

The brain layer, concretely:

1. **Hook enrichment (push)**: `herdmates hook` handlers, running as
   the lead's children on TeammateIdle/TaskCreated/TaskCompleted,
   compute signal-engine facts (waiting reason, deadlock, staleness)
   and post them to the lead's own inbox socket so the lead's model
   sees reasons alongside native events. Mechanism is documented
   (own-child socket delivery); still verify live before shipping
   (ADR-0010 feature-detect discipline). Exit-2 gating stays
   default-off.
2. **Lead-facing skill (pull)**: project-level skill verbs — `why
   <agent>`, `deadlocks`, `roster` — resolving the team from the
   lead's own session id. Teammates load the same skill, which is
   acceptable: facts are read-only.
3. **Steering goes through the lead** (native SendMessage), the human
   steers by talking to the lead. Info flows both ways; the lead
   leads.

Team-resolution ambiguity dissolves structurally: hooks fire inside a
lead session, tokens are per-pane, skill verbs key off the calling
session. No surface guesses "which team" anymore; #118 closes wontfix.

## Rollout

- **v3.0.0 re-baseline** (next release): delete legacy-v1 modules and
  the dropped v2 surfaces, fix #117, ship this ADR + doc updates
  (spec.md pillars, CONTEXT.md, HANDOFF, skill). Major bump because
  the CLI surface shrinks.
- **v3.1+**: brain layer (skill verbs first, then hook enrichment
  after the socket-post spike), sidebar priority states.

## Consequences

- The undocumented inbox-append dependency shrinks to one break-glass
  verb; #88332 remains the upstream ask legitimizing it.
- Losing the TUI board loses the at-a-glance task/mailbox view; the
  bet is the native panel + enriched lead + sidebar tokens cover it.
  Revisit only with dogfooding evidence (new ADR).
- Deleted code is recoverable from git; no second feature fence is
  maintained.
- ADR-0013's cut-line items that presumed the board (DAG lane view,
  comm graph, multi-team board) are implicitly dead unless revived
  against the new base.
