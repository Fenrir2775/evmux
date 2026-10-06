use crate::config;
use crate::config::macro_compiler;
use crate::config::serde::raw::MacroAction;
use crate::output::action::Action;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

#[derive(Deserialize, Serialize)]
struct MacroFile {
    actions: Vec<MacroAction>,
}

#[derive(Default)]
pub(crate) struct Macros {
    macros: HashMap<String, Arc<[Action]>>,
}

impl Macros {
    pub(crate) fn load() -> Result<Self> {
        let mut macros = HashMap::new();

        if !config::macros_dir().exists() {
            return Ok(Self { macros });
        }

        for entry in std::fs::read_dir(config::macros_dir())? {
            let path = entry?.path();

            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }

            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                eprintln!(
                    "Skipping macro file with invalid name: '{}'",
                    path.display()
                );
                continue;
            };

            match Self::load_file(&path) {
                Ok(actions) => {
                    macros.insert(name.into(), actions);
                }
                Err(e) => eprintln!("Failed to load macro '{name}': {e:#}"),
            }
        }

        Ok(Self { macros })
    }

    fn load_file(path: &Path) -> Result<Arc<[Action]>> {
        let content = std::fs::read_to_string(path)?;
        let macro_actions = toml::from_str::<MacroFile>(&content)?.actions;

        Ok(macro_compiler::compile_to_arc(&macro_actions))
    }

    pub(crate) fn get(&self, name: &str) -> Option<Arc<[Action]>> {
        self.macros.get(name).cloned()
    }

    #[cfg(test)]
    pub(crate) fn insert_for_test(&mut self, name: &str, actions: Arc<[Action]>) {
        self.macros.insert(name.to_string(), actions);
    }
}

#[cfg(test)]
mod tests {
    use evdev::KeyCode;
    use super::*;

    #[test]
    fn write_macro() {
        let f = MacroFile {
            actions: vec![
                MacroAction::Click(KeyCode::KEY_A),
                MacroAction::Click(KeyCode::KEY_B),
                MacroAction::Delay(1000),
                MacroAction::Click(KeyCode::KEY_C),
            ]
        };

        let t = toml::to_string_pretty(&f).unwrap();

        eprintln!("{t}\n");
    }
}