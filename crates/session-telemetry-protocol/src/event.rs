use serde::{Deserialize, Serialize};

/// Mirrors `packages/react-native/src/events.ts` — this is the wire/on-disk shape events
/// produced by the JS library must decode into. Field names use `camelCase` (via `serde`
/// rename) and the `type` tag uses `kebab-case` specifically to match that JS source, not
/// Rust convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    RemoteInput(RemoteInputEvent),
    Focus(FocusEvent),
    InteractionMarker(InteractionMarkerEvent),
    ReduxDispatch(ReduxDispatchEvent),
    Network(NetworkEvent),
    JsStall(JsStallEvent),
    ReactCommit(ReactCommitEvent),
    FrameTiming(FrameTimingEvent),
}

impl Event {
    /// Monotonic per-session ordering, assigned by the JS/native recorder. Chunk decoding and
    /// interaction-window construction (session-telemetry-session/-analysis) sort and index on
    /// this, not on `timestamp`.
    pub fn sequence(&self) -> u64 {
        match self {
            Event::RemoteInput(event) => event.sequence,
            Event::Focus(event) => event.sequence,
            Event::InteractionMarker(event) => event.sequence,
            Event::ReduxDispatch(event) => event.sequence,
            Event::Network(event) => event.sequence,
            Event::JsStall(event) => event.sequence,
            Event::ReactCommit(event) => event.sequence,
            Event::FrameTiming(event) => event.sequence,
        }
    }

    /// Monotonic source-clock milliseconds. Never wall-clock.
    pub fn timestamp(&self) -> f64 {
        match self {
            Event::RemoteInput(event) => event.timestamp,
            Event::Focus(event) => event.timestamp,
            Event::InteractionMarker(event) => event.timestamp,
            Event::ReduxDispatch(event) => event.timestamp,
            Event::Network(event) => event.timestamp,
            Event::JsStall(event) => event.timestamp,
            Event::ReactCommit(event) => event.timestamp,
            Event::FrameTiming(event) => event.timestamp,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RemoteInputEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub target_id: String,
    pub previous_target_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractionMarkerEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReduxDispatchEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub action_type: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub duration_ms: f64,
    pub request_bytes: Option<u64>,
    pub response_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsStallEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReactCommitPhase {
    Mount,
    Update,
    NestedUpdate,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactCommitEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub profiler_id: String,
    pub phase: ReactCommitPhase,
    pub actual_duration_ms: f64,
    pub base_duration_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTimingEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub duration_ms: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_focus_event_exactly_as_the_js_library_serializes_it() {
        let json = r#"{"type":"focus","sequence":2,"timestamp":12.5,"targetId":"card-1","previousTargetId":null}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::Focus(FocusEvent {
                sequence: 2,
                timestamp: 12.5,
                target_id: "card-1".to_string(),
                previous_target_id: None,
            })
        );
        assert_eq!(event.sequence(), 2);
        assert_eq!(event.timestamp(), 12.5);
    }

    #[test]
    fn decodes_a_react_commit_event_exactly_as_the_js_library_serializes_it() {
        let json = r#"{"type":"react-commit","sequence":6,"timestamp":7.0,"profilerId":"CatalogRow","phase":"nested-update","actualDurationMs":12.5,"baseDurationMs":8.1}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::ReactCommit(ReactCommitEvent {
                sequence: 6,
                timestamp: 7.0,
                profiler_id: "CatalogRow".to_string(),
                phase: ReactCommitPhase::NestedUpdate,
                actual_duration_ms: 12.5,
                base_duration_ms: 8.1,
            })
        );
    }

    #[test]
    fn decodes_a_network_event_with_null_byte_counts() {
        let json = r#"{"type":"network","sequence":4,"timestamp":5.0,"method":"GET","url":"https://api.example.com/program/482","status":200,"durationMs":42.0,"requestBytes":null,"responseBytes":null}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::Network(NetworkEvent {
                sequence: 4,
                timestamp: 5.0,
                method: "GET".to_string(),
                url: "https://api.example.com/program/482".to_string(),
                status: 200,
                duration_ms: 42.0,
                request_bytes: None,
                response_bytes: None,
            })
        );
    }

    #[test]
    fn round_trips_every_variant_through_json() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 1.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 2.0,
                target_id: "card-1".to_string(),
                previous_target_id: Some("card-0".to_string()),
            }),
            Event::InteractionMarker(InteractionMarkerEvent {
                sequence: 2,
                timestamp: 3.0,
                name: "demo:card-select".to_string(),
            }),
            Event::ReduxDispatch(ReduxDispatchEvent {
                sequence: 3,
                timestamp: 4.0,
                action_type: "catalog/itemFocused".to_string(),
                duration_ms: 4.2,
            }),
            Event::Network(NetworkEvent {
                sequence: 4,
                timestamp: 5.0,
                method: "GET".to_string(),
                url: "https://api.example.com/program/482".to_string(),
                status: 200,
                duration_ms: 42.0,
                request_bytes: None,
                response_bytes: Some(1024),
            }),
            Event::JsStall(JsStallEvent {
                sequence: 5,
                timestamp: 6.0,
                duration_ms: 58.0,
            }),
            Event::ReactCommit(ReactCommitEvent {
                sequence: 6,
                timestamp: 7.0,
                profiler_id: "CatalogRow".to_string(),
                phase: ReactCommitPhase::NestedUpdate,
                actual_duration_ms: 12.5,
                base_duration_ms: 8.1,
            }),
            Event::FrameTiming(FrameTimingEvent {
                sequence: 7,
                timestamp: 8.0,
                duration_ms: 48.2,
            }),
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let decoded: Event = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, event);
        }
    }

    #[test]
    fn rejects_an_unknown_type_tag() {
        let json = r#"{"type":"not-a-real-event","sequence":0,"timestamp":0}"#;

        assert!(serde_json::from_str::<Event>(json).is_err());
    }
}
