use crate::device::input_device::InputDevice;
use crate::input::reader::event_stream::RawEventStream;
use anyhow::{Context, Result};
use evdev::{EventType, KeyCode};

pub(crate) fn record_keypress(device: &InputDevice) -> Result<KeyCode> {
    eprintln!("record_keypress: {device:?}");

    let (_stream, rx) = RawEventStream::open(device)?;

    eprintln!("Press a key on '{}' (waiting for input...)", device.name());

    let mut pending = None;

    loop {
        let event = rx.recv().context("Failed to receive event")?;

        if event.event_type() != EventType::KEY {
            continue;
        }

        let key = KeyCode::new(event.code());

        match event.value() {
            1 => pending = Some(key),
            0 if pending == Some(key) => return Ok(key),
            _ => {}
        }
    }
}
