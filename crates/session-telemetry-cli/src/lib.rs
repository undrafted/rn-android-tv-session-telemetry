//! CLI argument parsing and pure formatting logic. Kept separate from `main.rs` so it's unit
//! testable without spawning a real `adb` process — this machine doesn't have one to test
//! against.

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use session_telemetry_adb::{DeviceInfo, DeviceState};
use session_telemetry_analysis::{Finding, Severity};

/// Most subcommands are parsed but not yet implemented — see `main.rs` for which ones actually
/// do something today.
#[derive(Parser, Debug, PartialEq)]
#[command(
    name = "session-telemetry",
    version,
    about = "RN Session Telemetry CLI"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Command {
    /// Check that required tooling (currently: adb) is available.
    Doctor,
    /// List Android devices/emulators visible to adb.
    Devices,
    /// Start recording a session.
    Record {
        #[arg(long, value_enum)]
        mode: RecordMode,
        #[arg(long)]
        device: String,
        #[arg(long)]
        name: String,
    },
    /// Stop the active recording.
    Stop,
    /// Analyze a recorded session.
    Analyze {
        /// Path to a chunk JSON file. Session-id lookup ("latest", by name) from a sessions
        /// directory isn't implemented yet — only direct file paths work today.
        session: String,
    },
    /// Generate a report for a recorded session.
    Report {
        /// Path to a chunk JSON file. Session-id lookup ("latest", by name) from a sessions
        /// directory isn't implemented yet — only direct file paths work today.
        session: String,
        /// Not implemented yet — no HTML report exists to open.
        #[arg(long)]
        open: bool,
    },
    /// Show current recording status.
    Status,
    /// Add a QA bookmark to the active recording.
    Mark { label: String },
    /// Pull a sealed session from the device.
    Pull {
        session: String,
        #[arg(long)]
        device: String,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RecordMode {
    Targeted,
    Qa,
}

impl RecordMode {
    pub fn label(&self) -> &'static str {
        match self {
            RecordMode::Targeted => "targeted",
            RecordMode::Qa => "qa",
        }
    }
}

/// The CLI's own record of "a recording is active" — no live ADB route to the device exists
/// yet (see architecture.mmd's still-planned `LivePull`), so this doesn't control anything on
/// the device directly. The app itself starts capturing on its own, via a profiling build's
/// `SessionTelemetry.install()`; this state file is local bookkeeping so `status`/`stop` can
/// report on a recording that's presumed to be running, keyed by the name/device the developer
/// gave `record`. Stored under `~/.session-telemetry/active-session.json` (see main.rs) so it's
/// consistent regardless of which directory the CLI is invoked from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub name: String,
    pub mode: RecordMode,
    pub device: String,
    pub started_at_unix_ms: u64,
}

impl SessionState {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<SessionState, serde_json::Error> {
        serde_json::from_str(json)
    }
}

fn format_duration_ms(duration_ms: u64) -> String {
    let total_seconds = duration_ms / 1000;
    format!("{}m {}s", total_seconds / 60, total_seconds % 60)
}

/// Confirmation printed by `session-telemetry record` once the device is validated and the
/// state file is written.
pub fn format_record_started(state: &SessionState) -> String {
    format!(
        "Recording \"{}\" ({} mode) on {}. Launch a profiling build on the device now — run \
         `session-telemetry stop` when done.",
        state.name,
        state.mode.label(),
        state.device
    )
}

/// Printed by `session-telemetry status`. `now_unix_ms` is a parameter (not read internally via
/// `SystemTime::now()`) so this stays pure and testable with a fixed clock reading.
pub fn format_status(state: Option<&SessionState>, now_unix_ms: u64) -> String {
    match state {
        None => "No active recording.".to_string(),
        Some(state) => format!(
            "Recording \"{}\" ({} mode) on {} — {} elapsed",
            state.name,
            state.mode.label(),
            state.device,
            format_duration_ms(now_unix_ms.saturating_sub(state.started_at_unix_ms))
        ),
    }
}

/// Printed by `session-telemetry stop`.
pub fn format_stop_summary(state: &SessionState, stopped_at_unix_ms: u64) -> String {
    format!(
        "Stopped \"{}\" on {} after {}. The captured .rnst chunks remain on the device until \
         pulled.",
        state.name,
        state.device,
        format_duration_ms(stopped_at_unix_ms.saturating_sub(state.started_at_unix_ms))
    )
}

fn device_state_label(state: &DeviceState) -> String {
    match state {
        DeviceState::Device => "device".to_string(),
        DeviceState::Offline => "offline".to_string(),
        DeviceState::Unauthorized => "unauthorized".to_string(),
        DeviceState::Other(raw) => raw.clone(),
    }
}

