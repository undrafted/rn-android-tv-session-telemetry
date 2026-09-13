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
    ClockSync(ClockSyncEvent),
    VisibleUpdate(VisibleUpdateEvent),
    SessionMetadata(SessionMetadataEvent),
    Selector(SelectorEvent),
    ResourceSamplingStarted(ResourceSamplingStartedEvent),
    ResourceSample(ResourceSampleEvent),
    ResourceSamplingStopped(ResourceSamplingStoppedEvent),
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
            Event::ClockSync(event) => event.sequence,
            Event::VisibleUpdate(event) => event.sequence,
            Event::SessionMetadata(event) => event.sequence,
            Event::Selector(event) => event.sequence,
            Event::ResourceSamplingStarted(event) => event.sequence,
            Event::ResourceSample(event) => event.sequence,
            Event::ResourceSamplingStopped(event) => event.sequence,
        }
    }

    /// Monotonic source-clock milliseconds. Never wall-clock (`ClockSyncEvent` is the one
    /// exception — see its own doc comment — but `timestamp()` here still returns its monotonic
    /// half, consistent with every other variant).
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
            Event::ClockSync(event) => event.timestamp,
            Event::VisibleUpdate(event) => event.timestamp,
            Event::SessionMetadata(event) => event.timestamp,
            Event::Selector(event) => event.timestamp,
            Event::ResourceSamplingStarted(event) => event.timestamp,
            Event::ResourceSample(event) => event.timestamp,
            Event::ResourceSamplingStopped(event) => event.timestamp,
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
    /// Optional for older chunks. Omit absent fields when reserializing so their checksums
    /// remain valid. React's elapsed render interval can contain pauses, unlike render work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_start_ms: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_time_ms: Option<f64>,
}

impl ReactCommitEvent {
    /// Renderer timestamps share the JS monotonic clock. Malformed or incomplete pairs are
    /// unavailable; do not substitute accumulated render work for elapsed time here.
    pub fn render_interval(&self) -> Option<(f64, f64)> {
        let (start, end) = (self.render_start_ms?, self.commit_time_ms?);
        (start.is_finite()
            && end.is_finite()
            && start >= 0.0
            && end >= start
            && self.timestamp.is_finite()
            && end <= self.timestamp)
            .then_some((start, end))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameTimingEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub duration_ms: f64,
}

/// A (monotonic, wall-clock) correspondence pair, emitted periodically by the JS library
/// (see `packages/react-native/src/index.ts`) so a workstation-side QA bookmark's wall-clock
/// reading (`session-telemetry mark`) can be mapped onto this session's own monotonic timeline
/// via `session_telemetry_analysis::ClockMap`. `timestamp` is the same `performance.now()`
/// domain every other event uses; `wall_clock_unix_ms` is `Date.now()` read at that same
/// instant — the one field in this protocol that legitimately is wall-clock, unlike every other
/// event's `timestamp`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockSyncEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub wall_clock_unix_ms: f64,
}

/// A best-effort confirmation that a focus change's visual result was scheduled to paint —
/// emitted by `FocusableView` (`packages/react-native/src/FocusableView.tsx`) from a
/// `requestAnimationFrame` callback right after `recordFocus`. This is the closest signal
/// available without deeper native compositor instrumentation: a scheduled JS frame callback is
/// not a guarantee the pixels were actually presented on screen, only that a render was queued
/// for the next frame. Treat `target_id` as identifying which focus change this confirms, and
/// the latency it yields as approximate, not frame-accurate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleUpdateEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub target_id: String,
}

/// Device/build/session context, emitted once from `SessionTelemetry.install()` (mirrors
/// `ClockSyncEvent`'s "one informational event in the same stream" shape, but emitted once, not
/// periodically — this describes the session itself, not a recurring measurement).
/// `device_model`/`os_version` come from React Native's own `Platform` module, already available
/// with no new native code; `app_version`/`build_type` are `None` unless the host app supplies
/// them via `InstallOptions` — the library has no way to know its host's own version or build
/// flavor on its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadataEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub device_model: String,
    pub os_version: String,
    pub app_version: Option<String>,
    pub build_type: Option<String>,
}

