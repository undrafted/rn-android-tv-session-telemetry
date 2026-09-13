//! Generates the deterministic fixture session at fixtures/sessions/catalog-navigation.json.
//! Run with: cargo run -p session-telemetry-session --example generate_fixture
//!
//! Three interactions: a quick, unremarkable focus move; a slow one taking 214ms to reach focus
//! change; and a rapid-fire burst that redundantly dispatches the same Redux action three
//! times. `session-telemetry analyze` on this fixture should produce exactly two findings, one
//! from each of the two detectors.

use session_telemetry_protocol::{
    Event, FocusEvent, InteractionMarkerEvent, ReduxDispatchEvent, RemoteInputEvent,
};
use session_telemetry_session::ChunkWriter;

fn main() {
    let mut writer = ChunkWriter::new();

    // Interaction A: fast, no finding expected.
    writer.push(Event::RemoteInput(RemoteInputEvent {
        sequence: 0,
        timestamp: 0.0,
        key: "up".to_string(),
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 1,
        timestamp: 2.0,
        action_type: "nav/moveUp".to_string(),
        duration_ms: 0.8,
    }));
    writer.push(Event::Focus(FocusEvent {
        sequence: 2,
        timestamp: 18.0,
        target_id: "card-0".to_string(),
        previous_target_id: Some("card-1".to_string()),
    }));

    // Interaction B: slow, 214ms to focus change.
    writer.push(Event::RemoteInput(RemoteInputEvent {
        sequence: 3,
        timestamp: 500.0,
        key: "right".to_string(),
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 4,
        timestamp: 504.0,
        action_type: "catalog/itemFocused".to_string(),
        duration_ms: 1.2,
    }));
    writer.push(Event::InteractionMarker(InteractionMarkerEvent {
        sequence: 5,
        timestamp: 510.0,
        name: "demo:card-select".to_string(),
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 6,
        timestamp: 610.0,
        action_type: "catalog/itemLoaded".to_string(),
        duration_ms: 2.5,
    }));
    writer.push(Event::Focus(FocusEvent {
        sequence: 7,
        timestamp: 714.0,
        target_id: "card-2".to_string(),
        previous_target_id: Some("card-0".to_string()),
    }));

    // Interaction C: rapid-fire input redundantly dispatches the same action three times before
    // focus settles — fast overall (50ms, no latency finding), but should trip the repeated-
    // dispatch detector.
    writer.push(Event::RemoteInput(RemoteInputEvent {
        sequence: 8,
        timestamp: 800.0,
        key: "right".to_string(),
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 9,
        timestamp: 802.0,
        action_type: "catalog/itemFocused".to_string(),
        duration_ms: 1.0,
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 10,
        timestamp: 804.0,
        action_type: "catalog/itemFocused".to_string(),
        duration_ms: 1.0,
    }));
    writer.push(Event::ReduxDispatch(ReduxDispatchEvent {
        sequence: 11,
        timestamp: 806.0,
        action_type: "catalog/itemFocused".to_string(),
        duration_ms: 1.0,
    }));
    writer.push(Event::Focus(FocusEvent {
        sequence: 12,
        timestamp: 850.0,
        target_id: "card-3".to_string(),
        previous_target_id: Some("card-2".to_string()),
    }));

    let chunk = writer.seal().expect("writer is non-empty");
    println!(
        "{}",
        serde_json::to_string_pretty(&chunk).expect("Chunk is serializable")
    );
}
