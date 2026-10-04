use crate::config;
use crate::config::macro_compiler;
use crate::output::action::Action;
use anyhow::Result;
use evdev::KeyCode;
use serde::Deserialize;
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

#[derive(SerializeDisplay, DeserializeFromStr)]
pub(crate) enum MacroAction {
    /// Single key press.
    Press(KeyCode),
    /// Single key release.
    Release(KeyCode),
    /// Single key click (press/release).
    Click(KeyCode),
    /// A relative mouse move.
    MoveRelative { x: i32, y: i32 },
    /// Delay in milliseconds.
    Delay(u64),
}

impl Display for MacroAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MacroAction::Press(key) => write!(f, "press({key:?})"),
            MacroAction::Release(key) => write!(f, "release({key:?})"),
            MacroAction::Click(key) => write!(f, "click({key:?})"),
            MacroAction::MoveRelative { x, y } => write!(f, "move_relative({x:?}, {y:?})"),
            MacroAction::Delay(d) => write!(f, "delay({d})"),
        }
    }
}

impl FromStr for MacroAction {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let (name, args) = s
            .split_once('(')
            .ok_or_else(|| format!("invalid macro action '{s}'"))?;
        let args = args
            .strip_suffix(')')
            .ok_or_else(|| format!("invalid macro action '{s}'"))?;

        let parse_key =
            |k: &str| KeyCode::from_str(k.trim()).map_err(|_| format!("unknown key: '{k}'"));

        match name {
            "press" => Ok(MacroAction::Press(parse_key(args)?)),
            "release" => Ok(MacroAction::Release(parse_key(args)?)),
            "click" => Ok(MacroAction::Click(parse_key(args)?)),
            "move_relative" => {
                let (x, y) = args
                    .split_once(',')
                    .ok_or_else(|| format!("invalid move relative args '{args}'"))?;
                Ok(MacroAction::MoveRelative {
                    x: x.trim().parse().map_err(|_| format!("invalid x: '{x}'"))?,
                    y: y.trim().parse().map_err(|_| format!("invalid y: '{y}'"))?,
                })
            }
            "delay" => Ok(MacroAction::Delay(
                args.trim()
                    .parse()
                    .map_err(|_| format!("invalid delay: '{args}'"))?,
            )),
            other => Err(format!("unknown macro action '{other}'")),
        }
    }
}

#[derive(Deserialize)]
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
}
