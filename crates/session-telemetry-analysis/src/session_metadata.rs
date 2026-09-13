use session_telemetry_protocol::{Event, SessionMetadataEvent};

/// The session's own `SessionMetadataEvent`, if the JS library emitted one — it's emitted once
/// per `install()` call (see `SessionMetadataEvent`'s own doc comment), so the first match is
/// the whole answer, unlike `clock_sync_samples_from_events`'s collection of periodic samples.
/// `None` for a session recorded by a library version that predates this event, not an error.
pub fn session_metadata_from_events(events: &[Event]) -> Option<&SessionMetadataEvent> {
    events.iter().find_map(|event| match event {
        Event::SessionMetadata(metadata) => Some(metadata),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::RemoteInputEvent;

    fn metadata(sequence: u64) -> SessionMetadataEvent {
        SessionMetadataEvent {
            sequence,
            timestamp: 0.0,
            device_model: "sdk_google_atv64_arm64".to_string(),
            os_version: "14".to_string(),
            app_version: Some("1.2.3".to_string()),
            build_type: Some("profiling".to_string()),
        }
    }

    #[test]
    fn returns_none_when_no_session_metadata_event_is_present() {
        let events = vec![Event::RemoteInput(RemoteInputEvent {
            sequence: 0,
            timestamp: 0.0,
            key: "right".to_string(),
        })];

        assert_eq!(session_metadata_from_events(&events), None);
    }

    #[test]
    fn returns_the_first_session_metadata_event() {
        let first = metadata(0);
        let events = vec![
            Event::SessionMetadata(first.clone()),
            Event::SessionMetadata(metadata(5)),
        ];

        assert_eq!(session_metadata_from_events(&events), Some(&first));
    }
}