/// Human-readable device listing for `session-telemetry devices`.
pub fn format_devices(devices: &[DeviceInfo]) -> String {
    if devices.is_empty() {
        return "No devices found.".to_string();
    }

    devices
        .iter()
        .map(|device| {
            let label = device.model.as_deref().unwrap_or("unknown model");
            format!(
                "{}\t{}\t{}",
                device.serial,
                device_state_label(&device.state),
                label
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Human-readable findings listing for `session-telemetry analyze`. Deliberately plain-text,
/// not JSON — this is a terminal summary, distinct from the JSON `session-telemetry-report`
/// output.
pub fn format_findings(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "No findings.".to_string();
    }

    findings
        .iter()
        .map(|finding| {
            let severity = match finding.severity {
                Severity::Warning => "warning",
                Severity::Critical => "critical",
            };
            format!(
                "[{severity}] {} (v{}): sequence {}-{}, {} {}",
                finding.detector,
                finding.detector_version,
                finding.sequence_start,
                finding.sequence_end,
                finding.value,
                finding.unit
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The OS command that opens a file with its default application, for `report --open`. Kept
/// separate from the actual `Command::new(...).status()` call (in main.rs) so the platform
/// selection is unit testable without spawning a real process — which, for this command
/// specifically, would pop open a browser window as a side effect of running the test suite.
pub fn opener_command() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devices_subcommand() {
        let cli = Cli::try_parse_from(["session-telemetry", "devices"]).unwrap();
        assert_eq!(cli.command, Command::Devices);
    }

    #[test]
    fn parses_record_with_its_flags() {
        let cli = Cli::try_parse_from([
            "session-telemetry",
            "record",
            "--mode",
            "targeted",
            "--device",
            "192.168.1.40:5555",
            "--name",
            "catalog-navigation",
        ])
        .unwrap();

        assert_eq!(
            cli.command,
            Command::Record {
                mode: RecordMode::Targeted,
                device: "192.168.1.40:5555".to_string(),
                name: "catalog-navigation".to_string(),
            }
        );
    }

    #[test]
    fn rejects_an_unknown_subcommand() {
        assert!(Cli::try_parse_from(["session-telemetry", "not-a-real-command"]).is_err());
    }

    fn sample_state() -> SessionState {
        SessionState {
            name: "catalog-navigation".to_string(),
            mode: RecordMode::Qa,
            device: "192.168.1.40:5555".to_string(),
            started_at_unix_ms: 1_000,
        }
    }

    #[test]
    fn session_state_round_trips_through_json() {
        let state = sample_state();
        let json = state.to_json().unwrap();
        assert_eq!(SessionState::from_json(&json).unwrap(), state);
    }

    #[test]
    fn format_status_reports_no_active_recording() {
        assert_eq!(format_status(None, 5_000), "No active recording.");
    }

    #[test]
    fn format_status_reports_elapsed_time() {
        let state = sample_state();
        assert_eq!(
            format_status(Some(&state), state.started_at_unix_ms + 125_000),
            "Recording \"catalog-navigation\" (qa mode) on 192.168.1.40:5555 — 2m 5s elapsed"
        );
    }

    #[test]
    fn format_record_started_names_the_session_and_device() {
        let message = format_record_started(&sample_state());
        assert!(message.contains("catalog-navigation"));
        assert!(message.contains("qa mode"));
        assert!(message.contains("192.168.1.40:5555"));
    }

    #[test]
    fn format_stop_summary_reports_elapsed_time() {
        let state = sample_state();
        let summary = format_stop_summary(&state, state.started_at_unix_ms + 65_000);
        assert!(summary.contains("1m 5s"));
        assert!(summary.contains("catalog-navigation"));
    }

    #[test]
    fn format_devices_reports_no_devices_found() {
        assert_eq!(format_devices(&[]), "No devices found.");
    }

    #[test]
    fn format_devices_falls_back_when_model_is_unknown() {
        let devices = vec![DeviceInfo {
            serial: "emulator-5554".to_string(),
            state: DeviceState::Offline,
            product: None,
            model: None,
            device: None,
            transport_id: None,
        }];

        assert_eq!(
            format_devices(&devices),
            "emulator-5554\toffline\tunknown model"
        );
    }

    #[test]
    fn format_findings_reports_no_findings() {
        assert_eq!(format_findings(&[]), "No findings.");
    }

    #[test]
    fn format_findings_includes_detector_severity_and_range() {
        let findings = vec![Finding {
            detector: "high-latency-focus-change",
            detector_version: 1,
            severity: Severity::Warning,
            sequence_start: 3,
            sequence_end: 7,
            value: 214.0,
            unit: "ms",
        }];

        assert_eq!(
            format_findings(&findings),
            "[warning] high-latency-focus-change (v1): sequence 3-7, 214 ms"
        );
    }

    #[test]
    fn opener_command_matches_this_platform() {
        let expected = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(target_os = "windows") {
            "start"
        } else {
            "xdg-open"
        };

        assert_eq!(opener_command(), expected);
    }
}
