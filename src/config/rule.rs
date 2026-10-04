use crate::config::macros::Macros;
use crate::config::serde::raw::RawRule;
use crate::output::action::Action;
use anyhow::{Result, anyhow};
use evdev::KeyCode;
use std::sync::Arc;

pub(crate) enum CompiledMacro {
    Static(Arc<[Action]>),
}

/// Define the rules how an input event is handled.
pub(crate) enum Rule {
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

impl Rule {
    pub(crate) fn resolve(raw_rule: RawRule, macros: &Macros) -> Result<Self> {
        let rule = match raw_rule {
            RawRule::KeyToSingle { from, to } => Rule::KeyToSingle { from, to },
            RawRule::KeyToMultiple { from, to } => Rule::KeyToMultiple { from, to },
            RawRule::Block { key } => Rule::Block { key },
            RawRule::Macro { from, name } => {
                let actions = macros
                    .get(&name)
                    .ok_or_else(|| anyhow!("macro '{name}' not found"))?;

                Rule::Macro {
                    from,
                    r#macro: CompiledMacro::Static(actions),
                }
            }
        };

        Ok(rule)
    }
}
