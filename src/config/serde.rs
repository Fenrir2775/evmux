pub(super) mod raw {
    use super::*;
    use evdev::{KeyCode, RelativeAxisCode};
    use serde::{Deserialize, Serialize};
    use serde_with::{DeserializeFromStr, SerializeDisplay};
    use std::fmt::Display;
    use std::str::FromStr;

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub(crate) struct RawProfile {
        #[serde(default)]
        pub(crate) rules: Vec<RawRule>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum RawRule {
        Key(RawKeyRule),
        RelativeAxis(RawRelativeAxisRule),
    }

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum RawKeyRule {
        KeyToSingle {
            #[serde(with = "keycode_serde")]
            from: KeyCode,
            #[serde(with = "keycode_serde")]
            to: KeyCode,
        },
        KeyToMultiple {
            #[serde(with = "keycode_serde")]
            from: KeyCode,
            #[serde(with = "keycode_vec_serde")]
            to: Vec<KeyCode>,
        },
        Block {
            #[serde(with = "keycode_serde")]
            key: KeyCode,
        },
        Macro {
            #[serde(with = "keycode_serde")]
            from: KeyCode,
            name: String,
        },
    }

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum RawRelativeAxisRule {
        #[serde(with = "relative_axis_serde")]
        Invert(RelativeAxisCode),
        Swap {
            #[serde(with = "relative_axis_serde")]
            a: RelativeAxisCode,
            #[serde(with = "relative_axis_serde")]
            b: RelativeAxisCode,
        },
        Scale {
            #[serde(with = "relative_axis_serde")]
            axis: RelativeAxisCode,
            factor: f32,
        },
    }

    #[derive(SerializeDisplay, DeserializeFromStr)]
    pub(crate) enum MacroAction {
        /// Single key press.
        Press(KeyCode),
        /// Single key release.
        Release(KeyCode),
        /// Single key click (press/release).
        Click(KeyCode),
        /// A relative mouse move.
        MoveRelative { axis: RelativeAxisCode, value: i32 },
        /// Delay in milliseconds.
        Delay(u64),
    }

    impl Display for MacroAction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                MacroAction::Press(key) => write!(f, "press({key:?})"),
                MacroAction::Release(key) => write!(f, "release({key:?})"),
                MacroAction::Click(key) => write!(f, "click({key:?})"),
                MacroAction::MoveRelative { axis, value } => write!(f, "move_relative({axis:?}, {value:?})"),
                MacroAction::Delay(d) => write!(f, "delay({d})"),
            }
        }
    }

    impl FromStr for MacroAction {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
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
                    let (axis, value) = args
                        .split_once(',')
                        .ok_or_else(|| format!("invalid move relative args '{args}'"))?;
                    Ok(MacroAction::MoveRelative {
                        axis: axis.trim().parse().map_err(|_| format!("invalid axis: '{axis}'"))?,
                        value: value.trim().parse().map_err(|_| format!("invalid value: '{value}'"))?,
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
}

pub(super) mod keycode_serde {
    use evdev::KeyCode;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(key: &KeyCode, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{:?}", key))
    }

    pub fn deserialize<'a, D: Deserializer<'a>>(d: D) -> Result<KeyCode, D::Error> {
        let name = String::deserialize(d)?;

        KeyCode::from_str(&name)
            .map_err(|_| serde::de::Error::custom(format!("unknown key: '{name}'")))
    }
}

pub(super) mod keycode_vec_serde {
    use evdev::KeyCode;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(keys: &[KeyCode], s: S) -> Result<S::Ok, S::Error> {
        let names: Vec<String> = keys.iter().map(|k| format!("{:?}", k)).collect();
        names.serialize(s)
    }

    pub fn deserialize<'a, D: Deserializer<'a>>(d: D) -> Result<Vec<KeyCode>, D::Error> {
        let names = Vec::<String>::deserialize(d)?;

        names
            .iter()
            .map(|name| {
                KeyCode::from_str(name)
                    .map_err(|_| D::Error::custom(format!("unknown key: '{}'", name)))
            })
            .collect()
    }
}

pub(super) mod relative_axis_serde {
    use evdev::RelativeAxisCode;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S: Serializer>(axis: &RelativeAxisCode, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("{:?}", axis))
    }

    pub fn deserialize<'a, D: Deserializer<'a>>(d: D) -> Result<RelativeAxisCode, D::Error> {
        let name = String::deserialize(d)?;

        RelativeAxisCode::from_str(&name)
            .map_err(|_| serde::de::Error::custom(format!("unknown axis: '{name}'")))
    }
}
