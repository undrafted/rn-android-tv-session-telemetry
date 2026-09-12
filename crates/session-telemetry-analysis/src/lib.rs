//! Clock mapping, interaction windows, metrics, and deterministic detectors.

mod clock;
mod detector;
mod interaction;

pub use clock::{ClockMap, ClockSyncSample};
pub use detector::{
    Finding, HIGH_LATENCY_FOCUS_CHANGE_DETECTOR, HIGH_LATENCY_FOCUS_CHANGE_DETECTOR_VERSION,
    REPEATED_REDUX_DISPATCH_DETECTOR, REPEATED_REDUX_DISPATCH_DETECTOR_VERSION, Severity,
    detect_high_latency_focus_changes, detect_repeated_redux_dispatches,
};
pub use interaction::{InteractionWindow, build_interaction_windows};
