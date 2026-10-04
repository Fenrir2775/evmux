use crate::config::macros::MacroAction;
use crate::output::action::{Action, Actions};
use std::sync::Arc;
use std::time::Duration;

/// Compiles a list of [`MacroAction`]s into a [`Actions`] sequence.
/// `Click` expands into a `Press`, a 5ms delay and a `Release`.
/// Everything else maps one to one to its [`Action`] equivalent.
pub(crate) fn compile(actions: &[MacroAction]) -> Actions {
    let mut sequence = Actions::new();

    for action in actions {
        match action {
            MacroAction::Press(key) => {
                sequence.push(Action::key(*key, 1));
            }
            MacroAction::Release(key) => {
                sequence.push(Action::key(*key, 0));
            }
            MacroAction::Click(key) => {
                sequence.push(Action::key(*key, 1));
                sequence.push(Action::Delay(Duration::from_millis(5)));
                sequence.push(Action::key(*key, 0));
            }
            MacroAction::MoveRelative { x, y } => {
                sequence.push(Action::relative_move(*x, *y));
            }
            MacroAction::Delay(duration) => {
                sequence.push(Action::Delay(Duration::from_millis(*duration)));
            }
        }
    }

    sequence
}

pub(crate) fn compile_to_arc(actions: &[MacroAction]) -> Arc<[Action]> {
    Arc::from(compile(actions).into_vec())
}

#[cfg(test)]
mod test {
    use super::*;
    use evdev::KeyCode;
    use smallvec::smallvec;

    #[test]
    fn press_release() {
        let macro_actions = vec![
            MacroAction::Press(KeyCode::KEY_LEFTCTRL),
            MacroAction::Release(KeyCode::KEY_LEFTCTRL),
        ];

        let result = compile(&macro_actions);

        let actions: Actions = smallvec![
            Action::key(KeyCode::KEY_LEFTCTRL, 1),
            Action::key(KeyCode::KEY_LEFTCTRL, 0),
        ];

        assert_eq!(result, actions);
    }

    #[test]
    fn click() {
        let macro_actions = vec![MacroAction::Click(KeyCode::KEY_ENTER)];
        let result = compile(&macro_actions);
        let actions: Actions = smallvec![
            Action::key(KeyCode::KEY_ENTER, 1),
            Action::delay_from_millis(5),
            Action::key(KeyCode::KEY_ENTER, 0),
        ];

        assert_eq!(result, actions);
    }

    #[test]
    fn move_relative() {
        let macro_actions = vec![MacroAction::MoveRelative { x: 100, y: 100 }];
        let result = compile(&macro_actions);
        let actions: Actions = smallvec![Action::relative_move(100, 100)];

        assert_eq!(result, actions);
    }

    #[test]
    fn delay() {
        let macro_actions = vec![MacroAction::Delay(250)];
        let result = compile(&macro_actions);
        let actions: Actions = smallvec![Action::delay_from_millis(250)];

        assert_eq!(result, actions);
    }
}
