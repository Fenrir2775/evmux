pub(super) mod raw {
    use evdev::KeyCode;
    use serde::{Deserialize, Serialize};
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub(crate) struct RawProfile {
        #[serde(default)]
        pub(crate) rules: Vec<RawRule>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub(crate) enum RawRule {
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
            name: String
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
                KeyCode::from_str(&name)
                    .map_err(|_| D::Error::custom(format!("unknown key: '{}'", name)))
            })
            .collect()
    }
}
