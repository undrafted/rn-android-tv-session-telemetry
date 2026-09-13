use crate::clock::ClockMap;

/// plan.md section 6's QA annotation row: "workstation timestamp, mapped session timestamp,
/// bookmark label." `workstation_timestamp` is whatever the workstation's own clock read when
/// `session-telemetry mark` ran (effectively wall-clock, not the session's monotonic source) —
/// `session_timestamp` is that same moment mapped onto the session's monotonic timeline via
/// `ClockMap`, which is what makes it safe to place the bookmark on the actual event timeline
/// even though the input reading itself isn't monotonic.
#[derive(Debug, Clone, PartialEq)]
pub struct QaBookmark {
    pub workstation_timestamp: f64,
    pub session_timestamp: f64,
    pub label: String,
}

/// Reuses the same generic `ClockMap` mechanism built for JS/native clock sync — it maps any
/// one clock domain onto another from sync samples, not something JS/native-specific, so there
/// is no need for a second mapping mechanism just because the input domain here is different.
pub fn create_bookmark(
    clock_map: &ClockMap,
    workstation_timestamp: f64,
    label: String,
) -> QaBookmark {
    QaBookmark {
        workstation_timestamp,
        session_timestamp: clock_map.map(workstation_timestamp),
        label,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ClockSyncSample;

    #[test]
    fn maps_the_workstation_timestamp_onto_the_session_timeline() {
        // reference = source + 1_000 exactly.
        let clock_map = ClockMap::from_samples(&[ClockSyncSample {
            source: 0.0,
            reference: 1_000.0,
        }])
        .unwrap();

        let bookmark =
            create_bookmark(&clock_map, 500.0, "carousel stopped responding".to_string());

        assert_eq!(bookmark.workstation_timestamp, 500.0);
        assert_eq!(bookmark.session_timestamp, 1_500.0);
        assert_eq!(bookmark.label, "carousel stopped responding");
    }

    #[test]
    fn preserves_the_label_verbatim() {
        let clock_map = ClockMap::from_samples(&[ClockSyncSample {
            source: 0.0,
            reference: 0.0,
        }])
        .unwrap();

        let bookmark = create_bookmark(&clock_map, 42.0, "navigation felt delayed".to_string());

        assert_eq!(bookmark.label, "navigation felt delayed");
    }
}
