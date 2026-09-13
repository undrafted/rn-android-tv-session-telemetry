//! Clock mapping, interaction windows, metrics, and deterministic detectors.

mod bookmark;
mod clock;
mod detector;
mod interaction;
mod resource_sampling;
mod selector_stats;
mod session_metadata;

pub use bookmark::{QaBookmark, create_bookmark};
pub use clock::{ClockMap, ClockSyncSample, clock_sync_samples_from_events};
pub use detector::{
    EXCESSIVE_COMMITS_DURING_RAPID_FOCUS_MOVEMENT_DETECTOR, Finding,
    HIGH_CPU_SUSTAINED_DURING_RESOURCE_SAMPLING_DETECTOR, HIGH_LATENCY_FOCUS_CHANGE_DETECTOR,
    HIGH_LATENCY_VISIBLE_UPDATE_DETECTOR, JS_STALL_DURING_INTERACTION_DETECTOR,
    MEMORY_GREW_ACROSS_REPEATED_RESOURCE_SAMPLING_WINDOWS_DETECTOR,
    NETWORK_COMPLETION_FOLLOWED_BY_COMMIT_DETECTOR,
    REACT_COMMIT_OVERLAPPING_DELAYED_FRAME_DETECTOR, REPEATED_NETWORK_REQUEST_DETECTOR,
    REPEATED_REDUX_DISPATCH_DETECTOR, REPEATED_SELECTOR_RECOMPUTATION_DETECTOR, Severity,
    Threshold, UNSTABLE_SELECTOR_REFERENCE_DETECTOR,
    detect_excessive_commits_during_rapid_focus_movement,
    detect_high_cpu_sustained_during_resource_sampling, detect_high_latency_focus_changes,
    detect_high_latency_visible_updates, detect_js_stalls_overlapping_interactions,
    detect_memory_growth_across_repeated_resource_sampling_windows,
    detect_network_completions_followed_by_commits,
    detect_react_commits_overlapping_delayed_frames, detect_repeated_network_requests,
    detect_repeated_redux_dispatches, detect_repeated_selector_recomputation,
    detect_unstable_selector_references,
};
pub use interaction::{InteractionWindow, build_interaction_windows};
pub use resource_sampling::{ResourceSamplingWindow, build_resource_sampling_windows};
pub use selector_stats::{SelectorStats, selector_stats};
pub use session_metadata::session_metadata_from_events;
