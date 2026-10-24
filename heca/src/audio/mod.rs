//! Shared sound sources and an open asset catalog. Playback stays off the UI thread.

mod assets;
mod registration;
mod source;

use std::collections::BTreeMap;

pub use heca_config::sound::{Sound, SoundSource};
pub use source::Audio;

/// An embedding can build a catalog with files or embedded audio through the same builder.
#[derive(Clone, Debug, Default)]
pub struct SoundLibrary {
    sounds: BTreeMap<String, Audio>,
}

impl SoundLibrary {
    pub fn new() -> Self { Self::default() }

    pub fn sound(mut self, name: impl Into<String>, source: Audio) -> Self {
        self.sounds.insert(name.into(), source);
        self
    }

    /// Choices for settings screens; includes added assets rather than a fixed program list.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.sounds.keys().map(String::as_str)
    }

    pub(crate) fn resolve(&self, source: &SoundSource) -> Result<Audio, String> {
        match source {
            SoundSource::File(path) => Ok(Audio::load(path)),
            SoundSource::Asset(name) => self.sounds.get(name).cloned()
                .ok_or_else(|| format!("unknown sound asset '{name}'")),
        }
    }
}

/// Bundled sounds plus assets registered by the embedding before startup. Does not play anything.
pub fn library() -> SoundLibrary {
    registration::append(assets::library())
}

#[cfg(test)]
mod tests;
