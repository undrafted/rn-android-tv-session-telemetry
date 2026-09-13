use crate::summary::SessionSummary;
use serde::Serialize;
use session_telemetry_analysis::{
    Finding, QaBookmark, SelectorStats, selector_stats, session_metadata_from_events,
};
use session_telemetry_protocol::{Event, SessionMetadataEvent};

/// A finding plus the raw events between its `sequence_start`/`sequence_end`, sorted by
/// sequence — self-contained evidence a JSON consumer can read without also fetching the full
/// session.
#[derive(Debug, Clone, Serialize)]
pub struct FindingWithEvidence<'a> {
    #[serde(flatten)]
    pub finding: &'a Finding,
    pub evidence: Vec<&'a Event>,
}

/// The single structured model both the JSON export and the HTML report render from — a script
/// can understand every finding from `to_json()` alone, without parsing terminal or HTML text.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report<'a> {
    pub summary: &'a SessionSummary,
    /// A missing callback cannot distinguish unsupported profiling, an unwrapped root,
    /// inactive capture, or no commits in this recording. Never interpret it as zero work.
    pub react_commit_capture: &'static str,
    /// `ClockMap::uncertainty_ms` at report time, or `None` when the session had no clock-sync
    /// samples to fit a map from (see `main.rs::load_mapped_bookmarks` for the same check).
    pub clock_uncertainty_ms: Option<f64>,
    /// The session's device/build context, if the JS library that recorded it emitted one — see
    /// `SessionMetadataEvent`'s own doc comment. `None` for a session recorded by an older
    /// library version, not an error.
    pub device_metadata: Option<&'a SessionMetadataEvent>,
    pub findings: Vec<FindingWithEvidence<'a>>,
    /// Per-selector aggregate stats across the whole session — see `SelectorStats`'s own doc
    /// comment. Empty when no selector was instrumented, not an error.
    pub selector_stats: Vec<SelectorStats>,
    pub bookmarks: &'a [QaBookmark],
    /// The full raw session, kept off the wire (`#[serde(skip)]`) so the JSON document stays
    /// bounded regardless of session length — consistent with this codebase's summary-first
    /// principle elsewhere (windowed timeline rendering, capped HTML timeline). Only used by
    /// `render_html`'s full session timeline; every finding's own evidence is already carried by
    /// `findings` above.
    #[serde(skip)]
    pub events: &'a [Event],
}

impl<'a> Report<'a> {
    pub fn new(
        summary: &'a SessionSummary,
        findings: &'a [Finding],
        events: &'a [Event],
        bookmarks: &'a [QaBookmark],
        clock_uncertainty_ms: Option<f64>,
    ) -> Report<'a> {
        Report {
            summary,
            react_commit_capture: if summary.react_commit_count > 0 {
                "observed"
            } else {
                "unavailable-or-not-observed"
            },
            clock_uncertainty_ms,
            device_metadata: session_metadata_from_events(events),
            findings: findings
                .iter()
                .map(|finding| FindingWithEvidence {
                    finding,
                    evidence: evidence_for(finding, events),
                })
                .collect(),
            selector_stats: selector_stats(events),
            bookmarks,
            events,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// The events between a finding's `sequence_start` and `sequence_end`, sorted by sequence —
/// shared by the JSON report and the HTML evidence timeline so both derive the same evidence
/// from the same finding, rather than filtering events twice.
pub(crate) fn evidence_for<'a>(finding: &Finding, events: &'a [Event]) -> Vec<&'a Event> {
    let mut window_events: Vec<&Event> = events
        .iter()
        .filter(|event| {
            let sequence = event.sequence();
            sequence >= finding.sequence_start && sequence <= finding.sequence_end
        })
        .collect();
    window_events.sort_by_key(|event| event.sequence());
    window_events
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_analysis::{
        HIGH_LATENCY_FOCUS_CHANGE_DETECTOR, Severity, Threshold, create_bookmark,
    };
    use session_telemetry_protocol::{FocusEvent, RemoteInputEvent};

    fn sample_finding() -> Finding {
        Finding {
            id: format!("{HIGH_LATENCY_FOCUS_CHANGE_DETECTOR}-0-1"),
            detector: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
            severity: Severity::Warning,
            sequence_start: 0,
            sequence_end: 1,
            value: 214.0,
            unit: "ms",
            thresholds: vec![Threshold {
                name: "warningMs",
                value: 100.0,
            }],
            summary: "Input-to-focus-change latency was 214ms.".to_string(),
        }
    }

