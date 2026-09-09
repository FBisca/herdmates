# Herdmates

**Claude Code agent teams, native in [herdr](https://herdr.dev).**

Herdmates makes herdr the visible home of Anthropic's native Claude Code
agent teams. The teams own spawn, mailboxes, membership, and lifecycle —
herdmates **hosts** them (every teammate lands as a real, steerable herdr
pane) and **feeds the native lead** (sidebar tokens + a durable recorder
over the documented team files, ADR-0015 — no parallel board/focus UI).
It never re-implements native-team features; it reads the files Claude
Code already writes and drives the herdr CLI.

## The flow

1. Open herdr, type `claude` (via the wrapper below) — your pane becomes
   the **team lead**, running under the teammux shim.
2. Ask Claude to spawn teammates — each one opens as a **real herdr
   pane** next to you, not a hidden background process.
3. Watch the **sidebar tokens** for live per-agent state with honest
   waiting-reason badges, right beside the panes themselves.
4. Claude Code's team hooks **push** events into an append-only
   **recorder** log — no polling lag, a durable trace if the lead
   crashes.

## Install

```bash
herdr plugin install caioniehues/herdmates
```

The install step runs `cargo install --path . --root "$HOME/.local"` —
herdr resolves manifest commands via `PATH` only (no shell, no relative
paths), and `~/.local/bin` is where `herdr` itself lives. A plain
`cargo build` is not enough.

**Recommended shell wrapper** — makes `claude` inside a herdr pane launch
as a teammux lead automatically (plain `claude` everywhere else,
`command claude` to bypass):

```zsh
# ~/.zshrc
claude() {
  if [[ -n "$HERDR_PANE_ID" ]] && command -v herdmates >/dev/null 2>&1; then
    herdmates teammux-launch "$@"
  else
    command claude "$@"
  fi
}
```

## Surfaces

### Teammux shim

A fake `tmux` executable on `PATH` plus a fake `TMUX` environment, set up
by `herdmates teammux-launch`. Claude Code's split-pane teammate mode
(`teammateMode: tmux`) calls what it thinks is tmux; the shim translates
every verb into `herdr pane` CLI calls over the herdr socket. Native
teammates land as first-class herdr panes — proven live end-to-end
(spawn, blockedBy DAG, dismissal; evidence under `docs/research/`).

- `herdmates teammux-launch [claude args...]` — **takeover (default)**:
  the current pane becomes the lead; all args pass through to claude
  (`--resume` works).
- `herdmates teammux-launch --split [claude args...]` — split a new pane
  for the lead instead.
- Set `HERDMATES_TEAMMATE_LAYOUT=tab` before launching the lead to move each
  teammate into its own unfocused Herdr tab after launch. The default keeps
  Claude's split-pane layout.
- Herdr sidebar identity uses `lead` for the lead and
  `teammate:<name>` for teammates; Claude's visible pane title remains `<name>`.
- Herdr-only by design: the shim's output surface IS herdr panes.
  Outside herdr, Claude Code falls back to in-process teammates.

### Ambient observation

- **Sidebar tokens**: teammate state published via
  `pane report-metadata` — the herdr sidebar becomes a zero-rendering
  fleet board. See [Sidebar setup](#sidebar-setup).

The earlier TUI board and focus pane surfaces are deleted (#118/#120,
ADR-0015): a per-session team dir made "which team?" structurally
ambiguous at open time for any standalone surface. Herdmates now feeds
the native lead directly instead; a lead-facing skill + hook enrichment
("brain layer") is planned post-v3.0 — see
[ADR-0015](docs/adr/0015-feature-rebaseline-lead-centric.md).

### Signal engine, recorder, hooks

- **Signal engine** — single source of teammate-state truth. Four
  waiting-reason classes with strict precedence (permission-prompt >
  blocked > stalled > turn-complete), two-tier stalled detection on
  transcript mtime. Doctrine: **never display a wrong reason** — degrade
  to reason-less "waiting" instead.
- **Recorder** — `herdmates record --team <name>`: append-only JSONL log
  of the engine's classified deltas (baselines, transitions, task
  deltas, hook signals) at `${XDG_STATE_HOME:-~/.local/state}/herdmates/
  recorder/{team}.jsonl`. Log schema = engine schema. Deltas-only, so
  growth is bounded by state changes rather than poll ticks, but an
  unattended long-running `record` still grows the file forever by
  default — pass `--max-log-bytes N` to rotate it once to
  `{team}.jsonl.1` (overwriting any previous `.1`) whenever it reaches
  `N` bytes; omit it (or pass `0`) for the previous unlimited behavior.
- **Hook companion** — `herdmates hook <event>` registered for Claude
  Code's three team hook events (`TeammateIdle` / `TaskCreated` /
  `TaskCompleted`) spools events per team; the recorder consumes the
  spool. Exit-2 gating capability exists but ships **default-off** and
  has no blocking predicate in v1.

