use nix::sys::eventfd::EventFd;
use std::sync::Arc;
use std::thread::JoinHandle;

/// Handle to an evdev reader thread.
pub(in crate::input) struct ReaderThread {
    join_handle: JoinHandle<()>,
    stop_event: Arc<EventFd>,
}

impl ReaderThread {
    pub(in crate::input) fn new(join_handle: JoinHandle<()>, stop_event: Arc<EventFd>) -> Self {
        Self {
            join_handle,
            stop_event,
        }
    }

    /// Signals the thread to stop via eventfd and waits for it to finish.
    pub(in crate::input) fn stop(self) {
        if let Err(err) = self.stop_event.write(1) {
            eprintln!("Failed to signal reader thread stop: {err}");
        }
        if let Err(err) = self.join_handle.join() {
            eprintln!("Reader thread panicked: {err:?}");
        }
    }
}
