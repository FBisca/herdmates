//! herdmates — Herdr plugin binary.
//!
//! Current surfaces: `teammux-launch`, `pump-board`, `record`, `hook`,
//! `doctor`.

use std::fmt::Display;
use std::process::ExitCode;

use herdmates::*;

fn main() -> ExitCode {
    paths::hydrate_environment();
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let args = args.collect::<Vec<_>>();

    match command.as_str() {
        "pump-board" => exit(pump::pump_board_command(&args)),
        "teammux-launch" => exit(teammux_launch::teammux_launch_command(&args)),
        // Issue #97 stage 2 (ADR-0013 §93 stage 2, docs/spec.md §4): minimal
        // recorder — polls the gather + signal-engine pipeline and appends
        // classified-observation deltas to an append-only JSONL log.
        "record" => exit(recorder::record_command(&args)),
        // Issue #100 stage 5 (ADR-0013 §93 stage 5, docs/spec.md §4):
        // push source for the three Claude Code team hook events
        // (TeammateIdle/TaskCreated/TaskCompleted) — appends to the spool
        // the engine/recorder consume. Distinct module from the legacy
        // `hook` (frozen v1.1.0, bound to `on-agent-status` above) —
        // different event source, different payload shape, no shared code.
        "hook" => team_hook::hook_command(&args),
        // Issue #106 static half: self-check subcommand (--probe half
        // blocked on #105, not implemented here).
        "doctor" => doctor::doctor_command(&args),
        "" | "help" | "--help" | "-h" => {
            eprintln!("herdmates <pump-board|teammux-launch|record|hook|doctor>");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown subcommand: {other}");
            ExitCode::FAILURE
        }
    }
}

fn exit(result: Result<(), impl Display>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
