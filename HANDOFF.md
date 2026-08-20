# Handoff — current state

Updated 2026-08-20 (post-v2.3.0, wave complete + human tickets resolved).

## Where things stand

- **v2.3.0 RELEASED** (pushed + tagged 2026-08-20): the full 2026-08-20
  wave (#104, #106–#110, #112–#115).
- **#105 CLOSED — shim E2E re-verified on Claude Code 2.1.237 + herdr
  0.8.0-preview.2026-08-18 (protocol 20, snapshot matches)**: plugin
  reinstalled, 3 team hooks registered in ~/.claude/settings.json
  (manual step — the plugin manifest does NOT register Claude Code
  hooks), doctor all-OK, teammate landed as a real pane, hooks spooled,
  pane-board rendered the live team. Evidence:
  `docs/research/shim-e2e-2026-08-20.md`. Dogfooding is LIVE again.
  Two bugs found → **#117** (zsh autocorrect hangs `--split` launch),
  **#118** (plugin-pane board dies silently with >1 team dir) — both
  ready-for-agent, the open backlog.
- **#111 CLOSED**: upstream asks filed as anthropics/claude-code#88331
  (pluggable teammate backend) and #88332 (inbox writer contract).
- **#116 CLOSED (ADR-0014)**: legacy v1 surface fenced behind cargo
  feature `legacy-v1` (default-on; installed binary unchanged). 15
  modules gated; CI runs clippy+tests in both configs (496 vs 316
  tests); live→legacy calls fail to compile without the feature
  (mutation-verified). Delete criterion: legacy verbs unused for a
  release or two of dogfooding. Committed local, unpushed.
- **NEXT: #117 + #118 (agent-ready), then keep dogfooding.** Unpushed
  commits on main (E2E evidence + ADR-0014 gate) batch into the next
  release on Caio's word.

## How to resume

1. `gh issue list` — the wave backlog, labels per
   `docs/agents/triage-labels.md`; blockers named in bodies.
2. `git log --oneline -15` — commit subjects narrate the build.
3. Project state canon: auto-memory
   (`~/.claude/projects/-home-caio-Projects-herdmates/memory/`), loaded
   automatically.
4. `docs/adr/0013-north-star-mission-control.md` + `docs/spec.md` — the
   north star; ADR-0012 for the pivot context.
5. `docs/learnings/` — per-issue wave learnings, newest first.

## Standing rules (unchanged)

- Pushes to `main` are releases: gate (fmt/clippy/tests — now also CI,
  `.github/workflows/ci.yml`), bump manifest version on behavior change,
  tag. **No push without Caio's word.**
- Hooks resolve the bare `herdmates` binary via PATH — reinstall
  (`cargo install --path . --root ~/.local`) after src changes or live
  hooks run stale.
- Frozen legacy surface per ADR-0012; its disposition is decision ticket
  #116. New evidence → new ADR, ask Caio.
- Re-verify after upstream updates: re-snapshot the herdr API schema
  after any herdr update; re-run the shim E2E after any Claude Code
  update (see `skills/herdmates/SKILL.md` "Version discipline").
