---
name: hook-payload-session-identity
description: Real hook-spool evidence that TeammateIdle payloads sometimes carry the lead's session_id/transcript_path, not the teammate's — attribution trap for signal-engine facts
metadata:
  type: project
---

Real spool data in `~/.local/state/herdmates/hook-spool/*.jsonl` shows two
TeammateIdle shapes: some entries carry the *teammate's* session_id +
transcript_path (`session-331dda90`, `session-5aa1f0cb`), while
`session-b1b58703.jsonl` has several distinct `teammate_name`s all sharing
one session_id equal to the lead's (the team name is derived from it).

**Why:** any code that treats `payload.transcript_path` as the named
teammate's liveness signal can compute a confidently wrong staleness reason
— exactly what ADR-0013's honest-degradation doctrine forbids.

**How to apply:** when reviewing enrichment/signal code fed by hook
payloads, check whether transcript-derived facts are guarded by "session_id
is the teammate's, not the lead's". Same evidence undercuts any claim that
team hooks always run as the *lead's* child (which socket a post reaches).

Related: [[verification-quirks]]
