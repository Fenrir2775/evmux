use crate::device::output_device::OutputDevice;
use crate::output::action::{Action, Actions};
use anyhow::Result;
use crossbeam_channel::Sender;
use std::thread;
use std::thread::JoinHandle;

/// The applications output runtime
pub(crate) struct OutputRuntime {
    output_tx: Sender<Actions>,
    handle: JoinHandle<()>,
}

impl OutputRuntime {
    /// Creates the virtual [`OutputDevice`] and starts the output thread.
    ///
    /// The thread processes actions from the channel in order.
    /// `Delay` actions block the thread intentionally.
    pub(crate) fn new() -> Result<Self> {
        let (output_tx, output_rx) = crossbeam_channel::bounded(1024);
        let mut device = OutputDevice::new()?;

        let handle = thread::spawn(move || {
            while let Ok(actions) = output_rx.recv() {
                for action in actions {
                    match action {
                        Action::Emit(events) => {
                            if let Err(e) = device.emit(&events) {
                                eprintln!("Error emitting events: {e:?}");
                            }
                        }
                        Action::Delay(delay) => {
                            thread::sleep(delay);
                        }
                    }
                }
            }
        });

        Ok(Self { output_tx, handle })
    }

    pub(crate) fn sender(&self) -> Sender<Actions> {
        self.output_tx.clone()
    }
}