## Sidebar setup

The plugin's event hooks already publish two tokens per team lead under
source id `herdmates-board` (`$task`, `$status`). Rendering is your own
`~/.config/herdr/config.toml`: merge
[`docs/sidebar-rows.toml`](docs/sidebar-rows.toml)'s
`[ui.sidebar.agents]` table in, then `herdr server reload-config`.

Hard-won facts (verified live against herdr 0.7.4):

- **Invalid token names fail silently** — `reload-config` reports
  `"partial"` and keeps the old layout. If an edit "does nothing," run
  `herdr config check` and re-verify every name (`state_text`, not
  `state_label`).
- **Keep values telegraphic** — ~20 visible columns at default sidebar
  width; herdmates enforces an 80-char wire cap but truncation is not a
  layout strategy.
- **Absent tokens omit the row** (safe to always configure), and
  **agent-less panes never appear** in the sidebar at all.

## Doctrine (why it behaves the way it does)

- **Honesty first**: never a wrong reason, never a predicted ETA, no
  silent coverage caps. Ambiguity degrades to an explicit error (e.g.
  team resolution lists candidates rather than guessing).
- **Native teams are the substrate**: spawn/messaging/lifecycle belong
  to Claude Code; herdmates only reads documented team files and drives
  the herdr CLI (`HERDR_BIN_PATH` is the entire plugin API).
- **Evidence hierarchy**: live behavior > source > docs (ADR-0010).
- **Read-only by default**: herdmates performs no team-file writes
  today; the confirmed-nudge write path is a break-glass-only fallback
  (ADR-0015), not a routine surface.

## Documentation map

| Where | What |
|---|---|
| [`docs/spec.md`](docs/spec.md) | North-star specification (pillars, build order, cut line) |
| [`docs/adr/`](docs/adr/) | All architecture decisions with the why — start at [0013](docs/adr/0013-north-star-mission-control.md) (north star) and [0012](docs/adr/0012-pivot-to-herdmates.md) (the pivot) |
| [`CONTEXT.md`](CONTEXT.md) | Domain glossary (current vocabulary first, legacy below) |
| [`docs/research/`](docs/research/) | Verified upstream facts: tmux verb inventory, live E2E evidence, hook payload capture |
| [`docs/reviews/`](docs/reviews/) | Review program records, incl. the 2026-07-17 whole-codebase review |
| [`docs/learnings/`](docs/learnings/) | Per-issue wave learnings |
| [`docs/legacy/spec-v1.md`](docs/legacy/spec-v1.md) | Frozen v1.x orchestration spec (tombstone, ADR-0012) |
| [`herdr-plugin.toml`](herdr-plugin.toml) | The plugin manifest — commands, panes, event hooks |

## Development

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test
cargo install --path . --root "$HOME/.local"   # user-seat binaries
herdr plugin link .                            # refresh manifest cache
```

- Pure logic (file-contract parsing, token formatting, verb mapping)
  stays separate from process-spawning code — testable without a live
  herdr.
- Herdr caches the manifest at link time — relink after any
  `herdr-plugin.toml` change.
- Pushes to `main` are releases: gated, version-bumped, tagged.

## Legacy: v1.x team orchestration (removed)

The original plugin spawned heterogeneous coding-agent teams (Claude +
Codex) under a coordinating "god" session with push-based status
reporting. Frozen at v1.1.0 (ADR-0012), then deleted outright in
v3.0.0 (#119, ADR-0015) — git history is the archive. Spec preserved
at [`docs/legacy/spec-v1.md`](docs/legacy/spec-v1.md) for reference
only.

## License

MIT
