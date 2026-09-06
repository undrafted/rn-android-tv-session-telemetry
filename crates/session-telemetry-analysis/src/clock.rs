/// A (source_clock, reference_clock) pair captured near the same real moment — plan.md section
/// 8.5's "clock-synchronization samples". Both are monotonic milliseconds from their own clock
/// domain (e.g. the JS engine's `performance.now()` vs. the Android collector's clock); neither
/// is wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockSyncSample {
    pub source: f64,
    pub reference: f64,
}

/// Maps timestamps from one monotonic clock domain onto another, fitted from sync samples.
/// Architecture decision #5 in plan.md section 14 ("monotonic clock synchronization model")
/// isn't formally written up yet; this is a first real, linear-fit implementation of it, not a
/// placeholder — two independent hardware clocks can drift relative to each other over a long
/// QA session, so this fits both an offset and a rate (`scale`), not just a constant offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockMap {
    offset: f64,
    scale: f64,
    /// Largest absolute residual across the fitted samples — a simple bound on how far a
    /// mapped timestamp could be off. Surfaced so findings can disclose clock uncertainty
    /// rather than presenting a mapped timestamp as exact (plan.md section 7).
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
}