/// One invocation of an application-opted-in selector, from `telemetrySelector`
/// (`packages/redux/src/index.ts`) — a transparent passthrough wrapper, so this never affects
/// the selector's own memoization semantics. `inputs_changed` compares this call's arguments
/// against the previous call's by reference (`Object.is` per position); `result_changed`
/// compares the returned reference the same way. `inputs_changed == false && result_changed ==
/// true` is the interesting case: the selector was called with the exact same arguments as last
/// time but returned a different object/array reference anyway — a broken/unstable selector,
/// not a real state change. Metadata only: never the selector's arguments or its result value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub selector_id: String,
    pub duration_ms: f64,
    pub inputs_changed: bool,
    pub result_changed: bool,
}

/// Opens an explicit CPU/memory sampling window. Unlike every other automatic signal in this
/// protocol (network, JS stalls, React commits), resource sampling is never started from
/// `install()` — its overhead is high enough that the application must open a window itself
/// (`SessionTelemetry.startResourceSampling()`), the same opt-in posture as Redux middleware.
/// `interval_ms` is the configured sampling cadence for this window, disclosed so a report
/// consumer can judge how coarse the samples inside it are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSamplingStartedEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub interval_ms: f64,
}

/// One periodic sample taken while a resource-sampling window is open (between a
/// `ResourceSamplingStartedEvent` and the next `ResourceSamplingStoppedEvent`). `cpu_utilization_percent`
/// is the process's CPU time delta since the previous sample divided by the elapsed wall-clock
/// delta, expressed as a percentage of one core — it can exceed 100 on a multi-core device
/// actively using more than one thread, and must not be presented as method-level attribution.
/// `native_heap_kb`/`java_heap_kb` are the
/// managed/native breakdown `Debug.getNativeHeapAllocatedSize()`/`Runtime` heap usage give
/// without the more expensive `ActivityManager.getProcessMemoryInfo` cross-process query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSampleEvent {
    pub sequence: u64,
    pub timestamp: f64,
    pub cpu_utilization_percent: f64,
    pub native_heap_kb: u64,
    pub java_heap_kb: u64,
}

