use crate::device::input_device::InputDevice;
use crate::input::reader::reader_thread::ReaderThread;
use crate::input::reader::spawn_readers::spawn_readers;
use anyhow::Result;
use crossbeam_channel::Receiver;
use evdev::InputEvent;

/// Raw input stream for one logical device.
pub(in crate::input) struct RawEventStream {
    readers: Vec<ReaderThread>,
}

impl RawEventStream {
    /// Spawn every reader thread for every evdev node.
    /// Returns the stream and a channel receiver for incoming events.
    pub(in crate::input) fn open(device: &InputDevice) -> Result<(Self, Receiver<InputEvent>)> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let readers = spawn_readers(device, tx)?;
        Ok((Self { readers }, rx))
    }
}

impl Drop for RawEventStream {
    /// Stops all reader threads.
    fn drop(&mut self) {
        for reader in self.readers.drain(..) {
            reader.stop()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::input_device::enumerate_devices;

    #[test]
    fn dropping_closes_receiver() {
        let device = &enumerate_devices()[1];
        let (stream, rx) = RawEventStream::open(device).unwrap();

        // grabbing seems to generate events, ignore them
        while rx.try_recv().is_ok() {}

        drop(stream);

        assert!(rx.recv().is_err());
    }
}
