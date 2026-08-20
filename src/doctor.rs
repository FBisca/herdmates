//! `herdmates doctor`: self-check subcommand (issue #106, static half —
//! `--probe` is out of scope here, blocked on issue #105). One line per
//! check, all run even when an earlier one fails, exit non-zero if any
//! check `Fail`s. Honest degrade wording throughout: a check that cannot
//! prove something absent says so rather than claiming certainty it
//! doesn't have (repo doctrine, see `team_hook`/`gather` module docs).
//!
//! Pure check logic (`check_*`) takes already-resolved primitive inputs
//! (an `Option<&str>` schema, an `Option<&str>` settings.json body, an
//! `Option<&Path>`) and returns a [`CheckResult`] — no I/O of its own, so
//! every check is directly testable without touching the real filesystem
//! or a live herdr. [`collect_checks`] is the thin impure layer that
//! resolves those inputs (env vars, file reads, one `HerdrApi::api_schema`
//! call) and hands them to the checks — same pure-core/impure-shell split
//! `gather.rs`/`teammux.rs` already use.

use crate::herdr::HerdrApi;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Checked in at `docs/herdr-api-schema.snapshot.json`; re-snapshotted
/// after every `herdr update` per the repo's CLAUDE.md rule. Embedded at
/// compile time (same `include_str!` pattern `socket.rs`'s
/// `SCHEMA_BASELINE` already uses for its own protocol baseline) so the
/// doctor binary never depends on running from a source checkout.
const CHECKED_IN_SCHEMA_SNAPSHOT: &str = include_str!("../docs/herdr-api-schema.snapshot.json");

