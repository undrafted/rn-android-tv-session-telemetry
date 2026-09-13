use serde::Serialize;
use session_telemetry_protocol::Event;

/// The report's entry point — a long session's report must open with a summary rather than
/// rendering every event at once. This is that summary's data, independent of whichever
/// renderer (JSON today, HTML later) presents it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub event_count: usize,
    pub remote_input_count: usize,
    pub focus_count: usize,
    pub interaction_marker_count: usize,
    pub redux_dispatch_count: usize,
    pub network_count: usize,
    pub js_stall_count: usize,
    pub react_commit_count: usize,
    pub frame_timing_count: usize,
    pub clock_sync_count: usize,
    pub visible_update_count: usize,
    pub session_metadata_count: usize,
    /// The chunk's own `ChunkManifest::loss_count` — not derivable from `events` alone (a lost
    /// event is, by definition, not among them), so this defaults to `0` here and the caller
    /// (`main.rs`, which has the manifest) sets it explicitly after construction. Always `0`
    /// today regardless: no gap/loss detection is wired up on the writer side yet, but the field
    /// is real schema, not a placeholder — see docs/measurement-semantics.md.
    pub loss_count: u32,
    pub sequence_start: Option<u64>,
    pub sequence_end: Option<u64>,
    pub timestamp_start: Option<f64>,
    pub timestamp_end: Option<f64>,
}

impl SessionSummary {
    pub fn from_events(events: &[Event]) -> SessionSummary {
        let mut summary = SessionSummary {
            event_count: events.len(),
            remote_input_count: 0,
            focus_count: 0,
            interaction_marker_count: 0,
            redux_dispatch_count: 0,
            network_count: 0,
            js_stall_count: 0,
            react_commit_count: 0,
            frame_timing_count: 0,
            clock_sync_count: 0,
            visible_update_count: 0,
            session_metadata_count: 0,
            loss_count: 0,
            sequence_start: None,
            sequence_end: None,
            timestamp_start: None,
            timestamp_end: None,
        };

        for event in events {
            match event {
                Event::RemoteInput(_) => summary.remote_input_count += 1,
                Event::Focus(_) => summary.focus_count += 1,
                Event::InteractionMarker(_) => summary.interaction_marker_count += 1,
                Event::ReduxDispatch(_) => summary.redux_dispatch_count += 1,
                Event::Network(_) => summary.network_count += 1,
                Event::JsStall(_) => summary.js_stall_count += 1,
                Event::ReactCommit(_) => summary.react_commit_count += 1,
                Event::FrameTiming(_) => summary.frame_timing_count += 1,
                Event::ClockSync(_) => summary.clock_sync_count += 1,
                Event::VisibleUpdate(_) => summary.visible_update_count += 1,
                Event::SessionMetadata(_) => summary.session_metadata_count += 1,
            }

            let sequence = event.sequence();
            let timestamp = event.timestamp();
            summary.sequence_start =
                Some(summary.sequence_start.map_or(sequence, |s| s.min(sequence)));
            summary.sequence_end = Some(summary.sequence_end.map_or(sequence, |s| s.max(sequence)));
            summary.timestamp_start = Some(
                summary
                    .timestamp_start
                    .map_or(timestamp, |t: f64| t.min(timestamp)),
            );
            summary.timestamp_end = Some(
                summary
                    .timestamp_end
                    .map_or(timestamp, |t: f64| t.max(timestamp)),
            );
        }

        summary
    }

