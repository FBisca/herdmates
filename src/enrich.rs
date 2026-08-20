//! Hook enrichment facts (issue #123, ADR-0015 §"brain layer" item 1).
//! The native team hook tells the lead's model *that* something happened
//! (a teammate went idle, a task was created/completed); this module
//! computes *why* — the signal-engine reason plus the team's dependency
//! shape — and formats it for [`crate::lead_post`] to push into the lead's
//! own inbox socket.
//!
//! Nothing here re-derives a signal: the classification is
//! [`signal_engine::classify`] verbatim (#90's single-source rule — the
//! sidebar badge and this message cannot disagree), the task graph comes
//! from `gather::team_task_dependencies`, and the inbox accelerator from
//! `gather::seconds_since_unread_inbox`. The only new fact this path adds
//! is the one the hook alone knows: on `TeammateIdle`, the teammate *is*
//! idle — a status fact with no herdr pane behind it, which is what makes
//! enrichment work for in-process teammates that own no pane at all.
//!
//! Honest degradation (ADR-0013) is the whole design constraint. A
//! reason-less "waiting" beats a confident wrong reason, so:
//! - `TaskCreated`/`TaskCompleted` carry no status fact, so their
//!   [`AgentActivity`] stays `Unknown` and `classify` degrades to
//!   `Waiting` unless a dependency actually blocks the agent;
//! - a clause with nothing behind it is omitted, never emitted with a
//!   zero (no task files readable → no task line at all, not "0 tasks");
//! - the message is prefixed `[herdmates]` so the lead's model can always
//!   tell an enrichment from a human turn.
//!
//! A note on the transcript fact: for in-process teammates the hook
//! payload's `transcript_path` can be the LEAD's transcript, not the
//! named teammate's (observed live 2026-08-20: two teammate names
//! sharing the lead's `session_id`/`transcript_path` in one hook spool).
//! A lead-transcript mtime says nothing about the teammate, so the
//! staleness fact is used only when the payload's `session_id` differs
//! from the team's `leadSessionId` — otherwise it degrades to "no fact".

use crate::gather::{GatherPaths, TaskDependency};
use crate::signal_engine::{
    classify, AgentActivity, ObservedFacts, StalledThresholds, StalledTier, WaitingReason,
};
use serde_json::Value;
use std::path::Path;
use std::time::SystemTime;

/// The three team hook events this crate registers (`team_hook`'s module
/// doc). Any other event name yields no enrichment at all — silence beats
/// posting a message about an event whose payload shape we've never seen.
const ENRICHED_EVENTS: [&str; 3] = ["TeammateIdle", "TaskCreated", "TaskCompleted"];

/// One blocked task and the incomplete tasks it waits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedEdge {
    pub id: String,
    pub blockers: Vec<String>,
}

/// What the team's task graph looks like right now. Only ever built from
/// a non-empty task set — an empty graph is `None`, so callers cannot
/// accidentally report "0 open tasks" for a team whose task dir simply
/// wasn't readable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskGraphSummary {
    pub open: usize,
    pub blocked: Vec<BlockedEdge>,
}

/// Everything [`compose`] needs, as plain data — no I/O, no clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentFacts {
    pub event: String,
    pub agent: Option<String>,
    pub task_id: Option<String>,
    pub task_subject: Option<String>,
    pub reason: WaitingReason,
    pub tasks: Option<TaskGraphSummary>,
}

/// Pure: fold the dependency edges into the summary. `None` when there
/// are no task files at all (see [`TaskGraphSummary`]).
pub fn summarize_tasks(deps: &[TaskDependency]) -> Option<TaskGraphSummary> {
    if deps.is_empty() {
        return None;
    }
    let open = deps.iter().filter(|dep| !dep.completed).count();
    let blocked = deps
        .iter()
        .filter(|dep| !dep.completed && !dep.incomplete_blockers.is_empty())
        .map(|dep| BlockedEdge {
            id: dep.id.clone(),
            blockers: dep.incomplete_blockers.clone(),
        })
        .collect();
    Some(TaskGraphSummary { open, blocked })
}

