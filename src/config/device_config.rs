use crate::device::input_device::InputDevice;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(crate) struct DeviceConfig {
    pub(crate) device: InputDevice,
    pub(crate) default_profile: Option<String>,
}
