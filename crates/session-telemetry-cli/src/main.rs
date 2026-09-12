use clap::Parser;
use session_telemetry_adb::parse_devices_output;
use session_telemetry_analysis::{
    Finding, build_interaction_windows, detect_high_latency_focus_changes,
    detect_repeated_redux_dispatches,
};
use session_telemetry_cli::{Cli, Command, format_devices, format_findings, opener_command};
use session_telemetry_report::{SessionSummary, render_html};
use session_telemetry_session::Chunk;
use std::process::{Command as ProcessCommand, exit};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => run_doctor(),
        Command::Devices => run_devices(),
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
