use crate::interaction::InteractionWindow;

pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR: &str = "high-latency-focus-change";
pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Critical,
}

/// A subset of the finding shape plan.md section 7 describes ("Every finding contains: a
/// stable finding ID, detector and version, severity, capability requirements, exact time
/// range, referenced event IDs, recorded values with units, uncertainty and event-loss
/// disclosure, neutral wording"). Capability requirements and loss/uncertainty disclosure
/// aren't included yet — nothing populates that data anywhere in the workspace yet either.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub detector: &'static str,
    pub detector_version: u32,
    pub severity: Severity,
    pub sequence_start: u64,
    pub sequence_end: u64,
    pub latency_ms: f64,
}

/// Flags interactions whose input-to-focus-change latency crosses a threshold — the first of
/// plan.md section 7's "Initial detectors" ("remote interaction with high visible-response
/// latency"), scoped to focus-change latency for now (see `interaction.rs` for why). The
/// 100ms/300ms thresholds are placeholders pending real device measurements, not values anyone
/// has actually measured — plan.md explicitly calls "documented thresholds" out as something
/// severity should be based on, and these aren't that yet.
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
                latency_ms,
            })
        })
        .collect()
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
        assert_eq!(findings[0].latency_ms, 150.0);
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
}
