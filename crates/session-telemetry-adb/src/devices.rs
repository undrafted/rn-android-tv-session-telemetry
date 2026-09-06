use serde::Serialize;

/// One row of `adb devices -l` output.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceInfo {
    pub serial: String,
    pub state: DeviceState,
    pub product: Option<String>,
    pub model: Option<String>,
    pub device: Option<String>,
    pub transport_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceState {
    Device,
    Offline,
    Unauthorized,
    /// Anything ADB reports that isn't one of the above (e.g. "no permissions").
    Other(String),
}

impl DeviceState {
    fn parse(raw: &str) -> Self {
        match raw {
            "device" => DeviceState::Device,
            "offline" => DeviceState::Offline,
            "unauthorized" => DeviceState::Unauthorized,
            other => DeviceState::Other(other.to_string()),
        }
    }
}

/// Parses the text `adb devices -l` prints to stdout. Not exercised against a real `adb`
/// binary yet — this machine doesn't have the Android toolchain (plan.md's Week 1 gate) — but
/// the format itself is stable ADB CLI output, so the parsing logic is real and testable
/// independent of that.
pub fn parse_devices_output(output: &str) -> Vec<DeviceInfo> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && *line != "List of devices attached")
        .map(parse_device_line)
        .collect()
}

fn parse_device_line(line: &str) -> DeviceInfo {
    let mut tokens = line.split_whitespace();
    let serial = tokens.next().unwrap_or_default().to_string();

    // The state isn't always one word — ADB reports things like "no permissions" on Linux
    // udev issues. Key:value metadata (product:x, transport_id:5, ...) always contains a
    // colon, so anything without one is treated as (part of) the state instead.
    let mut state_words = Vec::new();
    let mut product = None;
    let mut model = None;
    let mut device = None;
    let mut transport_id = None;

    for token in tokens {
        if let Some((key, value)) = token.split_once(':') {
            match key {
                "product" => product = Some(value.to_string()),
                "model" => model = Some(value.to_string()),
                "device" => device = Some(value.to_string()),
                "transport_id" => transport_id = Some(value.to_string()),
                // "usb:1-1" and any future keys: not part of DeviceInfo yet, ignored rather
                // than guessed at.
                _ => {}
            }
        } else {
            state_words.push(token);
        }
    }

    let state = DeviceState::parse(&state_words.join(" "));

    DeviceInfo {
        serial,
        state,
        product,
        model,
        device,
        transport_id,
    }
}

/// True for a `host:port` serial — a network-connected Android TV, the form the CLI's
/// `--device 192.168.1.40:5555` expects (plan.md section 5) — false for a USB serial or
/// emulator id.
pub fn is_network_serial(serial: &str) -> bool {
    match serial.rsplit_once(':') {
        Some((host, port)) => {
            !host.is_empty() && !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_realistic_devices_l_listing() {
        let output = "\
List of devices attached
192.168.1.40:5555     device product:walleye model:Pixel_2 device:walleye transport_id:2
0a388e93               device usb:1-1 product:sunfish model:Pixel_4a device:sunfish transport_id:1
emulator-5554          offline
unauth-serial          unauthorized

";

        let devices = parse_devices_output(output);

        assert_eq!(
            devices,
            vec![
                DeviceInfo {
                    serial: "192.168.1.40:5555".to_string(),
                    state: DeviceState::Device,
                    product: Some("walleye".to_string()),
                    model: Some("Pixel_2".to_string()),
                    device: Some("walleye".to_string()),
                    transport_id: Some("2".to_string()),
                },
                DeviceInfo {
                    serial: "0a388e93".to_string(),
                    state: DeviceState::Device,
                    product: Some("sunfish".to_string()),
                    model: Some("Pixel_4a".to_string()),
                    device: Some("sunfish".to_string()),
                    transport_id: Some("1".to_string()),
                },
                DeviceInfo {
                    serial: "emulator-5554".to_string(),
                    state: DeviceState::Offline,
                    product: None,
                    model: None,
                    device: None,
                    transport_id: None,
                },
                DeviceInfo {
                    serial: "unauth-serial".to_string(),
                    state: DeviceState::Unauthorized,
                    product: None,
                    model: None,
                    device: None,
                    transport_id: None,
                },
            ]
        );
    }

    #[test]
    fn no_devices_attached_yields_an_empty_list() {
        assert_eq!(parse_devices_output("List of devices attached\n\n"), vec![]);
    }

    #[test]
    fn unrecognized_multi_word_state_is_preserved_in_full() {
        let devices = parse_devices_output("List of devices attached\nabc123    no permissions\n");

        assert_eq!(
            devices[0].state,
            DeviceState::Other("no permissions".to_string())
        );
    }

    #[test]
    fn network_serials_are_detected() {
        assert!(is_network_serial("192.168.1.40:5555"));
        assert!(!is_network_serial("0a388e93"));
        assert!(!is_network_serial("emulator-5554"));
        assert!(!is_network_serial(":5555"));
    }
}
