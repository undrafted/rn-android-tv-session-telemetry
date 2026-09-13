//! CLI argument parsing and pure formatting logic. Kept separate from `main.rs` so it's unit
//! testable without spawning a real `adb` process — this machine doesn't have one to test
//! against.

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use session_telemetry_adb::{DeviceInfo, DeviceState};
use session_telemetry_analysis::{Finding, QaBookmark, Severity};
use std::time::SystemTime;

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
        /// Android application id to send the start signal to (e.g. com.example.app) — the app
        /// itself can also start on its own via a profiling build's `SessionTelemetry.install()`
        /// (bidirectional; see main.rs's send_broadcast), so this is best-effort: if the app
        /// isn't running yet, it'll pick up its own autonomous start when it launches instead.
        #[arg(long)]
        package: String,
    },
    /// Stop the active recording.
    Stop,
    /// Add a workstation-timestamped QA annotation to the active recording, for later mapping
    /// onto the session's own timeline via clock sync.
    Mark { label: String },
    /// Analyze a recorded session.
    Analyze {
        /// Path to a chunk JSON file, or "latest" for the most recently pulled one (by file
        /// modification time, under ./pulled-sessions/). Named lookup by session name isn't
        /// implemented yet — only a direct path or "latest" work today.
        session: String,
    },
    /// Generate a report for a recorded session.
    Report {
        /// Path to a chunk JSON file, or "latest" for the most recently pulled one (by file
        /// modification time, under ./pulled-sessions/). Named lookup by session name isn't
        /// implemented yet — only a direct path or "latest" work today.
        session: String,
        /// Open the generated HTML report with the OS default application once it's written.
        #[arg(long)]
        open: bool,
    },
    /// Show current recording status.
    Status,
    /// Pull a recorded session's .rnst chunks from the device.
    Pull {
        /// The on-device session directory name (a millisecond timestamp) under
        /// `rnst-sessions/`, or "latest" for the most recently created one.
        session: String,
        #[arg(long)]
        device: String,
        /// Android application id the session was recorded from — whichever app embeds this
        /// library and wrote the .rnst chunks (e.g. com.example.app).
        #[arg(long)]
        package: String,
        /// Local directory to write the pulled chunk files into. Defaults to
        /// ./pulled-sessions/<session>.
        #[arg(long)]
        out: Option<String>,
        /// How many pulled sessions to retain (by local modification time) in the destination's
        /// parent directory after this pull — older ones beyond that are deleted. Only prunes
        /// siblings under the same parent `--out` resolves into, never anything else on disk.
        #[arg(long, default_value_t = 10)]
        keep: usize,
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

/// A `session-telemetry mark` annotation — plan.md's "QA annotation" row: workstation
/// timestamp and bookmark label, captured with no live device bridge. Mapping it onto the
/// session's own timeline happens later, at `analyze`/`report` time, via `create_bookmark` in
/// session-telemetry-analysis and the clock-sync samples decoded from the pulled chunk (plus
/// `BookmarkFile::device_clock_offset_ms`, captured once at `record` time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub label: String,
    pub workstation_timestamp_unix_ms: u64,
}

/// What `stop` persists to `~/.session-telemetry/bookmarks/<name>-<started_at>.json`, and what
/// `pull` copies alongside the pulled chunks as `bookmarks.json` for `analyze`/`report` to read.
/// `device_clock_offset_ms` (device wall clock minus workstation wall clock, sampled once via
/// `adb shell date` at `record` time) corrects `Bookmark::workstation_timestamp_unix_ms` onto
/// the device's own wall-clock domain before it's mapped onto the session timeline — without it,
/// two machines with merely-different system clocks (not even actually desynced/wrong) would
/// silently place bookmarks at the wrong moment. `None` when the offset couldn't be measured
/// (`adb shell date` failed) — mapping falls back to assuming the two clocks already agree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkFile {
    pub device_clock_offset_ms: Option<i64>,
    pub bookmarks: Vec<Bookmark>,
}

