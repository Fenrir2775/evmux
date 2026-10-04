use evdev::{InputEvent, KeyCode, KeyEvent, RelativeAxisCode, RelativeAxisEvent};
use smallvec::{SmallVec, smallvec};
use std::time::Duration;

/// A common case sized vec.
pub(crate) type Actions = SmallVec<[Action; 12]>;

/// A batch of events.
type Events = SmallVec<[InputEvent; 4]>;

impl From<Action> for Actions {
    fn from(action: Action) -> Self {
        smallvec![action]
    }
}

/// A single output action.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum Action {
    Emit(Events),
    Delay(Duration),
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

    /// Creates a relative axis event in order:
    /// X, Y.
    pub(crate) fn relative_move(x: i32, y: i32) -> Self {
        Self::Emit(smallvec![
            *RelativeAxisEvent::new(RelativeAxisCode::REL_X, x),
            *RelativeAxisEvent::new(RelativeAxisCode::REL_Y, y),
        ])
    }

    /// Wrapper for Action::Delay(Duration::from_millis()).
    pub(crate) fn delay_from_millis(duration: u64) -> Self {
        Self::Delay(Duration::from_millis(duration))
    }
}
