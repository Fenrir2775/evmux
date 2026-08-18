use crate::config::serde::*;
use evdev::KeyCode;
use serde::{Deserialize, Serialize};

/// All actions which can be used in macro.
#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum MacroAction {
    /// Single key press.
    #[serde(with = "keycode_serde")]
    Press(KeyCode),
    /// Single key release.
    #[serde(with = "keycode_serde")]
    Release(KeyCode),
    /// Single key click (press/release).
    #[serde(with = "keycode_serde")]
    Click(KeyCode),
    /// A relative mouse move.
    MoveRelative { x: i32, y: i32 },
    /// Delay in milliseconds.
    Delay(u64),
}