impl BookmarkFile {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<BookmarkFile, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// The CLI's own record of "a recording is active", and the QA bookmarks captured against it
/// (see `Bookmark`). Recording is bidirectional: the app itself can start/end a session on its
/// own (`SessionTelemetry.install()`/`.stop()`), and `record`/`stop` can also bound one
/// explicitly via an ADB broadcast to `package` (see main.rs's `send_broadcast`) — this state
/// file is local bookkeeping either way, so `status`/`mark`/`stop` have something to act on.
/// Stored under `~/.session-telemetry/active-session.json` (see main.rs) so it's consistent
/// regardless of which directory the CLI is invoked from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub name: String,
    pub mode: RecordMode,
    pub device: String,
    pub package: String,
    pub started_at_unix_ms: u64,
    #[serde(default)]
    pub bookmarks: Vec<Bookmark>,
    /// Device wall clock minus workstation wall clock, in milliseconds, sampled once via
    /// `adb shell date` when `record` started — see `BookmarkFile::device_clock_offset_ms`.
    #[serde(default)]
    pub device_clock_offset_ms: Option<i64>,
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
    let bookmark_note = match state.bookmarks.len() {
        0 => String::new(),
        count => format!(" {count} bookmark(s) saved."),
    };
    format!(
        "Stopped \"{}\" on {} after {}. The captured .rnst chunks remain on the device until \
         pulled.{bookmark_note}",
        state.name,
        state.device,
        format_duration_ms(stopped_at_unix_ms.saturating_sub(state.started_at_unix_ms))
    )
}

