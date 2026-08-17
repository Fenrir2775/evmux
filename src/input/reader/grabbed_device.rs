use anyhow::Result;
use evdev::{Device, FetchEventsSynced};
use std::io;

/// An evdev device held in grabbed, non-blocking state.
pub(in crate::input) struct GrabbedDevice {
    device: Device,
}

impl GrabbedDevice {
    /// Grabs the `device` and switches it to non-blocking mode.
    pub(in crate::input) fn new(mut device: Device) -> Result<Self> {
        device.set_nonblocking(true)?;
        device.grab()?;

        Ok(Self { device })
    }

    pub(in crate::input) fn fetch_events(&mut self) -> io::Result<FetchEventsSynced<'_>> {
        self.device.fetch_events()
    }
}

impl Drop for GrabbedDevice {
    /// Ungrabs the device.
    fn drop(&mut self) {
        self.device.ungrab().ok();
        self.device.set_nonblocking(false).ok();
    }
}
