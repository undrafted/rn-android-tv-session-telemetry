//! Wire protocol and on-device session schema shared across the workspace.

mod event;

pub use event::{
    ClockSyncEvent, Event, FocusEvent, FrameTimingEvent, InteractionMarkerEvent, JsStallEvent,
    NetworkEvent, ReactCommitEvent, ReactCommitPhase, ReduxDispatchEvent, RemoteInputEvent,
    SessionMetadataEvent, VisibleUpdateEvent,
};
