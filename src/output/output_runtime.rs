use crate::device::input_device::InputDevice;
use crate::device::output_device::OutputDevice;
use crate::output::action::{Action, ActionBatch};
use crate::output::scheduler::Scheduler;
use anyhow::Result;
use crossbeam_channel::Sender;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

/// The applications output runtime
pub(crate) struct OutputRuntime {
    output_tx: Sender<(Arc<InputDevice>, ActionBatch)>,
    handle: JoinHandle<()>,
}

impl OutputRuntime {
    /// Creates the virtual [`OutputDevice`] and starts the output thread.
    pub(crate) fn new() -> Result<Self> {
        let (output_tx, output_rx) = crossbeam_channel::bounded(1024);
        let mut device = OutputDevice::new()?;

        let handle = thread::spawn(move || {
            let mut scheduler = Scheduler::default();

            loop {
                let timeout = scheduler
                    .next_timeout()
                    .unwrap_or(Duration::from_millis(50));

                match output_rx.recv_timeout(timeout) {
                    Ok((input_device, action)) => scheduler.push(input_device, action),
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                }

                for action in scheduler.get_upcoming() {
                    if let Action::Emit(events) = action
                        && let Err(e) = device.emit(&events)
                    {
                        eprintln!("Error emitting events: {e:?}");
                    }
                }
            }
        });

        Ok(Self { output_tx, handle })
    }

    pub(crate) fn sender(&self) -> Sender<(Arc<InputDevice>, ActionBatch)> {
        self.output_tx.clone()
    }
}