/// Pure: is nothing startable? True when every open task waits on another
/// incomplete task, so no agent can pick up work without one being
/// unblocked by hand.
///
// ponytail: "no open task is startable" rather than graph cycle
// detection — it catches the whole-graph cycle and the whole-graph
// dependency stall, which is what a lead can act on. Add real cycle
// detection only if a *partial* cycle (deadlocked subset beside
// startable work) is ever seen live.
pub fn is_deadlocked(summary: &TaskGraphSummary) -> bool {
    summary.open > 0 && summary.blocked.len() == summary.open
}

/// Pure: does `owner` (a task file's owner field) denote `agent`? Matches
/// the plain name and the `name@team` agent-id form, same two shapes
/// `gather::any_owned_task_blocked` accepts.
fn owned_by(owner: Option<&str>, agent: &str) -> bool {
    owner.is_some_and(|owner| {
        owner == agent || owner.split_once('@').is_some_and(|(name, _)| name == agent)
    })
}

/// Pure: the model-facing text. Dense, prefixed, at most three lines, and
/// every clause is dropped when the fact behind it is absent.
pub fn compose(facts: &EnrichmentFacts) -> String {
    let mut head = format!("[herdmates] {}", facts.event);
    if let Some(agent) = &facts.agent {
        head.push(' ');
        head.push_str(agent);
    }
    if let Some(task_id) = &facts.task_id {
        head.push_str(&format!(" task {task_id}"));
        if let Some(subject) = &facts.task_subject {
            head.push_str(&format!(" \"{subject}\""));
        }
    }
    head.push_str(&format!(" — reason: {}.", reason_phrase(facts.reason)));

    let mut lines = vec![head];
    if let Some(summary) = &facts.tasks {
        lines.push(task_line(summary));
        if is_deadlocked(summary) {
            lines.push(
                "deadlock: every open task waits on another incomplete task — nothing is startable."
                    .to_owned(),
            );
        }
    }
    lines.join("\n")
}

/// How many blocked edges to spell out before collapsing the rest into a
/// count — the message is read by a model in the lead's context, so it
/// stays short even for a 40-task team.
const MAX_LISTED_EDGES: usize = 3;

fn task_line(summary: &TaskGraphSummary) -> String {
    if summary.blocked.is_empty() {
        return format!("tasks: {} open, none blocked.", summary.open);
    }
    let listed: Vec<String> = summary
        .blocked
        .iter()
        .take(MAX_LISTED_EDGES)
        .map(|edge| format!("{} waits on {}", edge.id, edge.blockers.join(", ")))
        .collect();
    let overflow = summary.blocked.len().saturating_sub(MAX_LISTED_EDGES);
    let suffix = if overflow > 0 {
        format!("; +{overflow} more")
    } else {
        String::new()
    };
    format!(
        "tasks: {} open, {} blocked ({}{}).",
        summary.open,
        summary.blocked.len(),
        listed.join("; "),
        suffix
    )
}

/// Full-word rendering of an engine class for a model reader — the
/// sidebar's `signal_engine::reason_badge` is the telegraphic rendering of
/// the same value, for a human glancing at a pane title.
fn reason_phrase(reason: WaitingReason) -> String {
    match reason {
        WaitingReason::PermissionPrompt => "waiting on a permission prompt".to_owned(),
        WaitingReason::BlockedOnDependency => "blocked on an incomplete dependency".to_owned(),
        WaitingReason::Stalled { tier, secs } => {
            let tier = match tier {
                StalledTier::Quiet => "quiet",
                StalledTier::Stalled => "stalled",
            };
            format!("{tier} {}m with no transcript activity", secs / 60)
        }
        WaitingReason::TurnComplete => "turn complete".to_owned(),
        // The never-wrong-reason degrade, spelled out rather than dressed
        // up as a class the facts don't support.
        WaitingReason::Waiting => "waiting (no grounded reason)".to_owned(),
    }
}

