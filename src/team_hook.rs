//! `hook` subcommand (issue #100 stage 5, ADR-0013 §93 stage 5): push
//! source for the signal engine/recorder, fed by Claude Code's three
//! team hook events (`TeammateIdle`/`TaskCreated`/`TaskCompleted`, no
//! matcher support — coordinator registers one entry per event, each
//! invoking `herdmates hook <EventName>`). The hook is an ephemeral
//! process: read the event JSON on stdin, append one line to a spool,
//! exit fast. It runs inside Claude Code's own event path for every
//! team on this machine once registered user-scope, so it must never
//! block: malformed input, an unreadable spool directory, or any other
//! failure logs a line to stderr and still exits 0. The only path to a
//! non-zero exit is the gating capability below, and in v1 that path is
//! unreachable — no blocking predicate is implemented (issue #100 scope:
//! gating-ON logic is explicitly out of scope; this ships the plumbing,
//! never the trigger).
//!
//! Live payload capture (2026-07-17, throwaway project-local hook,
//! `.planning/2026-07-17-100-hook-companion-builder/findings.md`) found
//! three distinct field-set shapes, not two:
//! - a task event (`TaskCreated`/`TaskCompleted`) fired from a
//!   **teammate** session: session_id, transcript_path, cwd, prompt_id,
//!   hook_event_name, task_id, task_subject, task_description,
//!   teammate_name, team_name.
//! - the *same* task event fired from the **lead's own** session (#100
//!   M5, live dogfood finding): identical shape minus `teammate_name`
//!   AND `team_name` — both absent, not empty. Matches issue #91's note
//!   that pre-2.1.178 `team_name` is deprecated and consumers are
//!   expected to derive team from `session_id` instead.
//! - `TeammateIdle`: adds `permission_mode`, drops every task field.
//!
//! That asymmetry is why the spool line wraps the raw payload as an
//! opaque [`serde_json::Value`] instead of three-or-more hand-modeled
//! structs — a typed struct per event would have had to special-case
//! both `permission_mode` and an absent `team_name` from day one; the
//! envelope tolerates any future field Claude Code adds for free. It's
//! also why the spool bucket can't just read `payload.team_name`:
//! [`resolve_team_bucket`] falls back to matching `session_id` against
//! every team's `leadSessionId` (`gather::list_team_dirs` +
//! `teamfiles::read_team_config`) when `team_name` is absent or empty —
//! the honest resolution, since the `session-<8 chars>` team-name
//! convention only holds for implicit session teams, not user-named ones.
//!
//! ## Enrichment push (issue #123, ADR-0015 §"brain layer" item 1)
//!
//! Since v3.1 the hook does one more thing before exiting: it computes
//! the signal-engine facts behind the event (`enrich`) and posts them
//! into the hosting session's own inbox socket (`lead_post`). Delivery
//! into a live session is proven; WHICH session a teammate's hook
//! reaches (the lead's, per the documented hooks-run-as-lead's-children
//! model) is still live-unverified — see
//! `docs/research/hook-socket-enrichment-2026-08-20.md` §4. Everything
//! about it is best-effort: the
//! spool write happens first and unconditionally, and any missing env,
//! unresolvable path, or failed post degrades to silence plus exit 0.
//!
//! Spool path mirrors `recorder::default_log_path`'s exact
//! `${XDG_STATE_HOME:-~/.local/state}/herdmates/...` convention,
//! deliberately independent of the herdr-plugin install location (see
//! that function's doc comment) — distinct subpath (`hook-spool/` not
//! `recorder/`) so the two JSONL streams are never colocated as files,
//! only merged in-memory by a consumer.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// One appended spool line: envelope + raw payload verbatim. See module
/// doc comment for why the payload is untyped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct HookEnvelope {
    pub spool_v: u32,
    pub event: String,
    pub captured_unix: u64,
    pub payload: Value,
}

const SPOOL_V: u32 = 1;

