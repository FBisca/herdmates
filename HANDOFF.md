# Handoff — current state

Updated 2026-08-21 (v3.0.0 SHIPPED; v3.1 built, unpushed; brain-layer wave
done).

## Where things stand (2026-08-21)

- **v3.0.0 released** 2026-08-20 (pushed + tagged): #117 sh-c fix, #119
  legacy-v1 deleted (−13.9k loc), #120 board/focus cluster deleted (−2.8k),
  #121 version bump + manifest cleanup (only `[[build]]` remains) + ADR-0015
  doc alignment. 259 tests at release.
- **v3.1 complete but UNPUSHED on local main (4 commits, 321 tests, gate
  clean)**: #122 brain verbs `why`/`deadlocks`/`roster` (src/brain.rs;
  team resolved from CLAUDE_CODE_SESSION_ID vs leadSessionId), #123 hook
  enrichment via own-child socket post (src/lead_post.rs + src/enrich.rs;
  LIVE-verified delivery incl. verifiedPeerPid; pending-live items + runbook
  in docs/research/hook-socket-enrichment-2026-08-20.md §4–5), #124 sidebar
  5-state priority tokens ("!! needs-you" marked; reuses brain::five_state,
  single-source rule #90). Release = Caio's word + bump to 3.1.0 + tag.
- **Open tickets**: #125 (dead auto-pump `maybe_pump` + dead metadata.rs +
  lead-only state rendering — delete-vs-rewire), #126 (Stop-hook/asyncRewake
  push spike — external→lead injection prior art from agent-teams-mcp; verify
  asyncRewake against official docs first), #127 (boundary-crossing QA pairs
  for our seams + runnable golden-rules checks; the #121 stale-manifest bug
  was this class).
- **Upstream**: posted the socket-seam findings on claude-code#88332
  (issuecomment-5362847742) — fact-checked against the evidence doc.
- **Team/process** (user-level, affects work here): agent roster in
  ~/.claude/agents/ (implementer/reviewer/researcher/scribe/miner) — full
  toolsets, SendMessage reporting, opus reviewer gate on every non-trivial
  diff (measured: it caught real bugs both waves). Playbook-skills design
  done, build pending — see ~/.claude/playbook-handoff-2026-08-21.md.

## Previous state (2026-08-20, superseded above)

- **ADR-0015 accepted (2026-08-20): feature re-baseline, "the lead is
  mission control."** Full evidence trail in
  `docs/research/feature-rebaseline-brainstorm-2026-08-20.md`. Summary:
  herdmates feeds the native lead instead of building parallel team UIs.
  Keep: shim + doctor, signal engine + hooks/spool, recorder (black-box
  framing), sidebar tokens (+ priority states later). New (v3.1+): hook
  enrichment via the documented own-child socket post into the lead,
  lead-facing skill verbs (`why`/`deadlocks`/`roster`). Demoted:
  inbox-write → break-glass only (lead-down fallback). Delete outright:
  pane-board TUI, focus pane cluster (focus_pane/focusfile/attention/
  audit/jump), legacy v1 (15 fenced modules + god/codex-prompting
  skills), skills/atomizer.
- **v2.3.1 plan is dead**; next release is the **v3.0.0 re-baseline**
  (deletions + #117 fix + docs), then v3.1+ brain layer. Work is
  ticketed on GitHub — `gh issue list`, the re-baseline tickets name
  their blockers.
- #118 closed wontfix per ADR-0015 (team-ambiguity dissolves
  structurally: no surface guesses "which team" anymore). #117 stays
  open, folded into the v3.0.0 wave (fix: wrap the `pane run` line in
  `sh -c '…'` — quoted words are never spell-corrected).
- Still unpushed on main: the pre-review commits (E2E evidence,
  ADR-0014 gate, cross-session research) + this session's docs. All
  ship together with v3.0.0 on Caio's word.
- Key research facts (verified 2026-08-20, citations in the brainstorm
  doc): own-child socket injection into a session is DOCUMENTED
  (`CLAUDE_CODE_MESSAGING_SOCKET`/`_TOKEN`); team config dirs are
  removed on clean session end, task dirs persist; teammates load
  project skills; in-process is the default teammate mode; no external
  injection API is coming (#88331/#88332 no traction). AMQ stack
  (amq/amq-squad/amq-noc) = pattern donor, not scope threat.

## How to resume

1. `gh issue list` — the re-baseline tickets; work blockers-first.
2. `docs/adr/0015-feature-rebaseline-lead-centric.md` — the new base.
3. `docs/research/feature-rebaseline-brainstorm-2026-08-20.md` — why.
4. ADR-0013 doctrine still applies (honesty, risk layering); its
   surface list is superseded by ADR-0015.
5. Implement each ticket in a fresh session/context (they are
   self-contained by design).

## Standing rules (unchanged)

- Pushes to `main` are releases: gate (fmt/clippy/tests + CI), bump
  manifest version on behavior change, tag. **No push without Caio's
  word.**
- Hooks resolve the bare `herdmates` binary via PATH — reinstall
  (`cargo install --path . --root ~/.local`) after src changes or live
  hooks run stale.
- New evidence → new ADR, ask Caio.
- Re-verify after upstream updates: re-snapshot the herdr API schema
  after any herdr update; re-run the shim E2E after any Claude Code
  update (see `skills/herdmates/SKILL.md` "Version discipline").
