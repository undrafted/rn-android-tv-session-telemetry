use serde::Serialize;
use session_telemetry_protocol::Event;

/// The report's entry point — success criterion #13 in plan.md section 4 requires the report
/// to open with a session summary rather than rendering every event of a long session at once.
/// This is that summary's data, independent of whichever renderer (JSON today, HTML later)
/// presents it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub event_count: usize,
    pub remote_input_count: usize,
    pub focus_count: usize,
    pub interaction_marker_count: usize,
    pub redux_dispatch_count: usize,
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
        FocusEvent, InteractionMarkerEvent, ReduxDispatchEvent, RemoteInputEvent,
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

        assert_eq!(summary.event_count, 4);
        assert_eq!(summary.remote_input_count, 1);
        assert_eq!(summary.focus_count, 1);
        assert_eq!(summary.interaction_marker_count, 1);
        assert_eq!(summary.redux_dispatch_count, 1);
        assert_eq!(summary.sequence_start, Some(0));
        assert_eq!(summary.sequence_end, Some(3));
        assert_eq!(summary.timestamp_start, Some(0.0));
        assert_eq!(summary.timestamp_end, Some(214.0));
        assert_eq!(summary.duration_ms(), Some(214.0));
    }

    #[test]
    fn serializes_to_json() {
        let summary = SessionSummary::from_events(&sample_events());

        let json = summary.to_json().unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded["eventCount"], 4);
        assert_eq!(decoded["reduxDispatchCount"], 1);
    }
}
