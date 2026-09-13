use session_telemetry_protocol::{Event, FocusEvent, RemoteInputEvent, VisibleUpdateEvent};

/// One remote-control press and whatever happened after it, up to (but not including) the next
/// remote input. `focus` is the resulting focus change if one occurred; `visible_update` is the
/// best-effort confirmation that change was scheduled to paint (see `VisibleUpdateEvent`'s own
/// doc comment for why this is approximate, not frame-accurate). Either being `None` means that
/// signal wasn't observed for this specific input before either another input arrived or the
/// session ended.
///
/// Both `latency_ms()` (input-to-focus-change) and `visible_update_latency_ms()`
/// (input-to-visible-update) stay available side by side — don't conflate the two when
/// reading results from this module.
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionWindow {
    pub input: RemoteInputEvent,
    pub focus: Option<FocusEvent>,
    pub visible_update: Option<VisibleUpdateEvent>,
    /// Everything else (Redux dispatches, interaction markers, frame timings) that fell between
    /// the input and the window's end — evidence for detectors like "repeated Redux actions
    /// during one remote-input burst" or "commit overlapping a delayed frame".
    pub other_events: Vec<Event>,
}

impl InteractionWindow {
    /// Milliseconds from remote input to the resulting focus change, or `None` if no focus
    /// change was observed for this input.
    pub fn latency_ms(&self) -> Option<f64> {
        Some(self.focus.as_ref()?.timestamp - self.input.timestamp)
    }

    /// Milliseconds from remote input to the resulting visible update, or `None` if none was
    /// observed for this input.
    pub fn visible_update_latency_ms(&self) -> Option<f64> {
        Some(self.visible_update.as_ref()?.timestamp - self.input.timestamp)
    }
}

/// Groups events into one window per remote input. A window ends at the next `VisibleUpdate`
/// event (the window's ultimate result) or at the next `RemoteInput` event (a new input arrived
/// before this one produced a visible update), whichever comes first. `Focus` no longer ends the
/// window on its own — it's recorded and scanning continues, so events between a focus change
/// and its eventual visible update (redux dispatches, react commits, frame timings) still land
/// in this window rather than leaking into the next one.
pub fn build_interaction_windows(events: &[Event]) -> Vec<InteractionWindow> {
    let mut windows = Vec::new();

    for (start, event) in events.iter().enumerate() {
        let Event::RemoteInput(input) = event else {
            continue;
        };

        let mut other_events = Vec::new();
        let mut focus = None;
        let mut visible_update = None;

        for later_event in &events[start + 1..] {
            match later_event {
                Event::RemoteInput(_) => break,
                Event::Focus(focus_event) => {
                    focus = Some(focus_event.clone());
                }
                Event::VisibleUpdate(visible_update_event) => {
                    visible_update = Some(visible_update_event.clone());
                    break;
                }
                other => other_events.push(other.clone()),
            }
        }

        windows.push(InteractionWindow {
            input: input.clone(),
            focus,
            visible_update,
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

    fn visible_update(sequence: u64, timestamp: f64, target_id: &str) -> Event {
        Event::VisibleUpdate(VisibleUpdateEvent {
            sequence,
            timestamp,
            target_id: target_id.to_string(),
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
    fn pairs_an_input_with_its_resulting_visible_update() {
        let events = vec![
            remote_input(0, 0.0, "right"),
            focus(1, 4.0, "card-2"),
            visible_update(2, 214.0, "card-2"),
        ];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].latency_ms(), Some(4.0));
        assert_eq!(windows[0].visible_update_latency_ms(), Some(214.0));
    }

    #[test]
    fn events_between_focus_and_visible_update_are_still_collected_as_context() {
        let dispatch = Event::ReduxDispatch(ReduxDispatchEvent {
            sequence: 2,
            timestamp: 10.0,
            action_type: "catalog/itemLoaded".to_string(),
            duration_ms: 2.0,
        });
        let events = vec![
            remote_input(0, 0.0, "right"),
            focus(1, 4.0, "card-2"),
            dispatch.clone(),
            visible_update(3, 214.0, "card-2"),
        ];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows[0].other_events, vec![dispatch]);
        assert_eq!(windows[0].visible_update_latency_ms(), Some(214.0));
    }

    #[test]
    fn a_window_with_no_visible_update_before_the_next_input_has_no_result() {
        let events = vec![
            remote_input(0, 0.0, "right"),
            focus(1, 4.0, "card-2"),
            remote_input(2, 50.0, "right"),
            visible_update(3, 214.0, "card-2"),
        ];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 2);
        assert_eq!(windows[0].visible_update, None);
        assert_eq!(windows[0].visible_update_latency_ms(), None);
        // The second input claims the visible update instead.
        assert_eq!(windows[1].visible_update_latency_ms(), Some(164.0));
    }

    #[test]
    fn an_input_at_the_end_of_the_session_has_no_result() {
        let events = vec![focus(0, 10.0, "card-1"), remote_input(1, 20.0, "right")];

        let windows = build_interaction_windows(&events);

        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0].focus, None);
    }
}
