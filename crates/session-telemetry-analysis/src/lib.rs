//! Clock mapping, interaction windows, metrics, and deterministic detectors.

mod bookmark;
mod clock;
mod detector;
mod interaction;
mod session_metadata;

pub use bookmark::{QaBookmark, create_bookmark};
pub use clock::{ClockMap, ClockSyncSample, clock_sync_samples_from_events};
pub use detector::{
    EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR,
    EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR_VERSION, Finding,
    HIGH_LATENCY_FOCUS_CHANGE_DETECTOR, HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION,
    HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR, HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR_VERSION,
    JS_STALL_DURING_INTERACTION_DETECTOR, JS_STALL_DURING_INTERACTION_DETECTOR_VERSION,
    NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR,
    NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR_VERSION,
    REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR,
    REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR_VERSION, REPEATED_NETWORK_REQUEST_DETECTOR,
    REPEATED_NETWORK_REQUEST_DETECTOR_VERSION, REPEATED_REDUX_DISPATCH_DETECTOR,
    REPEATED_REDUX_DISPATCH_DETECTOR_VERSION, Severity, Threshold,
    detect_excessive_commits_during_rapid_focus_movement, detect_high_latency_focus_changes,
    detect_high_latency_visible_updates, detect_js_stalls_overlapping_interactions,
    detect_network_completions_followed_by_commits,
    detect_react_commits_overlapping_delayed_frames, detect_repeated_network_requests,
    detect_repeated_redux_dispatches,
};
pub use interaction::{InteractionWindow, build_interaction_windows};
pub use session_metadata::session_metadata_from_events;
