use crate::config::profile::Profile;
use crate::config::rule::{CompiledMacro, Rule};
use crate::output::action::{Action, Actions};
use evdev::{EventType, InputEvent, KeyCode};
use smallvec::smallvec;

#[derive(Debug, Clone, Copy)]
struct InputKey {
    key: KeyCode,
    value: i32,
}

/// Evaluates a single input event against the active profiles rules.
///
/// Non-key events and unmatched events pass through unchanged.
pub(super) fn evaluate(event: InputEvent, profile: &Profile) -> Actions {
    if event.event_type() != EventType::KEY {
        return smallvec![Action::event(event)];
    }

    let key_event = InputKey {
        key: KeyCode::new(event.code()),
        value: event.value(),
    };

    for rule in &profile.rules {
        if let Some(actions) = match_rule(rule, key_event) {
            return actions;
        }
    }

    smallvec![Action::event(event)]
}

/// Returns the actions for the first rule that matches `key_event`, or
/// `None` if the rule does not apply.
fn match_rule(rule: &Rule, key_event: InputKey) -> Option<Actions> {
    match rule {
        Rule::KeyToSingle { from, to } if key_event.key == *from => {
            Some(Actions::from(Action::key(*to, key_event.value)))
        }

        Rule::KeyToMultiple { from, to } if key_event.key == *from => {
            let keys = to.iter().copied();
            let action = if key_event.value == 0 {
                Action::keys(keys.rev(), key_event.value)
            } else {
                Action::keys(keys, key_event.value)
            };
            Some(Actions::from(action))
        }

        Rule::Block { key } if key_event.key == *key => Some(Actions::new()),

        Rule::Macro { from, r#macro } if key_event.key == *from => Some(if key_event.value == 1 {
            match r#macro {
                CompiledMacro::Static(actions) => actions.iter().cloned().collect(),
            }
        } else {
            Actions::new()
        }),

        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::KeyEvent;
    use std::sync::Arc;

    #[test]
    fn key_to_single() {
        let rule = Rule::KeyToSingle {
            from: KeyCode::KEY_A,
            to: KeyCode::KEY_B,
        };

        let event = InputKey {
            key: KeyCode::KEY_A,
            value: 1,
        };

        let result = match_rule(&rule, event);
        let expected = Some(smallvec![Action::key(KeyCode::KEY_B, 1)]);

        assert_eq!(result, expected);
    }

    #[test]
    fn no_match() {
        let rule = Rule::KeyToSingle {
            from: KeyCode::KEY_A,
            to: KeyCode::KEY_B,
        };

        let event = InputKey {
            key: KeyCode::KEY_C,
            value: 1,
        };

        let result = match_rule(&rule, event);

        assert_eq!(result, None);
    }

    #[test]
    fn key_to_multiple() {
        let rule = Rule::KeyToMultiple {
            from: KeyCode::KEY_A,
            to: vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C],
        };

        let event = InputKey {
            key: KeyCode::KEY_A,
            value: 1,
        };

        let result = match_rule(&rule, event);
        let expected = Some(smallvec![Action::keys(
            [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C],
            1,
        )]);

        assert_eq!(result, expected);
    }

    #[test]
    fn block() {
        let rule = Rule::Block {
            key: KeyCode::KEY_A,
        };

        let event = InputKey {
            key: KeyCode::KEY_A,
            value: 1,
        };

        let result = match_rule(&rule, event);
        let expected = Some(Actions::new());

        assert_eq!(result, expected);
    }

    #[test]
    fn passthrough_without_profile() {
        let event = *KeyEvent::new(KeyCode::KEY_A, 1);
        let profile = Profile::default();
        let result = evaluate(event, &profile);

        let expected: Actions = smallvec![Action::event(event)];

        assert_eq!(result, expected);
    }

    #[test]
    fn passthrough_no_match() {
        let profile = Profile {
            rules: vec![Rule::KeyToSingle {
                from: KeyCode::KEY_B,
                to: KeyCode::KEY_C,
            }],
            ..Default::default()
        };

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);
        let result = evaluate(event, &profile);
        let expected: Actions = smallvec![Action::event(event)];

        assert_eq!(result, expected);
    }

    #[test]
    fn event_blocked() {
        let profile = Profile {
            rules: vec![Rule::Block {
                key: KeyCode::KEY_A,
            }],
            ..Default::default()
        };

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);
        let result = evaluate(event, &profile);

        assert!(result.is_empty())
    }

    #[test]
    fn first_match() {
        let profile = Profile {
            rules: vec![
                Rule::KeyToSingle {
                    from: KeyCode::KEY_A,
                    to: KeyCode::KEY_B,
                },
                Rule::KeyToSingle {
                    from: KeyCode::KEY_A,
                    to: KeyCode::KEY_C,
                },
            ],
            ..Default::default()
        };

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);
        let result = evaluate(event, &profile);
        let expected: Actions = smallvec![Action::key(KeyCode::KEY_B, 1)];

        assert_eq!(result, expected);
    }

    #[test]
    fn macros_triggered_key_pressed() {
        let rule = Rule::Macro {
            from: KeyCode::KEY_A,
            r#macro: CompiledMacro::Static(Arc::from([
                Action::key(KeyCode::KEY_B, 1),
                Action::delay_from_millis(5),
                Action::key(KeyCode::KEY_B, 0),
            ])),
        };

        for value in 0..=2 {
            let result = match_rule(
                &rule,
                InputKey {
                    key: KeyCode::KEY_A,
                    value,
                },
            );

            if value != 1 {
                assert_eq!(result, Some(Actions::new()));
            } else {
                assert_eq!(
                    result,
                    Some(smallvec![
                        Action::key(KeyCode::KEY_B, 1),
                        Action::delay_from_millis(5),
                        Action::key(KeyCode::KEY_B, 0)
                    ])
                );
            }
        }
    }
}
