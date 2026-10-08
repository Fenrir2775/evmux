use crate::device::input_device::InputDevice;
use crate::output::action::{Action, ActionBatch};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Eq)]
struct QueuedAction {
    when: Instant,
    action: Action,
}

impl PartialEq for QueuedAction {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when
    }
}

impl PartialOrd for QueuedAction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Reversed compare for min-heap
impl Ord for QueuedAction {
    fn cmp(&self, other: &Self) -> Ordering {
        other.when.cmp(&self.when,)
    }
}

struct DeviceQueue {
    pending: BinaryHeap<QueuedAction>,
    blocked_until: Instant,
}

#[derive(Default)]
pub(crate) struct Scheduler {
    queues: HashMap<Arc<InputDevice>, DeviceQueue>,
}

impl Scheduler {
    pub(crate) fn push(&mut self, device: Arc<InputDevice>, batch: ActionBatch) {
        let now = Instant::now();
        let queue = self
            .queues
            .entry(device)
            .or_insert_with(|| DeviceQueue {
                pending: BinaryHeap::new(),
                blocked_until: now,
            });
        let mut when = now.max(queue.blocked_until);

        // build the queue
        for action in batch.actions {
            match action {
                Action::Emit(_) => {
                    queue.pending.push(QueuedAction { when, action });
                }
                Action::Delay(delay) => when += delay,
            }
        }

        // queue blocks only if macros blocking is set to true,
        // otherwise events get emitted instantly
        if batch.blocking {
            queue.blocked_until = when;
        }
    }

    /// Returns the next timeout for the receiver if any.
    pub(crate) fn next_timeout(&self) -> Option<Duration> {
        self.queues
            .values()
            .filter_map(|dq| dq.pending.peek())
            .map(|qa| qa.when)
            .min()
            .map(|due| due.saturating_duration_since(Instant::now()))
    }

    /// Get all upcoming events.
    pub(crate) fn get_upcoming(&mut self) -> Vec<Action> {
        let mut upcoming = vec![];

        for queue in self.queues.values_mut() {
            while let Some(queued) = queue.pending.peek() {
                if queued.when >= Instant::now() {
                    break;
                }
                upcoming.push(queue.pending.pop().unwrap().action);
            }
        }

        upcoming
    }
}
