use evdev::{InputEvent, KeyCode, KeyEvent, RelativeAxisCode, RelativeAxisEvent};
use smallvec::{SmallVec, smallvec};
use std::time::Duration;

/// A common case sized vec.
pub(crate) type Actions = SmallVec<[Action; 12]>;

/// A batch of events.
type Events = SmallVec<[InputEvent; 4]>;

/// A single output action.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum Action {
    Emit(Events),
    Delay(Duration),
}

impl From<Action> for Actions {
    fn from(action: Action) -> Self {
        smallvec![action]
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct ActionBatch {
    pub(crate) actions: Actions,
    pub(crate) blocking: bool,
}

impl From<Action> for ActionBatch {
    fn from(action: Action) -> Self {
        Self { actions: Actions::from(action), blocking: false }
    }
}

impl From<Actions> for ActionBatch {
    fn from(actions: Actions) -> Self {
        Self { actions, blocking: false }
    }
}

impl Action {
    /// Creates an [`Action::Emit`] from an `InputEvent`.
    pub(crate) fn event(event: InputEvent) -> Self {
        Self::Emit(smallvec![event])
    }

    /// Creates a single key action.
    pub(crate) fn key(key: KeyCode, value: i32) -> Self {
        Self::event(*KeyEvent::new(key, value))
    }

    /// Creates multiple key actions.
    pub(crate) fn keys(keys: impl IntoIterator<Item = KeyCode>, value: i32) -> Self {
        Self::Emit(
            keys.into_iter()
                .map(|key| *KeyEvent::new(key, value))
                .collect(),
        )
    }

    /// Creates a relative axis event.
    pub(crate) fn relative_axis(axis: RelativeAxisCode, value: i32) -> Self {
        Self::event(*RelativeAxisEvent::new(axis, value))
    }

    /// Wrapper for Action::Delay(Duration::from_millis()).
    pub(crate) fn delay_from_millis(duration: u64) -> Self {
        Self::Delay(Duration::from_millis(duration))
    }
}
