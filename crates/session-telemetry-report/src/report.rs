use crate::summary::SessionSummary;
use serde::Serialize;
use session_telemetry_analysis::{Finding, QaBookmark};
use session_telemetry_protocol::Event;

/// Bumped when the report document's own shape changes (distinct from `ChunkManifest`'s
/// `SCHEMA_VERSION`, which versions the on-disk session format, not this derived document) —
/// lets a downstream consumer decide whether it can parse a given report directly.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

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
    pub schema_version: u32,
    pub summary: &'a SessionSummary,
    /// `ClockMap::uncertainty_ms` at report time, or `None` when the session had no clock-sync
    /// samples to fit a map from (see `main.rs::load_mapped_bookmarks` for the same check).
    pub clock_uncertainty_ms: Option<f64>,
    pub findings: Vec<FindingWithEvidence<'a>>,
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
            schema_version: REPORT_SCHEMA_VERSION,
            summary,
            clock_uncertainty_ms,
            findings: findings
                .iter()
                .map(|finding| FindingWithEvidence {
                    finding,
                    evidence: evidence_for(finding, events),
                })
                .collect(),
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
        HIGH_LATENCY_FOCUS_CHANGE_DETECTOR, HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION, Severity,
        Threshold, create_bookmark,
    };
    use session_telemetry_protocol::{FocusEvent, RemoteInputEvent};

    fn sample_finding() -> Finding {
        Finding {
            id: format!("{HIGH_LATENCY_FOCUS_CHANGE_DETECTOR}-0-1"),
            detector: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
            detector_version: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION,
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
    fn schema_version_and_raw_events_are_carried_but_events_are_not_serialized() {
        let events = sample_events();
        let summary = SessionSummary::from_events(&events);

        let report = Report::new(&summary, &[], &events, &[], None);
        let json = report.to_json().unwrap();

        assert_eq!(report.events.len(), 2);
        assert!(json.contains("\"schemaVersion\": 1"));
        assert!(!json.contains("\"remote-input\""));
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