const HOOK_EVENTS: [&str; 3] = ["TeammateIdle", "TaskCreated", "TaskCompleted"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckStatus {
    Ok,
    /// Informational only — never fails the run (e.g. the teams-env hint,
    /// which cannot prove anything about the lead session from here).
    Info,
    Warn,
    Fail,
}

impl CheckStatus {
    fn label(self) -> &'static str {
        match self {
            CheckStatus::Ok => "OK",
            CheckStatus::Info => "INFO",
            CheckStatus::Warn => "WARN",
            CheckStatus::Fail => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub message: String,
}

impl CheckResult {
    fn new(name: &'static str, status: CheckStatus, message: impl Into<String>) -> Self {
        Self {
            name,
            status,
            message: message.into(),
        }
    }
}

// ─── pure checks ─────────────────────────────────────────────────────────────

/// binary: `current_exe` resolves and `herdmates` is findable on PATH.
/// Both absent/unresolved is a `Fail` — nothing else in this process's
/// own identity can be trusted otherwise. Both present but pointing at
/// different files is `Warn` only (the documented "stale-binary trap":
/// hooks run whatever's on PATH, not necessarily this process).
fn check_binary(current_exe: Option<&Path>, path_copy: Option<&Path>) -> CheckResult {
    match (current_exe, path_copy) {
        (None, _) => CheckResult::new(
            "binary",
            CheckStatus::Fail,
            "cannot resolve this process's own executable path (current_exe failed)",
        ),
        (Some(_), None) => CheckResult::new(
            "binary",
            CheckStatus::Fail,
            "`herdmates` is not on PATH — hooks and anything else invoking it by bare name will fail",
        ),
        (Some(exe), Some(path_copy)) if exe == path_copy => CheckResult::new(
            "binary",
            CheckStatus::Ok,
            format!("on PATH at {}", path_copy.display()),
        ),
        (Some(exe), Some(path_copy)) => CheckResult::new(
            "binary",
            CheckStatus::Warn,
            format!(
                "PATH resolves `herdmates` to {} but this process is running from {} — reinstall (`cargo install --path . --root \"$HOME/.local\"`) if you expect them to match",
                path_copy.display(),
                exe.display()
            ),
        ),
    }
}

/// hooks: `~/.claude/settings.json` registers all 3 team hooks
/// (`TeammateIdle`/`TaskCreated`/`TaskCompleted`) invoking `herdmates
/// hook`. Lenient: for each event, check the settings JSON's `hooks.
/// {event}` entry mentions `herdmates` somewhere, rather than parsing
/// Claude Code's hook-registration shape exactly — future fields there
/// are not this check's business. Absent/malformed file is `Fail` with
/// the reason; missing per-event registrations name exactly which ones.
fn check_hooks(settings_json: Option<&str>) -> CheckResult {
    let Some(json) = settings_json else {
        return CheckResult::new(
            "hooks",
            CheckStatus::Fail,
            "~/.claude/settings.json not found or unreadable",
        );
    };
    let value: Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(error) => {
            return CheckResult::new(
                "hooks",
                CheckStatus::Fail,
                format!("~/.claude/settings.json is not valid JSON: {error}"),
            );
        }
    };
    let Some(hooks) = value.get("hooks") else {
        return CheckResult::new(
            "hooks",
            CheckStatus::Fail,
            "~/.claude/settings.json has no \"hooks\" key",
        );
    };
    let missing: Vec<&str> = HOOK_EVENTS
        .into_iter()
        .filter(|event| {
            !hooks
                .get(event)
                .is_some_and(|entry| entry.to_string().contains("herdmates"))
        })
        .collect();
    if missing.is_empty() {
        CheckResult::new(
            "hooks",
            CheckStatus::Ok,
            "all 3 team hooks (TeammateIdle/TaskCreated/TaskCompleted) registered",
        )
    } else {
        CheckResult::new(
            "hooks",
            CheckStatus::Fail,
            format!(
                "missing a `herdmates hook` registration for: {}",
                missing.join(", ")
            ),
        )
    }
}

/// state dir: `${XDG_STATE_HOME:-~/.local/state}/herdmates` exists.
/// Missing is `Warn`, not `Fail` — it is created lazily on the first
/// `hook`/`record` invocation, so its absence on a fresh install is
/// expected, not broken. `state_dir == None` (env unresolved) is worded
/// distinctly from "doesn't exist yet".
fn check_state_dir(state_dir: Option<&Path>, exists: bool) -> CheckResult {
    match state_dir {
        None => CheckResult::new(
            "state dir",
            CheckStatus::Warn,
            "cannot resolve XDG_STATE_HOME or HOME to locate the state directory",
        ),
        Some(path) if exists => CheckResult::new(
            "state dir",
            CheckStatus::Ok,
            format!("{} exists", path.display()),
        ),
        Some(path) => CheckResult::new(
            "state dir",
            CheckStatus::Warn,
            format!(
                "{} does not exist yet — it is created on first `hook`/`record` invocation, so this is expected before either has run",
                path.display()
            ),
        ),
    }
}

/// herdr: reachable (`api_schema()` succeeded) and its schema's
/// `protocol` matches the checked-in snapshot's. Unreachable is `Fail`
/// (nothing herdr-backed works without it); a protocol mismatch is
/// `Warn` only, naming both numbers and pointing at the repo's
/// re-snapshot rule — an older/newer herdr is a drift to fix, not
/// necessarily a broken one.
fn check_herdr(live_schema: Option<&str>, checked_in_schema: &str) -> CheckResult {
    let Some(live_schema) = live_schema else {
        return CheckResult::new(
            "herdr",
            CheckStatus::Fail,
            "herdr is not reachable (api_schema call failed) — is a herdr server running?",
        );
    };
    let live_protocol = schema_protocol(live_schema);
    let checked_in_protocol = schema_protocol(checked_in_schema);
    match (live_protocol, checked_in_protocol) {
        (Some(live), Some(checked_in)) if live == checked_in => CheckResult::new(
            "herdr",
            CheckStatus::Ok,
            format!("reachable, protocol {live} matches the checked-in snapshot"),
        ),
        (Some(live), Some(checked_in)) => CheckResult::new(
            "herdr",
            CheckStatus::Warn,
            format!(
                "reachable, but live protocol {live} differs from the checked-in snapshot's {checked_in} — re-snapshot docs/herdr-api-schema.snapshot.json (repo rule) and diff"
            ),
        ),
        _ => CheckResult::new(
            "herdr",
            CheckStatus::Warn,
            "reachable, but could not read a \"protocol\" field from the live or checked-in schema",
        ),
    }
}

fn schema_protocol(schema: &str) -> Option<u64> {
    serde_json::from_str::<Value>(schema)
        .ok()?
        .get("protocol")?
        .as_u64()
}

/// teams env: `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` set in *this*
/// process's environment. Info-only, never `Warn`/`Fail` — this process
/// is not the lead session, so its absence here proves nothing about
/// whether the lead has it set.
fn check_teams_env(set_here: bool) -> CheckResult {
    if set_here {
        CheckResult::new(
            "teams env",
            CheckStatus::Info,
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS is set in this process's environment",
        )
    } else {
        CheckResult::new(
            "teams env",
            CheckStatus::Info,
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS is not set here — this does not prove the lead session lacks it, only that this process doesn't have it",
        )
    }
}

// ─── impure collection + rendering ──────────────────────────────────────────

/// Every directory on `PATH`, in order — split out so [`path_copy_of`] is
/// directly testable without touching the real `PATH` env var.
fn path_dirs(path_env: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    path_env
        .map(|path| std::env::split_paths(path).collect())
        .unwrap_or_default()
}

/// First `herdmates` found on `PATH`, canonicalized. `None` if `PATH` is
/// unset/empty or no directory on it has one.
fn path_copy_of(path_dirs: &[PathBuf], binary_name: &str) -> Option<PathBuf> {
    path_dirs
        .iter()
        .map(|dir| dir.join(binary_name))
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
}

fn state_dir_path() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")))
        .map(|base| base.join("herdmates"))
}

