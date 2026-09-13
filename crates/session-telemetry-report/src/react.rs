use serde::Serialize;
use session_telemetry_analysis::build_interaction_windows;
use session_telemetry_protocol::Event;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactRenderStats {
    pub commit_count: usize,
    pub measured_commit_count: usize,
    pub total_render_work_ms: Option<f64>,
    pub longest_render_work_ms: Option<f64>,
}

impl ReactRenderStats {
    fn add(&mut self, duration: f64) {
        self.commit_count += 1;
        if duration.is_finite() && duration >= 0.0 {
            self.measured_commit_count += 1;
            self.total_render_work_ms = Some(self.total_render_work_ms.unwrap_or(0.0) + duration);
            self.longest_render_work_ms =
                Some(self.longest_render_work_ms.unwrap_or(0.0).max(duration));
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactInteractionStats {
    pub input_sequence: u64,
    pub input_key: String,
    #[serde(flatten)]
    pub stats: ReactRenderStats,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactSummary {
    /// Separate profiler totals must not be added together: nested profilers can overlap.
    pub by_profiler: BTreeMap<String, ReactRenderStats>,
    pub interactions: Vec<ReactInteractionStats>,
}

impl ReactSummary {
    pub fn from_events(events: &[Event]) -> Self {
        let mut by_profiler = BTreeMap::<String, ReactRenderStats>::new();
        for event in events {
            if let Event::ReactCommit(commit) = event {
                by_profiler
                    .entry(commit.profiler_id.clone())
                    .or_default()
                    .add(commit.actual_duration_ms);
            }
        }
        let interactions = build_interaction_windows(events)
            .into_iter()
            .map(|window| {
                let mut stats = ReactRenderStats::default();
                for event in window.other_events {
                    if let Event::ReactCommit(commit) = event {
                        stats.add(commit.actual_duration_ms);
                    }
                }
                ReactInteractionStats {
                    input_sequence: window.input.sequence,
                    input_key: window.input.key,
                    stats,
                }
            })
            .collect();
        Self {
            by_profiler,
            interactions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{
        ReactCommitEvent, ReactCommitPhase, RemoteInputEvent, VisibleUpdateEvent,
    };

    fn commit(sequence: u64, id: &str, duration: f64) -> Event {
        Event::ReactCommit(ReactCommitEvent {
            sequence,
            timestamp: sequence as f64,
            profiler_id: id.into(),
            phase: ReactCommitPhase::Update,
            actual_duration_ms: duration,
            base_duration_ms: 500.0,
            render_start_ms: Some(0.0),
            commit_time_ms: Some(sequence as f64),
        })
    }

    #[test]
    fn groups_render_work_and_respects_interaction_boundaries() {
        let events = vec![
            commit(0, "App", 8.0),
            Event::RemoteInput(RemoteInputEvent {
                sequence: 1,
                timestamp: 1.0,
                key: "right".into(),
            }),
            commit(2, "App", 2.0),
            commit(3, "App", 4.0),
            Event::VisibleUpdate(VisibleUpdateEvent {
                sequence: 4,
                timestamp: 4.0,
                target_id: "card".into(),
            }),
            commit(5, "Other", 7.0),
            Event::RemoteInput(RemoteInputEvent {
                sequence: 6,
                timestamp: 6.0,
                key: "left".into(),
            }),
        ];
        let summary = ReactSummary::from_events(&events);
        assert_eq!(summary.by_profiler["App"].total_render_work_ms, Some(14.0));
        assert_eq!(summary.by_profiler["Other"].commit_count, 1);
        assert_eq!(
            summary.interactions[0].stats,
            ReactRenderStats {
                commit_count: 2,
                measured_commit_count: 2,
                total_render_work_ms: Some(6.0),
                longest_render_work_ms: Some(4.0)
            }
        );
        assert_eq!(summary.interactions[1].stats, ReactRenderStats::default());
    }

    #[test]
    fn invalid_work_is_counted_but_not_summed() {
        let summary = ReactSummary::from_events(&[
            commit(1, "App", f64::NAN),
            commit(2, "App", -1.0),
            commit(3, "App", 0.0),
        ]);
        assert_eq!(
            summary.by_profiler["App"],
            ReactRenderStats {
                commit_count: 3,
                measured_commit_count: 1,
                total_render_work_ms: Some(0.0),
                longest_render_work_ms: Some(0.0)
            }
        );
        assert!(ReactSummary::from_events(&[]).by_profiler.is_empty());
    }
}