/// Pure: builds the envelope from an already-parsed payload. No I/O.
pub(crate) fn build_envelope(event: &str, payload: Value, now: SystemTime) -> HookEnvelope {
    HookEnvelope {
        spool_v: SPOOL_V,
        event: event.to_owned(),
        captured_unix: now
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0),
        payload,
    }
}

/// Pure: one JSONL line for `envelope`, no trailing newline.
pub(crate) fn envelope_line(envelope: &HookEnvelope) -> String {
    serde_json::to_string(envelope).expect("HookEnvelope always serializes")
}

/// Pure: `payload`'s own `team_name` field, when present and non-empty.
fn payload_team_name(payload: &Value) -> Option<&str> {
    payload
        .get("team_name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
}

/// Pure: which spool bucket `payload` belongs to. `team_name` wins when
/// present (the common case: task events fired from a teammate session,
/// and `TeammateIdle`). Otherwise (#100 M5: lead-session task events
/// carry no `team_name` at all) falls back to matching the payload's
/// `session_id` against `teams`' `leadSessionId`s — the only field a
/// lead-session payload reliably carries that ties it back to a team.
/// `teams` is `(team_directory_name, lead_session_id)` pairs, pre-read
/// by the impure [`discover_teams`] so this stays a pure function over
/// plain data. No match (or no `session_id` at all) degrades to the
/// fixed `"_unknown"` bucket — still worth capturing, never dropped.
pub(crate) fn resolve_team_bucket(payload: &Value, teams: &[(String, Option<String>)]) -> String {
    if let Some(name) = payload_team_name(payload) {
        return name.to_owned();
    }
    let session_id = payload.get("session_id").and_then(Value::as_str);
    if let Some(session_id) = session_id {
        if let Some((team, _)) = teams
            .iter()
            .find(|(_, lead_session_id)| lead_session_id.as_deref() == Some(session_id))
        {
            return team.clone();
        }
    }
    "_unknown".to_owned()
}

/// Impure: every team under `gather::GatherPaths::from_env`'s
/// `teams_root`, paired with its `leadSessionId` (`None` when the config
/// has none). Reuses `gather::list_team_dirs` + `teamfiles::
/// read_team_config` verbatim — no second XDG/teams_root derivation.
/// A team whose `config.json` fails to parse is skipped, same
/// degrade-on-malformed-file policy `gather::gather_team` already uses.
/// Cheap: a handful of small files, read once per hook invocation.
/// Every team under `teams_root`, paired with its `leadSessionId` (`None`
/// when the config has none). `teams_root` mirrors `gather::GatherPaths`'s
/// injectable-path pattern (issue #114 e2e test seam — a tempdir in
/// tests, [`hook_command`]'s real `GatherPaths::from_env().teams_root`
/// in production). A team whose `config.json` fails to parse is skipped,
/// same degrade-on-malformed-file policy `gather::gather_team` already
/// uses. `None` (root unresolved) degrades to no teams. Cheap: a handful
/// of small files, read once per hook invocation.
fn discover_teams_at(teams_root: Option<&Path>) -> Vec<(String, Option<String>)> {
    let Some(teams_root) = teams_root else {
        return Vec::new();
    };
    crate::gather::list_team_dirs(teams_root)
        .into_iter()
        .filter_map(|team| {
            let config_path = teams_root.join(&team).join("config.json");
            let config = crate::teamfiles::read_team_config(&config_path).ok()?;
            Some((team, config.lead_session_id))
        })
        .collect()
}

/// Pure core of the spool path, parameterized on the two env vars it
/// consults (same test seam as `recorder::default_log_path_from` —
/// mutating real process env vars would race other tests). `None` when
/// neither resolves; callers degrade to "skip the write, log it" rather
/// than treating an unresolvable HOME as fatal.
fn spool_path_from(
    xdg_state_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
    team_bucket: &str,
) -> Option<PathBuf> {
    let base = xdg_state_home
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".local/state")))?;
    Some(spool_path_at(&base, team_bucket))
}