/// Closes a resource-sampling window opened by a matching `ResourceSamplingStartedEvent`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSamplingStoppedEvent {
    pub sequence: u64,
    pub timestamp: f64,
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
                render_start_ms: None,
                commit_time_ms: None,
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
    fn decodes_a_clock_sync_event_exactly_as_the_js_library_serializes_it() {
        let json = r#"{"type":"clock-sync","sequence":9,"timestamp":100.0,"wallClockUnixMs":1700000000123.0}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::ClockSync(ClockSyncEvent {
                sequence: 9,
                timestamp: 100.0,
                wall_clock_unix_ms: 1_700_000_000_123.0,
            })
        );
        assert_eq!(event.sequence(), 9);
        assert_eq!(event.timestamp(), 100.0);
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
                render_start_ms: None,
                commit_time_ms: None,
            }),
            Event::FrameTiming(FrameTimingEvent {
                sequence: 7,
                timestamp: 8.0,
                duration_ms: 48.2,
            }),
            Event::ClockSync(ClockSyncEvent {
                sequence: 8,
                timestamp: 9.0,
                wall_clock_unix_ms: 1_700_000_000_000.0,
            }),
            Event::VisibleUpdate(VisibleUpdateEvent {
                sequence: 9,
                timestamp: 10.0,
                target_id: "card-2".to_string(),
            }),
            Event::SessionMetadata(SessionMetadataEvent {
                sequence: 10,
                timestamp: 11.0,
                device_model: "sdk_google_atv64_arm64".to_string(),
                os_version: "14".to_string(),
                app_version: Some("1.2.3".to_string()),
                build_type: Some("profiling".to_string()),
            }),
            Event::Selector(SelectorEvent {
                sequence: 11,
                timestamp: 12.0,
                selector_id: "catalog/selectVisibleItemIds".to_string(),
                duration_ms: 0.8,
                inputs_changed: false,
                result_changed: true,
            }),
            Event::ResourceSamplingStarted(ResourceSamplingStartedEvent {
                sequence: 12,
                timestamp: 13.0,
                interval_ms: 500.0,
            }),
            Event::ResourceSample(ResourceSampleEvent {
                sequence: 13,
                timestamp: 14.0,
                cpu_utilization_percent: 42.5,
                native_heap_kb: 1024,
                java_heap_kb: 2048,
            }),
            Event::ResourceSamplingStopped(ResourceSamplingStoppedEvent {
                sequence: 14,
                timestamp: 15.0,
            }),
        ];

        for event in events {
            let json = serde_json::to_string(&event).unwrap();
            let decoded: Event = serde_json::from_str(&json).unwrap();
            assert_eq!(decoded, event);
        }
    }

    #[test]
    fn decodes_a_visible_update_event_exactly_as_the_js_library_serializes_it() {
        let json =
            r#"{"type":"visible-update","sequence":11,"timestamp":214.0,"targetId":"card-2"}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::VisibleUpdate(VisibleUpdateEvent {
                sequence: 11,
                timestamp: 214.0,
                target_id: "card-2".to_string(),
            })
        );
        assert_eq!(event.sequence(), 11);
        assert_eq!(event.timestamp(), 214.0);
    }

    #[test]
    fn decodes_a_session_metadata_event_with_null_app_fields() {
        let json = r#"{"type":"session-metadata","sequence":0,"timestamp":0.0,"deviceModel":"sdk_google_atv64_arm64","osVersion":"14","appVersion":null,"buildType":null}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::SessionMetadata(SessionMetadataEvent {
                sequence: 0,
                timestamp: 0.0,
                device_model: "sdk_google_atv64_arm64".to_string(),
                os_version: "14".to_string(),
                app_version: None,
                build_type: None,
            })
        );
    }

    #[test]
    fn decodes_a_selector_event_exactly_as_the_js_library_serializes_it() {
        let json = r#"{"type":"selector","sequence":3,"timestamp":50.0,"selectorId":"catalog/selectVisibleItemIds","durationMs":0.8,"inputsChanged":false,"resultChanged":true}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::Selector(SelectorEvent {
                sequence: 3,
                timestamp: 50.0,
                selector_id: "catalog/selectVisibleItemIds".to_string(),
                duration_ms: 0.8,
                inputs_changed: false,
                result_changed: true,
            })
        );
        assert_eq!(event.sequence(), 3);
        assert_eq!(event.timestamp(), 50.0);
    }

    #[test]
    fn decodes_a_resource_sample_event_exactly_as_the_js_library_serializes_it() {
        let json = r#"{"type":"resource-sample","sequence":13,"timestamp":14.0,"cpuUtilizationPercent":42.5,"nativeHeapKb":1024,"javaHeapKb":2048}"#;

        let event: Event = serde_json::from_str(json).unwrap();

        assert_eq!(
            event,
            Event::ResourceSample(ResourceSampleEvent {
                sequence: 13,
                timestamp: 14.0,
                cpu_utilization_percent: 42.5,
                native_heap_kb: 1024,
                java_heap_kb: 2048,
            })
        );
        assert_eq!(event.sequence(), 13);
        assert_eq!(event.timestamp(), 14.0);
    }

    #[test]
    fn decodes_resource_sampling_started_and_stopped_events() {
        let started_json = r#"{"type":"resource-sampling-started","sequence":12,"timestamp":13.0,"intervalMs":500.0}"#;
        let stopped_json = r#"{"type":"resource-sampling-stopped","sequence":14,"timestamp":15.0}"#;

        assert_eq!(
            serde_json::from_str::<Event>(started_json).unwrap(),
            Event::ResourceSamplingStarted(ResourceSamplingStartedEvent {
                sequence: 12,
                timestamp: 13.0,
                interval_ms: 500.0,
            })
        );
        assert_eq!(
            serde_json::from_str::<Event>(stopped_json).unwrap(),
            Event::ResourceSamplingStopped(ResourceSamplingStoppedEvent {
                sequence: 14,
                timestamp: 15.0,
            })
        );
    }

    #[test]
    fn rejects_an_unknown_type_tag() {
        let json = r#"{"type":"not-a-real-event","sequence":0,"timestamp":0}"#;

        assert!(serde_json::from_str::<Event>(json).is_err());
    }
}
