//! ADB device discovery, process supervision, and session retrieval.

mod devices;

pub use devices::{DeviceInfo, DeviceState, is_network_serial, parse_devices_output};
