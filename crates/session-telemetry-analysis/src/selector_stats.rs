use serde::Serialize;
use session_telemetry_protocol::Event;
use std::collections::BTreeMap;

/// Aggregate stats for one instrumented selector across the whole session — plan.md's own
/// "Selector invocation, recomputation, duration, and unstable-result rates" metric.
/// `recomputation_count` counts invocations where `inputs_changed` was `false` (the selector ran
/// again despite unchanged arguments — real work regardless of whether the caller needed it).
/// `unstable_result_count` counts the subset of those that also returned a different reference
/// (`result_changed == true`) — a selector whose memoization is broken, not just re-invoked.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorStats {
    pub selector_id: String,
    pub invocation_count: usize,
    pub total_duration_ms: f64,
    pub max_duration_ms: f64,
    pub recomputation_count: usize,
    pub unstable_result_count: usize,
}

/// One entry per distinct `selector_id` that appeared in `events`, sorted by id for a
/// deterministic report/HTML ordering.
pub fn selector_stats(events: &[Event]) -> Vec<SelectorStats> {
    let mut by_id: BTreeMap<&str, SelectorStats> = BTreeMap::new();

    for event in events {
        let Event::Selector(selector) = event else {
            continue;
        };
        let stats = by_id
            .entry(selector.selector_id.as_str())
            .or_insert_with(|| SelectorStats {
                selector_id: selector.selector_id.clone(),
                invocation_count: 0,
                total_duration_ms: 0.0,
                max_duration_ms: 0.0,
                recomputation_count: 0,
                unstable_result_count: 0,
            });
        stats.invocation_count += 1;
        stats.total_duration_ms += selector.duration_ms;
        stats.max_duration_ms = stats.max_duration_ms.max(selector.duration_ms);
        if !selector.inputs_changed {
            stats.recomputation_count += 1;
            if selector.result_changed {
                stats.unstable_result_count += 1;
            }
        }
    }

    by_id.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::SelectorEvent;

    fn selector(
        sequence: u64,
        selector_id: &str,
        duration_ms: f64,
        inputs_changed: bool,
        result_changed: bool,
    ) -> Event {
        Event::Selector(SelectorEvent {
            sequence,
            timestamp: sequence as f64,
            selector_id: selector_id.to_string(),
            duration_ms,
            inputs_changed,
            result_changed,
        })
    }

    #[test]
    fn empty_events_yield_no_stats() {
        assert_eq!(selector_stats(&[]), vec![]);
    }

    #[test]
    fn aggregates_invocation_count_and_durations_per_selector() {
        let events = vec![
            selector(0, "catalog/selectVisibleItemIds", 1.0, true, true),
            selector(1, "catalog/selectVisibleItemIds", 3.0, true, true),
        ];

        let stats = selector_stats(&events);

        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].selector_id, "catalog/selectVisibleItemIds");
        assert_eq!(stats[0].invocation_count, 2);
        assert_eq!(stats[0].total_duration_ms, 4.0);
        assert_eq!(stats[0].max_duration_ms, 3.0);
    }

    #[test]
    fn counts_recomputations_only_when_inputs_are_unchanged() {
        let events = vec![
            selector(0, "s", 1.0, true, true),
            selector(1, "s", 1.0, false, false),
            selector(2, "s", 1.0, true, true),
        ];

        let stats = selector_stats(&events);

        assert_eq!(stats[0].recomputation_count, 1);
    }

    #[test]
    fn counts_unstable_results_only_among_recomputations() {
        let events = vec![
            // Inputs changed, result also different - not unstable, just a real update.
            selector(0, "s", 1.0, true, true),
            // Inputs unchanged but result changed - the broken-memoization case.
            selector(1, "s", 1.0, false, true),
            // Inputs unchanged and result stable - a properly memoized re-invocation.
            selector(2, "s", 1.0, false, false),
        ];

        let stats = selector_stats(&events);

        assert_eq!(stats[0].recomputation_count, 2);
        assert_eq!(stats[0].unstable_result_count, 1);
    }

    #[test]
    fn keeps_distinct_selectors_separate_and_sorted_by_id() {
        let events = vec![
            selector(0, "z/selector", 1.0, true, true),
            selector(1, "a/selector", 2.0, true, true),
        ];

        let stats = selector_stats(&events);

        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].selector_id, "a/selector");
        assert_eq!(stats[1].selector_id, "z/selector");
    }

    #[test]
    fn ignores_non_selector_events() {
        let events = vec![Event::Selector(SelectorEvent {
            sequence: 0,
            timestamp: 0.0,
            selector_id: "s".to_string(),
            duration_ms: 1.0,
            inputs_changed: true,
            result_changed: true,
        })];
        let mut with_other = events.clone();
        with_other.push(Event::RemoteInput(
            session_telemetry_protocol::RemoteInputEvent {
                sequence: 1,
                timestamp: 1.0,
                key: "right".to_string(),
            },
        ));

        assert_eq!(selector_stats(&with_other), selector_stats(&events));
    }
}
