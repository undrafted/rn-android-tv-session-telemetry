use session_telemetry_protocol::Event;

/// A (source_clock, reference_clock) pair captured near the same real moment. Both are
/// monotonic milliseconds from their own clock domain (e.g. the JS engine's `performance.now()`
/// vs. the Android collector's clock); neither is wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockSyncSample {
    pub source: f64,
    pub reference: f64,
}

/// Extracts clock-sync samples from a decoded session's events, in the (source, reference)
/// shape `ClockMap::from_samples` expects for mapping a QA bookmark's wall-clock reading onto
/// this session's own monotonic timeline: `source` is the event's `wallClockUnixMs` (the
/// device's own wall clock — the same domain a workstation's `session-telemetry mark` timestamp
/// is assumed to share, see `bookmark.rs`), `reference` is its `timestamp` (this session's
/// monotonic domain, same as every other event's `timestamp`).
pub fn clock_sync_samples_from_events(events: &[Event]) -> Vec<ClockSyncSample> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ClockSync(sample) => Some(ClockSyncSample {
                source: sample.wall_clock_unix_ms,
                reference: sample.timestamp,
            }),
            _ => None,
        })
        .collect()
}

/// Maps timestamps from one monotonic clock domain onto another, fitted from sync samples.
/// Two independent hardware clocks can drift relative to each other over a long QA session, so
/// this fits both an offset and a rate (`scale`), not just a constant offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockMap {
    offset: f64,
    scale: f64,
    /// Largest absolute residual across the fitted samples — a simple bound on how far a
    /// mapped timestamp could be off. Surfaced so findings can disclose clock uncertainty
    /// rather than presenting a mapped timestamp as exact.
    pub uncertainty_ms: f64,
}

impl ClockMap {
    /// `None` for zero samples — there's nothing to map from. A single sample can only anchor
    /// an offset (scale is assumed 1.0); two or more fit both via least squares.
    pub fn from_samples(samples: &[ClockSyncSample]) -> Option<ClockMap> {
        match samples.len() {
            0 => None,
            1 => Some(ClockMap {
                offset: samples[0].reference - samples[0].source,
                scale: 1.0,
                uncertainty_ms: 0.0,
            }),
            n => {
                let count = n as f64;
                let sum_x: f64 = samples.iter().map(|s| s.source).sum();
                let sum_y: f64 = samples.iter().map(|s| s.reference).sum();
                let sum_xy: f64 = samples.iter().map(|s| s.source * s.reference).sum();
                let sum_xx: f64 = samples.iter().map(|s| s.source * s.source).sum();

                // Zero variance in `source` (e.g. duplicate sync samples) makes the least
                // squares formula divide by zero; fall back to averaging offsets at scale 1
                // rather than producing NaN/infinity.
                let denominator = count * sum_xx - sum_x * sum_x;
                let (offset, scale) = if denominator.abs() < f64::EPSILON {
                    let average_offset =
                        samples.iter().map(|s| s.reference - s.source).sum::<f64>() / count;
                    (average_offset, 1.0)
                } else {
                    let scale = (count * sum_xy - sum_x * sum_y) / denominator;
                    let offset = (sum_y - scale * sum_x) / count;
                    (offset, scale)
                };

                let uncertainty_ms = samples
                    .iter()
                    .map(|s| (offset + scale * s.source - s.reference).abs())
                    .fold(0.0, f64::max);

                Some(ClockMap {
                    offset,
                    scale,
                    uncertainty_ms,
                })
            }
        }
    }

    pub fn map(&self, source_time: f64) -> f64 {
        self.offset + self.scale * source_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{ClockSyncEvent, Event, RemoteInputEvent};

    #[test]
    fn zero_samples_yields_no_map() {
        assert!(ClockMap::from_samples(&[]).is_none());
    }

    #[test]
    fn a_single_sample_anchors_a_constant_offset() {
        let map = ClockMap::from_samples(&[ClockSyncSample {
            source: 100.0,
            reference: 1_000.0,
        }])
        .unwrap();

        assert_eq!(map.map(100.0), 1_000.0);
        assert_eq!(map.map(150.0), 1_050.0);
        assert_eq!(map.uncertainty_ms, 0.0);
    }

    #[test]
    fn fits_offset_and_scale_from_a_perfectly_linear_relationship() {
        // reference = 2 * source + 500, exactly, at three points.
        let samples = [
            ClockSyncSample {
                source: 0.0,
                reference: 500.0,
            },
            ClockSyncSample {
                source: 10.0,
                reference: 520.0,
            },
            ClockSyncSample {
                source: 20.0,
                reference: 540.0,
            },
        ];

        let map = ClockMap::from_samples(&samples).unwrap();

        assert!((map.map(30.0) - 560.0).abs() < 1e-9);
        assert!(map.uncertainty_ms < 1e-9);
    }

    #[test]
    fn reports_uncertainty_when_samples_dont_fit_perfectly() {
        let samples = [
            ClockSyncSample {
                source: 0.0,
                reference: 1_000.0,
            },
            ClockSyncSample {
                source: 100.0,
                reference: 1_105.0,
            },
        ];

        let map = ClockMap::from_samples(&samples).unwrap();

        // A 2-point fit always passes through both points exactly (uncertainty ~0); real drift
        // only shows up with a third, non-collinear sample.
        let samples_with_drift = [
            samples[0],
            samples[1],
            ClockSyncSample {
                source: 50.0,
                reference: 1_040.0,
            },
        ];
        let drifted_map = ClockMap::from_samples(&samples_with_drift).unwrap();

        assert!(map.uncertainty_ms < 1e-9);
        assert!(drifted_map.uncertainty_ms > 0.0);
    }

    #[test]
    fn duplicate_source_samples_dont_panic_or_produce_nan() {
        let samples = [
            ClockSyncSample {
                source: 42.0,
                reference: 1_000.0,
            },
            ClockSyncSample {
                source: 42.0,
                reference: 1_002.0,
            },
        ];

        let map = ClockMap::from_samples(&samples).unwrap();

        assert!(map.map(42.0).is_finite());
    }

    #[test]
    fn extracts_clock_sync_samples_and_ignores_other_event_types() {
        let events = vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            }),
            Event::ClockSync(ClockSyncEvent {
                sequence: 1,
                timestamp: 50.0,
                wall_clock_unix_ms: 1_700_000_000_000.0,
            }),
        ];

        let samples = clock_sync_samples_from_events(&events);

        assert_eq!(
            samples,
            vec![ClockSyncSample {
                source: 1_700_000_000_000.0,
                reference: 50.0,
            }]
        );
    }
}
