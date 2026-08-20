# CONTEXT.md — domain glossary

Ubiquitous language for **herdmates** (formerly herdr-agent-team). One
meaning per word; challenge drift here first. Pivot vocabulary first;
legacy orchestration terms (still accurate for the v1.x surface) below.

## Herdmates (pivot vocabulary, ADR-0012)

- **Herdmates** — this plugin: Claude Code teammates, native in herdr.
  Base surfaces per ADR-0015: shim, signal engine + recorder + hook
  companion, sidebar tokens, plus the brain layer (below). The TUI
  board and focus pane surfaces named in earlier drafts of this file
  are deleted (#120); see the Brain layer section.
- **Native team** — a Claude Code agent-teams session (lead + teammates)
  as shipped by Anthropic: spawn, mailboxes, lifecycle all upstream. We
  never re-implement it; we host and observe it.
- **Lead** — the Claude Code session that spawns and coordinates a native
  team (Anthropic's term). Replaces "god" in pivot vocabulary.
- **Teammate** — a full independent Claude Code session spawned into a
  native team by the lead. In split-pane mode it occupies its own pane and
  is directly steerable.
- **Teammate mode** — Claude Code's display setting (`teammateMode`):
  `in-process` (rendered inside the lead's terminal) or split-pane
  (tmux/iTerm2 today). The shim's goal: make herdr a working split-pane
  host.
- **Shim** (mechanism name: **teammux**) — a fake `tmux` executable +
  `TMUX` env inside herdr panes that translates Claude Code's tmux
  invocations into `herdr pane` CLI calls, so teammates land as real herdr
  panes.
- **Recon spike** — the shim's go/no-go gate: log every tmux invocation a
  real team session makes (logging wrapper in real tmux), produce the verb
  inventory + verb→herdr mapping. Kill signals: control mode (`tmux -C`)
  or unmappable verbs.
- **Team files** — the documented on-disk state of a native team:
  `~/.claude/teams/{team}/config.json` (members, session IDs, pane IDs)
  and `inboxes/{agent}.json` (mailboxes). The boards' data source.
- **Agent board** *(removed by ADR-0015, #120)* — was "what are the
  agents doing?", two forms: sidebar-token board (D1, kept — see
  **Sidebar token** below) and TUI board (D2, deleted; #118's
  multi-team ambiguity was structural, not fixable in the surface).
- **Focus pane** *(removed by ADR-0015, #120)* — was "what should the
  human be doing?": current task, single next action, decision queue.
  Superseded by the native lead + brain layer.
- **Focus file** *(removed by ADR-0015, #120)* — was the plain-file
  contract the focus pane rendered and the atomizer skill wrote.
- **Atomizer** *(removed by ADR-0015, #120)* — was the companion skill
  breaking a task dump into a single next action for the focus file.
- **Plugin pane** — herdr surface: a plugin-declared TUI process in a
  herdr-managed pane (`plugin.pane.open`; placements overlay/split/tab/
  zoomed). Real pane, full pane APIs.
- **Popup pane** — herdr surface: session-modal floating pane, no pane id,
  invisible to pane/agent APIs, swallows all input, dies with its command.
  Quick-glance only.
- **Sidebar token** — named display value (`--token name=value`, rendered
  as `$name` in `[ui.sidebar.agents] rows`) attached to a pane via
  `pane report-metadata`. Display-only; never semantic state.

## Mission control (north-star vocabulary, ADR-0013; board/focus pillars
superseded by ADR-0015, see Brain layer section below)

- **Mission control** *(superseded by ADR-0015)* — was the monitor/
  steer/gate stack over a native team: signal engine + board + focus
  pane + inbox-write steering + recorder + hook companion. The base is
  now the Host/Facts/Record/Ambient/Brain layers in ADR-0015.
- **Signal engine** — the shared library module that classifies each
  teammate into one waiting-reason class from team files, herdr CLI,
  and transcript mtime. Single source of blocked/stalled facts; the
  recorder, sidebar tokens, and brain layer all consume it, none
  re-derives.
- **Waiting-reason** — the one badge a teammate carries: four classes,
  precedence top-down — permission-prompt (pane-backed only, native
  herdr Blocked) > blocked-on-dependency (idle + owned task with
  incomplete `blockedBy`) > stalled > turn-complete (unbadged).
  Never display a wrong reason; degrade to reason-less "waiting".
- **Quiet / stalled** — the two stalled tiers: quiet (soft, 5 min,
  tolerates false positives) and stalled (hard, 10 min, all signals
  must hold). Liveness ground truth = session-transcript mtime;
  unread-inbox entry older than ~2 min accelerates quiet→stalled.
- **Status-lag deadlock** — the failure mode the stalled class exists
  to surface: a team quietly wedged (mailbox not draining, task never
  progressing) while every teammate looks idle.
- **Inbox-write steering** *(demoted by ADR-0015 to break-glass
  only)* — nudging a teammate by writing to its mailbox under `.lock`
  + read-filter-atomic-rename discipline. Human-confirmed only.
- **Suggested nudge** *(dormant — depended on the deleted TUI board)*
  — a pre-composed inbox message offered for a stuck teammate; the
  human reviews and confirms before any write.
- **Recorder** — minimal append-only log of the signal engine's
  classified observations (state transitions, reason changes,
  task-file deltas). Log schema = engine output schema. Replay UI is
  post-v1.
- **Hook companion** — the plugin-shipped hooks on the three team
  events (`TeammateIdle`/`TaskCreated`/`TaskCompleted`): a push
  source into engine + recorder; exit-2 gating capability exists but
  ships default-off.
- **Honest proxy** — the only progress rendering allowed: done/total
  counts and per-task elapsed. ETA time prediction is banned.
- **JSONL tier** — the deferred post-v1 surfaces requiring session-
  JSONL parsing: context bar, cost footer, per-agent activity tail.
  V1 uses transcript mtime as a stat only.
- **Attention queue** *(removed by ADR-0015, #120)* — was a focus-pane
  term (per #90 disposition): the ordered human-needing items the
  focus pane rendered below the single next action.

## Brain layer (ADR-0015, v3.1+)

The base ADR-0015 defines to replace the board/focus pillars above.
Herdmates feeds the native lead instead of building parallel
human-facing team UIs; the layer table is Host (shim, doctor) / Facts
(signal engine, gather, hook spool) / Record (recorder) / Ambient
(sidebar tokens) / Brain (below) / Break-glass (inbox-write).

- **Brain layer** — the lead-facing enrichment layer: push (hook
  enrichment) and pull (lead-facing skill), both resolving the team
  from the lead's own session id, so no surface ever guesses "which
  team". Not yet implemented (v3.1+ per ADR-0015 rollout).
- **Hook enrichment** — the brain layer's push half: `herdmates hook`
  handlers, running as the lead's children on TeammateIdle/
  TaskCreated/TaskCompleted, compute signal-engine facts and post them
  into the lead's own inbox socket (`CLAUDE_CODE_MESSAGING_SOCKET`/
  `_TOKEN`) so the lead's model sees reasons alongside native events.
- **Lead-facing skill** — the brain layer's pull half: project-level
  skill verbs (`why <agent>`, `deadlocks`, `roster`) resolving the
  team from the calling session's own id. Teammates load the same
  skill; facts are read-only so that's fine.
- **Break-glass** — the inbox-file nudge's demoted role post-ADR-0015:
  a fallback channel only, used when the lead itself is down.

## Legacy orchestration vocabulary (v1.x surface, frozen at v1.1.0)

- **Team** — a named set of workers participating in one run, initially
  spawned from a spec or later adopted, plus their run state. One team ↔ one
  run dir.
- **Worker** — a single coding-agent CLI (claude, codex, …) running in its own
  Herdr workspace as part of a team. Identified by unique worker `name`.
- **God (agent)** — the user's main interactive agent session that coordinates
  the team: spawns, briefs, receives reports, decides. Exactly one per team;
  the plugin never spawns a god. (Term borrowed from herdr-orchestrate.)
- **Topology** — who may talk to whom. **Star**: workers ↔ god only. **Mesh**:
  workers also message each other peer-to-peer. Per-team flag; star is default.
- **Brief** — a per-worker instruction file the worker reads at launch.
  Delivered as a one-line pointer injection, never inline text.
- **Report** — a worker's durable output file at `<run>/inbox/<worker>.md`.
  Written by the worker before it goes idle/done.
- **Result ready** — a worker outcome whose report is finalized and safe for
  the god to consume. Mere report-path existence is not sufficient.
- **Completion sentinel** — a worker-emitted attention signal that follows a
  result becoming ready. It is not durable completion truth by itself.
- **Pointer injection** — the submission mechanism: one line typed into a pane
  naming durable file paths. Payload stays on disk; context stays lean.
- **Inbox** — the run dir's `inbox/` directory: report files + `events.jsonl`.
- **Run-board** — the durable record of a team run (`run.toml` + worker
  protocols + inbox): who was spawned, where, current lifecycle state.
- **Launcher table** — data-driven config mapping agent kind → launch argv,
  submission-verification policy, repository-authored AGENTS.md capability,
  and mid-turn queueability (`queues_midturn`). Adding an agent = adding a
  table entry.
- **Msg verb** — the plugin subcommand (`herdr-agent-team msg <target>
  <text>`) that is the only messaging channel workers are ever briefed on.
  Resolves name → pane, submits via `pane run`, verifies submission,
  readiness-gates per launcher policy (ADR-0008).
- **Outbox** — `<run>/outbox/<target>/` queue of pending messages for a
  target that can't safely receive mid-turn; drained in order by the status
  hook when the target flips idle/done. Counterpart of the inbox.
- **Queued** — an instruction is durably retained in the outbox but has not
  been submitted to its target.
- **Submitted** — Herdr has accepted the request to place an instruction into
  the target pane. This does not prove the worker read or acted on it.
- **Acknowledged** — the target worker has produced explicit evidence that it
  received an instruction.
- **Message lifecycle** — **Queued / Submitted / Acknowledged**. **Queued**:
  the message sits in the outbox awaiting drain (`MessageOutcome::Enqueued`).
  **Submitted**: the text was typed into the target pane's input and
  submission was verified per launcher policy — this is what the code and the
  durable audit event call `delivered` (**Delivered → Submitted**; the word
  is kept in `MessageOutcome::Delivered` and `events.jsonl` for
  compatibility).
- **Attention lifecycle** — the owned raise/observe/clear cycle of a worker's
  explicit attention request. Raised by the worker (`msg god <text>
  --attention`), persisted in durable run state (`attention_pending`),
  observable on the inbox/board and every metadata publish, and cleared only
  by an explicit god-side ack (`msg <worker> <text> --ack`). Status flips
  never consume it.
- **Queues mid-turn** — launcher-table property: whether a mid-turn `pane
  run` into that agent's TUI queues as a pending user message (claude:
  verified true; codex: verified true) or risks interrupting the turn.
- **Setup command** — team-spec command run inside each fresh worktree before
  the worker launches (project preflight: symlinks, deps, skip-worktree).
- **Worker protocol** — one immutable generated file per worker at
  `<run>/protocols/<worker>.md`: identity, report protocol, and (mesh only) the
  peer table + message envelope. It is passed by absolute-path pointer and is
  distinct from the repository's authored `AGENTS.md`, which remains untouched.
- **Status flip** — a Herdr agent-status transition (idle/working/blocked/
  done/unknown). Flips to `blocked`/`done` trigger the report flow.
- **Evidence hierarchy / authority tags** — claim authority labels: `live`
  (current observed behavior), `source` (local upstream source), and `preview`
  (runtime schema probe required), per ADR-0010.
- **Adopted worker** — an existing pane registered into a run as a full
  worker (`team adopt`, ADR-0009): protocol generated at adoption,
  `adopted = true` in run state. Kill releases it (notice injected)
  instead of closing its pane.
- **Released** — terminal lifecycle of an adopted worker after `team
  kill`: no longer a team member, pane untouched, report protocol void.
