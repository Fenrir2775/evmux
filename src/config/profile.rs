use crate::config::rule::Rule;
use serde::{Deserialize, Serialize};

/// A profile describes the remapping rules for a specific [`InputDevice`].
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub(crate) struct Profile {
    /// The profiles name.
    #[serde(skip)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) rules: Vec<Rule>,
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

# Macro: trigger a sequence of actions on a keypress
# [[rules]]
# type = "macro"
# from = "KEY_F1"
# macro_actions = []
"# //TODO: macro_actions
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