//! Generates the deterministic fixture session at fixtures/sessions/catalog-navigation.json.
//! Run with: cargo run -p session-telemetry-session --example generate_fixture
//!
//! Two interactions: a quick, unremarkable focus move, and a slow one matching the 214ms
//! example trace in plan.md section 1 exactly, so `session-telemetry analyze` on this fixture
//! produces one real finding.

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

    // Interaction B: matches plan.md section 1's example trace, 214ms to focus change.
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

    let chunk = writer.seal().expect("writer is non-empty");
    println!(
        "{}",
        serde_json::to_string_pretty(&chunk).expect("Chunk is serializable")
    );
}
