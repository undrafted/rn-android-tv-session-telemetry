//! Wire protocol and on-device session schema shared across the workspace.

mod event;

pub use event::{
    ClockSyncEvent, Event, FocusEvent, FrameTimingEvent, InteractionMarkerEvent, JsStallEvent,
    LifecycleEvent, LifecycleState, NetworkEvent, ReactCommitEvent, ReactCommitPhase,
    ReduxDispatchEvent, RemoteInputEvent, ResourceSampleEvent, ResourceSamplingStartedEvent,
    ResourceSamplingStoppedEvent, SelectorEvent, SessionMetadataEvent, VisibleUpdateEvent,
};
