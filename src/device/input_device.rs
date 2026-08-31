use evdev::{BusType, Device, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Groups the raw devices from `/dev/input` into a single logical device.
#[derive(Serialize, Deserialize, Debug, Clone, Default, Eq, Hash, PartialEq)]
pub(crate) struct InputDevice {
    name: String,
    vendor_id: u16,
    product_id: u16,
    physical_path: String,
}

impl InputDevice {
    fn new(name: String, vendor_id: u16, product_id: u16, physical_path: String) -> Self {
        Self {
            name,
            vendor_id,
            product_id,
            physical_path,
        }
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    pub(crate) fn product_id(&self) -> u16 {
        self.product_id
    }

    pub(crate) fn physical_path(&self) -> &str {
        &self.physical_path
    }

    /// Returns a list with all evdev devices which matches vendor/product id and physical path.
    pub(crate) fn matching_devices(&self) -> Vec<Device> {
        evdev::enumerate()
            .filter(|(_, d)| {
                is_input_device(d)
                && d.input_id().vendor() == self.vendor_id
                && d.input_id().product() == self.product_id
                && d.physical_path()
                    .map(|p| p.starts_with(&self.physical_path))
                    .unwrap_or(false)
            })
            .map(|(_, d)| d)
            .collect()
    }
}

#[cfg(test)]
impl InputDevice {
    pub(crate) fn new_for_test(vendor_id: u16, product_id: u16, name: &str) -> Self {
        Self::new(
            name.to_string(),
            vendor_id,
            product_id,
            format!("test-path-{name}"),
        )
    }
}

/// Checks if the given device is an input device.
fn is_input_device(device: &Device) -> bool {
    if device.input_id().bus_type() == BusType::BUS_VIRTUAL {
        return false;
    }
    if let Some(r_axes) = device.supported_relative_axes() {
        return r_axes.contains(RelativeAxisCode::REL_X)
            || r_axes.contains(RelativeAxisCode::REL_WHEEL);
    }
    if let Some(keys) = device.supported_keys() {
        return keys.contains(KeyCode::KEY_A) || keys.contains(KeyCode::BTN_LEFT);
    }

    false
}

/// Find the longest common name out of the given list.
fn longest_common_name(names: &[&str]) -> String {
    if names.is_empty() {
        return String::new();
    }

    let first_words: Vec<&str> = names[0].split_whitespace().collect();

    let common_name = first_words
        .iter()
        .enumerate()
        .take_while(|(i, word)| {
            names
                .iter()
                .all(|name| name.split_whitespace().nth(*i) == Some(**word))
        })
        .map(|(_, word)| *word)
        .collect::<Vec<_>>();

    common_name.join(" ")
}

/// Scans `/dev/input` and returns a deduplicated list of logical input devices.
pub(crate) fn enumerate_devices() -> Vec<InputDevice> {
    let mut grouped: HashMap<(u16, u16, String), Vec<String>> = HashMap::new();

    for (_, d) in evdev::enumerate() {
        if !is_input_device(&d) {
            continue;
        }

        let ids = d.input_id();

        // just use the first part of the path
        // usb-0000:2f:00.3-4/input3 -> usb-0000:2f:00.3-4
        let prefix = d
            .physical_path()
            .and_then(|s| s.rsplit_once('/'))
            .map(|(prefix, _)| prefix.to_string())
            .unwrap_or_default();

        // groups every evdev device, identified by its vendor/product id and physical path
        grouped
            .entry((ids.vendor(), ids.product(), prefix))
            .or_default()
            .push(d.name().unwrap_or("Unknown Device").to_string());
    }

    grouped
        .into_iter()
        .map(|((vendor_id, product_id, prefix), names)| {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            InputDevice::new(longest_common_name(&names), vendor_id, product_id, prefix)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_longest_common_name() {
        let names = vec![
            "Razer Naga Trinity Mouse",
            "Razer Naga Trinity Keyboard",
            "Razer Naga Trinity",
        ];
        let common_name = "Razer Naga Trinity";
        let result = longest_common_name(&names);

        assert_eq!(result, common_name);
    }

    #[test]
    fn test_enumerate_devices() {
        let devices = enumerate_devices();
        assert!(!devices.is_empty());

        println!("{:?}", devices)
    }
}
