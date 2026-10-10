//! Settings defaults read from the embedded configuration.

use serde::de::{value::{Error, UnitDeserializer}, DeserializeOwned};
use std::sync::OnceLock;

fn value(path: &[&str]) -> Option<&'static toml::Value> {
    static SETTINGS: OnceLock<toml::Value> = OnceLock::new();
    let settings = SETTINGS.get_or_init(|| {
        let config: toml::Value = toml::from_str(crate::loader::CONFIG_DEFAULT)
            .expect("embedded config.default.toml must be valid TOML");
        config.get("settings").cloned()
            .expect("embedded config.default.toml must contain [settings]")
    });
    path.iter().try_fold(settings, |value, key| value.get(*key))
}

fn decode<T: DeserializeOwned>(value: &toml::Value, path: &[&str]) -> T {
    value.clone().try_into().unwrap_or_else(|error| {
        panic!("invalid embedded settings.{}: {error}", path.join("."))
    })
}

/// Read a field's file default. Absent optional values deserialize as None.
pub(super) fn field<T: DeserializeOwned>(path: &[&str]) -> T {
    if let Some(value) = value(path) {
        return decode(value, path);
    }
    T::deserialize(UnitDeserializer::<Error>::new()).unwrap_or_else(|_| {
        panic!("embedded config.default.toml is missing settings.{}", path.join("."))
    })
}
