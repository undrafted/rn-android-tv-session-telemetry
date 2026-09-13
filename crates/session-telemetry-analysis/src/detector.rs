use crate::interaction::InteractionWindow;
use session_telemetry_protocol::Event;
use std::collections::HashMap;

pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR: &str = "high-latency-focus-change";
pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION: u32 = 1;

pub const REPEATED_REDUX_DISPATCH_DETECTOR: &str = "repeated-redux-dispatch";
pub const REPEATED_REDUX_DISPATCH_DETECTOR_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Critical,
}

/// A finding: which detector produced it, at what version, how severe, the exact sequence
/// range it covers, and the measured value. Capability requirements and loss/uncertainty
/// disclosure aren't included yet — nothing populates that data anywhere in the workspace yet
/// either.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub detector: &'static str,
    pub detector_version: u32,
    pub severity: Severity,
    pub sequence_start: u64,
    pub sequence_end: u64,
    /// The measurement this finding is based on — a latency in ms, a repeat count, etc.
    /// Generic rather than e.g. `latency_ms` because not every detector measures a duration.
    pub value: f64,
    pub unit: &'static str,
}

/// Flags interactions whose input-to-focus-change latency crosses a threshold, scoped to
/// focus-change latency for now (see `interaction.rs` for why). The 100ms/300ms thresholds are
/// placeholders pending real device measurements, not values anyone has actually measured.
pub fn detect_high_latency_focus_changes(windows: &[InteractionWindow]) -> Vec<Finding> {
    const WARNING_THRESHOLD_MS: f64 = 100.0;
    const CRITICAL_THRESHOLD_MS: f64 = 300.0;

    windows
        .iter()
        .filter_map(|window| {
            let latency_ms = window.latency_ms()?;
            let severity = if latency_ms >= CRITICAL_THRESHOLD_MS {
                Severity::Critical
            } else if latency_ms >= WARNING_THRESHOLD_MS {
                Severity::Warning
            } else {
                return None;
            };

            let focus = window.focus.as_ref().expect("latency_ms() returned Some");
            Some(Finding {
                detector: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
                detector_version: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION,
                severity,
                sequence_start: window.input.sequence,
                sequence_end: focus.sequence,
                value: latency_ms,
                unit: "ms",
            })
        })
        .collect()
}

/// Flags interaction windows where the same Redux action type was dispatched more than once. A
/// repeat threshold of 2 (i.e. any duplicate) is a starting guess, not a measured number, same
/// caveat as the latency thresholds above.
pub fn detect_repeated_redux_dispatches(windows: &[InteractionWindow]) -> Vec<Finding> {
    const REPEAT_THRESHOLD: usize = 2;

    let mut findings = Vec::new();

    for window in windows {
        let mut by_action_type: HashMap<&str, Vec<u64>> = HashMap::new();
        for event in &window.other_events {
            if let Event::ReduxDispatch(dispatch) = event {
                by_action_type
                    .entry(dispatch.action_type.as_str())
                    .or_default()
                    .push(dispatch.sequence);
            }
        }

        for sequences in by_action_type.into_values() {
            if sequences.len() < REPEAT_THRESHOLD {
                continue;
            }
            findings.push(Finding {
                detector: REPEATED_REDUX_DISPATCH_DETECTOR,
                detector_version: REPEATED_REDUX_DISPATCH_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start: window.input.sequence,
                sequence_end: *sequences.iter().max().expect("non-empty"),
                value: sequences.len() as f64,
                unit: "dispatches",
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{FocusEvent, RemoteInputEvent};

    fn window_with_latency(latency_ms: f64) -> InteractionWindow {
        InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: Some(FocusEvent {
                sequence: 1,
                timestamp: latency_ms,
                target_id: "card-2".to_string(),
                previous_target_id: None,
            }),
            other_events: Vec::new(),
        }
    }

    #[test]
    fn below_the_warning_threshold_produces_no_finding() {
        assert_eq!(
            detect_high_latency_focus_changes(&[window_with_latency(50.0)]),
            vec![]
        );
    }

    #[test]
    fn between_the_thresholds_is_a_warning() {
        let findings = detect_high_latency_focus_changes(&[window_with_latency(150.0)]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].value, 150.0);
        assert_eq!(findings[0].unit, "ms");
    }

    #[test]
    fn at_or_above_the_critical_threshold_is_critical() {
        let findings = detect_high_latency_focus_changes(&[window_with_latency(300.0)]);

        assert_eq!(findings[0].severity, Severity::Critical);
    }

    #[test]
    fn a_window_with_no_focus_change_produces_no_finding() {
        let window = InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: None,
            other_events: Vec::new(),
        };

        assert_eq!(detect_high_latency_focus_changes(&[window]), vec![]);
    }

    fn dispatch(sequence: u64, action_type: &str) -> Event {
        Event::ReduxDispatch(session_telemetry_protocol::ReduxDispatchEvent {
            sequence,
            timestamp: sequence as f64,
            action_type: action_type.to_string(),
            duration_ms: 1.0,
        })
    }

    #[test]
    fn a_single_dispatch_is_not_repeated() {
        let window = InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: None,
            other_events: vec![dispatch(1, "catalog/itemFocused")],
        };

        assert_eq!(detect_repeated_redux_dispatches(&[window]), vec![]);
    }

    #[test]
    fn the_same_action_type_dispatched_twice_is_flagged() {
        let window = InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: None,
            other_events: vec![
                dispatch(1, "catalog/itemFocused"),
                dispatch(2, "catalog/itemFocused"),
            ],
        };

        let findings = detect_repeated_redux_dispatches(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, REPEATED_REDUX_DISPATCH_DETECTOR);
        assert_eq!(findings[0].sequence_start, 0);
        assert_eq!(findings[0].sequence_end, 2);
        assert_eq!(findings[0].value, 2.0);
        assert_eq!(findings[0].unit, "dispatches");
    }

    #[test]
    fn different_action_types_are_not_conflated() {
        let window = InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: None,
            other_events: vec![
                dispatch(1, "catalog/itemFocused"),
                dispatch(2, "nav/moveRight"),
            ],
        };

        assert_eq!(detect_repeated_redux_dispatches(&[window]), vec![]);
    }
}