// ─── impure: gather the facts for one hook event ────────────────────────────

/// Build the enrichment text for one hook event, or `None` when there is
/// nothing honest to say (an event we don't enrich). Reads task files, the
/// team config, one inbox file and one transcript stat — all of them
/// degrade to "no fact" on any error, never to a guess.
pub fn enrichment_for_event(
    paths: &GatherPaths,
    team: &str,
    event: &str,
    payload: &Value,
    now: SystemTime,
) -> Option<String> {
    if !ENRICHED_EVENTS.contains(&event) {
        return None;
    }
    let agent = payload
        .get("teammate_name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());

    let deps = crate::gather::team_task_dependencies(paths, team);
    let tasks = summarize_tasks(&deps);

    let facts = ObservedFacts {
        // The one fact only the hook knows. Task events say nothing about
        // the agent's activity, so they stay Unknown and let `classify`
        // degrade.
        agent_status: if event == "TeammateIdle" {
            AgentActivity::Idle
        } else {
            AgentActivity::Unknown
        },
        owned_task_blocked_by_incomplete: agent.is_some_and(|agent| {
            deps.iter().any(|dep| {
                !dep.completed
                    && !dep.incomplete_blockers.is_empty()
                    && owned_by(dep.owner.as_deref(), agent)
            })
        }),
        // #123 review fix: only trust the payload transcript as the
        // teammate's own when its session_id is NOT the team lead's —
        // in-process teammates can carry the lead's transcript_path, and
        // a lead mtime would produce the exact wrong-reason class
        // ADR-0013 forbids. Unknown ownership → no fact.
        seconds_since_transcript_activity: transcript_is_teammates_own(paths, team, payload)
            .then(|| {
                payload
                    .get("transcript_path")
                    .and_then(Value::as_str)
                    .and_then(|path| seconds_since_mtime(Path::new(path), now))
            })
            .flatten(),
        seconds_since_unread_inbox: agent.and_then(|agent| {
            crate::gather::seconds_since_unread_inbox(
                &paths
                    .teams_root
                    .join(team)
                    .join("inboxes")
                    .join(format!("{agent}.json")),
                now,
            )
        }),
    };

    Some(compose(&EnrichmentFacts {
        event: event.to_owned(),
        agent: agent.map(str::to_owned),
        task_id: payload
            .get("task_id")
            .and_then(Value::as_str)
            .map(str::to_owned),
        task_subject: payload
            .get("task_subject")
            .and_then(Value::as_str)
            .map(str::to_owned),
        reason: classify(&facts, &StalledThresholds::default()),
        tasks,
    }))
}

/// True only when the event payload's `session_id` is present, the team
/// config's `leadSessionId` is present, and they differ — i.e. the
/// transcript in the payload provably belongs to a non-lead session.
/// Any missing side means ownership is unknowable → `false` → the
/// staleness fact degrades away (ADR-0013).
fn transcript_is_teammates_own(paths: &GatherPaths, team: &str, payload: &Value) -> bool {
    let payload_session = payload.get("session_id").and_then(Value::as_str);
    let lead_session =
        crate::teamfiles::read_team_config(&paths.teams_root.join(team).join("config.json"))
            .ok()
            .and_then(|config| config.lead_session_id);
    matches!(
        (payload_session, lead_session.as_deref()),
        (Some(session), Some(lead)) if session != lead
    )
}

/// Stat-only liveness, same doctrine as `gather::resolve_transcript_mtime`
/// (ADR-0013 forbids parsing transcript content). The hook is handed the
/// transcript path directly in its payload, so no projects-root search is
/// needed here.
fn seconds_since_mtime(path: &Path, now: SystemTime) -> Option<u64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dep(id: &str, completed: bool, owner: Option<&str>, blockers: &[&str]) -> TaskDependency {
        TaskDependency {
            id: id.to_owned(),
            subject: None,
            completed,
            owner: owner.map(str::to_owned),
            incomplete_blockers: blockers.iter().map(|b| (*b).to_owned()).collect(),
        }
    }

    fn facts(reason: WaitingReason) -> EnrichmentFacts {
        EnrichmentFacts {
            event: "TeammateIdle".to_owned(),
            agent: Some("alpha".to_owned()),
            task_id: None,
            task_subject: None,
            reason,
            tasks: None,
        }
    }

    // ── summarize_tasks / is_deadlocked (pure) ───────────────────────────────

    #[test]
    fn summarize_tasks_is_none_for_an_empty_task_set() {
        // Never "0 open tasks" — an unreadable/empty task dir has no facts.
        assert_eq!(summarize_tasks(&[]), None);
    }

    #[test]
    fn summarize_tasks_counts_open_and_lists_blocked_edges() {
        let deps = [
            dep("1", true, None, &[]),
            dep("2", false, None, &[]),
            dep("3", false, Some("alpha"), &["2"]),
        ];
        let summary = summarize_tasks(&deps).unwrap();
        assert_eq!(summary.open, 2);
        assert_eq!(
            summary.blocked,
            [BlockedEdge {
                id: "3".to_owned(),
                blockers: vec!["2".to_owned()],
            }]
        );
    }

    #[test]
    fn deadlock_only_when_every_open_task_is_blocked() {
        let stalled =
            summarize_tasks(&[dep("1", false, None, &["2"]), dep("2", false, None, &["1"])])
                .unwrap();
        assert!(is_deadlocked(&stalled));

        let startable =
            summarize_tasks(&[dep("1", false, None, &[]), dep("2", false, None, &["1"])]).unwrap();
        assert!(!is_deadlocked(&startable));
    }

    #[test]
    fn an_all_completed_graph_is_not_a_deadlock() {
        let summary = summarize_tasks(&[dep("1", true, None, &[])]).unwrap();
        assert!(!is_deadlocked(&summary));
    }

    // ── owned_by (pure) ──────────────────────────────────────────────────────

    #[test]
    fn owned_by_matches_the_plain_name_and_the_agent_id_form() {
        assert!(owned_by(Some("alpha"), "alpha"));
        assert!(owned_by(Some("alpha@team"), "alpha"));
        assert!(!owned_by(Some("beta"), "alpha"));
        assert!(!owned_by(Some("alphabet"), "alpha"));
        assert!(!owned_by(None, "alpha"));
    }

    // ── compose (pure) ───────────────────────────────────────────────────────

    #[test]
    fn compose_leads_with_the_prefix_event_agent_and_reason() {
        let text = compose(&facts(WaitingReason::TurnComplete));
        assert_eq!(
            text,
            "[herdmates] TeammateIdle alpha — reason: turn complete."
        );
    }

    #[test]
    fn compose_spells_out_the_reason_less_degrade_rather_than_guessing() {
        let text = compose(&facts(WaitingReason::Waiting));
        assert!(
            text.ends_with("reason: waiting (no grounded reason)."),
            "{text}"
        );
    }

    #[test]
    fn compose_omits_the_task_line_entirely_when_no_task_facts_exist() {
        let text = compose(&facts(WaitingReason::TurnComplete));
        assert_eq!(text.lines().count(), 1);
        assert!(!text.contains("tasks:"));
    }

    #[test]
    fn compose_names_the_task_on_a_task_event() {
        let mut f = facts(WaitingReason::Waiting);
        f.event = "TaskCompleted".to_owned();
        f.task_id = Some("7".to_owned());
        f.task_subject = Some("Ship the thing".to_owned());
        let text = compose(&f);
        assert!(
            text.starts_with("[herdmates] TaskCompleted alpha task 7 \"Ship the thing\" —"),
            "{text}"
        );
    }

    #[test]
    fn compose_reports_blocked_edges_and_the_deadlock_line() {
        let mut f = facts(WaitingReason::BlockedOnDependency);
        f.tasks = summarize_tasks(&[dep("1", false, None, &["2"]), dep("2", false, None, &["1"])]);
        let text = compose(&f);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(
            lines[1],
            "tasks: 2 open, 2 blocked (1 waits on 2; 2 waits on 1)."
        );
        assert!(lines[2].starts_with("deadlock:"), "{}", lines[2]);
    }

    #[test]
    fn compose_says_none_blocked_when_the_graph_is_clear() {
        let mut f = facts(WaitingReason::TurnComplete);
        f.tasks = summarize_tasks(&[dep("1", false, None, &[])]);
        let text = compose(&f);
        assert!(text.contains("tasks: 1 open, none blocked."), "{text}");
        assert!(!text.contains("deadlock"));
    }

    #[test]
    fn compose_collapses_a_long_blocked_list() {
        let mut f = facts(WaitingReason::BlockedOnDependency);
        let deps: Vec<TaskDependency> = (1..=5)
            .map(|n| dep(&n.to_string(), false, None, &["9"]))
            .collect();
        f.tasks = summarize_tasks(&deps);
        let text = compose(&f);
        assert!(text.contains("+2 more"), "{text}");
    }

    #[test]
    fn stalled_reason_renders_in_whole_minutes() {
        let text = compose(&facts(WaitingReason::Stalled {
            tier: StalledTier::Stalled,
            secs: 725,
        }));
        assert!(
            text.ends_with("reason: stalled 12m with no transcript activity."),
            "{text}"
        );
    }

    // ── enrichment_for_event (impure, tempdir) ───────────────────────────────

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("herdmates-enrich-{}-{label}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn paths(&self) -> GatherPaths {
            GatherPaths {
                teams_root: self.0.join("teams"),
                tasks_root: self.0.join("tasks"),
                projects_root: self.0.join("projects"),
            }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn enrichment_is_none_for_an_event_we_do_not_model() {
        let dir = TempDir::new("unmodelled");
        assert_eq!(
            enrichment_for_event(
                &dir.paths(),
                "team-x",
                "SessionStart",
                &json!({}),
                SystemTime::UNIX_EPOCH,
            ),
            None
        );
    }

    #[test]
    fn idle_with_a_blocked_owned_task_names_the_dependency_reason() {
        let dir = TempDir::new("blocked");
        let paths = dir.paths();
        let tasks_dir = paths.tasks_root.join("team-x");
        std::fs::create_dir_all(&tasks_dir).unwrap();
        std::fs::write(tasks_dir.join("1.json"), r#"{"id":"1","status":"pending"}"#).unwrap();
        std::fs::write(
            tasks_dir.join("2.json"),
            r#"{"id":"2","status":"pending","owner":"alpha","blockedBy":["1"]}"#,
        )
        .unwrap();

        let text = enrichment_for_event(
            &paths,
            "team-x",
            "TeammateIdle",
            &json!({"teammate_name": "alpha", "team_name": "team-x"}),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap();

        assert!(
            text.starts_with(
                "[herdmates] TeammateIdle alpha — reason: blocked on an incomplete dependency."
            ),
            "{text}"
        );
        assert!(
            text.contains("tasks: 2 open, 1 blocked (2 waits on 1)."),
            "{text}"
        );
    }

    #[test]
    fn idle_with_no_task_files_degrades_to_a_single_reason_line() {
        let dir = TempDir::new("no-tasks");
        let text = enrichment_for_event(
            &dir.paths(),
            "team-x",
            "TeammateIdle",
            &json!({"teammate_name": "alpha"}),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap();
        // Idle is a real status fact, nothing else is: turn complete, one
        // line, no invented task clause.
        assert_eq!(
            text,
            "[herdmates] TeammateIdle alpha — reason: turn complete."
        );
    }

    #[test]
    fn a_task_event_from_the_lead_session_carries_no_agent_and_stays_reason_less() {
        // #100 M5 shape: lead-fired task events have neither teammate_name
        // nor team_name. Nothing grounds a status, so the reason degrades.
        let dir = TempDir::new("lead-task");
        let text = enrichment_for_event(
            &dir.paths(),
            "team-x",
            "TaskCreated",
            &json!({"task_id": "9", "task_subject": "Do it", "session_id": "lead-1"}),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap();
        assert_eq!(
            text,
            "[herdmates] TaskCreated task 9 \"Do it\" — reason: waiting (no grounded reason)."
        );
    }

    #[test]
    fn an_unread_inbox_accelerates_an_idle_teammate_into_the_quiet_tier() {
        let dir = TempDir::new("inbox");
        let paths = dir.paths();
        let inbox_dir = paths.teams_root.join("team-x").join("inboxes");
        std::fs::create_dir_all(&inbox_dir).unwrap();
        std::fs::write(
            inbox_dir.join("alpha.json"),
            r#"[{"from":"lead","text":"hi","timestamp":"2026-08-20T00:00:00Z","msgV":1,"msg_id":"m1","type":"message","read":false}]"#,
        )
        .unwrap();

        // Ten minutes after the message landed.
        let now = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_787_184_600);
        let text = enrichment_for_event(
            &paths,
            "team-x",
            "TeammateIdle",
            &json!({"teammate_name": "alpha"}),
            now,
        )
        .unwrap();
        assert!(text.contains("reason: quiet 10m"), "{text}");
    }

    // ── #123 review fix: lead-transcript staleness must not be attributed
    //    to a teammate (ADR-0013 wrong-reason class) ────────────────────────

    fn write_team_config(paths: &GatherPaths, team: &str, lead: &str) {
        let team_dir = paths.teams_root.join(team);
        std::fs::create_dir_all(&team_dir).unwrap();
        std::fs::write(
            team_dir.join("config.json"),
            format!(r#"{{"leadSessionId":"{lead}"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn a_lead_owned_transcript_grounds_no_staleness_for_a_teammate() {
        let dir = TempDir::new("lead-transcript");
        let paths = dir.paths();
        write_team_config(&paths, "team-x", "lead-1");
        let transcript = dir.0.join("lead-1.jsonl");
        std::fs::write(&transcript, "x").unwrap();
        let mtime = std::fs::metadata(&transcript).unwrap().modified().unwrap();

        // 12 quiet minutes on the LEAD's transcript; the payload's
        // session_id matches leadSessionId, so the fact must degrade away
        // and idle stays "turn complete" instead of a wrong "quiet 12m".
        let text = enrichment_for_event(
            &paths,
            "team-x",
            "TeammateIdle",
            &json!({
                "teammate_name": "alpha",
                "session_id": "lead-1",
                "transcript_path": transcript.to_str().unwrap(),
            }),
            mtime + std::time::Duration::from_secs(720),
        )
        .unwrap();
        assert_eq!(
            text,
            "[herdmates] TeammateIdle alpha — reason: turn complete."
        );
    }

    #[test]
    fn a_teammates_own_transcript_still_grounds_staleness() {
        let dir = TempDir::new("own-transcript");
        let paths = dir.paths();
        write_team_config(&paths, "team-x", "lead-1");
        let transcript = dir.0.join("mate-2.jsonl");
        std::fs::write(&transcript, "x").unwrap();
        let mtime = std::fs::metadata(&transcript).unwrap().modified().unwrap();

        let text = enrichment_for_event(
            &paths,
            "team-x",
            "TeammateIdle",
            &json!({
                "teammate_name": "alpha",
                "session_id": "mate-2",
                "transcript_path": transcript.to_str().unwrap(),
            }),
            mtime + std::time::Duration::from_secs(720),
        )
        .unwrap();
        assert!(text.contains("reason: stalled 12m"), "{text}");
    }
}
