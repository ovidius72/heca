//! Sound choices shared by configuration, embeddings and future settings screens.

use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A file selected by the user, or an asset supplied by Heca/an embedding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SoundSource {
    File(PathBuf),
    Asset(String),
}

/// A sound and its optional per-event controls. String filenames remain compatible.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "Value", into = "Properties")]
pub struct Sound {
    pub source: SoundSource,
    pub enabled: bool,
    /// None inherits the application's playback volume.
    pub volume: Option<f32>,
}

impl Sound {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        Self { source: SoundSource::File(path.into()), enabled: true, volume: None }
    }

    pub fn asset(name: impl Into<String>) -> Self {
        Self { source: SoundSource::Asset(name.into()), enabled: true, volume: None }
    }

    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn volume(mut self, volume: f32) -> Self { self.volume = Some(volume); self }
}

impl Serialize for SoundSource {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::File(path) => {
                let path = path.to_str().ok_or_else(|| serde::ser::Error::custom("sound path must be UTF-8"))?;
                format!("file:{path}")
            }
            Self::Asset(name) => format!("asset:{name}"),
        };
        serializer.serialize_str(&value)
    }
}

impl<'de> Deserialize<'de> for SoundSource {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if let Some(name) = value.strip_prefix("asset:") {
            if name.is_empty() { return Err(serde::de::Error::custom("sound asset must not be empty")) }
            Ok(Self::Asset(name.into()))
        } else {
            let path = value.strip_prefix("file:").unwrap_or(&value);
            if path.is_empty() { return Err(serde::de::Error::custom("sound file must not be empty")) }
            Ok(Self::File(path.into()))
        }
    }
}

impl From<PathBuf> for Sound {
    fn from(path: PathBuf) -> Self { Self::load(path) }
}

impl From<&str> for Sound {
    fn from(path: &str) -> Self { Self::load(path) }
}

impl From<String> for Sound {
    fn from(path: String) -> Self { Self::load(path) }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Value {
    File(PathBuf),
    Properties(Properties),
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Properties {
    source: SoundSource,
    #[serde(default = "enabled")]
    enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    volume: Option<f32>,
}

const fn enabled() -> bool { true }

impl TryFrom<Value> for Sound {
    type Error = String;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let properties = match value {
            Value::File(path) => return Ok(Self::load(path)),
            Value::Properties(properties) => properties,
        };
        if properties.volume.is_some_and(|volume| !volume.is_finite() || !(0.0..=1.0).contains(&volume)) {
            return Err("sound volume must be between 0 and 1".into());
        }
        Ok(Self { source: properties.source, enabled: properties.enabled, volume: properties.volume })
    }
}

impl From<Sound> for Properties {
    fn from(sound: Sound) -> Self {
        Self { source: sound.source, enabled: sound.enabled, volume: sound.volume }
    }
}

#[cfg(test)]
mod tests;
