//! Own-child post into the hosting session's inbox socket (issue #123,
//! ADR-0015 §"brain layer" item 1 — the push path). A Claude Code session
//! binds a per-session Unix domain socket and hands its own children the
//! address (`CLAUDE_CODE_MESSAGING_SOCKET`) plus a child token
//! (`CLAUDE_CODE_MESSAGING_TOKEN`). Team hooks run as the lead's children,
//! so a hook — and only a hook, or another own-child — can inject a
//! message the lead's model sees. There is no general external-process
//! injection API (`docs/research/cross-session-messaging-2026-08-20.md`;
//! upstream #27441/#53049 unshipped), which is exactly why this module
//! exists instead of a daemon.
//!
//! ## Protocol
//!
//! Newline-delimited JSON over the stream socket, at most two lines:
//!
//! ```text
//! {"type":"auth","token":"<CLAUDE_CODE_MESSAGING_TOKEN>"}
//! {"type":"user","message":{"role":"user","content":"<text>"}}
//! ```
//!
//! Authority `[source]` per ADR-0010: this framing is not in the public
//! docs — it is the literal injection recipe the shipped Claude Code
//! bundle logs at inbox startup (`[uds-messaging] Inject messages (auth
//! line REQUIRED|optional here): { echo '{"type":"auth","token":"'"$CLAUDE
//! _CODE_MESSAGING_TOKEN"'"}'; echo '{"type":"user","message":{"role":
//! "user","content":"hello"}}'; } | socat - UNIX-CONNECT:<socket>`,
//! extracted from v2.1.237 on 2026-08-20 and recorded in
//! `docs/research/hook-socket-enrichment-2026-08-20.md`). The same bundle
//! shows the auth line is *optional* on platforms where the inbox could
//! not publish its key file, so the token is modelled as `Option` and the
//! auth frame is simply omitted when absent rather than faked.
//!
//! ## Honest degradation (ADR-0013)
//!
//! [`SocketTarget::from_env`] is a feature-detect (ADR-0010 preview rule):
//! no `CLAUDE_CODE_MESSAGING_SOCKET` in the environment means this build of
//! Claude Code did not bind an inbox for us, and every caller degrades to a
//! silent no-op. A refused connection, a timeout, or a session that holds
//! the message for user approval are all equally non-fatal — a hook must
//! never block or fail its host (see `team_hook`'s module contract).

use std::io::{Read as _, Write as _};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

/// A resolved own-child inbox socket. Construct only via
/// [`SocketTarget::from_env`] in production; tests build one against a
/// fake `UnixListener`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketTarget {
    pub path: PathBuf,
    /// `None` when the host published no child token — the auth frame is
    /// then omitted, matching the bundle's "auth is optional on this
    /// platform" degraded mode.
    pub token: Option<String>,
}

impl SocketTarget {
    /// Feature-detect the hosting session's inbox. `None` (the silent
    /// no-op case) when the socket env var is absent or empty — i.e. we
    /// are not running as a Claude Code session's child, or this build
    /// never bound an inbox.
    pub fn from_env() -> Option<Self> {
        let path = std::env::var_os("CLAUDE_CODE_MESSAGING_SOCKET")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)?;
        Some(Self {
            path,
            token: std::env::var("CLAUDE_CODE_MESSAGING_TOKEN")
                .ok()
                .filter(|token| !token.is_empty()),
        })
    }
}

/// Pure: the exact bytes [`post`] writes. Split out so the framing is
/// unit-testable without a socket (repo convention: pure logic separate
/// from process/IO code). `serde_json` does the escaping — `text` is
/// model-composed prose and will contain quotes and newlines.
pub fn frames(token: Option<&str>, text: &str) -> String {
    let mut out = String::new();
    if let Some(token) = token {
        out.push_str(&serde_json::json!({"type": "auth", "token": token}).to_string());
        out.push('\n');
    }
    out.push_str(
        &serde_json::json!({
            "type": "user",
            "message": {"role": "user", "content": text},
        })
        .to_string(),
    );
    out.push('\n');
    out
}

/// A hook is on Claude Code's own event path: every socket operation is
/// hard-bounded so a wedged or slow inbox can never stall the host.
const IO_TIMEOUT: Duration = Duration::from_millis(500);
/// Replies (delivery/hold receipts) are drained only so the peer sees a
/// clean close; nothing parses them today.
const MAX_REPLY_BYTES: u64 = 4096;

