use clap::Parser;
use session_telemetry_adb::{DeviceState, parse_devices_output};
use session_telemetry_analysis::{
    Finding, build_interaction_windows, detect_high_latency_focus_changes,
    detect_repeated_redux_dispatches,
};
use session_telemetry_cli::{
    Cli, Command, RecordMode, SessionState, format_devices, format_findings, format_record_started,
    format_status, format_stop_summary, opener_command,
};
use session_telemetry_report::{SessionSummary, render_html};
use session_telemetry_session::Chunk;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, exit};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => run_doctor(),
        Command::Devices => run_devices(),
        Command::Record { mode, device, name } => run_record(mode, device, name),
        Command::Stop => run_stop(),
        Command::Status => run_status(),
        Command::Analyze { session } => run_analyze(&session),
        Command::Report { session, open } => run_report(&session, open),
        other => println!("`{other:?}` is not implemented yet."),
    }
}

fn run_doctor() {
    match ProcessCommand::new("adb").arg("version").output() {
        Ok(output) if output.status.success() => println!("adb: found"),
        Ok(_) => println!("adb: found, but `adb version` exited non-zero"),
        Err(_) => println!("adb: not found on PATH"),
    }
}

fn run_devices() {
    match ProcessCommand::new("adb").args(["devices", "-l"]).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("{}", format_devices(&parse_devices_output(&stdout)));
        }
        Ok(output) => {
            eprintln!(
                "adb exited with an error:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            exit(1);
        }
        Err(_) => {
            eprintln!("adb not found on PATH. Run `session-telemetry doctor` for details.");
            exit(1);
        }
    }
}

/// `~/.session-telemetry/active-session.json` — consistent regardless of which directory the
/// CLI is invoked from. Falls back to the current directory if `HOME` isn't set, so this still
/// works (just less conveniently) rather than panicking.
fn state_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".session-telemetry")
        .join("active-session.json")
}

fn load_session_state() -> Option<SessionState> {
    let contents = std::fs::read_to_string(state_file_path()).ok()?;
    SessionState::from_json(&contents).ok()
}

fn save_session_state(state: &SessionState) {
    let path = state_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = state.to_json() {
        let _ = std::fs::write(&path, json);
    }
}

fn clear_session_state() {
    let _ = std::fs::remove_file(state_file_path());
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

/// True if `adb devices -l` lists `device` as connected and authorized — `record` validates
/// this up front rather than writing session state for a device that was never reachable.
fn device_is_connected(device: &str) -> bool {
    let Ok(output) = ProcessCommand::new("adb").args(["devices", "-l"]).output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_devices_output(&stdout)
        .iter()
        .any(|info| info.serial == device && info.state == DeviceState::Device)
}

fn run_record(mode: RecordMode, device: String, name: String) {
    if load_session_state().is_some() {
        eprintln!(
            "A recording is already active. Run `session-telemetry stop` first, or `session-telemetry status` to see what's running."
        );
        exit(1);
    }
    if !device_is_connected(&device) {
        eprintln!(
            "Device {device} is not connected (or not authorized). Run `session-telemetry devices` to check."
        );
        exit(1);
    }

    let state = SessionState {
        name,
        mode,
        device,
        started_at_unix_ms: now_unix_ms(),
    };
    save_session_state(&state);
    println!("{}", format_record_started(&state));
}

fn run_stop() {
    match load_session_state() {
        Some(state) => {
            println!("{}", format_stop_summary(&state, now_unix_ms()));
            clear_session_state();
        }
        None => {
            eprintln!("No active recording.");
            exit(1);
        }
    }
}

fn run_status() {
    println!(
        "{}",
        format_status(load_session_state().as_ref(), now_unix_ms())
    );
}

/// `session` is a path to a chunk JSON file for now — see the `Analyze`/`Report` doc comments
/// in lib.rs for why "latest"/named lookup isn't implemented yet.
fn load_chunk(session: &str) -> Chunk {
    let bytes = std::fs::read(session).unwrap_or_else(|err| {
        eprintln!("could not read {session}: {err}");
        exit(1);
    });

    Chunk::decode(&bytes).unwrap_or_else(|err| {
        eprintln!("could not decode {session}: {err}");
        exit(1);
    })
}

fn collect_findings(chunk: &Chunk) -> Vec<Finding> {
    let windows = build_interaction_windows(&chunk.events);

    let mut findings = detect_high_latency_focus_changes(&windows);
    findings.extend(detect_repeated_redux_dispatches(&windows));
    findings.sort_by_key(|finding| finding.sequence_start);
    findings
}

fn run_analyze(session: &str) {
    let chunk = load_chunk(session);
    println!("{}", format_findings(&collect_findings(&chunk)));
}

fn run_report(session: &str, open: bool) {
    let chunk = load_chunk(session);
    let summary = SessionSummary::from_events(&chunk.events);
    let findings = collect_findings(&chunk);

    match summary.to_json() {
        Ok(json) => println!("{json}"),
        Err(err) => {
            eprintln!("could not generate report: {err}");
            exit(1);
        }
    }

    let html_path = format!("{session}.html");
    let html = render_html(&summary, &findings);
    if let Err(err) = std::fs::write(&html_path, html) {
        eprintln!("could not write HTML report to {html_path}: {err}");
        exit(1);
    }
    println!("HTML report written to {html_path}");

    if open
        && let Err(err) = ProcessCommand::new(opener_command())
            .arg(&html_path)
            .status()
    {
        eprintln!("could not open {html_path}: {err}");
    }
}
