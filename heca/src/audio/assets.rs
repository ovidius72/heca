//! Pack inventory only. Event selection belongs to configuration, never this catalog.

use super::{Audio, SoundLibrary};

pub(super) fn library() -> SoundLibrary {
    SoundLibrary::new()
        .sound("attention", Audio::from_bytes(include_bytes!("../../assets/sounds/attention1.mp3").as_slice()))
        .sound("completed", Audio::from_bytes(include_bytes!("../../assets/sounds/completed1.mp3").as_slice()))
        .sound("error", Audio::from_bytes(include_bytes!("../../assets/sounds/error1.mp3").as_slice()))
        .sound("locked", Audio::from_bytes(include_bytes!("../../assets/sounds/locked.mp3").as_slice()))
        .sound("success", Audio::from_bytes(include_bytes!("../../assets/sounds/success1.mp3").as_slice()))
}
