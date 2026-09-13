use crate::interaction::InteractionWindow;
use serde::Serialize;
use session_telemetry_protocol::Event;
use std::collections::HashMap;

pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR: &str = "high-latency-focus-change";
pub const HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION: u32 = 1;

pub const HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR: &str = "high-latency-visible-update";
pub const HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR_VERSION: u32 = 1;

pub const REPEATED_REDUX_DISPATCH_DETECTOR: &str = "repeated-redux-dispatch";
pub const REPEATED_REDUX_DISPATCH_DETECTOR_VERSION: u32 = 1;

pub const JS_STALL_DURING_INTERACTION_DETECTOR: &str = "js-stall-during-interaction";
pub const JS_STALL_DURING_INTERACTION_DETECTOR_VERSION: u32 = 1;

pub const REPEATED_NETWORK_REQUEST_DETECTOR: &str = "repeated-network-request";
pub const REPEATED_NETWORK_REQUEST_DETECTOR_VERSION: u32 = 1;

pub const REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR: &str =
    "react-commit-overlapping-delayed-frame";
pub const REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR_VERSION: u32 = 1;

pub const NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR: &str =
    "network-completion-followed-by-commit";
pub const NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR_VERSION: u32 = 1;

pub const EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR: &str =
    "excessive-commits-during-rapid-focus-movement";
pub const EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR_VERSION: u32 = 1;

pub const REPEATED_SELECTOR_RECOMPUTATION_DETECTOR: &str = "repeated-selector-recomputation";
pub const REPEATED_SELECTOR_RECOMPUTATION_DETECTOR_VERSION: u32 = 1;

pub const UNSTABLE_SELECTOR_REFERENCE_DETECTOR: &str = "unstable-selector-reference";
pub const UNSTABLE_SELECTOR_REFERENCE_DETECTOR_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    Critical,
}

/// One threshold value a finding was evaluated against, named so a JSON consumer can tell which
/// threshold it is without depending on detector-specific knowledge (e.g. `"criticalMs"`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Threshold {
    pub name: &'static str,
    pub value: f64,
}

/// A finding: which detector produced it, at what version, how severe, the exact sequence
/// range it covers, the measured value, the threshold(s) it crossed, and a plain-language
/// summary. Loss/uncertainty disclosure lives at the report level
/// (`session_telemetry_report::Report::clock_uncertainty_ms`, `SessionSummary::loss_count`), not
/// per finding, and likewise capability context comes from `SessionSummary`'s per-event-type
/// counts rather than a field here — a consumer can already see which signals produced zero
/// events without this struct duplicating that.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable across re-analysis of the same session: `"{detector}-{sequence_start}-{sequence_end}"`.
    pub id: String,
    pub detector: &'static str,
    pub detector_version: u32,
    pub severity: Severity,
    pub sequence_start: u64,
    pub sequence_end: u64,
    /// The measurement this finding is based on — a latency in ms, a repeat count, etc.
    /// Generic rather than e.g. `latency_ms` because not every detector measures a duration.
    pub value: f64,
    pub unit: &'static str,
    pub thresholds: Vec<Threshold>,
    pub summary: String,
}

fn finding_id(detector: &'static str, sequence_start: u64, sequence_end: u64) -> String {
    format!("{detector}-{sequence_start}-{sequence_end}")
}

/// Flags interactions whose input-to-focus-change latency crosses a threshold — an earlier,
/// intermediate signal than `detect_high_latency_visible_updates` below (see `interaction.rs`'s
/// doc comment for why the two are kept separate rather than one replacing the other). The
/// 100ms/300ms thresholds are placeholders pending real device measurements, not values anyone
/// has actually measured.
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
            let sequence_start = window.input.sequence;
            let sequence_end = focus.sequence;
            Some(Finding {
                id: finding_id(
                    HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
                detector_version: HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION,
                severity,
                sequence_start,
                sequence_end,
                value: latency_ms,
                unit: "ms",
                thresholds: vec![
                    Threshold {
                        name: "warningMs",
                        value: WARNING_THRESHOLD_MS,
                    },
                    Threshold {
                        name: "criticalMs",
                        value: CRITICAL_THRESHOLD_MS,
                    },
                ],
                summary: format!("Input-to-focus-change latency was {latency_ms:.0}ms."),
            })
        })
        .collect()
}

