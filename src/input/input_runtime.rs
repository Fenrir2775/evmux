use crate::config::profile::Profile;
use crate::device::input_device::InputDevice;
use crate::input::reader::event_stream::RawEventStream;
use crate::input::rule_engine;
use crate::output::action::{Action, ActionBatch, Actions};
use anyhow::Result;
use crossbeam_channel::Sender;
use evdev::{EventType, KeyCode};
use std::collections::HashSet;
use std::sync::Arc;
use std::thread;
use std::thread::JoinHandle;

pub(crate) struct InputRuntime {
    stream: RawEventStream,
    process: JoinHandle<()>,
}

impl InputRuntime {
    pub(crate) fn start(
        device: &InputDevice,
        profile: Arc<Profile>,
        output_tx: Sender<(Arc<InputDevice>, ActionBatch)>,
    ) -> Result<Self> {
        let (stream, raw_rx) = RawEventStream::open(device)?;
        let device = Arc::new(device.clone());

        let process = thread::spawn(move || {
            let mut pressed_keys = HashSet::new();

            while let Ok(event) = raw_rx.recv() {
                let action_batch = rule_engine::evaluate(event, &profile);
                track_emitted_keys(&action_batch.actions, &mut pressed_keys);

                if output_tx.send((device.clone(), action_batch)).is_err() {
                    break;
                }
            }

            // release all pressed keys
            let actions: Actions = pressed_keys
                .iter()
                .map(|k| Action::key(KeyCode::new(*k), 0))
                .collect();

            output_tx.send((device.clone(), actions.into())).ok();
        });

        Ok(Self { stream, process })
    }

    pub(crate) fn stop(self) {
        drop(self.stream);

        if let Err(e) = self.process.join() {
            eprintln!("Process thread panicked: {e:?}");
        }
    }
}

/// Tracks keys as they are emitted.
fn track_emitted_keys(actions: &Actions, pressed_keys: &mut HashSet<u16>) {
    for action in actions {
        let Action::Emit(events) = action else {
            continue;
        };

        for event in events {
            if event.event_type() != EventType::KEY {
                continue;
            }

            match event.value() {
                0 => {
                    pressed_keys.remove(&event.code());
                }
                1 | 2 => {
                    pressed_keys.insert(event.code());
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smallvec::smallvec;

    #[test]
    fn track_remapped_keys() {
        let mut pressed_keys = HashSet::new();
        let actions = smallvec![
            Action::key(KeyCode::KEY_A, 1),
            Action::key(KeyCode::KEY_B, 1),
            Action::key(KeyCode::KEY_C, 1),
            Action::key(KeyCode::KEY_B, 0),
        ];

        track_emitted_keys(&actions, &mut pressed_keys);

        assert_eq!(
            pressed_keys,
            HashSet::from_iter([KeyCode::KEY_A.code(), KeyCode::KEY_C.code(),])
        );
    }
}
