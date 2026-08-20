# Feature re-baseline brainstorm — 2026-08-20

Input to the base-feature review Caio requested (pane-board #118 was the
trigger: "tbh I feel like we can drop this feature. we need to review all
our base features and rediscuss them"). Sources: mission-control feature
landscape (2026-07-16), cmux comparative, cross-session-messaging research
(2026-08-20), a fresh prior-art web survey (2026-08-20, agent
`prior-art-web`), official agent-teams docs fetched directly, upstream
issue/changelog checks. A claude-code-guide deep-dive died on usage
limits; its gaps were filled by the survey + our own day-old research.
Decisions land in a future ADR-0015; nothing here is decided yet.

## Verdicts already taken this session (Caio, 2026-08-20)

| Module | Verdict |
|---|---|
| Shim (`teammux`, `teammux-launch`) | **KEEP** (Caio pre-verdict) |
| Observability core (signal engine + hooks/spool + recorder) | **KEEP** |
| Sidebar tokens (`pump-board` → `pane report-metadata`) | **KEEP** |
| Pane-board TUI (plugin pane, #118) | **DROP** |
| Focus pane (`focus`, focus file, atomizer) | **DROP** |
| Legacy v1 surface (15 fenced modules, ADR-0014) | **DELETE now** |
| Dropped-module code | delete outright, git is the archive |
| Board steering (jump + nudge) | **OPEN — this brainstorm** |
| Doctor | keep (assumed, with shim) |

In-flight work parked: #117/#118 commented, 5 commits unpushed, v2.3.1
on hold until the review ADR lands.

## New facts (all verified 2026-08-20)

1. **Team files are the canonical identity map** — `config.json`
   `members[] {name, agentId, agentType}` plus, in split-pane mode, the
   pane ids. External observers need no registry, no `$TMUX_PANE`
   correlation. (Official docs, fetched directly.)
2. **Hook-driven state beats polling** across every serious prior-art
   tool; content-scrapers (agents-dashboard) are visibly the weak tier.
   Our spool→debounced-pump pipeline is already the right shape.
3. **In-process is the default teammate mode.** Teammates without panes
   are the common case in the wild; our shim is an opt-in enhancement
   (and forced on Caio's machine via zshrc + settings). Observability
   must stay pane-independent — ours is (file-based).
4. **Still no external injection API.** Cross-session sockets are
   own-child-only (2.1.224 research holds). Upstream: #88331 zero
   traction, #88332 open with only our own cross-ref. Changelog
   2.1.224→now: QoL only (`notify_when_idle`, refusal reporting,
   teammates inherit the lead's model). Do not plan around upstream
   movement.
5. **Nobody ships**: mailbox-graph rendering, native task-DAG rendering,
   task-status-lag (deadlock) detection, black-box recorder surviving a
   lead crash. Still whitespace, unchanged since July.
6. **Best steering UX in the field**: cmux's Feed (unified attention
   queue) and one-key jump-to-neediest-pane. Best ambient UX: zellaude's
   priority-differentiated rendering (distinct pulse for
   permission-prompt vs steady working color).

## Doc-verified upgrades (official docs pasted by Caio, 2026-08-20)

- **Own-child socket injection is documented.** A hook or Bash child of a
  session may post into that session's inbox socket
  (`CLAUDE_CODE_MESSAGING_SOCKET` + `CLAUDE_CODE_MESSAGING_TOKEN`,
  first-line auth frame). Team hooks run as the lead's children → our
  hook handler can compute signal-engine facts and deliver them to the
  lead as a real message. Hook enrichment (idea 1 below) therefore has a
  **documented mechanism**, not a feature-detect hope. Caveat: delivery
  passes inbound controls; `bypassPermissions` sessions hold unverified
  messages.
- **Team config dir is removed on clean session end**; task list dir
  persists (retention-swept). Stale `~/.claude/teams/*` dirs = crashed
  or killed sessions only. Persisting task dirs strengthen the
  recorder/black-box case.
- **Exit-2 gating on TeammateIdle/TaskCreated/TaskCompleted is
  official** (feedback goes to the teammate, keeps it working /
  blocks the transition).
- **Teammates load project/user skills and MCP** (subagent-definition
  `skills`/`mcpServers` frontmatter is ignored for teammates) — a
  project-level herdmates skill reaches the lead AND every teammate.
- `--teammate-mode` flag exists (experimental); teammates inherit the
  lead's model and effort; `notify_when_idle` for cross-session waits.

## AMQ stack patterns (amq / amq-squad / amq-noc, read 2026-08-20)

Their doctrine lines we should steal outright:

- **"Terminals are runtime surfaces, not the source of truth."** The
  durable rail (files) is authoritative; pane injection is best-effort.
  Matches our team-files-first architecture; adopt the phrasing.
- **Dual-channel directives**: deliver to the lead's live surface when
  up, durable mailbox when down — their `direct-lead` = our nudge
  fallback shape.
- **Skill-first CLI integration**: amq ships an agent skill so agents
  drive the queue themselves — same shape as our lead-facing skill.
- **Operator gates**: leads may plan/dispatch/review, but pushes, tags,
  releases, merges need verified human approval bound to an exact
  action+target. Our hook-gating (exit-2, default-off) could grow into
  this; post-v3 idea.
- **Preview-first, confirm-gated controls** everywhere (their launch
  plans default to No). Matches our confirmed-nudge doctrine.

We do NOT adopt their stack: they own a parallel mailbox/team layer;
our charter is native Claude Code teams only.

## The central re-frame: the lead IS mission control

Every surface we've struggled with (pane-board, focus pane) was a
human-facing UI built *beside* the native team. The native lead session
already ships the core of mission control: an in-terminal agent panel,
the task list (ctrl+t), TeammateIdle notifications, and — decisively —
**native SendMessage to every teammate**. What the lead *lacks* is
exactly what our signal engine produces: reasons ("stalled 12m,
unread inbox, permission-prompt"), deadlock detection, and a durable
record.

Proposed integration: **herdmates feeds the lead instead of competing
with it.**

1. **Hook enrichment (push facts into the lead's context).** Our
   `herdmates hook` handlers run inside the lead session when
   TeammateIdle/TaskCreated/TaskCompleted fire. Instead of only
   spooling, they can return context so the lead *sees the reason*
   alongside the native event: "e2e-probe idle — signal engine:
   permission-prompt in pane w1A:p9, 4m; task #3 blocked on #2".
   *Needs a live spike: which team hook events feed
   additionalContext/systemMessage back to the model (documented for
   some hook types; unverified for team hooks). Exit-2 gating is
   documented and stays default-off.*
2. **Skill for the lead (pull facts on demand).** `skills/herdmates`
   grows lead-facing verbs: `herdmates why <agent>`, `herdmates
   deadlocks`, `herdmates roster` — explicit team from the lead's own
   session id, so team resolution is never ambiguous. The lead answers
   "who's stuck and why?" by running our CLI.
3. **Steering goes through the lead, natively.** The human tells the
   lead (or the lead decides); the lead uses native SendMessage. Our
   undocumented inbox-file append (`inbox-write`) stops being the
   steering path — it survives, if at all, only as a break-glass verb
   for a wedged lead. This shrinks our dependence on the fragile
   undocumented inbox contract (#88332 keeps standing as the ask that
   would legitimize the break-glass path).
4. **Human keeps the ambient layer, herdr-native.** Sidebar tokens
   stay (hook-driven, already debounced). Add priority-differentiated
   states (permission-prompt distinct from working/idle — zellaude
   pattern) via `--state-label`. "Jump" is herdr's own pane focus; no
   TUI needed.
5. **Recorder = the black box.** Native resume famously fails for
   in-process teammates; an external append-only record of classified
   observations is the only durable trace after a lead crash. Nobody
   ships this. Keep, keep cheap.
6. **Deadlock detection** (task done but never marked complete,
   dependents blocked — documented native bug): signal-engine feature,
   surfaced BOTH as a sidebar badge and through hook
   enrichment/skill to the lead.

Why this dissolves the #118 problem: every surface is scoped naturally —
hooks fire *in* a lead session (its team is known), sidebar tokens are
per-pane, skill verbs resolve the team from the lead's own session.
Nothing ever has to guess "which team" again. The only surfaces that
needed guessing were the human-facing aggregators we just dropped.

## Proposed new base (for ADR-0015)

| Layer | Modules | Fate |
|---|---|---|
| Host | shim (`teammux`, `teammux-launch`), doctor | keep; fix #117 |
| Facts | signal engine, team-file gather, hook companion/spool | keep |
| Record | recorder | keep (black-box framing) |
| Ambient (human) | sidebar tokens via `pane report-metadata` | keep; add priority states |
| Brain (lead) | hook enrichment + lead-facing skill verbs | **new — the replacement for board/focus/steering UIs** |
| Break-glass | `inbox-write` nudge | demote or delete (open) |
| Deleted | pane-board TUI, focus pane, jump, legacy v1 (15 modules) | delete outright |

## Resolutions (Caio, same day)

1. **Steering is lead-centric, bidirectional info flow** ("it goes back
   and forth of info BUT the team lead leads"). Nudge follows the
   amq-noc `direct-lead` pattern: deliver through the lead when it's
   live; inbox-write survives only as the lead-down fallback channel.
2. **ADR first, spike after** — hook enrichment is feature-detect-first
   (ADR-0010); the skill (pull) path works regardless.
3. **Recorder: engine observations only.** Caio flagged the strong
   resemblance to omriariav/amq-noc (NOC console over the amq/amq-squad
   stack). Verdict: **pattern donor, not scope threat** — steal their
   5-state status vocabulary (online/needs-you/blocked/waiting/stale),
   the dual-channel direct-lead delivery, and preview-first
   confirm-gated controls; our substrate stays native CC team files
   (their stack owns its own mailbox layer, different niche).
4. **Release shape: v3.0.0 re-baseline first** (deletions + #117 fix +
   ADR-0015 + docs), lead-brain features in v3.1+.

Decisions codified in ADR-0015.
