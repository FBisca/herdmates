# Hook enrichment via own-child socket post — 2026-08-20

Evidence for issue #123 (ADR-0015 §"brain layer" item 1: the push path).
Authority tags per ADR-0010: `[live]` = observed behavior on this machine,
`[source]` = read out of the shipped Claude Code bundle, `[doc]` = official
documentation.

Environment: Claude Code **v2.1.237**, Linux (CachyOS), session socket
`$XDG_RUNTIME_DIR/cc-socks/<pid>.sock`.

## 1. Mechanism

A Claude Code session binds a per-session Unix domain socket and exports the
address plus a child token to its own child processes:

- `CLAUDE_CODE_MESSAGING_SOCKET` — path to the session's inbox socket.
- `CLAUDE_CODE_MESSAGING_TOKEN` — the child token for the auth frame.

Only a session's own child may use them; there is no general external-process
injection API (`docs/research/cross-session-messaging-2026-08-20.md`; upstream
#27441/#53049 unshipped) `[doc]`. Team hooks run as the lead's children, which
is what makes `herdmates hook` able to talk to the lead at all.

### Wire protocol `[source]`

Newline-delimited JSON over the stream socket, at most two lines:

```text
{"type":"auth","token":"<CLAUDE_CODE_MESSAGING_TOKEN>"}
{"type":"user","message":{"role":"user","content":"<text>"}}
```

This is not in the public docs. It is the literal injection recipe the bundle
logs at inbox startup, extracted from the v2.1.237 binary on 2026-08-20:

```text
[uds-messaging] Inject messages (auth line REQUIRED|optional here):
{ echo '{"type":"auth","token":"'"$CLAUDE_CODE_MESSAGING_TOKEN"'"}';
  echo '{"type":"user","message":{"role":"user","content":"hello"}}'; }
| socat - UNIX-CONNECT:<socket>
```

Two further facts from the same region of the bundle `[source]`:

- **The auth line is conditional.** The log line renders `REQUIRED` or
  `optional` from the inbox's own `authRequired` state; when publishing the
  key file fails, the inbox runs in a degraded mode where "peers will send
  unauthenticated". Hence `SocketTarget::token` is an `Option` and the auth
  frame is omitted rather than faked when no token is exported.
- **Held messages get receipts.** The inbox can answer a post with a
  `peer_message_status` of `held` / `denied` / `expired` ("Your message is
  held for the recipient user's approval before it reaches their Claude
  session (permission-mode parity)"). Our client drains and discards any
  reply — a held enrichment is not an error worth failing a hook over.

## 2. What the fake-server test proves

`src/lead_post.rs` tests, no Claude Code involved:

- `frames_are_the_auth_line_then_the_user_message_line` — exact two-line
  NDJSON encoding, newline-terminated.
- `frames_omit_the_auth_line_when_no_token_was_published` — degraded mode.
- `frames_escape_quotes_and_newlines_in_the_message_text` — a multi-line
  enrichment stays one frame (serde_json escaping, not string concatenation).
- `post_writes_both_frames_and_half_closes` — against a real `UnixListener`
  stand-in: both frames arrive in order and the client half-closes, so the
  server's `read_to_string` returns instead of hanging.
- `post_errors_instead_of_panicking_when_nothing_is_listening`.

`src/team_hook.rs` adds the end-to-end pair:
`hook_command_from_posts_the_enrichment_to_the_inbox_socket` (a real
`TeammateIdle` payload + a blocked owned task arrives at a fake inbox as one
`user` frame carrying the engine's reason) and
`a_dead_inbox_socket_still_spools_and_still_exits_zero`.

## 3. What the live probe proves `[live]`

`cargo test --lib -- --ignored --nocapture live_socket_post_probe`, run
2026-08-20 22:11 from inside a live Claude Code v2.1.237 session (the test is
`#[ignore]`d precisely because it posts into whatever session hosts the
runner):

```text
socket=/run/user/1000/cc-socks/955870.sock token_present=true
reply_bytes=0 reply=""
```

The post was **delivered to the receiving session's model**, confirmed in that
session's transcript:

```json
{"type":"queue-operation","operation":"enqueue","content":"[herdmates #123 spike probe] …"}
{"type":"user","isMeta":true,"promptSource":"system","permissionMode":"default",
 "origin":{"kind":"peer","from":"unknown","verifiedPeerPid":1248675},
 "message":{"role":"user","content":"Another Claude session sent a message:\n[herdmates #123 spike probe] …"}}
```

So, live-verified:

1. An own-child process can connect, authenticate, and post; the inbox accepts
   the frames and closes with no reply bytes.
2. The message reaches the receiving session's model as a real turn — it is
   enqueued while the session is mid-turn and delivered after.
3. Linux verifies the poster by **process evidence**: the delivered turn
   carries `verifiedPeerPid` = the posting process's pid, recorded even though
   that process had already exited by delivery time.
4. Claude Code **re-frames** the post: it is presented to the model as
   `Another Claude session sent a message:` with a peer-permission warning
   appended, tagged `origin.kind = "peer"`, `from = "unknown"`. It is *not*
   presented as the user typing. Our `[herdmates]` message prefix is therefore
   load-bearing — it is the only thing identifying the sender.

## 4. Still pending live verification

- **Inbound-control hold.** The probe session ran `permissionMode: "default"`
  and the message was delivered immediately. The documented hold path
  (`crossSessionInbound` accept/hold/refuse; the bundle's `held`/`denied`/
  `expired` receipts) was **not** exercised. A `bypassPermissions` lead may
  hold the enrichment for approval, which would make enrichment lag or
  silently not appear. Untested.
- **From inside a real team hook.** The probe ran from `cargo test`, a child
  of a Claude Code session — the same relationship a hook has, but not
  literally a `TeammateIdle` hook invocation. Whether hook children get the
  same env (they should: the bundle unions these vars into the hook
  environment) is unverified live.
- **Which session a teammate's hook reaches.** A hook fired inside a
  *teammate* session posts into that teammate's socket, not the lead's. The
  lead-facing value therefore depends on the lead firing the hook (or on
  team-level hook registration semantics). Not measured.
- **Naming the sender.** No frame field was found that would replace
  `from: "unknown"` with `herdmates`. Not searched exhaustively.
- **Rate/volume.** No idea what a busy team's event rate does to the lead's
  context. Enrichment is one short message per event today; if a wave of
  `TaskCompleted` events floods the lead, throttling becomes necessary.

## 5. Live-spike runbook (for Caio)

Run inside a **real lead session** with an experimental team up:

```bash
# 1. Direct socket check, from the lead's own terminal (child process):
cd ~/Projects/herdmates
cargo test --lib -- --ignored --nocapture live_socket_post_probe
# expect: socket=… token_present=true, and the probe text appearing in
# the lead's conversation as "Another Claude session sent a message".

# 2. Full path, through the registered hook:
export CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1
# with `herdmates hook TeammateIdle` registered user-scope, spawn a
# teammate, give it a trivial task, and wait for it to go idle.
# expect: a "[herdmates] TeammateIdle <name> — reason: …" turn in the
# lead, plus the usual spool line:
tail -1 "${XDG_STATE_HOME:-$HOME/.local/state}/herdmates/hook-spool/<team>.jsonl"

# 3. Hold-path check: repeat (2) in a lead started with
# --permission-mode bypassPermissions and see whether the enrichment is
# delivered, held for approval, or dropped.
```

If step 2 produces a spool line but no turn in the lead, the enrichment post
failed: the hook prints one stderr line
(`herdmates hook: enrichment post failed for …`) and still exits 0 by design,
so check the hook's stderr rather than looking for a non-zero exit.