/// Impure collector: resolves every check's real-world inputs, then hands
/// them to the pure `check_*` functions above.
fn collect_checks<H: HerdrApi>(herdr: &H) -> Vec<CheckResult> {
    let current_exe = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok());
    let dirs = path_dirs(std::env::var_os("PATH").as_deref());
    let path_copy = path_copy_of(&dirs, "herdmates");

    let settings_path =
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".claude/settings.json"));
    let settings_json = settings_path.and_then(|path| std::fs::read_to_string(path).ok());

    let state_dir = state_dir_path();
    let state_dir_exists = state_dir.as_deref().is_some_and(Path::is_dir);

    let live_schema = herdr.api_schema().ok();

    let teams_env_set = std::env::var_os("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS").is_some();

    vec![
        check_binary(current_exe.as_deref(), path_copy.as_deref()),
        check_hooks(settings_json.as_deref()),
        check_state_dir(state_dir.as_deref(), state_dir_exists),
        check_herdr(live_schema.as_deref(), CHECKED_IN_SCHEMA_SNAPSHOT),
        check_teams_env(teams_env_set),
    ]
}

fn render(checks: &[CheckResult]) {
    for check in checks {
        println!(
            "[{}] {}: {}",
            check.status.label(),
            check.name,
            check.message
        );
    }
}

