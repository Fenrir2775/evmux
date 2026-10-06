use crate::config::profile::Profile;
use crate::config::rule::{CompiledMacro, KeyRule, RelativeAxisRule, Rule};
use crate::output::action::{Action, Actions};
use evdev::{EventType, InputEvent, KeyCode, RelativeAxisCode};
use smallvec::smallvec;
use std::sync::atomic::Ordering;

/// Evaluates a single input event against the active profiles rules.
///
/// Non-key events and unmatched events pass through unchanged.
pub(super) fn evaluate(event: InputEvent, profile: &Profile) -> Actions {
    match event.event_type() {
        EventType::KEY | EventType::RELATIVE => {
            for rule in &profile.rules {
                if let Some(actions) = match_rule(rule, event) {
                    return actions;
                }
            }
        }
        _ => return smallvec![Action::event(event)],
    }

    smallvec![Action::event(event)]
}

/// Returns the actions for the first rule that matches the `input_event`, or
/// `None` if the rule does not apply.
fn match_rule(rule: &Rule, input_event: InputEvent) -> Option<Actions> {
    Some(match rule {
        Rule::Key(key_rule) => {
            let event_key = KeyCode::new(input_event.code());
            let value = input_event.value();

            match key_rule {
                KeyRule::KeyToSingle { from, to } if event_key == *from => {
                    Actions::from(Action::key(*to, value))
                }
                KeyRule::KeyToMultiple { from, to } if event_key == *from => {
                    let keys = to.iter().copied();
                    let action = if value == 0 {
                        Action::keys(keys.rev(), value)
                    } else {
                        Action::keys(keys, value)
                    };

                    Actions::from(action)
                }
                KeyRule::Block { key } if event_key == *key => Actions::new(),
                KeyRule::Macro { from, r#macro } if event_key == *from => {
                    if value == 1 {
                        match r#macro {
                            CompiledMacro::Static(actions) => actions.iter().cloned().collect(),
                        }
                    } else {
                        Actions::new()
                    }
                }
                _ => return None,
            }
        }
        Rule::RelativeAxis(axis_rule) => {
            let event_axis = RelativeAxisCode(input_event.code());
            let value = input_event.value();

            match axis_rule {
                RelativeAxisRule::Invert(axis) if event_axis == *axis => {
                    Actions::from(Action::relative_axis(*axis, -value))
                }
                RelativeAxisRule::Swap { a, b } if event_axis == *a => {
                    Actions::from(Action::relative_axis(*b, value))
                }
                RelativeAxisRule::Swap { a, b } if event_axis == *b => {
                    Actions::from(Action::relative_axis(*a, value))
                }
                RelativeAxisRule::Scale {
                    axis,
                    factor,
                    remain,
                } if event_axis == *axis => {
                    let prev = f32::from_bits(remain.load(Ordering::Relaxed));
                    let scaled = value as f32 * factor + prev;
                    let emitted = scaled.round();
                    remain.store((scaled - emitted).to_bits(), Ordering::Relaxed);

                    Actions::from(Action::relative_axis(*axis, emitted as i32))
                }
                _ => return None,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::{KeyEvent, RelativeAxisEvent};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU32;

    #[test]
    fn key_to_single() {
        let rule = Rule::Key(KeyRule::KeyToSingle {
            from: KeyCode::KEY_A,
            to: KeyCode::KEY_B,
        });

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);

        let result = match_rule(&rule, event);
        let expected = Some(smallvec![Action::key(KeyCode::KEY_B, 1)]);

        assert_eq!(result, expected);
    }

    #[test]
    fn no_match() {
        let rule = Rule::Key(KeyRule::KeyToSingle {
            from: KeyCode::KEY_A,
            to: KeyCode::KEY_B,
        });

        let event = *KeyEvent::new(KeyCode::KEY_C, 1);

        let result = match_rule(&rule, event);

        assert_eq!(result, None);
    }

    #[test]
    fn key_to_multiple() {
        let rule = Rule::Key(KeyRule::KeyToMultiple {
            from: KeyCode::KEY_A,
            to: vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C],
        });

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);

        let result = match_rule(&rule, event);
        let expected = Some(smallvec![Action::keys(
            [KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C],
            1,
        )]);

        assert_eq!(result, expected);
    }

    #[test]
    fn block() {
        let rule = Rule::Key(KeyRule::Block {
            key: KeyCode::KEY_A,
        });

        let event = *KeyEvent::new(KeyCode::KEY_A, 1);

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
            rules: vec![Rule::Key(KeyRule::KeyToSingle {
                from: KeyCode::KEY_B,
                to: KeyCode::KEY_C,
            })],
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
            rules: vec![Rule::Key(KeyRule::Block {
                key: KeyCode::KEY_A,
            })],
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
                Rule::Key(KeyRule::KeyToSingle {
                    from: KeyCode::KEY_A,
                    to: KeyCode::KEY_B,
                }),
                Rule::Key(KeyRule::KeyToSingle {
                    from: KeyCode::KEY_A,
                    to: KeyCode::KEY_C,
                }),
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
        let rule = Rule::Key(KeyRule::Macro {
            from: KeyCode::KEY_A,
            r#macro: CompiledMacro::Static(Arc::from([
                Action::key(KeyCode::KEY_B, 1),
                Action::delay_from_millis(5),
                Action::key(KeyCode::KEY_B, 0),
            ])),
        });

        for value in 0..=2 {
            let result = match_rule(
                &rule,
                *KeyEvent::new(KeyCode::KEY_A, value),
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

    #[test]
    fn axis_invert_matching_axis() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Invert(RelativeAxisCode::REL_X));

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_X, 5);
        let result = match_rule(&rule, event);

        assert_eq!(
            result,
            Some(smallvec![Action::relative_axis(RelativeAxisCode::REL_X, -5)])
        );
    }

    #[test]
    fn axis_invert_no_match() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Invert(RelativeAxisCode::REL_X));

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_Y, 5);
        let result = match_rule(&rule, event);

        assert_eq!(result, None);
    }

    #[test]
    fn axis_swap_a_to_b() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Swap {
            a: RelativeAxisCode::REL_X,
            b: RelativeAxisCode::REL_Y,
        });

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_X, 7);
        let result = match_rule(&rule, event);

        assert_eq!(
            result,
            Some(smallvec![Action::relative_axis(RelativeAxisCode::REL_Y, 7)])
        );
    }

    #[test]
    fn axis_swap_b_to_a() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Swap {
            a: RelativeAxisCode::REL_X,
            b: RelativeAxisCode::REL_Y,
        });

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_Y, 7);
        let result = match_rule(&rule, event);

        assert_eq!(
            result,
            Some(smallvec![Action::relative_axis(RelativeAxisCode::REL_X, 7)])
        );
    }

    #[test]
    fn axis_swap_no_match() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Swap {
            a: RelativeAxisCode::REL_X,
            b: RelativeAxisCode::REL_Y,
        });

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_WHEEL, 7);
        let result = match_rule(&rule, event);

        assert_eq!(result, None);
    }

    #[test]
    fn axis_scale_halves_even_value() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Scale {
            axis: RelativeAxisCode::REL_X,
            factor: 0.5,
            remain: AtomicU32::new(0),
        });

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_X, 2);
        let result = match_rule(&rule, event);

        assert_eq!(
            result,
            Some(smallvec![Action::relative_axis(RelativeAxisCode::REL_X, 1)])
        );
    }

    #[test]
    fn axis_scale_no_match() {
        let rule = Rule::RelativeAxis(RelativeAxisRule::Scale {
            axis: RelativeAxisCode::REL_X,
            factor: 0.5,
            remain: AtomicU32::new(0),
        });

        let event = *RelativeAxisEvent::new(RelativeAxisCode::REL_Y, 1);
        let result = match_rule(&rule, event);

        assert_eq!(result, None);
    }
}
