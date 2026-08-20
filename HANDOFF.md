# Handoff — current state

Updated 2026-08-20 (post-v2.2.0, wave "audit + skill recon" in progress).

## Where things stand

- **v2.2.0 RELEASED** (pushed + tagged 2026-07-17): mission-control v1
  complete (stages 0–5, #95–#101), #102 resolve_team liveness filter,
  #103 teammux-launch takeover mode.
- **2026-08-20 investigation wave**: full codebase+strategy audit and a
  skill-recon pass (inspiration repos + official docs validation) ran;
  findings live as issues #104–#116. Reports (private artifacts):
  "Herdmates Field Audit" and "Herdmates Skill Recon" in Caio's artifact
  gallery.
- **Dogfood-in-anger RESUMED by decision 2026-08-20** — it had silently
  stalled (plugin uninstalled, zero commits 2026-07-18…08-19, #104
  untriaged). Reinstall + shim E2E re-run is #105 (human-present).
- **Wave progress (2026-08-20 EOD)**: CLOSED #104 #106 #107 #108 #109 #110
  #112 #113 #114 #115 — all committed local, 475 tests green, gate clean.
  #106 doctor (static) also CLOSED — 496 tests green.
  **NEXT: batch v2.3.0 release on Caio's word (bump Cargo.toml +
  herdr-plugin.toml, tag, push; manifest changed → relink after install).** Open human tickets: #105 (shim E2E +
  reinstall, human-present), #111 (upstream asks — drafts in issue
  comments), #116 (legacy disposition — recommendation in issue
  comments).
- Upstream drift to absorb: Claude Code 2.1.237 (shim proven on 2.1.211;
  three team-relevant changes since — see #105), herdr 0.8.2 at
  `herdrdev/herdr` (org moved, Apache-2.0, `herdr --skill`,
  `truncated:true` reads — see #108).

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