/// `base` + team bucket → full spool path. Split out of [`spool_path_from`]
/// so [`hook_command_from`] (issue #114 e2e test seam) can be handed an
/// already-resolved base directly (a tempdir in tests), instead of the
/// `XDG_STATE_HOME`/`HOME` env vars `spool_path_from` reads.
fn spool_path_at(base: &Path, team_bucket: &str) -> PathBuf {
    base.join("herdmates/hook-spool")
        .join(format!("{team_bucket}.jsonl"))
}

/// `pub(crate)` so `recorder.rs` (issue #100 M3) resolves the exact same
/// spool path a hook invocation would have written to, without
/// duplicating the XDG resolution a third time.
pub(crate) fn default_spool_path(team_bucket: &str) -> Option<PathBuf> {
    spool_path_from(
        std::env::var_os("XDG_STATE_HOME"),
        std::env::var_os("HOME"),
        team_bucket,
    )
}

/// Single O_APPEND write, one line, no lock — matches
/// `recorder::append_records`'s precedent: append-only + consumer-
/// tolerant (a reader mid-write sees a complete prior line or nothing
/// new, never a torn one, since one `writeln!` call is one write
/// syscall for a line this short). Creates the parent dir if needed;
/// never truncates or rewrites existing lines.
fn append_line(spool_path: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = spool_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(spool_path)?;
    writeln!(file, "{line}")
}

// ─── gating: capability ships, no predicate wired in v1 ────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct GateConfig {
    #[serde(default)]
    enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GateDecision {
    Allow,
    // ponytail: unconstructed outside #[cfg(test)] until a post-v1 gating
    // predicate exists to produce it — `exit_status`'s test exercises the
    // mapping directly. Not dead in the sense that matters: the match arm
    // consuming it (`exit_status`) is real, shipped plumbing.
    #[allow(dead_code)]
    Block,
}

/// Pure: parses `${XDG_CONFIG_HOME:-~/.config}/herdmates/hook-gate.toml`
/// contents. Returns `None` — the only default — unless the file is
/// present, valid TOML, AND explicitly sets `enabled = true`. Absence,
/// unreadable, malformed, or `enabled = false` are all `None`: opting
/// in requires one explicit, unambiguous line.
pub(crate) fn parse_gate_config(raw: &str) -> Option<GateConfig> {
    let config: GateConfig = toml::from_str(raw).ok()?;
    config.enabled.then_some(config)
}

/// Pure: decides whether this event should block Claude Code (exit 2)
/// or pass (exit 0). Issue #100 scope explicitly excludes gating-ON
/// *logic* — no predicate exists yet, so this always returns `Allow`
/// regardless of `config`, even when the operator has opted in. The
/// `Option<&GateConfig>` parameter and the `Block` variant are the
/// shipped plumbing a post-v1 predicate will read; today nothing in
/// this function can produce `Block`, which is what the mandatory
/// "default config cannot exit 2" test (and this doc comment) asserts.
pub(crate) fn decide_gate(_config: Option<&GateConfig>, _envelope: &HookEnvelope) -> GateDecision {
    GateDecision::Allow
}

/// Pure: `GateDecision` → process exit status. Split out so the
/// `Block` → 2 mapping is directly testable even though `decide_gate`
/// never produces `Block` in v1 (see its doc comment) — proves the
/// plumbing works without needing a live blocking predicate to exercise it.
pub(crate) fn exit_status(decision: GateDecision) -> u8 {
    match decision {
        GateDecision::Allow => 0,
        GateDecision::Block => 2,
    }
}

fn default_gate_config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("herdmates/hook-gate.toml"))
}

fn load_gate_config() -> Option<GateConfig> {
    let path = default_gate_config_path()?;
    let raw = std::fs::read_to_string(path).ok()?;
    parse_gate_config(&raw)
}

// ─── `herdmates hook <event>` subcommand ────────────────────────────────────

