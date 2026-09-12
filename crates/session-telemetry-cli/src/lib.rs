//! CLI argument parsing and pure formatting logic. Kept separate from `main.rs` so it's unit
//! testable without spawning a real `adb` process — this machine doesn't have one to test
//! against (plan.md's Week 1 gate).

use clap::{Parser, Subcommand, ValueEnum};
use session_telemetry_adb::{DeviceInfo, DeviceState};
use session_telemetry_analysis::{Finding, Severity};

/// Mirrors the CLI usage shown in plan.md section 5. Most subcommands are parsed but not yet
/// implemented — see `main.rs` for which ones actually do something today.
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

#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum RecordMode {
    Targeted,
    Qa,
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
/// not JSON — this is a terminal summary (plan.md section 5's "Terminal summary" deliverable),
/// distinct from the JSON `session-telemetry-report` output.
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
                "[{severity}] {} (v{}): sequence {}-{}, {:.1}ms",
                finding.detector,
                finding.detector_version,
                finding.sequence_start,
                finding.sequence_end,
                finding.latency_ms
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
            latency_ms: 214.0,
        }];

        assert_eq!(
            format_findings(&findings),
            "[warning] high-latency-focus-change (v1): sequence 3-7, 214.0ms"
        );
    }
}
