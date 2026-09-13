use session_telemetry_protocol::{Event, ResourceSampleEvent};

/// One explicit CPU/memory sampling window: everything between a `ResourceSamplingStartedEvent`
/// and its matching `ResourceSamplingStoppedEvent` (or the end of the session, if the app never
/// closed it). Unlike `InteractionWindow`, this is not anchored to remote input — the app opens
/// this window around whatever it decides is worth measuring (app startup, a stream/playback
/// request), which commonly has no remote input inside it at all. Building windows this way
/// instead of reusing `build_interaction_windows` is deliberate: a resource-sampling window's
/// boundary is an explicit app decision, not inferred from input timing.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceSamplingWindow<'a> {
    pub start_sequence: u64,
    pub start_timestamp: f64,
    pub interval_ms: f64,
    /// `None` if the session ended (or was pulled) before the app called `stopResourceSampling()`.
    pub end_sequence: Option<u64>,
    pub end_timestamp: Option<f64>,
    pub samples: Vec<&'a ResourceSampleEvent>,
}

impl<'a> ResourceSamplingWindow<'a> {
    pub fn average_cpu_utilization_percent(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let total: f64 = self
            .samples
            .iter()
            .map(|sample| sample.cpu_utilization_percent)
            .sum();
        Some(total / self.samples.len() as f64)
    }

    pub fn max_cpu_utilization_percent(&self) -> Option<f64> {
        self.samples
            .iter()
            .map(|sample| sample.cpu_utilization_percent)
            .fold(None, |max, value| {
                Some(max.map_or(value, |m: f64| m.max(value)))
            })
    }

    /// Combined native+JS heap at the window's first sample, or `None` if it has no samples.
    pub fn starting_memory_kb(&self) -> Option<u64> {
        self.samples
            .first()
            .map(|sample| sample.native_heap_kb + sample.java_heap_kb)
    }

    /// Combined native+JS heap at the window's last sample, or `None` if it has no samples.
    pub fn ending_memory_kb(&self) -> Option<u64> {
        self.samples
            .last()
            .map(|sample| sample.native_heap_kb + sample.java_heap_kb)
    }
}

/// Groups the session's resource-sampling events into windows, in the order they were opened.
/// A `ResourceSample` outside any open window (a malformed/truncated capture) is dropped rather
/// than attributed to the wrong window — same "don't guess" discipline as
/// `ReactCommitEvent::render_interval`.
pub fn build_resource_sampling_windows(events: &[Event]) -> Vec<ResourceSamplingWindow<'_>> {
    let mut sorted: Vec<&Event> = events.iter().collect();
    sorted.sort_by_key(|event| event.sequence());

    let mut windows = Vec::new();
    let mut current: Option<ResourceSamplingWindow> = None;

    for event in sorted {
        match event {
            Event::ResourceSamplingStarted(started) => {
                if let Some(window) = current.take() {
                    windows.push(window);
                }
                current = Some(ResourceSamplingWindow {
                    start_sequence: started.sequence,
                    start_timestamp: started.timestamp,
                    interval_ms: started.interval_ms,
                    end_sequence: None,
                    end_timestamp: None,
                    samples: Vec::new(),
                });
            }
            Event::ResourceSample(sample) => {
                if let Some(window) = current.as_mut() {
                    window.samples.push(sample);
                }
            }
            Event::ResourceSamplingStopped(stopped) => {
                if let Some(mut window) = current.take() {
                    window.end_sequence = Some(stopped.sequence);
                    window.end_timestamp = Some(stopped.timestamp);
                    windows.push(window);
                }
            }
            _ => {}
        }
    }

    if let Some(window) = current.take() {
        windows.push(window);
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{ResourceSamplingStartedEvent, ResourceSamplingStoppedEvent};

    fn started(sequence: u64, timestamp: f64, interval_ms: f64) -> Event {
        Event::ResourceSamplingStarted(ResourceSamplingStartedEvent {
            sequence,
            timestamp,
            interval_ms,
        })
    }

    fn sample(sequence: u64, timestamp: f64, cpu: f64, native_kb: u64, java_kb: u64) -> Event {
        Event::ResourceSample(ResourceSampleEvent {
            sequence,
            timestamp,
            cpu_utilization_percent: cpu,
            native_heap_kb: native_kb,
            java_heap_kb: java_kb,
        })
    }

    fn stopped(sequence: u64, timestamp: f64) -> Event {
        Event::ResourceSamplingStopped(ResourceSamplingStoppedEvent {
            sequence,
            timestamp,
        })
    }

    #[test]
    fn no_resource_sampling_events_yield_no_windows() {
        let events = vec![Event::RemoteInput(
            session_telemetry_protocol::RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
        )];

        assert_eq!(build_resource_sampling_windows(&events), vec![]);
    }

    #[test]
    fn a_closed_window_collects_its_samples_and_bounds() {
        let events = vec![
            started(0, 0.0, 500.0),
            sample(1, 500.0, 20.0, 1000, 2000),
            sample(2, 1000.0, 80.0, 1200, 2100),
            stopped(3, 1500.0),
        ];

        let windows = build_resource_sampling_windows(&events);

        assert_eq!(windows.len(), 1);
        let window = &windows[0];
        assert_eq!(window.start_sequence, 0);
        assert_eq!(window.end_sequence, Some(3));
        assert_eq!(window.samples.len(), 2);
        assert_eq!(window.average_cpu_utilization_percent(), Some(50.0));
        assert_eq!(window.max_cpu_utilization_percent(), Some(80.0));
        assert_eq!(window.starting_memory_kb(), Some(3000));
        assert_eq!(window.ending_memory_kb(), Some(3300));
    }

    #[test]
    fn an_unclosed_window_at_session_end_is_still_returned() {
        let events = vec![started(0, 0.0, 500.0), sample(1, 500.0, 10.0, 500, 500)];

        let windows = build_resource_sampling_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].end_sequence, None);
        assert_eq!(windows[0].samples.len(), 1);
    }

    #[test]
    fn multiple_windows_in_one_session_stay_separate_and_ordered() {
        let events = vec![
            started(0, 0.0, 500.0),
            sample(1, 500.0, 10.0, 1000, 1000),
            stopped(2, 1000.0),
            started(3, 5000.0, 500.0),
            sample(4, 5500.0, 90.0, 2000, 2000),
            stopped(5, 6000.0),
        ];

        let windows = build_resource_sampling_windows(&events);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].start_sequence, 0);
        assert_eq!(windows[1].start_sequence, 3);
        assert_eq!(windows[1].average_cpu_utilization_percent(), Some(90.0));
    }

    #[test]
    fn samples_outside_any_window_are_dropped_not_misattributed() {
        let events = vec![sample(0, 0.0, 10.0, 1000, 1000), started(1, 1.0, 500.0)];

        let windows = build_resource_sampling_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].samples.len(), 0);
    }

    #[test]
    fn a_new_started_event_without_a_stop_closes_the_previous_window() {
        // Defensive against a malformed/truncated capture missing a stop event - a second
        // start shouldn't silently merge two windows' samples together.
        let events = vec![
            started(0, 0.0, 500.0),
            sample(1, 500.0, 10.0, 1000, 1000),
            started(2, 1000.0, 500.0),
            sample(3, 1500.0, 90.0, 2000, 2000),
        ];

        let windows = build_resource_sampling_windows(&events);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].samples.len(), 1);
        assert_eq!(windows[0].end_sequence, None);
        assert_eq!(windows[1].samples.len(), 1);
    }
}
