# Cross-session messaging vs herdmates — 2026-08-20

Claude Code v2.1.224 (2026-08-07) shipped stable cross-session peer
messaging (SendMessage/ListAgents between sessions; `crossSessionInbound`
accept/hold/refuse). Researched against official docs
(code.claude.com/docs: cross-session-messaging, agent-teams, settings)
by a claude-code-guide agent; authority `[doc]` unless noted.

## Facts that matter to us

- **Two separate mailbox systems.** Team inboxes
  (`~/.claude/teams/{team}/inboxes/{agent}.json`, persisted, validated
  on read) are for within-team messages only. Cross-session messages
  travel over per-session Unix domain sockets (same machine) or
  Anthropic servers (cross-machine) and are delivery-only — no
  documented disk persistence.
- **Teammates do NOT appear in a non-team session's ListAgents**; teams
  keep their own roster. A non-team session can still message a
  teammate that binds a cross-session socket, but the message lands in
  the socket, not the team inbox file.
- **No external-process injection API.** Only a session's own child
  process (hook/Bash) may POST to its inbox socket
  (`CLAUDE_CODE_MESSAGING_SOCKET` + `CLAUDE_CODE_MESSAGING_TOKEN` —
  an own-child carve-out, not a general API). A general external API is
  requested upstream (anthropics/claude-code#27441, #53049) but not
  shipped.

## Consequences for herdmates

1. **Nudge stays an inbox-file write.** A Rust binary cannot reach a
   session's cross-session socket; the undocumented team-inbox append
   remains the only channel. Upstream ask
   anthropics/claude-code#88332 stands unchanged — #27441/#53049 are
   precedent for its option (2) (CLI/IPC affordance) and are now
   cross-referenced there.
2. **Board/recorder evidence gap (known, accepted).** Cross-session
   messages to/from teammates bypass the team inbox files, so the
   mailbox tail can't show them. Delivery-only + undocumented
   persistence means there is nothing to observe; not a bug, a
   documented blind spot.
3. No impact on the shim, signal engine, or hooks.