/// Printed by `session-telemetry mark` once the bookmark is appended to the active session's
/// state.
pub fn format_mark_confirmation(label: &str, bookmark_count: usize) -> String {
    format!("Marked \"{label}\" ({bookmark_count} bookmark(s) so far this session).")
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

/// Human-readable bookmark listing for `session-telemetry analyze`, mirroring `format_findings`.
/// `bookmarks` are already mapped onto the session timeline (see `create_bookmark`) by the
/// caller — this only renders them.
pub fn format_bookmarks(bookmarks: &[QaBookmark]) -> String {
    if bookmarks.is_empty() {
        return String::new();
    }

    bookmarks
        .iter()
        .map(|bookmark| {
            format!(
                "[bookmark] {:.0} ms: {}",
                bookmark.session_timestamp, bookmark.label
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

/// Given candidate session chunk files (path + a recency indicator, e.g. file modification
/// time), returns the most recent one's path — the "latest" resolution for `analyze`/`report`.
/// `None` if `candidates` is empty. Pure decision logic, same split as
/// `session_telemetry_adb::sessions_to_prune` vs. the real directory walk around it in main.rs.
pub fn resolve_latest_session_file(candidates: &[(String, SystemTime)]) -> Option<&str> {
    candidates
        .iter()
        .max_by_key(|(_, modified)| *modified)
        .map(|(path, _)| path.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
            "--package",
            "com.rnsessiontelemetry.tvfixture",
        ])
        .unwrap();

        assert_eq!(
            cli.command,
            Command::Record {
                mode: RecordMode::Targeted,
                device: "192.168.1.40:5555".to_string(),
                name: "catalog-navigation".to_string(),
                package: "com.rnsessiontelemetry.tvfixture".to_string(),
            }
        );
    }

    #[test]
    fn parses_mark_with_its_label() {
        let cli = Cli::try_parse_from(["session-telemetry", "mark", "carousel stopped responding"])
            .unwrap();

        assert_eq!(
            cli.command,
            Command::Mark {
                label: "carousel stopped responding".to_string()
            }
        );
    }

    #[test]
    fn rejects_an_unknown_subcommand() {
        assert!(Cli::try_parse_from(["session-telemetry", "not-a-real-command"]).is_err());
    }

    #[test]
    fn parses_pull_with_its_flags() {
        let cli = Cli::try_parse_from([
            "session-telemetry",
            "pull",
            "latest",
            "--device",
            "emulator-5554",
            "--package",
            "com.rnsessiontelemetry.tvfixture",
        ])
        .unwrap();

        assert_eq!(
            cli.command,
            Command::Pull {
                session: "latest".to_string(),
                device: "emulator-5554".to_string(),
                package: "com.rnsessiontelemetry.tvfixture".to_string(),
                out: None,
                keep: 10,
            }
        );
    }

    #[test]
    fn parses_pull_with_an_explicit_out_directory_and_keep_count() {
        let cli = Cli::try_parse_from([
            "session-telemetry",
            "pull",
            "1789277657612",
            "--device",
            "emulator-5554",
            "--package",
            "com.rnsessiontelemetry.tvfixture",
            "--out",
            "./sessions/first",
            "--keep",
            "5",
        ])
        .unwrap();

        assert_eq!(
            cli.command,
            Command::Pull {
                session: "1789277657612".to_string(),
                device: "emulator-5554".to_string(),
                package: "com.rnsessiontelemetry.tvfixture".to_string(),
                out: Some("./sessions/first".to_string()),
                keep: 5,
            }
        );
    }

    fn sample_state() -> SessionState {
        SessionState {
            name: "catalog-navigation".to_string(),
            mode: RecordMode::Qa,
            device: "192.168.1.40:5555".to_string(),
            package: "com.rnsessiontelemetry.tvfixture".to_string(),
            started_at_unix_ms: 1_000,
            bookmarks: Vec::new(),
            device_clock_offset_ms: None,
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
    fn format_stop_summary_omits_the_bookmark_note_when_there_are_none() {
        let summary = format_stop_summary(&sample_state(), sample_state().started_at_unix_ms);
        assert!(!summary.contains("bookmark"));
    }

    #[test]
    fn format_stop_summary_mentions_the_bookmark_count() {
        let mut state = sample_state();
        state.bookmarks.push(Bookmark {
            label: "carousel stopped responding".to_string(),
            workstation_timestamp_unix_ms: 1_500,
        });
        state.bookmarks.push(Bookmark {
            label: "navigation felt delayed".to_string(),
            workstation_timestamp_unix_ms: 2_500,
        });

        let summary = format_stop_summary(&state, state.started_at_unix_ms);

        assert!(summary.contains("2 bookmark(s) saved"));
    }

    #[test]
    fn format_mark_confirmation_includes_the_label_and_count() {
        let message = format_mark_confirmation("carousel stopped responding", 2);

        assert!(message.contains("carousel stopped responding"));
        assert!(message.contains('2'));
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
    fn format_bookmarks_is_empty_when_there_are_none() {
        assert_eq!(format_bookmarks(&[]), "");
    }

    #[test]
    fn format_bookmarks_includes_the_mapped_timestamp_and_label() {
        let bookmarks = vec![QaBookmark {
            workstation_timestamp: 1_700_000_000_500.0,
            session_timestamp: 4_200.0,
            label: "carousel stopped responding".to_string(),
        }];

        assert_eq!(
            format_bookmarks(&bookmarks),
            "[bookmark] 4200 ms: carousel stopped responding"
        );
    }

    #[test]
    fn bookmark_file_round_trips_through_json() {
        let file = BookmarkFile {
            device_clock_offset_ms: Some(-9_155_000),
            bookmarks: vec![Bookmark {
                label: "navigation felt delayed".to_string(),
                workstation_timestamp_unix_ms: 1_700_000_000_000,
            }],
        };

        let json = file.to_json().unwrap();
        assert_eq!(BookmarkFile::from_json(&json).unwrap(), file);
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

    #[test]
    fn resolve_latest_session_file_picks_the_most_recently_modified() {
        let base = SystemTime::UNIX_EPOCH;
        let candidates = vec![
            (
                "pulled-sessions/a/chunk-00000.rnst".to_string(),
                base + Duration::from_secs(1),
            ),
            (
                "pulled-sessions/b/chunk-00000.rnst".to_string(),
                base + Duration::from_secs(3),
            ),
            (
                "pulled-sessions/c/chunk-00000.rnst".to_string(),
                base + Duration::from_secs(2),
            ),
        ];

        assert_eq!(
            resolve_latest_session_file(&candidates),
            Some("pulled-sessions/b/chunk-00000.rnst")
        );
    }

    #[test]
    fn resolve_latest_session_file_returns_none_when_nothing_was_pulled() {
        assert_eq!(resolve_latest_session_file(&[]), None);
    }
}