/// `herdmates hook <EventName>`: reads the hook event JSON from stdin,
/// appends one spool line, exits. Returns `ExitCode` directly (not the
/// crate's usual `Result`-through-`exit()` convention in `main.rs`)
/// because the gating capability needs to reach `ExitCode::from(2)` on
/// a future `GateDecision::Block` — a plain `Result<(), Error>` can only
/// reach the crate's existing 0/1 exit codes.
/// Hard ceilings on the stdin read, enforcing the module's "must never
/// block" invariant against a parent that holds the pipe open or floods it
/// (2026-07-17 review, finding 7). Team-event payloads are sub-KB; 1 MiB /
/// 5 s are generous.
const STDIN_READ_TIMEOUT: Duration = Duration::from_secs(5);
const STDIN_READ_MAX_BYTES: u64 = 1024 * 1024;

/// Reads `reader` (size-capped) on a helper thread and gives up after
/// `timeout`. `None` on read error or timeout — the caller exits 0 either
/// way per the module's all-failures-are-silent contract. On timeout the
/// helper thread is abandoned; the process exits immediately after, which
/// reaps it. Generic over `reader` (issue #114: real stdin in production,
/// an in-memory `Cursor` in tests — the direct seam `read_stdin_bounded`
/// used to hardcode) rather than a stdin-specific function.
fn read_bounded(reader: impl Read + Send + 'static, timeout: Duration) -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut input = String::new();
        let result = reader
            .take(STDIN_READ_MAX_BYTES)
            .read_to_string(&mut input)
            .map(|_| input);
        let _ = sender.send(result);
    });
    receiver.recv_timeout(timeout).ok()?.ok()
}

/// `herdmates hook <EventName>`: reads the hook event JSON from stdin,
/// appends one spool line, exits. Thin wrapper resolving real stdin/env
/// over [`hook_command_from`], the testable core.
pub fn hook_command(args: &[String]) -> ExitCode {
    let event_name = args
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let paths = crate::gather::GatherPaths::from_env();
    let spool_base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")));
    // Feature-detect (ADR-0010): absent socket env → `None` → the
    // enrichment push is skipped silently. Also the test seam: the unit
    // tests below always pass `None`, so `cargo test` inside a Claude Code
    // session can never post into the developer's own lead.
    let socket = crate::lead_post::SocketTarget::from_env();
    hook_command_from(
        &event_name,
        std::io::stdin(),
        paths.as_ref(),
        spool_base.as_deref(),
        socket.as_ref(),
        SystemTime::now(),
    )
}

/// Testable core of [`hook_command`]. Returns `ExitCode` directly (not the
/// crate's usual `Result`-through-`exit()` convention in `main.rs`)
/// because the gating capability needs to reach `ExitCode::from(2)` on
/// a future `GateDecision::Block` — a plain `Result<(), Error>` can only
/// reach the crate's existing 0/1 exit codes.
///
/// `reader` stands in for stdin, `paths`/`spool_base` for the real
/// env-resolved paths [`discover_teams_at`]/`default_spool_path` use — the
/// same explicit-parameter seam `spool_path_from`/`GatherPaths` already
/// use elsewhere, so a test can point both at a tempdir without touching
/// real process env vars (issue #114). `socket` is the same seam for the
/// enrichment push (#123): `None` means "no inbox socket", the silent
/// no-op every test takes.
fn hook_command_from(
    event_name: &str,
    reader: impl Read + Send + 'static,
    paths: Option<&crate::gather::GatherPaths>,
    spool_base: Option<&Path>,
    socket: Option<&crate::lead_post::SocketTarget>,
    now: SystemTime,
) -> ExitCode {
    let Some(input) = read_bounded(reader, STDIN_READ_TIMEOUT) else {
        eprintln!("herdmates hook: failed to read stdin for {event_name} (error or timeout)");
        return ExitCode::SUCCESS;
    };

    let payload: Value = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("herdmates hook: malformed JSON payload for {event_name}: {error}");
            return ExitCode::SUCCESS;
        }
    };

    let envelope = build_envelope(event_name, payload, now);
    let teams = discover_teams_at(paths.map(|paths| paths.teams_root.as_path()));
    let bucket = resolve_team_bucket(&envelope.payload, &teams);

    match spool_base {
        Some(base) => {
            let spool_path = spool_path_at(base, &bucket);
            if let Err(error) = append_line(&spool_path, &envelope_line(&envelope)) {
                eprintln!(
                    "herdmates hook: failed to append spool entry at {}: {error}",
                    spool_path.display()
                );
            }
        }
        None => {
            eprintln!("herdmates hook: cannot resolve spool directory (set XDG_STATE_HOME or HOME)")
        }
    }

    push_enrichment(paths, socket, &bucket, event_name, &envelope.payload, now);

    ExitCode::from(exit_status(decide_gate(
        load_gate_config().as_ref(),
        &envelope,
    )))
}

