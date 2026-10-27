//! Limits for the local client/server transport.

use std::num::{NonZeroU64, NonZeroUsize};
use serde::{Deserialize, Serialize};
use super::schema::declare_settings;

declare_settings! {
    defaults: ["connection"];

    /// Resource limits applied to each local connection.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct ConnectionConfig {
        /// Largest serialized message accepted from either peer, in bytes.
        pub max_frame_bytes: NonZeroUsize,
        /// Maximum number of queued messages per direction.
        pub queue_capacity: NonZeroUsize,
        /// Maximum time to identify a peer or await a one-shot RPC response.
        pub request_timeout_ms: NonZeroU64,
        /// Delay between connection attempts while starting a background server.
        pub connect_retry_ms: NonZeroU64,
        /// Largest decoded image resource accepted by a client, including all animation frames.
        pub max_image_bytes: NonZeroUsize,
        /// Maximum terminal grid area accepted from a connection.
        pub max_terminal_cells: NonZeroUsize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{CONFIG_DEFAULT, config_from_layers};

    #[test]
    fn defaults_come_from_the_embedded_file() {
        let file: toml::Value = toml::from_str(CONFIG_DEFAULT).unwrap();
        let expected: ConnectionConfig = file["settings"]["connection"].clone().try_into().unwrap();
        assert_eq!(ConnectionConfig::default(), expected);
        assert_eq!(toml::from_str::<ConnectionConfig>("").unwrap(), expected);
        assert_eq!(config_from_layers("").unwrap().settings.connection, expected);
    }

    #[test]
    fn partial_overrides_keep_other_file_defaults() {
        let config = config_from_layers("[settings.connection]\nqueue_capacity = 3").unwrap();
        assert_eq!(config.settings.connection.queue_capacity.get(), 3);
        assert_eq!(config.settings.connection.max_frame_bytes, ConnectionConfig::default().max_frame_bytes);
    }

    #[test]
    fn limits_must_be_positive() {
        for field in ["max_frame_bytes", "queue_capacity"] {
            for value in ["0", "-1", "'unlimited'"] {
                let input = format!("[settings.connection]\n{field} = {value}");
                assert!(config_from_layers(&input).is_err(), "{input}");
            }
        }
    }
}
