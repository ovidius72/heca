//! Limits for the long-lived services extensions run inside the server.

use std::num::{NonZeroU64, NonZeroUsize};
use serde::{Deserialize, Serialize};
use super::schema::declare_settings;

declare_settings! {
    defaults: ["services"];

    /// Resource limits applied to each extension service.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ServicesConfig {
        /// Messages a service's inbox holds before senders are told it is full.
        pub inbox_capacity: NonZeroUsize,
        /// Longest the server waits at shutdown for services to finish stopping.
        pub stop_timeout_ms: NonZeroU64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{CONFIG_DEFAULT, config_from_layers};

    #[test]
    fn defaults_come_from_the_embedded_file() {
        let file: toml::Value = toml::from_str(CONFIG_DEFAULT).unwrap();
        let expected: ServicesConfig = file["settings"]["services"].clone().try_into().unwrap();
        assert_eq!(ServicesConfig::default(), expected);
        assert_eq!(config_from_layers("").unwrap().settings.services, expected);
    }

    #[test]
    fn limits_must_be_positive() {
        for field in ["inbox_capacity", "stop_timeout_ms"] {
            let input = format!("[settings.services]\n{field} = 0");
            assert!(config_from_layers(&input).is_err(), "{input}");
        }
    }
}
