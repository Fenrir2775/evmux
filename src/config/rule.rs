use crate::config::macro_action::MacroAction;
use crate::config::serde::*;
use evdev::KeyCode;
use serde::{Deserialize, Serialize};

/// Define the rules how an input event is handled.
#[derive(Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Rule {
    /// Maps a single key to another single key.
    KeyToSingle {
        #[serde(with = "keycode_serde")]
        from: KeyCode,
        #[serde(with = "keycode_serde")]
        to: KeyCode,
    },
    /// Maps a single key to a sequence of keys.
    KeyToMultiple {
        #[serde(with = "keycode_serde")]
        from: KeyCode,
        #[serde(with = "keycode_vec_serde")]
        to: Vec<KeyCode>,
    },
    /// Suppress the specified key.
    Block {
        #[serde(with = "keycode_serde")]
        key: KeyCode,
    },
    /// Maps a key to a macro.
    Macro {
        #[serde(with = "keycode_serde")]
        from: KeyCode,
        macro_actions: Vec<MacroAction>,
    },
}
