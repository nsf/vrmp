use sdl2::keyboard::Keycode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::action::Action;

#[derive(Serialize, Deserialize)]
pub enum Trigger {
    None,
    #[serde(serialize_with = "keycode_se", deserialize_with = "keycode_de")]
    Key(Keycode),
}

fn keycode_se<S>(v: &Keycode, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(&v.name())
}

fn keycode_de<'de, D>(d: D) -> Result<Keycode, D::Error>
where
    D: Deserializer<'de>,
{
    let s: &str = Deserialize::deserialize(d)?;
    match Keycode::from_name(s) {
        Some(k) => Ok(k),
        None => Err(serde::de::Error::custom("invalid keycode")),
    }
}

#[derive(Serialize, Deserialize)]
pub struct Controls {
    #[serde(default = "default_control_map")]
    control_map: Vec<(Trigger, Action)>,
}

fn default_control_map() -> Vec<(Trigger, Action)> {
    vec![(
        Trigger::Key(Keycode::Space),
        Action::Command(vec!["cycle".to_owned(), "pause".to_owned()]),
    )]
}