    /// `None` for an empty session — there's no meaningful duration to report.
    pub fn duration_ms(&self) -> Option<f64> {
        Some(self.timestamp_end? - self.timestamp_start?)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{
        FocusEvent, FrameTimingEvent, InteractionMarkerEvent, JsStallEvent, NetworkEvent,
        ReactCommitEvent, ReactCommitPhase, ReduxDispatchEvent, RemoteInputEvent,
    };

    fn sample_events() -> Vec<Event> {
        vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 4.0,
                target_id: "card-1".to_string(),
                previous_target_id: None,
            }),
            Event::InteractionMarker(InteractionMarkerEvent {
                sequence: 2,
                timestamp: 10.0,
                name: "demo:card-select".to_string(),
            }),
            Event::ReduxDispatch(ReduxDispatchEvent {
                sequence: 3,
                timestamp: 214.0,
                action_type: "catalog/itemFocused".to_string(),
                duration_ms: 4.2,
            }),
            Event::Network(NetworkEvent {
                sequence: 4,
                timestamp: 220.0,
                method: "GET".to_string(),
                url: "https://api.example.com/program/482".to_string(),
                status: 200,
                duration_ms: 42.0,
                request_bytes: None,
                response_bytes: Some(1024),
            }),
            Event::JsStall(JsStallEvent {
                sequence: 5,
                timestamp: 230.0,
                duration_ms: 58.0,
            }),
            Event::ReactCommit(ReactCommitEvent {
                sequence: 6,
                timestamp: 240.0,
                profiler_id: "CatalogRow".to_string(),
                phase: ReactCommitPhase::Update,
                actual_duration_ms: 12.5,
                base_duration_ms: 8.1,
            }),
            Event::FrameTiming(FrameTimingEvent {
                sequence: 7,
                timestamp: 250.0,
                duration_ms: 48.2,
            }),
        ]
    }

    #[test]
    fn empty_session_has_no_range_but_a_zero_count() {
        let summary = SessionSummary::from_events(&[]);

        assert_eq!(summary.event_count, 0);
        assert_eq!(summary.sequence_start, None);
        assert_eq!(summary.timestamp_start, None);
        assert_eq!(summary.duration_ms(), None);
    }

    #[test]
    fn counts_each_event_type_and_computes_the_range() {
        let summary = SessionSummary::from_events(&sample_events());

        assert_eq!(summary.event_count, 8);
        assert_eq!(summary.remote_input_count, 1);
        assert_eq!(summary.focus_count, 1);
        assert_eq!(summary.interaction_marker_count, 1);
        assert_eq!(summary.redux_dispatch_count, 1);
        assert_eq!(summary.network_count, 1);
        assert_eq!(summary.js_stall_count, 1);
        assert_eq!(summary.react_commit_count, 1);
        assert_eq!(summary.frame_timing_count, 1);
        assert_eq!(summary.visible_update_count, 0);
        assert_eq!(summary.session_metadata_count, 0);
        assert_eq!(summary.sequence_start, Some(0));
        assert_eq!(summary.sequence_end, Some(7));
        assert_eq!(summary.timestamp_start, Some(0.0));
        assert_eq!(summary.timestamp_end, Some(250.0));
        assert_eq!(summary.duration_ms(), Some(250.0));
    }

    #[test]
    fn serializes_to_json() {
        let mut summary = SessionSummary::from_events(&sample_events());
        summary.loss_count = 3;

        let json = summary.to_json().unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded["eventCount"], 8);
        assert_eq!(decoded["reduxDispatchCount"], 1);
        assert_eq!(decoded["frameTimingCount"], 1);
        assert_eq!(decoded["lossCount"], 3);
    }

    #[test]
    fn counts_visible_update_and_session_metadata_events() {
        let events = vec![
            Event::VisibleUpdate(session_telemetry_protocol::VisibleUpdateEvent {
                sequence: 0,
                timestamp: 0.0,
                target_id: "card-2".to_string(),
            }),
            Event::SessionMetadata(session_telemetry_protocol::SessionMetadataEvent {
                sequence: 1,
                timestamp: 0.0,
                device_model: "sdk_google_atv64_arm64".to_string(),
                os_version: "14".to_string(),
                app_version: None,
                build_type: None,
            }),
        ];

        let summary = SessionSummary::from_events(&events);

        assert_eq!(summary.visible_update_count, 1);
        assert_eq!(summary.session_metadata_count, 1);
    }

    #[test]
    fn loss_count_defaults_to_zero_since_it_cant_be_derived_from_events_alone() {
        let summary = SessionSummary::from_events(&sample_events());

        assert_eq!(summary.loss_count, 0);
    }
}