/// Flags interactions whose input-to-visible-update latency crosses a threshold. This metric
/// is more meaningful than focus-change latency alone because it reflects when the user
/// actually saw something change, not just when application state settled. Relies on
/// `VisibleUpdateEvent`, itself a best-effort proxy (see its own doc comment) — this detector
/// inherits that same uncertainty, not a frame-accurate guarantee. The 100ms/300ms thresholds
/// are placeholders pending real device measurements, same caveat as every other detector's
/// thresholds.
pub fn detect_high_latency_visible_updates(windows: &[InteractionWindow]) -> Vec<Finding> {
    const WARNING_THRESHOLD_MS: f64 = 100.0;
    const CRITICAL_THRESHOLD_MS: f64 = 300.0;

    windows
        .iter()
        .filter_map(|window| {
            let latency_ms = window.visible_update_latency_ms()?;
            let severity = if latency_ms >= CRITICAL_THRESHOLD_MS {
                Severity::Critical
            } else if latency_ms >= WARNING_THRESHOLD_MS {
                Severity::Warning
            } else {
                return None;
            };

            let visible_update = window
                .visible_update
                .as_ref()
                .expect("visible_update_latency_ms() returned Some");
            let sequence_start = window.input.sequence;
            let sequence_end = visible_update.sequence;
            Some(Finding {
                id: finding_id(
                    HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR,
                detector_version: HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR_VERSION,
                severity,
                sequence_start,
                sequence_end,
                value: latency_ms,
                unit: "ms",
                thresholds: vec![
                    Threshold {
                        name: "warningMs",
                        value: WARNING_THRESHOLD_MS,
                    },
                    Threshold {
                        name: "criticalMs",
                        value: CRITICAL_THRESHOLD_MS,
                    },
                ],
                summary: format!("Input-to-visible-update latency was {latency_ms:.0}ms."),
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
            let sequence_start = window.input.sequence;
            let sequence_end = *sequences.iter().max().expect("non-empty");
            let count = sequences.len();
            findings.push(Finding {
                id: finding_id(REPEATED_REDUX_DISPATCH_DETECTOR, sequence_start, sequence_end),
                detector: REPEATED_REDUX_DISPATCH_DETECTOR,
                detector_version: REPEATED_REDUX_DISPATCH_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start,
                sequence_end,
                value: count as f64,
                unit: "dispatches",
                thresholds: vec![Threshold {
                    name: "repeatThreshold",
                    value: REPEAT_THRESHOLD as f64,
                }],
                summary: format!(
                    "The same Redux action type was dispatched {count} times within one interaction."
                ),
            });
        }
    }

    findings
}

/// Flags interaction windows where a JS event-loop stall was recorded. `other_events` only ever
/// contains events that fell inside this window's time range (see `build_interaction_windows`),
/// so a stall's presence here already means it overlapped the interaction — no separate
/// interval-overlap check is needed. Any captured stall is at least a Warning: the JS-side
/// monitor (`startStallMonitor`) already filters out normal timer jitter before it ever records
/// one, so by the time a `JsStallEvent` exists here it's already a real block, not noise.
pub fn detect_js_stalls_overlapping_interactions(windows: &[InteractionWindow]) -> Vec<Finding> {
    const CRITICAL_THRESHOLD_MS: f64 = 150.0;

    let mut findings = Vec::new();

    for window in windows {
        for event in &window.other_events {
            let Event::JsStall(stall) = event else {
                continue;
            };
            let severity = if stall.duration_ms >= CRITICAL_THRESHOLD_MS {
                Severity::Critical
            } else {
                Severity::Warning
            };
            let sequence_start = window.input.sequence;
            let sequence_end = stall.sequence;
            findings.push(Finding {
                id: finding_id(
                    JS_STALL_DURING_INTERACTION_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: JS_STALL_DURING_INTERACTION_DETECTOR,
                detector_version: JS_STALL_DURING_INTERACTION_DETECTOR_VERSION,
                severity,
                sequence_start,
                sequence_end,
                value: stall.duration_ms,
                unit: "ms",
                thresholds: vec![Threshold {
                    name: "criticalMs",
                    value: CRITICAL_THRESHOLD_MS,
                }],
                summary: format!(
                    "A {:.0}ms JavaScript stall overlapped this interaction.",
                    stall.duration_ms
                ),
            });
        }
    }

    findings
}

/// Flags interaction windows where an equivalent request (same method and normalized URL — see
/// `normalizeUrl` on the JS side) was made more than once. Excludes long-running requests
/// (`duration_ms >= STREAM_DURATION_THRESHOLD_MS`) from grouping: a persistent connection (SSE,
/// a chunked/streaming response) legitimately keeps reporting under the same URL for its whole
/// lifetime, and that isn't the "app fired the same request twice by mistake" pattern this
/// detector targets. A stream reconnecting rapidly and repeatedly — each attempt short-lived —
/// still gets flagged, which is correct: that's a real symptom, not expected streaming
/// behavior. There's no explicit stream/one-off marker in the protocol yet, so this is a
/// duration heuristic, not a certain classification — same caveat as every other detector's
/// thresholds, and the repeat-threshold itself matches `detect_repeated_redux_dispatches`.
pub fn detect_repeated_network_requests(windows: &[InteractionWindow]) -> Vec<Finding> {
    const REPEAT_THRESHOLD: usize = 2;
    const STREAM_DURATION_THRESHOLD_MS: f64 = 2_000.0;

    let mut findings = Vec::new();

    for window in windows {
        let mut by_request: HashMap<(&str, &str), Vec<u64>> = HashMap::new();
        for event in &window.other_events {
            if let Event::Network(request) = event {
                if request.duration_ms >= STREAM_DURATION_THRESHOLD_MS {
                    continue;
                }
                by_request
                    .entry((request.method.as_str(), request.url.as_str()))
                    .or_default()
                    .push(request.sequence);
            }
        }

        for sequences in by_request.into_values() {
            if sequences.len() < REPEAT_THRESHOLD {
                continue;
            }
            let sequence_start = window.input.sequence;
            let sequence_end = *sequences.iter().max().expect("non-empty");
            let count = sequences.len();
            findings.push(Finding {
                id: finding_id(
                    REPEATED_NETWORK_REQUEST_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: REPEATED_NETWORK_REQUEST_DETECTOR,
                detector_version: REPEATED_NETWORK_REQUEST_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start,
                sequence_end,
                value: count as f64,
                unit: "requests",
                thresholds: vec![
                    Threshold {
                        name: "repeatThreshold",
                        value: REPEAT_THRESHOLD as f64,
                    },
                    Threshold {
                        name: "streamDurationMs",
                        value: STREAM_DURATION_THRESHOLD_MS,
                    },
                ],
                summary: format!(
                    "The same network request was made {count} times within one interaction."
                ),
            });
        }
    }

    findings
}

/// Flags a React commit whose `[timestamp - actualDurationMs, timestamp]` span overlaps a
/// delayed frame's `[timestamp - durationMs, timestamp]` span within the same interaction
/// window — a commit that's plausibly the actual cause of that frame running long, not just
/// coincidentally nearby in time.
pub fn detect_react_commits_overlapping_delayed_frames(
    windows: &[InteractionWindow],
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for window in windows {
        let commits = window.other_events.iter().filter_map(|event| match event {
            Event::ReactCommit(commit) => Some(commit),
            _ => None,
        });

        for commit in commits {
            let commit_start = commit.timestamp - commit.actual_duration_ms;
            let commit_end = commit.timestamp;

            let frames = window.other_events.iter().filter_map(|event| match event {
                Event::FrameTiming(frame) => Some(frame),
                _ => None,
            });

            for frame in frames {
                let frame_start = frame.timestamp - frame.duration_ms;
                let frame_end = frame.timestamp;
                if commit_start > frame_end || frame_start > commit_end {
                    continue;
                }

                let sequence_start = commit.sequence.min(frame.sequence);
                let sequence_end = commit.sequence.max(frame.sequence);
                findings.push(Finding {
                    id: finding_id(
                        REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR,
                        sequence_start,
                        sequence_end,
                    ),
                    detector: REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR,
                    detector_version: REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR_VERSION,
                    severity: Severity::Warning,
                    sequence_start,
                    sequence_end,
                    value: frame.duration_ms,
                    unit: "ms",
                    thresholds: Vec::new(),
                    summary: format!(
                        "A React commit overlapped a delayed frame lasting {:.0}ms.",
                        frame.duration_ms
                    ),
                });
            }
        }
    }

    findings
}

/// Flags a network request immediately followed (the very next event recorded in this
/// interaction window) by a React commit — synchronous JS work reacting to the response and
/// triggering a re-render, rather than an unrelated commit that just happened to land somewhere
/// later in the same window.
pub fn detect_network_completions_followed_by_commits(
    windows: &[InteractionWindow],
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for window in windows {
        for (index, event) in window.other_events.iter().enumerate() {
            let Event::Network(request) = event else {
                continue;
            };
            let Some(Event::ReactCommit(commit)) = window.other_events.get(index + 1) else {
                continue;
            };

            let sequence_start = request.sequence;
            let sequence_end = commit.sequence;
            let delay_ms = commit.timestamp - request.timestamp;
            findings.push(Finding {
                id: finding_id(
                    NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR,
                detector_version: NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start,
                sequence_end,
                value: delay_ms,
                unit: "ms",
                thresholds: Vec::new(),
                summary: format!(
                    "A network completion was immediately followed by a React commit {delay_ms:.0}ms later."
                ),
            });
        }
    }

    findings
}

/// Flags a burst of React commits produced while the user moved focus rapidly — consecutive
/// remote inputs less than `RAPID_GAP_MS` apart, grouped into one burst regardless of how many
/// inputs it spans. A common Android TV performance problem: a naive list/grid re-renders far
/// more than the user's actual input rate justifies. `RAPID_GAP_MS`/`COMMIT_THRESHOLD` are
/// placeholders pending real device measurements, same caveat as every other detector's
/// thresholds.
pub fn detect_excessive_commits_during_rapid_focus_movement(
    windows: &[InteractionWindow],
) -> Vec<Finding> {
    const RAPID_GAP_MS: f64 = 150.0;
    const COMMIT_THRESHOLD: usize = 3;

    let mut findings = Vec::new();
    let mut burst_start_index = 0;

    for index in 1..=windows.len() {
        let burst_continues = index < windows.len()
            && windows[index].input.timestamp - windows[index - 1].input.timestamp <= RAPID_GAP_MS;
        if burst_continues {
            continue;
        }

        let burst = &windows[burst_start_index..index];
        if burst.len() >= 2 {
            let commit_sequences: Vec<u64> = burst
                .iter()
                .flat_map(|window| &window.other_events)
                .filter_map(|event| match event {
                    Event::ReactCommit(commit) => Some(commit.sequence),
                    _ => None,
                })
                .collect();

            if commit_sequences.len() >= COMMIT_THRESHOLD {
                let sequence_start = burst[0].input.sequence;
                let sequence_end = *commit_sequences.iter().max().expect("checked len above");
                let count = commit_sequences.len();
                findings.push(Finding {
                    id: finding_id(
                        EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR,
                        sequence_start,
                        sequence_end,
                    ),
                    detector: EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR,
                    detector_version:
                        EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR_VERSION,
                    severity: Severity::Warning,
                    sequence_start,
                    sequence_end,
                    value: count as f64,
                    unit: "commits",
                    thresholds: vec![
                        Threshold {
                            name: "rapidGapMs",
                            value: RAPID_GAP_MS,
                        },
                        Threshold {
                            name: "commitThreshold",
                            value: COMMIT_THRESHOLD as f64,
                        },
                    ],
                    summary: format!(
                        "{count} React commits occurred during a burst of rapid focus movement."
                    ),
                });
            }
        }

        burst_start_index = index;
    }

    findings
}

/// Flags interaction windows where the same instrumented selector was invoked more than once
/// with unchanged inputs (`inputs_changed == false` each time) — evidence the caller isn't
/// de-duplicating/memoizing its own calls, so the selector does real work repeatedly for no
/// reason even if its own result stays stable. The repeat threshold matches
/// `detect_repeated_redux_dispatches`'s own reasoning — a starting guess, not a measured number.
pub fn detect_repeated_selector_recomputation(windows: &[InteractionWindow]) -> Vec<Finding> {
    const REPEAT_THRESHOLD: usize = 2;

    let mut findings = Vec::new();

    for window in windows {
        let mut by_selector_id: HashMap<&str, Vec<u64>> = HashMap::new();
        for event in &window.other_events {
            if let Event::Selector(selector) = event
                && !selector.inputs_changed
            {
                by_selector_id
                    .entry(selector.selector_id.as_str())
                    .or_default()
                    .push(selector.sequence);
            }
        }

        for sequences in by_selector_id.into_values() {
            if sequences.len() < REPEAT_THRESHOLD {
                continue;
            }
            let sequence_start = window.input.sequence;
            let sequence_end = *sequences.iter().max().expect("non-empty");
            let count = sequences.len();
            findings.push(Finding {
                id: finding_id(
                    REPEATED_SELECTOR_RECOMPUTATION_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: REPEATED_SELECTOR_RECOMPUTATION_DETECTOR,
                detector_version: REPEATED_SELECTOR_RECOMPUTATION_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start,
                sequence_end,
                value: count as f64,
                unit: "invocations",
                thresholds: vec![Threshold {
                    name: "repeatThreshold",
                    value: REPEAT_THRESHOLD as f64,
                }],
                summary: format!(
                    "An instrumented selector was invoked {count} times with unchanged inputs within one interaction."
                ),
            });
        }
    }

    findings
}

/// Flags an instrumented selector that returned a different reference despite unchanged inputs
/// (`inputs_changed == false && result_changed == true` — a broken/unstable selector, not a
/// real state change) whose window also contains a burst of React commits, evidence the
/// unstable reference is plausibly the actual cause of those extra re-renders rather than just
/// coincidentally nearby. The commit threshold mirrors
/// `detect_excessive_commits_during_rapid_focus_movement`'s own placeholder reasoning.
pub fn detect_unstable_selector_references(windows: &[InteractionWindow]) -> Vec<Finding> {
    const COMMIT_THRESHOLD: usize = 2;

    let mut findings = Vec::new();

    for window in windows {
        let commit_count = window
            .other_events
            .iter()
            .filter(|event| matches!(event, Event::ReactCommit(_)))
            .count();
        if commit_count < COMMIT_THRESHOLD {
            continue;
        }

        for event in &window.other_events {
            let Event::Selector(selector) = event else {
                continue;
            };
            if selector.inputs_changed || !selector.result_changed {
                continue;
            }

            let sequence_start = window.input.sequence;
            let sequence_end = selector.sequence;
            findings.push(Finding {
                id: finding_id(
                    UNSTABLE_SELECTOR_REFERENCE_DETECTOR,
                    sequence_start,
                    sequence_end,
                ),
                detector: UNSTABLE_SELECTOR_REFERENCE_DETECTOR,
                detector_version: UNSTABLE_SELECTOR_REFERENCE_DETECTOR_VERSION,
                severity: Severity::Warning,
                sequence_start,
                sequence_end,
                value: commit_count as f64,
                unit: "commits",
                thresholds: vec![Threshold {
                    name: "commitThreshold",
                    value: COMMIT_THRESHOLD as f64,
                }],
                summary: format!(
                    "Selector \"{}\" returned a new reference with unchanged inputs, overlapping {commit_count} React commits.",
                    selector.selector_id
                ),
            });
        }
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{FocusEvent, RemoteInputEvent, VisibleUpdateEvent};

    fn window_with_visible_update_latency(latency_ms: f64) -> InteractionWindow {
        InteractionWindow {
            input: RemoteInputEvent {
                sequence: 0,
                timestamp: 0.0,
                key: "right".to_string(),
            },
            focus: None,
            visible_update: Some(VisibleUpdateEvent {
                sequence: 1,
                timestamp: latency_ms,
                target_id: "card-2".to_string(),
            }),
            other_events: Vec::new(),
        }
    }

    #[test]
    fn visible_update_below_the_warning_threshold_produces_no_finding() {
        assert_eq!(
            detect_high_latency_visible_updates(&[window_with_visible_update_latency(50.0)]),
            vec![]
        );
    }

    #[test]
    fn visible_update_between_the_thresholds_is_a_warning() {
        let findings =
            detect_high_latency_visible_updates(&[window_with_visible_update_latency(150.0)]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].value, 150.0);
        assert_eq!(findings[0].unit, "ms");
        assert_eq!(findings[0].detector, HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR);
    }

    #[test]
    fn visible_update_at_or_above_the_critical_threshold_is_critical() {
        let findings =
            detect_high_latency_visible_updates(&[window_with_visible_update_latency(300.0)]);

        assert_eq!(findings[0].severity, Severity::Critical);
    }

    #[test]
    fn a_window_with_no_visible_update_produces_no_visible_update_finding() {
        let window = window_with_latency(150.0);

        assert_eq!(detect_high_latency_visible_updates(&[window]), vec![]);
    }

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
            visible_update: None,
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
            visible_update: None,
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
            visible_update: None,
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
            visible_update: None,
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
            visible_update: None,
            other_events: vec![
                dispatch(1, "catalog/itemFocused"),
                dispatch(2, "nav/moveRight"),
            ],
        };

        assert_eq!(detect_repeated_redux_dispatches(&[window]), vec![]);
    }

    fn window_at(
        input_sequence: u64,
        input_timestamp: f64,
        other_events: Vec<Event>,
    ) -> InteractionWindow {
        InteractionWindow {
            input: RemoteInputEvent {
                sequence: input_sequence,
                timestamp: input_timestamp,
                key: "right".to_string(),
            },
            focus: None,
            visible_update: None,
            other_events,
        }
    }

    fn js_stall(sequence: u64, timestamp: f64, duration_ms: f64) -> Event {
        Event::JsStall(session_telemetry_protocol::JsStallEvent {
            sequence,
            timestamp,
            duration_ms,
        })
    }

    #[test]
    fn a_js_stall_during_an_interaction_is_a_warning_below_the_critical_threshold() {
        let window = window_at(0, 0.0, vec![js_stall(1, 10.0, 60.0)]);

        let findings = detect_js_stalls_overlapping_interactions(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, JS_STALL_DURING_INTERACTION_DETECTOR);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(findings[0].value, 60.0);
    }

    #[test]
    fn a_js_stall_at_or_above_the_critical_threshold_is_critical() {
        let window = window_at(0, 0.0, vec![js_stall(1, 10.0, 150.0)]);

        let findings = detect_js_stalls_overlapping_interactions(&[window]);

        assert_eq!(findings[0].severity, Severity::Critical);
    }

    #[test]
    fn no_js_stall_produces_no_finding() {
        let window = window_at(0, 0.0, vec![dispatch(1, "catalog/itemFocused")]);

        assert_eq!(detect_js_stalls_overlapping_interactions(&[window]), vec![]);
    }

    fn network(sequence: u64, timestamp: f64, method: &str, url: &str, duration_ms: f64) -> Event {
        Event::Network(session_telemetry_protocol::NetworkEvent {
            sequence,
            timestamp,
            method: method.to_string(),
            url: url.to_string(),
            status: 200,
            duration_ms,
            request_bytes: None,
            response_bytes: None,
        })
    }

    #[test]
    fn the_same_method_and_url_requested_twice_is_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                network(1, 1.0, "GET", "https://api.example.com/program/482", 20.0),
                network(2, 2.0, "GET", "https://api.example.com/program/482", 20.0),
            ],
        );

        let findings = detect_repeated_network_requests(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, REPEATED_NETWORK_REQUEST_DETECTOR);
        assert_eq!(findings[0].value, 2.0);
        assert_eq!(findings[0].unit, "requests");
    }

    #[test]
    fn different_urls_are_not_conflated_as_repeated_requests() {
        let window = window_at(
            0,
            0.0,
            vec![
                network(1, 1.0, "GET", "https://api.example.com/program/482", 20.0),
                network(2, 2.0, "GET", "https://api.example.com/program/999", 20.0),
            ],
        );

        assert_eq!(detect_repeated_network_requests(&[window]), vec![]);
    }

    #[test]
    fn long_running_requests_under_the_same_url_are_not_flagged_as_repeated() {
        // A persistent stream reporting twice under the same URL (e.g. reconnect after a long
        // first connection) shouldn't read as "the app fired a duplicate request by mistake".
        let window = window_at(
            0,
            0.0,
            vec![
                network(
                    1,
                    1.0,
                    "GET",
                    "https://api.example.com/live-stream",
                    5_000.0,
                ),
                network(
                    2,
                    5_001.0,
                    "GET",
                    "https://api.example.com/live-stream",
                    5_000.0,
                ),
            ],
        );

        assert_eq!(detect_repeated_network_requests(&[window]), vec![]);
    }

    #[test]
    fn a_stream_reconnecting_rapidly_with_short_attempts_is_still_flagged() {
        // Each attempt is short-lived (unlike a real stream holding the connection open), so
        // this reads as a genuine reconnect-loop symptom, not expected streaming behavior.
        let window = window_at(
            0,
            0.0,
            vec![
                network(1, 1.0, "GET", "https://api.example.com/live-stream", 50.0),
                network(2, 60.0, "GET", "https://api.example.com/live-stream", 50.0),
            ],
        );

        let findings = detect_repeated_network_requests(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].value, 2.0);
    }

    fn react_commit(sequence: u64, timestamp: f64, actual_duration_ms: f64) -> Event {
        Event::ReactCommit(session_telemetry_protocol::ReactCommitEvent {
            sequence,
            timestamp,
            profiler_id: "CatalogRow".to_string(),
            phase: session_telemetry_protocol::ReactCommitPhase::Update,
            actual_duration_ms,
            base_duration_ms: actual_duration_ms,
        })
    }

    fn frame_timing(sequence: u64, timestamp: f64, duration_ms: f64) -> Event {
        Event::FrameTiming(session_telemetry_protocol::FrameTimingEvent {
            sequence,
            timestamp,
            duration_ms,
        })
    }

    #[test]
    fn an_overlapping_commit_and_delayed_frame_is_flagged() {
        // Commit spans [90, 100]; frame spans [95, 130] - they overlap.
        let window = window_at(
            0,
            0.0,
            vec![react_commit(1, 100.0, 10.0), frame_timing(2, 130.0, 35.0)],
        );

        let findings = detect_react_commits_overlapping_delayed_frames(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].detector,
            REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR
        );
        assert_eq!(findings[0].sequence_start, 1);
        assert_eq!(findings[0].sequence_end, 2);
        assert_eq!(findings[0].value, 35.0);
    }

    #[test]
    fn a_commit_well_before_a_delayed_frame_does_not_overlap() {
        // Commit spans [0, 10]; frame spans [200, 235] - no overlap.
        let window = window_at(
            0,
            0.0,
            vec![react_commit(1, 10.0, 10.0), frame_timing(2, 235.0, 35.0)],
        );

        assert_eq!(
            detect_react_commits_overlapping_delayed_frames(&[window]),
            vec![]
        );
    }

    #[test]
    fn a_network_request_immediately_followed_by_a_commit_is_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                network(1, 10.0, "GET", "/x", 5.0),
                react_commit(2, 25.0, 5.0),
            ],
        );

        let findings = detect_network_completions_followed_by_commits(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].detector,
            NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR
        );
        assert_eq!(findings[0].sequence_start, 1);
        assert_eq!(findings[0].sequence_end, 2);
        assert_eq!(findings[0].value, 15.0);
    }

    #[test]
    fn a_network_request_not_immediately_followed_by_a_commit_is_not_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                network(1, 10.0, "GET", "/x", 5.0),
                dispatch(2, "catalog/itemLoaded"),
                react_commit(3, 25.0, 5.0),
            ],
        );

        assert_eq!(
            detect_network_completions_followed_by_commits(&[window]),
            vec![]
        );
    }

    #[test]
    fn a_burst_of_rapid_inputs_with_excessive_commits_is_flagged() {
        let windows = vec![
            window_at(0, 0.0, vec![react_commit(1, 5.0, 1.0)]),
            window_at(2, 50.0, vec![react_commit(3, 55.0, 1.0)]),
            window_at(4, 100.0, vec![react_commit(5, 105.0, 1.0)]),
        ];

        let findings = detect_excessive_commits_during_rapid_focus_movement(&windows);

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].detector,
            EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR
        );
        assert_eq!(findings[0].sequence_start, 0);
        assert_eq!(findings[0].sequence_end, 5);
        assert_eq!(findings[0].value, 3.0);
    }

    #[test]
    fn inputs_spaced_far_apart_are_not_a_rapid_burst() {
        let windows = vec![
            window_at(0, 0.0, vec![react_commit(1, 5.0, 1.0)]),
            window_at(2, 1000.0, vec![react_commit(3, 1005.0, 1.0)]),
            window_at(4, 2000.0, vec![react_commit(5, 2005.0, 1.0)]),
        ];

        assert_eq!(
            detect_excessive_commits_during_rapid_focus_movement(&windows),
            vec![]
        );
    }

    #[test]
    fn a_rapid_burst_under_the_commit_threshold_is_not_flagged() {
        let windows = vec![
            window_at(0, 0.0, vec![react_commit(1, 5.0, 1.0)]),
            window_at(2, 50.0, vec![]),
        ];

        assert_eq!(
            detect_excessive_commits_during_rapid_focus_movement(&windows),
            vec![]
        );
    }

    fn selector(
        sequence: u64,
        selector_id: &str,
        inputs_changed: bool,
        result_changed: bool,
    ) -> Event {
        Event::Selector(session_telemetry_protocol::SelectorEvent {
            sequence,
            timestamp: sequence as f64,
            selector_id: selector_id.to_string(),
            duration_ms: 0.5,
            inputs_changed,
            result_changed,
        })
    }

    #[test]
    fn a_selector_invoked_twice_with_unchanged_inputs_is_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", false, false),
                selector(2, "catalog/selectVisibleItemIds", false, false),
            ],
        );

        let findings = detect_repeated_selector_recomputation(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(
            findings[0].detector,
            REPEATED_SELECTOR_RECOMPUTATION_DETECTOR
        );
        assert_eq!(findings[0].value, 2.0);
        assert_eq!(findings[0].unit, "invocations");
    }

    #[test]
    fn a_selector_invocation_with_changed_inputs_does_not_count_as_a_recomputation() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", true, true),
                selector(2, "catalog/selectVisibleItemIds", true, true),
            ],
        );

        assert_eq!(detect_repeated_selector_recomputation(&[window]), vec![]);
    }

    #[test]
    fn different_selectors_are_not_conflated_as_repeated() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", false, false),
                selector(2, "catalog/selectSortOrder", false, false),
            ],
        );

        assert_eq!(detect_repeated_selector_recomputation(&[window]), vec![]);
    }

    #[test]
    fn an_unstable_selector_overlapping_repeated_commits_is_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", false, true),
                react_commit(2, 5.0, 1.0),
                react_commit(3, 10.0, 1.0),
            ],
        );

        let findings = detect_unstable_selector_references(&[window]);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, UNSTABLE_SELECTOR_REFERENCE_DETECTOR);
        assert_eq!(findings[0].sequence_end, 1);
        assert_eq!(findings[0].value, 2.0);
    }

    #[test]
    fn an_unstable_selector_without_enough_overlapping_commits_is_not_flagged() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", false, true),
                react_commit(2, 5.0, 1.0),
            ],
        );

        assert_eq!(detect_unstable_selector_references(&[window]), vec![]);
    }

    #[test]
    fn a_stable_selector_result_is_not_flagged_even_with_repeated_commits() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", false, false),
                react_commit(2, 5.0, 1.0),
                react_commit(3, 10.0, 1.0),
            ],
        );

        assert_eq!(detect_unstable_selector_references(&[window]), vec![]);
    }

    #[test]
    fn a_new_result_from_changed_inputs_is_not_flagged_as_unstable() {
        let window = window_at(
            0,
            0.0,
            vec![
                selector(1, "catalog/selectVisibleItemIds", true, true),
                react_commit(2, 5.0, 1.0),
                react_commit(3, 10.0, 1.0),
            ],
        );

        assert_eq!(detect_unstable_selector_references(&[window]), vec![]);
    }
}
