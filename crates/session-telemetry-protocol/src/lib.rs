//! Wire protocol and on-device session schema shared across the workspace.

mod event;

pub use event::{Event, FocusEvent, InteractionMarkerEvent, ReduxDispatchEvent, RemoteInputEvent};