fn exit_code_for(checks: &[CheckResult]) -> ExitCode {
    if checks.iter().any(|check| check.status == CheckStatus::Fail) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// `herdmates doctor`: static self-check (issue #106; `--probe` is a
/// separate, blocked follow-up — not implemented here). Runs every check
/// regardless of earlier failures, prints one line each, exits non-zero
/// if any `Fail`ed.
pub fn doctor_command(_args: &[String]) -> ExitCode {
    let herdr = crate::herdr::HerdrClient::from_env();
    let checks = collect_checks(&herdr);
    render(&checks);
    exit_code_for(&checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::herdr::test_support::FakeHerdr;

    // ── check_binary ─────────────────────────────────────────────────────────

    #[test]
    fn check_binary_fails_when_current_exe_is_unresolved() {
        let result = check_binary(None, Some(Path::new("/usr/local/bin/herdmates")));
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn check_binary_fails_when_not_on_path() {
        let result = check_binary(Some(Path::new("/opt/herdmates")), None);
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn check_binary_ok_when_current_exe_matches_the_path_copy() {
        let path = Path::new("/home/x/.local/bin/herdmates");
        let result = check_binary(Some(path), Some(path));
        assert_eq!(result.status, CheckStatus::Ok);
    }

    #[test]
    fn check_binary_warns_on_a_stale_path_copy() {
        let result = check_binary(
            Some(Path::new("/home/x/target/debug/herdmates")),
            Some(Path::new("/home/x/.local/bin/herdmates")),
        );
        assert_eq!(result.status, CheckStatus::Warn);
    }

    // ── check_hooks ──────────────────────────────────────────────────────────

    #[test]
    fn check_hooks_fails_when_settings_file_is_absent() {
        assert_eq!(check_hooks(None).status, CheckStatus::Fail);
    }

    #[test]
    fn check_hooks_fails_on_malformed_json() {
        assert_eq!(check_hooks(Some("not json")).status, CheckStatus::Fail);
    }

    #[test]
    fn check_hooks_fails_when_no_hooks_key_at_all() {
        assert_eq!(check_hooks(Some("{}")).status, CheckStatus::Fail);
    }

    #[test]
    fn check_hooks_ok_when_all_three_events_are_registered() {
        let settings = r#"{"hooks":{
            "TeammateIdle": [{"hooks":[{"type":"command","command":"herdmates hook TeammateIdle"}]}],
            "TaskCreated": [{"hooks":[{"type":"command","command":"herdmates hook TaskCreated"}]}],
            "TaskCompleted": [{"hooks":[{"type":"command","command":"herdmates hook TaskCompleted"}]}]
        }}"#;
        assert_eq!(check_hooks(Some(settings)).status, CheckStatus::Ok);
    }

    #[test]
    fn check_hooks_fails_and_names_missing_events() {
        let settings = r#"{"hooks":{
            "TeammateIdle": [{"hooks":[{"type":"command","command":"herdmates hook TeammateIdle"}]}]
        }}"#;
        let result = check_hooks(Some(settings));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("TaskCreated"));
        assert!(result.message.contains("TaskCompleted"));
        assert!(!result.message.contains("TeammateIdle"));
    }

    // ── check_state_dir ──────────────────────────────────────────────────────

    #[test]
    fn check_state_dir_warns_when_unresolvable() {
        assert_eq!(check_state_dir(None, false).status, CheckStatus::Warn);
    }

    #[test]
    fn check_state_dir_ok_when_it_exists() {
        let result = check_state_dir(Some(Path::new("/home/x/.local/state/herdmates")), true);
        assert_eq!(result.status, CheckStatus::Ok);
    }

    #[test]
    fn check_state_dir_warns_honestly_when_missing_not_fails() {
        let result = check_state_dir(Some(Path::new("/home/x/.local/state/herdmates")), false);
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.message.contains("first"));
    }

    // ── check_herdr ──────────────────────────────────────────────────────────

    fn schema_with_protocol(protocol: u64) -> String {
        format!(r#"{{"protocol":{protocol}}}"#)
    }

    #[test]
    fn check_herdr_fails_when_unreachable() {
        assert_eq!(check_herdr(None, "{}").status, CheckStatus::Fail);
    }

    #[test]
    fn check_herdr_ok_when_protocols_match() {
        let schema = schema_with_protocol(20);
        let result = check_herdr(Some(&schema), &schema);
        assert_eq!(result.status, CheckStatus::Ok);
    }

    #[test]
    fn check_herdr_warns_on_protocol_mismatch_naming_both_numbers() {
        let live = schema_with_protocol(21);
        let checked_in = schema_with_protocol(20);
        let result = check_herdr(Some(&live), &checked_in);
        assert_eq!(result.status, CheckStatus::Warn);
        assert!(result.message.contains(&21.to_string()));
        assert!(result.message.contains(&20.to_string()));
    }

    // ── check_teams_env ──────────────────────────────────────────────────────

    #[test]
    fn check_teams_env_is_always_info_never_fail_or_warn() {
        assert_eq!(check_teams_env(true).status, CheckStatus::Info);
        assert_eq!(check_teams_env(false).status, CheckStatus::Info);
    }

    // ── exit_code_for / render mapping ───────────────────────────────────────

    fn ok(name: &'static str) -> CheckResult {
        CheckResult::new(name, CheckStatus::Ok, "fine")
    }

    fn warn(name: &'static str) -> CheckResult {
        CheckResult::new(name, CheckStatus::Warn, "hmm")
    }

    fn fail(name: &'static str) -> CheckResult {
        CheckResult::new(name, CheckStatus::Fail, "broken")
    }

    #[test]
    fn exit_code_is_success_when_no_check_failed() {
        let checks = vec![ok("a"), warn("b"), check_teams_env(true)];
        assert_eq!(exit_code_for(&checks), ExitCode::SUCCESS);
    }

    #[test]
    fn exit_code_is_failure_when_any_check_failed() {
        let checks = vec![ok("a"), warn("b"), fail("c")];
        assert_eq!(exit_code_for(&checks), ExitCode::FAILURE);
    }

    // ── path_copy_of ─────────────────────────────────────────────────────────

    #[test]
    fn path_copy_of_finds_the_first_matching_directory() {
        let dir =
            std::env::temp_dir().join(format!("herdmates-doctor-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let binary_path = dir.join("herdmates");
        std::fs::write(&binary_path, "#!/bin/sh\n").unwrap();

        let found = path_copy_of(&[dir.clone(), PathBuf::from("/nonexistent")], "herdmates");
        assert_eq!(found, Some(binary_path.canonicalize().unwrap()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_copy_of_none_when_no_directory_has_it() {
        assert_eq!(
            path_copy_of(&[PathBuf::from("/nonexistent")], "herdmates"),
            None
        );
    }

    // ── collect_checks: smoke test over a real FakeHerdr ────────────────────

    #[test]
    fn collect_checks_returns_one_result_per_check() {
        let herdr = FakeHerdr::default();
        let checks = collect_checks(&herdr);
        assert_eq!(checks.len(), 5);
    }
}
