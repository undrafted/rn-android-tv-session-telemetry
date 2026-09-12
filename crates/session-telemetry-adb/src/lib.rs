//! ADB device discovery, process supervision, and session retrieval.

mod devices;
mod retention;

pub use devices::{DeviceInfo, DeviceState, is_network_serial, parse_devices_output};
pub use retention::{prune_sealed_sessions_dir, sessions_to_prune};