    fn sample_events() -> Vec<Event> {
        vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 214.0,
                target_id: "card-2".to_string(),
                previous_target_id: None,
            }),
        ]
    }

    #[test]
    fn missing_commits_disclose_unknown_capture_in_json_and_html() {
        let summary = SessionSummary::from_events(&[]);
        let report = Report::new(&summary, &[], &[], &[], None);
        assert_eq!(report.react_commit_capture, "unavailable-or-not-observed");
        assert!(
            report
                .to_json()
                .unwrap()
                .contains("unavailable-or-not-observed")
        );
        assert!(crate::render_html(&report).contains("not a measurement of zero React work"));
    }

    #[test]
    fn real_commit_events_disclose_observed_capture_and_render_work_semantics() {
        let events = vec![Event::ReactCommit(
            session_telemetry_protocol::ReactCommitEvent {
                sequence: 1,
                timestamp: 50.0,
                profiler_id: "App".to_string(),
                phase: session_telemetry_protocol::ReactCommitPhase::Mount,
                actual_duration_ms: 20.0,
                base_duration_ms: 25.0,
                render_start_ms: Some(10.0),
                commit_time_ms: Some(40.0),
            },
        )];
        let summary = SessionSummary::from_events(&events);
        let findings = vec![sample_finding()];
        let report = Report::new(&summary, &findings, &events, &[], None);
        assert_eq!(report.react_commit_capture, "observed");
        assert!(
            report
                .to_json()
                .unwrap()
                .contains("\"renderStartMs\": 10.0")
        );
        assert!(report.to_json().unwrap().contains("\"commitTimeMs\": 40.0"));
        let html = crate::render_html(&report);
        assert!(html.contains("elapsed 30.000 ms (may include pauses)"));
        assert!(html.contains("20 ms render work"));
        assert!(
            report
                .to_json()
                .unwrap()
                .contains("\"reactCommitCapture\": \"observed\"")
        );
        assert!(
            crate::render_html(&report).contains("not commit-phase or screen-presentation time")
        );
    }

    #[test]
    fn attaches_evidence_events_within_a_findings_sequence_range() {
        let events = sample_events();
        let findings = vec![sample_finding()];
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &findings, &events, &[], None);

        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].evidence.len(), 2);
        assert_eq!(report.findings[0].evidence[0].sequence(), 0);
        assert_eq!(report.findings[0].evidence[1].sequence(), 1);
    }

    #[test]
    fn raw_events_are_carried_but_not_serialized() {
        let events = sample_events();
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &[], &events, &[], None);
        let json = report.to_json().unwrap();

        assert_eq!(report.events.len(), 2);
        assert!(!json.contains("schemaVersion"));
        assert!(!json.contains("\"remote-input\""));
    }

    #[test]
    fn carries_the_sessions_device_metadata_when_present() {
        let metadata = session_telemetry_protocol::SessionMetadataEvent {
            sequence: 0,
            timestamp: 0.0,
            device_model: "sdk_google_atv64_arm64".to_string(),
            os_version: "14".to_string(),
            app_version: None,
            build_type: None,
        };
        let events = vec![Event::SessionMetadata(metadata.clone())];
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &[], &events, &[], None);

        assert_eq!(report.device_metadata, Some(&metadata));
    }

    #[test]
    fn device_metadata_is_none_when_absent() {
        let events = sample_events();
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &[], &events, &[], None);

        assert_eq!(report.device_metadata, None);
    }

    #[test]
    fn carries_aggregated_selector_stats() {
        let events = vec![Event::Selector(session_telemetry_protocol::SelectorEvent {
            sequence: 0,
            timestamp: 0.0,
            selector_id: "catalog/selectVisibleItemIds".to_string(),
            duration_ms: 0.5,
            inputs_changed: true,
            result_changed: true,
        })];
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &[], &events, &[], None);

        assert_eq!(report.selector_stats.len(), 1);
        assert_eq!(
            report.selector_stats[0].selector_id,
            "catalog/selectVisibleItemIds"
        );
    }

    #[test]
    fn matches_the_golden_json_fixture() {
        let events = sample_events();
        let findings = vec![sample_finding()];
        let mut summary = SessionSummary::from_events(&events);
        summary.loss_count = 1;
        let clock_map = session_telemetry_analysis::ClockMap::from_samples(&[
            session_telemetry_analysis::ClockSyncSample {
                source: 1_700_000_000_000.0,
                reference: 0.0,
            },
            session_telemetry_analysis::ClockSyncSample {
                source: 1_700_000_000_500.0,
                reference: 500.0,
            },
        ])
        .unwrap();
        let bookmarks = vec![create_bookmark(
            &clock_map,
            1_700_000_000_100.0,
            "carousel stopped responding".to_string(),
        )];

        let report = Report::new(
            &summary,
            &findings,
            &events,
            &bookmarks,
            Some(clock_map.uncertainty_ms),
        );

        let json = report.to_json().unwrap();
        let golden = include_str!("../tests/fixtures/report.golden.json");

        assert_eq!(json.trim(), golden.trim());
    }
}