/// Connect, write the frames, half-close, drain whatever the inbox
/// answers. Returns the drained reply (usually empty) — the live spike
/// reads it, production ignores it.
pub fn post(target: &SocketTarget, text: &str) -> std::io::Result<String> {
    let mut stream = UnixStream::connect(&target.path)?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.write_all(frames(target.token.as_deref(), text).as_bytes())?;
    stream.flush()?;
    // Half-close so the peer sees end-of-request; the read side stays open
    // for a receipt.
    stream.shutdown(Shutdown::Write)?;
    let mut reply = Vec::new();
    let _ = stream.take(MAX_REPLY_BYTES).read_to_end(&mut reply);
    Ok(String::from_utf8_lossy(&reply).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_socket_path() -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "herdmates-lead-post-{}-{sequence}.sock",
            std::process::id()
        ))
    }

    // ── frames (pure) ────────────────────────────────────────────────────────

    #[test]
    fn frames_are_the_auth_line_then_the_user_message_line() {
        let out = frames(Some("tok"), "hello");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], r#"{"token":"tok","type":"auth"}"#);
        assert_eq!(
            lines[1],
            r#"{"message":{"content":"hello","role":"user"},"type":"user"}"#
        );
        assert!(out.ends_with('\n'), "every frame is newline-terminated");
    }

    #[test]
    fn frames_omit_the_auth_line_when_no_token_was_published() {
        let out = frames(None, "hello");
        assert_eq!(out.lines().count(), 1);
        assert!(out.starts_with(r#"{"message":"#));
    }

    #[test]
    fn frames_escape_quotes_and_newlines_in_the_message_text() {
        let out = frames(None, "line one\nsays \"hi\"");
        assert_eq!(
            out.lines().count(),
            1,
            "an embedded newline must not become a second frame"
        );
        let parsed: serde_json::Value = serde_json::from_str(out.trim_end()).unwrap();
        assert_eq!(parsed["message"]["content"], "line one\nsays \"hi\"");
    }

    // ── post against a fake inbox server ─────────────────────────────────────

    /// Proves the wire behavior end to end against a stand-in server: the
    /// exact two NDJSON frames arrive in order, and the client half-closes
    /// so the server sees EOF instead of hanging.
    #[test]
    fn post_writes_both_frames_and_half_closes() {
        let path = temp_socket_path();
        let listener = UnixListener::bind(&path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut connection, _) = listener.accept().unwrap();
            let mut received = String::new();
            // Returns only on EOF — i.e. only if the client half-closed.
            connection.read_to_string(&mut received).unwrap();
            received
        });

        let target = SocketTarget {
            path: path.clone(),
            token: Some("tok".to_owned()),
        };
        post(&target, "enrichment").unwrap();

        let received = server.join().unwrap();
        let lines: Vec<&str> = received.lines().collect();
        assert_eq!(lines.len(), 2);
        let auth: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(auth["type"], "auth");
        assert_eq!(auth["token"], "tok");
        let message: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(message["type"], "user");
        assert_eq!(message["message"]["role"], "user");
        assert_eq!(message["message"]["content"], "enrichment");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn post_errors_instead_of_panicking_when_nothing_is_listening() {
        let target = SocketTarget {
            path: temp_socket_path(),
            token: None,
        };
        assert!(post(&target, "x").is_err());
    }

    // ── live spike (ADR-0010: live = behavior) ───────────────────────────────

    /// The live half of the #123 spike. `#[ignore]`d because it posts a
    /// real message into whatever Claude Code session hosts the test
    /// runner — never on a plain `cargo test`. Run it from inside a lead
    /// session (see the runbook in
    /// `docs/research/hook-socket-enrichment-2026-08-20.md`):
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture live_socket_post_probe
    /// ```
    #[test]
    #[ignore = "posts a real message into the hosting Claude Code session"]
    fn live_socket_post_probe() {
        let Some(target) = SocketTarget::from_env() else {
            panic!(
                "no CLAUDE_CODE_MESSAGING_SOCKET in this environment — \
                 run this from inside a Claude Code session"
            );
        };
        println!(
            "socket={} token_present={}",
            target.path.display(),
            target.token.is_some()
        );
        let reply = post(
            &target,
            "[herdmates #123 spike probe] Own-child socket post reached this session. \
             No action needed — ignore this message.",
        )
        .expect("post to the hosting session's inbox socket");
        println!("reply_bytes={} reply={reply:?}", reply.len());
    }
}
