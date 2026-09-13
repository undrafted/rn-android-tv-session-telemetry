use clap::Parser;
use session_telemetry_adb::{
    DeviceState, parse_devices_output, parse_find_output, resolve_session_dir,
};
use session_telemetry_analysis::{
    Finding, build_interaction_windows, detect_excessive_commits_during_rapid_focus_movement,
    detect_high_latency_focus_changes, detect_js_stalls_overlapping_interactions,
    detect_network_completions_followed_by_commits,
    detect_react_commits_overlapping_delayed_frames, detect_repeated_network_requests,
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
        Command::Pull {
            session,
            device,
            package,
            out,
        } => run_pull(&session, &device, &package, out),
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

/// Runs `find` inside the target app's private storage via `adb shell run-as` — the mechanism
/// that actually works for a debuggable app's private files, unlike plain `adb pull` (which
/// runs as the `shell` user and can't read another app's `/data/user/0/<package>/...` without
/// root). Returns the parsed paths, or an empty list if the device/package/adb itself isn't
/// reachable — every call site treats "found nothing" and "couldn't ask" the same way.
fn find_on_device(device: &str, package: &str, find_args: &[&str]) -> Vec<String> {
    let mut args = vec!["-s", device, "shell", "run-as", package, "find"];
    args.extend(find_args);
    let Ok(output) = ProcessCommand::new("adb").args(&args).output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_find_output(&String::from_utf8_lossy(&output.stdout))
}

/// Same `run-as` mechanism as `find_on_device`, but `cat`s one file's raw bytes back rather
/// than listing paths.
fn cat_on_device(device: &str, package: &str, remote_path: &str) -> Option<Vec<u8>> {
    let output = ProcessCommand::new("adb")
        .args(["-s", device, "shell", "run-as", package, "cat", remote_path])
        .output()
        .ok()?;
    output.status.success().then_some(output.stdout)
}

fn run_pull(session: &str, device: &str, package: &str, out: Option<String>) {
    let session_dirs = find_on_device(
        device,
        package,
        &[
            "files/rnst-sessions",
            "-mindepth",
            "1",
            "-maxdepth",
            "1",
            "-type",
            "d",
        ],
    );
    if session_dirs.is_empty() {
        eprintln!(
            "No sessions found on {device} for {package}. Is a profiling build installed and recording?"
        );
        exit(1);
    }

    let Some(session_dir) = resolve_session_dir(session, &session_dirs) else {
        let available: Vec<&str> = session_dirs
            .iter()
            .map(|dir| dir.rsplit('/').next().unwrap_or(dir))
            .collect();
        eprintln!(
            "No session \"{session}\" found on {device} for {package}. Available: {}",
            available.join(", ")
        );
        exit(1);
    };
    let session_dir = session_dir.to_string();
    let session_name = session_dir.rsplit('/').next().unwrap_or(&session_dir);

    let remote_files = find_on_device(
        device,
        package,
        &[&session_dir, "-type", "f", "-name", "*.rnst"],
    );
    if remote_files.is_empty() {
        eprintln!("Session {session_name} has no sealed .rnst chunks yet.");
        exit(1);
    }

    let out_dir = out.unwrap_or_else(|| format!("pulled-sessions/{session_name}"));
    if let Err(err) = std::fs::create_dir_all(&out_dir) {
        eprintln!("could not create {out_dir}: {err}");
        exit(1);
    }

    let mut pulled_count = 0;
    for remote_file in &remote_files {
        let Some(bytes) = cat_on_device(device, package, remote_file) else {
            eprintln!("could not pull {remote_file}, skipping");
            continue;
        };
        let file_name = remote_file.rsplit('/').next().unwrap_or(remote_file);
        let local_path = PathBuf::from(&out_dir).join(file_name);
        if let Err(err) = std::fs::write(&local_path, bytes) {
            eprintln!("could not write {}: {err}", local_path.display());
            continue;
        }
        pulled_count += 1;
    }

    println!("Pulled {pulled_count} chunk(s) from {session_name} into {out_dir}/");
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
    findings.extend(detect_js_stalls_overlapping_interactions(&windows));
    findings.extend(detect_repeated_network_requests(&windows));
    findings.extend(detect_react_commits_overlapping_delayed_frames(&windows));
    findings.extend(detect_network_completions_followed_by_commits(&windows));
    findings.extend(detect_excessive_commits_during_rapid_focus_movement(
        &windows,
    ));
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
