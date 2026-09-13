use serde::Serialize;
use session_telemetry_analysis::build_resource_sampling_windows;
use session_telemetry_protocol::Event;

/// Report-facing summary of one resource-sampling window — the JSON/HTML-serializable
/// counterpart to `session_telemetry_analysis::ResourceSamplingWindow`, which borrows from the
/// event slice and isn't itself `Serialize`. `closed` distinguishes a window the app explicitly
/// stopped from one still open when the session was pulled (the session ended, or the app never
/// called `stopResourceSampling()`) — never presented as equivalent.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSamplingWindowSummary {
    pub start_sequence: u64,
    pub end_sequence: Option<u64>,
    pub interval_ms: f64,
    pub sample_count: usize,
    pub closed: bool,
    pub average_cpu_utilization_percent: Option<f64>,
    pub max_cpu_utilization_percent: Option<f64>,
    pub starting_memory_kb: Option<u64>,
    pub ending_memory_kb: Option<u64>,
}

/// One entry per resource-sampling window in the session, in the order they were opened. Empty
/// when the application never called `startResourceSampling()` — resource sampling is off by
/// default, so an empty list here means exactly that, not a measurement gap.
pub fn resource_sampling_summary(events: &[Event]) -> Vec<ResourceSamplingWindowSummary> {
    build_resource_sampling_windows(events)
        .iter()
        .map(|window| ResourceSamplingWindowSummary {
            start_sequence: window.start_sequence,
            end_sequence: window.end_sequence,
            interval_ms: window.interval_ms,
            sample_count: window.samples.len(),
            closed: window.end_sequence.is_some(),
            average_cpu_utilization_percent: window.average_cpu_utilization_percent(),
            max_cpu_utilization_percent: window.max_cpu_utilization_percent(),
            starting_memory_kb: window.starting_memory_kb(),
            ending_memory_kb: window.ending_memory_kb(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{
        ResourceSampleEvent, ResourceSamplingStartedEvent, ResourceSamplingStoppedEvent,
    };

    #[test]
    fn no_resource_sampling_events_yield_an_empty_summary() {
        assert_eq!(resource_sampling_summary(&[]), vec![]);
    }

    #[test]
    fn summarizes_a_closed_window() {
        let events = vec![
            Event::ResourceSamplingStarted(ResourceSamplingStartedEvent {
                sequence: 0,
                timestamp: 0.0,
                interval_ms: 500.0,
            }),
            Event::ResourceSample(ResourceSampleEvent {
                sequence: 1,
                timestamp: 500.0,
                cpu_utilization_percent: 40.0,
                native_heap_kb: 1000,
                java_heap_kb: 1000,
            }),
            Event::ResourceSample(ResourceSampleEvent {
                sequence: 2,
                timestamp: 1000.0,
                cpu_utilization_percent: 60.0,
                native_heap_kb: 1100,
                java_heap_kb: 1100,
            }),
            Event::ResourceSamplingStopped(ResourceSamplingStoppedEvent {
                sequence: 3,
                timestamp: 1500.0,
            }),
        ];

        let summary = resource_sampling_summary(&events);

        assert_eq!(summary.len(), 1);
        assert!(summary[0].closed);
        assert_eq!(summary[0].sample_count, 2);
        assert_eq!(summary[0].average_cpu_utilization_percent, Some(50.0));
        assert_eq!(summary[0].max_cpu_utilization_percent, Some(60.0));
        assert_eq!(summary[0].starting_memory_kb, Some(2000));
        assert_eq!(summary[0].ending_memory_kb, Some(2200));
    }

    #[test]
    fn an_unclosed_window_is_disclosed_as_not_closed() {
        let events = vec![Event::ResourceSamplingStarted(
            ResourceSamplingStartedEvent {
                sequence: 0,
                timestamp: 0.0,
                interval_ms: 500.0,
            },
        )];

        let summary = resource_sampling_summary(&events);

        assert_eq!(summary.len(), 1);
        assert!(!summary[0].closed);
        assert_eq!(summary[0].end_sequence, None);
    }
}
