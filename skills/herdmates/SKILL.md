---
name: herdmates
description: "Host and observe Claude Code agent teams in herdr via the herdmates plugin. Use when the user mentions herdmates, teammux, the agent board / pane-board, the focus pane, nudging a teammate, or asks to launch, monitor, jump to, or record a Claude Code team running in herdr panes. Not for general herdr pane control (use the herdr skill) and not for orchestrating work yourself — native Claude Code teams own spawn, messaging, and lifecycle."
---

# Herdmates

Herdmates makes herdr the visible home of Claude Code's native agent
teams: every teammate lands as a real herdr pane (the teammux shim), and
a mission-control board observes the documented team files. It **never
re-implements native-team features** — spawn, mailboxes, membership, and
lifecycle belong to Claude Code; herdmates reads
`~/.claude/teams/{team}/` and drives the herdr CLI.

For general pane/agent control (splits, `pane run`, `agent prompt`), use
the `herdr` skill — or print the one bundled with the running binary:
`herdr --skill` (herdr ≥ 0.8.0). This skill covers only the herdmates
surfaces.

## Preconditions — check before any command

```bash
command -v herdmates            # binary on PATH (~/.local/bin)
test -n "${HERDR_PANE_ID:-}"    # inside a herdr-managed pane (pane surfaces only)
```

- No binary → the plugin is not installed. Install with
  `herdr plugin install caioniehues/herdmates` (runs
  `cargo install --path . --root "$HOME/.local"`; a plain `cargo build`
  is NOT enough — hooks resolve the bare `herdmates` name via PATH).
- Native teams are experimental: spawning teammates requires
  `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` in the lead session's
  environment. Without it there are no team files and every board is
  honestly empty.
- Outside herdr, `teammux-launch` is pointless: Claude Code falls back
  to in-process teammates. Say so and stop rather than launching anyway.

## Launch a team lead (teammux shim)

```bash
herdmates teammux-launch [claude args...]           # takeover (default): THIS pane becomes the lead
herdmates teammux-launch --split [claude args...]   # split a new pane for the lead instead
```

All arguments pass through to `claude` (`--resume` works). The launcher
puts a fake `tmux` on PATH plus a fake `TMUX` so Claude Code's
split-pane teammate mode (`teammateMode: tmux`) drives herdr instead:
each spawned teammate opens as a first-class herdr pane beside the lead.

Takeover requires unix exec; in environments without it, use `--split`.

**Do NOT use `herdmates spawn` / `status` / `msg` / `board` to start or
drive a team** — those are the frozen v1 legacy surface (ADR-0012); the
manifest still lists them but native Claude Code teams replaced them.
`teammux-launch` is the only way in; the lead's Claude session spawns
teammates itself.

## Observe a team

```bash
herdmates pane-board [--team <name>] [--interval-secs N]   # read-only TUI board
```

Four regions: overview line, per-agent rows with waiting-reason badges,
the native task list (`~/.claude/tasks/`), and the mailbox tail. Wakes
event-driven on hook-spool growth, falls back to polling. Keys: `j`/`k`
select agent · `g` jump to its pane · `n` nudge (confirm `y`/`Enter`,
cancel `Esc`) · `q` quit.

Badges come from the signal engine (single source of teammate-state
truth; strict precedence permission-prompt > blocked > stalled >
turn-complete). Doctrine: **never display a wrong reason** — a teammate
the engine cannot classify shows reason-less "waiting", and that is
correct behavior, not a bug to work around.

With several live teams and no `--team`, resolution refuses to guess and
lists candidates — pass `--team` explicitly.

Other observation surfaces:

- **Sidebar tokens** — the plugin's event hooks publish `$task`/`$status`
  per team lead (source id `herdmates-board`); rendering needs the
  `[ui.sidebar.agents]` table from `docs/sidebar-rows.toml` merged into
  `~/.config/herdr/config.toml`, then `herdr server reload-config`.
  Invalid token names fail SILENTLY (`"partial"` reload keeps the old
  layout) — after any config edit, verify with `herdr config check`.
- **Focus pane** — `herdmates focus`: the human's single next action +
  decision queue from `~/.local/share/herdmates/focus.md`, plus the
  attention queue (`j`/`k` move, `Enter` jump, `d` mark done). Fed by
  the same signal engine as the board, so the two cannot disagree. The
  `atomizer` skill writes the focus file; never hand-edit it.
- **Jump** — `herdmates jump`: focuses the workspace+tab of the
  highest-priority attention item that has a pane. Herdr cannot focus
  finer than a tab; landing on the right tab is the contract.
- **Recorder** — `herdmates record --team <name> [--interval-secs N]
  [--log-path P]`: append-only JSONL of classified deltas at
  `~/.local/state/herdmates/recorder/{team}.jsonl`. Deltas only
  (baseline / transition / task_delta / hook signal), never every-tick
  spam. The log grows without bound — there is no built-in rotation;
  delete or archive old team logs yourself.

## Hooks (already wired — do not re-register)

The plugin manifest registers `herdmates hook <event>` for Claude Code's
three team hook events (`TeammateIdle` / `TaskCreated` /
`TaskCompleted`); they append to a per-team spool that the board and
recorder consume. Exit-2 gating capability exists but ships default-off.

**Stale-binary trap:** hooks run whatever `herdmates` is on PATH. After
any source change, `cargo install --path . --root "$HOME/.local"` again
or live hooks keep running the old binary silently. No herdr-side reload
is needed for a binary-only change (hooks re-exec fresh each event);
`herdr plugin link .` is required only after editing
`herdr-plugin.toml` (herdr caches the manifest at link time).

## The one write: confirmed nudge

The board's `n` key is the ONLY team-file write herdmates performs — an
appended inbox message, human-confirmed, under an OS advisory lock with
read-modify-atomic-rename. Everything else is read-only by doctrine.
Upstream does not document a contract for external inbox writers
(verified against official docs 2026-08-20), so treat the nudge as
best-effort: if a teammate seems not to have received one, check its
inbox file rather than re-nudging in a loop.

## Failure handling

| symptom | meaning | action |
| --- | --- | --- |
| `command -v herdmates` empty | not installed, or install used `cargo build` | `herdr plugin install caioniehues/herdmates`, re-check PATH |
| board shows no team | no live team, or `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` unset in the lead, or the team's session ended (config is removed at session end) | verify the env var in the lead pane, then `ls ~/.claude/teams/` |
| team resolution lists candidates | several live teams | re-run with `--team <name>` — never pick one silently |
| board stale while agents work | hooks running a stale or missing binary | reinstall the binary, confirm the manifest hooks fire (`~/.local/state/herdmates`) |
| sidebar rows unchanged after config edit | silent `"partial"` reload from an invalid token name | `herdr config check`; token is `state_text`, not `state_label` |
| teammates spawn in-process instead of panes | launched without the shim, or outside herdr | relaunch via `teammux-launch` from inside a herdr pane |

## Version discipline

Shim behavior was live-verified against Claude Code 2.1.211 and herdr
0.7.4 (evidence under `docs/research/`). Both dependencies move fast —
after upgrading either, re-verify the shim end-to-end (spawn one
teammate, confirm it lands as a pane) before trusting it, and re-snapshot
`docs/herdr-api-schema.snapshot.json` after any herdr update. When live
behavior disagrees with this file, live behavior wins (ADR-0010); update
this file.
