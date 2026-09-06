use session_telemetry_protocol::{Event, FocusEvent, RemoteInputEvent};

/// One remote-control press and whatever happened after it, up to (but not including) the next
/// remote input. `focus` is the resulting focus change if one occurred; `None` means no focus
/// change was observed for this specific input before either another input arrived or the
/// session ended.
///
/// This measures input-to-*focus-change* latency, one of the "Core metrics" in plan.md section
/// 7. It is not yet input-to-*visible-update* latency (the plan's headline metric) — that needs
/// an explicit visible-update marker, which is architecture decision #8 in section 14 and isn't
/// decided yet. Don't conflate the two when reading results from this module.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionWindow {
    pub input: RemoteInputEvent,
    pub focus: Option<FocusEvent>,
    /// Everything else (Redux dispatches, interaction markers) that fell between the input and
    /// the resulting focus change (or the window's end) — evidence for detectors like "repeated
    /// Redux actions during one remote-input burst" (plan.md section 7, detector #4).
    pub other_events: Vec<Event>,
}

impl InteractionWindow {
    /// Milliseconds from remote input to the resulting focus change, or `None` if no focus
    /// change was observed for this input.
    pub fn latency_ms(&self) -> Option<f64> {
        Some(self.focus.as_ref()?.timestamp - self.input.timestamp)
    }
}

/// Groups events into one window per remote input. A window ends at the next `Focus` event (the
/// window's result) or at the next `RemoteInput` event (a new input arrived before this one
/// produced a focus change), whichever comes first.
pub fn build_interaction_windows(events: &[Event]) -> Vec<InteractionWindow> {
    let mut windows = Vec::new();

    for (start, event) in events.iter().enumerate() {
        let Event::RemoteInput(input) = event else {
            continue;
        };

        let mut other_events = Vec::new();
        let mut focus = None;

        for later_event in &events[start + 1..] {
            match later_event {
                Event::RemoteInput(_) => break,
                Event::Focus(focus_event) => {
                    focus = Some(focus_event.clone());
                    break;
                }
                other => other_events.push(other.clone()),
            }
        }

        windows.push(InteractionWindow {
            input: input.clone(),
            focus,
            other_events,
        });
    }

    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{InteractionMarkerEvent, ReduxDispatchEvent};

    fn remote_input(sequence: u64, timestamp: f64, key: &str) -> Event {
        Event::RemoteInput(RemoteInputEvent {
            sequence,
            timestamp,
            key: key.to_string(),
        })
    }

    fn focus(sequence: u64, timestamp: f64, target_id: &str) -> Event {
        Event::Focus(FocusEvent {
            sequence,
            timestamp,
            target_id: target_id.to_string(),
            previous_target_id: None,
        })
    }

    #[test]
    fn pairs_an_input_with_its_resulting_focus_change() {
        let events = vec![remote_input(0, 0.0, "right"), focus(1, 214.0, "card-2")];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].latency_ms(), Some(214.0));
    }

    #[test]
    fn an_input_with_no_focus_change_before_the_next_input_has_no_result() {
        let events = vec![
            remote_input(0, 0.0, "right"),
            remote_input(1, 50.0, "right"),
            focus(2, 214.0, "card-2"),
        ];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].focus, None);
        assert_eq!(windows[0].latency_ms(), None);
        // The second input claims the focus change instead.
        assert_eq!(windows[1].latency_ms(), Some(164.0));
    }

    #[test]
    fn collects_intervening_events_as_context() {
        let dispatch = Event::ReduxDispatch(ReduxDispatchEvent {
            sequence: 1,
            timestamp: 4.0,
            action_type: "catalog/itemFocused".to_string(),
            duration_ms: 1.0,
        });
        let marker = Event::InteractionMarker(InteractionMarkerEvent {
            sequence: 2,
            timestamp: 6.0,
            name: "demo:card-select".to_string(),
        });
        let events = vec![
            remote_input(0, 0.0, "right"),
            dispatch.clone(),
            marker.clone(),
            focus(3, 214.0, "card-2"),
        ];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows[0].other_events, vec![dispatch, marker]);
    }

    #[test]
    fn an_input_at_the_end_of_the_session_has_no_result() {
        let events = vec![focus(0, 10.0, "card-1"), remote_input(1, 20.0, "right")];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].focus, None);
    }
}