/// Push half of the brain layer (issue #123, ADR-0015 §"brain layer"
/// item 1): compute the signal-engine facts behind this event and post
/// them into the hosting lead's own inbox socket, so the lead's model
/// reads *why* alongside Claude Code's native *what*.
///
/// Every arm degrades silently, because this runs on Claude Code's event
/// path and the module contract is "never block, never fail the host":
/// no socket env (not a session child, or an older Claude Code) → no-op;
/// no resolvable paths → no-op; an event we don't model → no message; a
/// refused, timed-out, or held post → one stderr line and exit 0. The
/// spool write above already happened either way, so an unreachable lead
/// never costs us the observation.
fn push_enrichment(
    paths: Option<&crate::gather::GatherPaths>,
    socket: Option<&crate::lead_post::SocketTarget>,
    team: &str,
    event_name: &str,
    payload: &Value,
    now: SystemTime,
) {
    let (Some(paths), Some(socket)) = (paths, socket) else {
        return;
    };
    let Some(text) = crate::enrich::enrichment_for_event(paths, team, event_name, payload, now)
    else {
        return;
    };
    if let Err(error) = crate::lead_post::post(socket, &text) {
        eprintln!("herdmates hook: enrichment post failed for {event_name}: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "herdmates-team-hook-tests-{}-{sequence}",
                std::process::id()
            ));
            std::fs::create_dir_all(&path).expect("create team_hook test dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // ── build_envelope / envelope_line (pure) ──────────────────────────────

    #[test]
    fn build_envelope_stamps_the_event_name_and_spool_version() {
        let envelope = build_envelope(
            "TaskCreated",
            json!({"task_id": "1"}),
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(42),
        );
        assert_eq!(envelope.spool_v, 1);
        assert_eq!(envelope.event, "TaskCreated");
        assert_eq!(envelope.captured_unix, 42);
        assert_eq!(envelope.payload, json!({"task_id": "1"}));
    }

    #[test]
    fn envelope_line_round_trips_through_serde_json() {
        let envelope = build_envelope(
            "TeammateIdle",
            json!({"permission_mode": "acceptEdits"}),
            SystemTime::UNIX_EPOCH,
        );
        let line = envelope_line(&envelope);
        let parsed: HookEnvelope = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed, envelope);
    }

    #[test]
    fn envelope_preserves_unknown_payload_fields_verbatim() {
        // Drift tolerance: a field this crate has never seen must survive
        // the round trip unchanged, not get dropped or renamed.
        let payload = json!({"team_name": "t", "brand_new_field_from_a_future_claude_code": 7});
        let envelope = build_envelope("TaskCreated", payload.clone(), SystemTime::UNIX_EPOCH);
        let parsed: HookEnvelope = serde_json::from_str(&envelope_line(&envelope)).unwrap();
        assert_eq!(parsed.payload, payload);
    }

    // ── resolve_team_bucket (pure) ───────────────────────────────────────────

    #[test]
    fn resolve_team_bucket_prefers_team_name_when_present() {
        assert_eq!(
            resolve_team_bucket(&json!({"team_name": "session-x"}), &[]),
            "session-x"
        );
    }

    #[test]
    fn resolve_team_bucket_falls_back_when_team_name_is_missing_or_empty() {
        assert_eq!(resolve_team_bucket(&json!({}), &[]), "_unknown");
        assert_eq!(
            resolve_team_bucket(&json!({"team_name": ""}), &[]),
            "_unknown"
        );
        assert_eq!(
            resolve_team_bucket(&json!({"team_name": 7}), &[]),
            "_unknown"
        );
    }

    /// team_name wins even when a team in `teams` matches session_id —
    /// the fallback is strictly for when team_name is absent, never a
    /// tie-breaker.
    #[test]
    fn resolve_team_bucket_team_name_wins_over_a_matching_session_id() {
        let payload = json!({"team_name": "explicit-team", "session_id": "s1"});
        let teams = [("session-derived-team".to_owned(), Some("s1".to_owned()))];
        assert_eq!(resolve_team_bucket(&payload, &teams), "explicit-team");
    }

    /// #100 M5: a lead-session task event has no team_name at all —
    /// resolves via session_id -> leadSessionId instead.
    #[test]
    fn resolve_team_bucket_derives_from_session_id_when_team_name_absent() {
        let payload = json!({"session_id": "lead-session-1", "task_id": "1"});
        let teams = [
            ("team-a".to_owned(), Some("other-session".to_owned())),
            ("team-b".to_owned(), Some("lead-session-1".to_owned())),
        ];
        assert_eq!(resolve_team_bucket(&payload, &teams), "team-b");
    }

    #[test]
    fn resolve_team_bucket_falls_back_when_session_id_matches_no_team() {
        let payload = json!({"session_id": "unmatched-session"});
        let teams = [("team-a".to_owned(), Some("other-session".to_owned()))];
        assert_eq!(resolve_team_bucket(&payload, &teams), "_unknown");
    }

    #[test]
    fn resolve_team_bucket_falls_back_when_session_id_is_also_absent() {
        let payload = json!({"hook_event_name": "TeammateIdle"});
        let teams = [("team-a".to_owned(), Some("some-session".to_owned()))];
        assert_eq!(resolve_team_bucket(&payload, &teams), "_unknown");
    }

    // ── spool_path_from (pure) ────────────────────────────────────────────

    #[test]
    fn spool_path_uses_xdg_state_home_when_set() {
        let path =
            spool_path_from(Some("/xdg-state".into()), Some("/home/x".into()), "team-x").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/xdg-state/herdmates/hook-spool/team-x.jsonl")
        );
    }

    #[test]
    fn spool_path_falls_back_to_home_local_state() {
        let path = spool_path_from(None, Some("/home/x".into()), "team-x").unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/x/.local/state/herdmates/hook-spool/team-x.jsonl")
        );
    }

    #[test]
    fn spool_path_is_none_when_neither_env_var_resolves() {
        assert_eq!(spool_path_from(None, None, "team-x"), None);
    }

    // ── append_line (integration: real temp-dir I/O) ────────────────────────

    #[test]
    fn append_line_creates_parent_dirs_and_appends_one_line() {
        let dir = TempDir::new();
        let spool_path = dir.path().join("nested/team-x.jsonl");
        append_line(&spool_path, "first").unwrap();
        append_line(&spool_path, "second").unwrap();
        let contents = std::fs::read_to_string(&spool_path).unwrap();
        assert_eq!(contents, "first\nsecond\n");
    }

    #[test]
    fn append_line_never_truncates_prior_lines() {
        let dir = TempDir::new();
        let spool_path = dir.path().join("team-x.jsonl");
        std::fs::write(&spool_path, "existing\n").unwrap();
        append_line(&spool_path, "new").unwrap();
        assert_eq!(
            std::fs::read_to_string(&spool_path).unwrap(),
            "existing\nnew\n"
        );
    }

    // ── parse_gate_config / decide_gate (pure) ──────────────────────────────

    #[test]
    fn parse_gate_config_is_none_when_absent_or_empty() {
        assert_eq!(parse_gate_config(""), None);
    }

    #[test]
    fn parse_gate_config_is_none_when_explicitly_disabled() {
        assert_eq!(parse_gate_config("enabled = false"), None);
    }

    #[test]
    fn parse_gate_config_is_none_on_malformed_toml() {
        assert_eq!(parse_gate_config("not valid toml {{{"), None);
    }

    #[test]
    fn parse_gate_config_is_some_only_on_explicit_opt_in() {
        assert_eq!(
            parse_gate_config("enabled = true"),
            Some(GateConfig { enabled: true })
        );
    }

    /// Mandatory per the #100 brief: the default (non-opted-in) hook
    /// configuration must be structurally incapable of exiting 2 under
    /// any code path. `decide_gate` has no predicate in v1 at all, so
    /// this holds for every config state — `None` (the default), and
    /// even `Some` (opted in) — proving the exit-2 branch is dead code
    /// in this release, not just config-gated.
    #[test]
    fn decide_gate_never_blocks_regardless_of_config_or_payload() {
        let envelope = build_envelope(
            "TaskCreated",
            json!({"task_id": "1"}),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(decide_gate(None, &envelope), GateDecision::Allow);
        assert_eq!(
            decide_gate(Some(&GateConfig { enabled: true }), &envelope),
            GateDecision::Allow
        );
    }

    #[test]
    fn exit_status_maps_allow_to_zero_and_block_to_two() {
        assert_eq!(exit_status(GateDecision::Allow), 0);
        assert_eq!(exit_status(GateDecision::Block), 2);
    }

    // ── read_bounded (issue #114) ────────────────────────────────────────────

    #[test]
    fn read_bounded_returns_short_input_verbatim() {
        let result = read_bounded(
            std::io::Cursor::new(b"hello".to_vec()),
            Duration::from_secs(1),
        );
        assert_eq!(result.as_deref(), Some("hello"));
    }

    #[test]
    fn read_bounded_truncates_input_over_the_byte_cap() {
        let oversize = vec![b'x'; (STDIN_READ_MAX_BYTES + 1) as usize];
        let result = read_bounded(std::io::Cursor::new(oversize), Duration::from_secs(1));
        assert_eq!(result.unwrap().len() as u64, STDIN_READ_MAX_BYTES);
    }

    // ── hook_command_from: end-to-end (issue #114) ───────────────────────────

    /// Real captured field shape for a `TaskCreated` event fired from a
    /// teammate session (this module's doc comment, live capture
    /// 2026-07-17, `docs/research/hook-companion-surface-2026-07-16.md`):
    /// session_id, transcript_path, cwd, prompt_id, hook_event_name,
    /// task_id, task_subject, task_description, teammate_name, team_name.
    #[test]
    fn hook_command_from_writes_the_spool_entry_for_a_real_task_created_payload() {
        let dir = TempDir::new();
        let spool_base = dir.path().join("state");
        let payload = json!({
            "session_id": "abc123-session",
            "transcript_path": "/home/x/.claude/projects/-home-x-proj/abc123.jsonl",
            "cwd": "/home/x/proj",
            "prompt_id": "prompt-1",
            "hook_event_name": "TaskCreated",
            "task_id": "task-42",
            "task_subject": "Ship the thing",
            "task_description": "Do the work",
            "teammate_name": "alpha",
            "team_name": "wave3",
        });

        let exit_code = hook_command_from(
            "TaskCreated",
            std::io::Cursor::new(payload.to_string().into_bytes()),
            None, // team_name is present, so the session_id->team fallback is never consulted
            Some(&spool_base),
            None, // #123: no inbox socket → the enrichment push is a silent no-op
            SystemTime::UNIX_EPOCH + Duration::from_secs(100),
        );

        assert_eq!(format!("{exit_code:?}"), format!("{:?}", ExitCode::SUCCESS));

        let spool_path = spool_base.join("herdmates/hook-spool/wave3.jsonl");
        let line = std::fs::read_to_string(&spool_path).expect("spool file written");
        let envelope: HookEnvelope = serde_json::from_str(line.lines().next().unwrap()).unwrap();
        assert_eq!(envelope.event, "TaskCreated");
        assert_eq!(envelope.captured_unix, 100);
        assert_eq!(envelope.payload["task_id"], "task-42");
        assert_eq!(envelope.payload["teammate_name"], "alpha");
    }

    // ── enrichment push (issue #123) ─────────────────────────────────────────

    /// End to end over a fake inbox server: a real `TeammateIdle` payload
    /// plus a blocked owned task must reach the socket as one `user`
    /// frame carrying the engine's reason.
    #[test]
    fn hook_command_from_posts_the_enrichment_to_the_inbox_socket() {
        use std::io::Read as _;

        let dir = TempDir::new();
        let paths = crate::gather::GatherPaths {
            teams_root: dir.path().join("teams"),
            tasks_root: dir.path().join("tasks"),
            projects_root: dir.path().join("projects"),
        };
        let tasks_dir = paths.tasks_root.join("wave3");
        std::fs::create_dir_all(&tasks_dir).unwrap();
        std::fs::write(tasks_dir.join("1.json"), r#"{"id":"1","status":"pending"}"#).unwrap();
        std::fs::write(
            tasks_dir.join("2.json"),
            r#"{"id":"2","status":"pending","owner":"alpha","blockedBy":["1"]}"#,
        )
        .unwrap();

        let socket_path = dir.path().join("inbox.sock");
        let listener = std::os::unix::net::UnixListener::bind(&socket_path).unwrap();
        let server = std::thread::spawn(move || {
            let (mut connection, _) = listener.accept().unwrap();
            let mut received = String::new();
            connection.read_to_string(&mut received).unwrap();
            received
        });

        let payload = json!({
            "session_id": "abc",
            "hook_event_name": "TeammateIdle",
            "permission_mode": "acceptEdits",
            "teammate_name": "alpha",
            "team_name": "wave3",
        });
        let exit_code = hook_command_from(
            "TeammateIdle",
            std::io::Cursor::new(payload.to_string().into_bytes()),
            Some(&paths),
            Some(&dir.path().join("state")),
            Some(&crate::lead_post::SocketTarget {
                path: socket_path,
                token: Some("tok".to_owned()),
            }),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(format!("{exit_code:?}"), format!("{:?}", ExitCode::SUCCESS));

        let received = server.join().unwrap();
        let message: Value = serde_json::from_str(received.lines().nth(1).unwrap()).unwrap();
        let content = message["message"]["content"].as_str().unwrap();
        assert!(
            content.starts_with(
                "[herdmates] TeammateIdle alpha — reason: blocked on an incomplete dependency."
            ),
            "{content}"
        );
        assert!(content.contains("tasks: 2 open, 1 blocked"), "{content}");
    }

    /// The host must never notice an unreachable lead: the spool line is
    /// still written and the process still exits 0.
    #[test]
    fn a_dead_inbox_socket_still_spools_and_still_exits_zero() {
        let dir = TempDir::new();
        let spool_base = dir.path().join("state");
        let paths = crate::gather::GatherPaths {
            teams_root: dir.path().join("teams"),
            tasks_root: dir.path().join("tasks"),
            projects_root: dir.path().join("projects"),
        };
        let exit_code = hook_command_from(
            "TeammateIdle",
            std::io::Cursor::new(json!({"team_name": "wave3"}).to_string().into_bytes()),
            Some(&paths),
            Some(&spool_base),
            Some(&crate::lead_post::SocketTarget {
                path: dir.path().join("nothing-is-listening.sock"),
                token: None,
            }),
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(format!("{exit_code:?}"), format!("{:?}", ExitCode::SUCCESS));
        assert!(spool_base
            .join("herdmates/hook-spool/wave3.jsonl")
            .is_file());
    }

    #[test]
    fn hook_command_from_exits_success_and_skips_the_spool_on_malformed_json() {
        let exit_code = hook_command_from(
            "TeammateIdle",
            std::io::Cursor::new(b"not json".to_vec()),
            None,
            None,
            None,
            SystemTime::UNIX_EPOCH,
        );
        assert_eq!(format!("{exit_code:?}"), format!("{:?}", ExitCode::SUCCESS));
    }
}
