use anyhow::Result;
use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, InputEvent, InputId, KeyCode, RelativeAxisCode};

/// The virtual output device.
pub(crate) struct OutputDevice {
    device: VirtualDevice,
}

impl OutputDevice {
    /// Creates the virtual device with the full evdev key range,
    /// and relative axes for movement and scrolling.
    ///
    /// Registered as a virtual device `evmux_virtual_device`.
    pub(crate) fn new() -> Result<Self> {
        let mut keys = AttributeSet::new();

        for i in 0..=0x2e7u16 {
            keys.insert(KeyCode::new(i))
        }

        let device = VirtualDevice::builder()?
            .name("evmux_virtual_device")
            .with_relative_axes(&AttributeSet::<RelativeAxisCode>::from_iter([
                RelativeAxisCode::REL_X,
                RelativeAxisCode::REL_Y,
                RelativeAxisCode::REL_WHEEL,
                RelativeAxisCode::REL_HWHEEL,
            ]))?
            .with_keys(&keys)?
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0, 0, 0))
            .build()?;

        Ok(Self { device })
    }

    /// Emits a batch of input events.
    pub(crate) fn emit(&mut self, events: &[InputEvent]) -> Result<()> {
        self.device.emit(events)?;

        Ok(())
    }
}
