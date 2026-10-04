use crate::config::macros::Macros;
use crate::config::rule::Rule;
use crate::config::serde::raw::RawProfile;
use anyhow::Result;

/// A profile describes the remapping rules for a specific [`InputDevice`].
#[derive(Default)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) rules: Vec<Rule>,
}

impl Profile {
    pub(crate) fn parse(name: &str, text: &str, macros: &Macros) -> Result<Self> {
        let raw: RawProfile = toml::from_str(text)?;

        let rules = raw
            .rules
            .into_iter()
            .map(|r| Rule::resolve(r, macros))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            name: name.into(),
            rules,
        })
    }
}

pub(crate) fn profile_template(name: &str) -> String {
    format!(
        r#"# Profile: {name}
# Each Rule maps an input event to an output action.
# An empty or missing rules list means passthrough.

# Remap a single key to another
# [[rules]]
# type = "key_to_single"
# from = "KEY_CAPSLOCK"
# to = "KEY_LEFTCTRL"

# Remap a key to a sequence of keys
# [[rules]]
# type = "key_to_multiple"
# from = "KEY_A"
# to = "[KEY_LEFTCTRL, KEY_C]"

# Suppress a key
# [[rules]]
# type = "block"
# key = "KEY_SYSRQ"

# Macro: runs a macro from evmux/macros for the specified key
# [[rules]]
# type = "macro"
# from = "KEY_F1"
# name = desktop
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_output() {
        let template = profile_template("test_template");

        eprintln!("{template:#}");
    }
}
