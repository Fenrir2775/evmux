use crate::config::macros::Macros;
use crate::config::serde::raw::{RawKeyRule, RawRelativeAxisRule, RawRule};
use crate::output::action::Action;
use anyhow::{Result, anyhow};
use evdev::{KeyCode, RelativeAxisCode};
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

pub(crate) enum CompiledMacro {
    Static(Arc<[Action]>),
}

/// Define the rules how an input event is handled.
pub(crate) enum Rule {
    Key(KeyRule),
    RelativeAxis(RelativeAxisRule),
}

pub(crate) enum KeyRule {
    /// Maps a single key to another single key.
    KeyToSingle { from: KeyCode, to: KeyCode },
    /// Maps a single key to a sequence of keys.
    KeyToMultiple { from: KeyCode, to: Vec<KeyCode> },
    /// Suppress the specified key.
    Block { key: KeyCode },
    /// Maps a key to a macro.
    Macro {
        from: KeyCode,
        r#macro: CompiledMacro,
    },
}

pub(crate) enum RelativeAxisRule {
    /// Invert a relative axis.
    Invert(RelativeAxisCode),
    /// Swap an axis with another.
    Swap {
        a: RelativeAxisCode,
        b: RelativeAxisCode,
    },
    /// Scale an axis by the specified `factor`.
    Scale {
        axis: RelativeAxisCode,
        factor: f32,
        remain: AtomicU32,
    },
}

impl Rule {
    pub(crate) fn resolve(raw_rule: RawRule, macros: &Macros) -> Result<Self> {
        Ok(match raw_rule {
            RawRule::Key(key_rule) => match key_rule {
                RawKeyRule::KeyToSingle { from, to } => {
                    Self::Key(KeyRule::KeyToSingle { from, to })
                }
                RawKeyRule::KeyToMultiple { from, to } => {
                    Self::Key(KeyRule::KeyToMultiple { from, to })
                }
                RawKeyRule::Block { key } => Self::Key(KeyRule::Block { key }),
                RawKeyRule::Macro { from, name } => {
                    let actions = macros
                        .get(&name)
                        .ok_or_else(|| anyhow!("macro '{name}' not found"))?;

                    Self::Key(KeyRule::Macro {
                        from,
                        r#macro: CompiledMacro::Static(actions),
                    })
                }
            },
            RawRule::RelativeAxis(axis_rule) => match axis_rule {
                RawRelativeAxisRule::Invert(axis) => {
                    Self::RelativeAxis(RelativeAxisRule::Invert(axis))
                }
                RawRelativeAxisRule::Swap { a, b } => {
                    Self::RelativeAxis(RelativeAxisRule::Swap { a, b })
                }
                RawRelativeAxisRule::Scale { axis, factor } => {
                    if factor <= 0.0 || !factor.is_finite() {
                        anyhow::bail!(
                            "invalid scale factor {factor} for axis {axis:?}: instead of negative scaling, use `invert`"
                        );
                    }

                    Self::RelativeAxis(RelativeAxisRule::Scale {
                        axis,
                        factor,
                        remain: AtomicU32::new(0),
                    })
                }
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::config::macro_compiler;
    use crate::config::serde::raw::MacroAction;
    use super::*;

    fn macros_with(name: &str, actions: &[MacroAction]) -> Macros {
        let mut macros = Macros::default();
        macros.insert_for_test(name, macro_compiler::compile_to_arc(actions));
        macros
    }

    #[test]
    fn resolve_key_to_single() {
        let raw = RawRule::Key(RawKeyRule::KeyToSingle {
            from: KeyCode::KEY_A,
            to: KeyCode::KEY_B,
        });

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(
            rule,
            Rule::Key(KeyRule::KeyToSingle { from, to })
                if from == KeyCode::KEY_A && to == KeyCode::KEY_B
        ));
    }

    #[test]
    fn resolve_key_to_multiple() {
        let raw = RawRule::Key(RawKeyRule::KeyToMultiple {
            from: KeyCode::KEY_A,
            to: vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C],
        });

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(
            rule,
            Rule::Key(KeyRule::KeyToMultiple { from, to })
                if from == KeyCode::KEY_A && to == vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_C]
        ));
    }

    #[test]
    fn resolve_block() {
        let raw = RawRule::Key(RawKeyRule::Block { key: KeyCode::KEY_A });

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(rule, Rule::Key(KeyRule::Block { key }) if key == KeyCode::KEY_A));
    }

    #[test]
    fn resolve_macro() {
        let macros = macros_with("combo", &[MacroAction::Press(KeyCode::KEY_B)]);
        let raw = RawRule::Key(RawKeyRule::Macro {
            from: KeyCode::KEY_A,
            name: "combo".into(),
        });

        let rule = Rule::resolve(raw, &macros).unwrap();

        assert!(matches!(
            rule,
            Rule::Key(KeyRule::Macro { from, r#macro: CompiledMacro::Static(actions) })
                if from == KeyCode::KEY_A && !actions.is_empty()
        ));
    }

    #[test]
    fn reject_macro_when_missing() {
        let raw = RawRule::Key(RawKeyRule::Macro {
            from: KeyCode::KEY_A,
            name: "missing".into(),
        });

        let result = Rule::resolve(raw, &Macros::default());

        assert!(result.is_err());
    }

    #[test]
    fn resolve_invert() {
        let raw = RawRule::RelativeAxis(RawRelativeAxisRule::Invert(RelativeAxisCode::REL_X));

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(
            rule,
            Rule::RelativeAxis(RelativeAxisRule::Invert(axis)) if axis == RelativeAxisCode::REL_X
        ));
    }

    #[test]
    fn resolve_swap() {
        let raw = RawRule::RelativeAxis(RawRelativeAxisRule::Swap {
            a: RelativeAxisCode::REL_X,
            b: RelativeAxisCode::REL_Y,
        });

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(
            rule,
            Rule::RelativeAxis(RelativeAxisRule::Swap { a, b })
                if a == RelativeAxisCode::REL_X && b == RelativeAxisCode::REL_Y
        ));
    }

    #[test]
    fn resolve_scale_with_valid_factor() {
        let raw = RawRule::RelativeAxis(RawRelativeAxisRule::Scale {
            axis: RelativeAxisCode::REL_X,
            factor: 0.5,
        });

        let rule = Rule::resolve(raw, &Macros::default()).unwrap();

        assert!(matches!(
            rule,
            Rule::RelativeAxis(RelativeAxisRule::Scale { axis, factor, .. })
                if axis == RelativeAxisCode::REL_X && factor == 0.5
        ));
    }

    #[test]
    fn reject_zero_factor() {
        let raw = RawRule::RelativeAxis(RawRelativeAxisRule::Scale {
            axis: RelativeAxisCode::REL_X,
            factor: 0.0,
        });

        assert!(Rule::resolve(raw, &Macros::default()).is_err());
    }

    #[test]
    fn reject_negative_factor() {
        let raw = RawRule::RelativeAxis(RawRelativeAxisRule::Scale {
            axis: RelativeAxisCode::REL_X,
            factor: -0.5,
        });

        assert!(Rule::resolve(raw, &Macros::default()).is_err());
    }
}