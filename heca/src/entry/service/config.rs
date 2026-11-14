//! An extension's own table in the configuration, read as a type of its own.

use heca_config::loader::AppConfig;
use serde::de::DeserializeOwned;

/// **Why an extension's `[extensions.<name>]` table could not be read** — it does not fit the type
/// the extension asked for. Says which table and what is wrong, so a user can fix their file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionConfigError {
    extension: String,
    message: String,
}

impl ExtensionConfigError {
    /// The extension whose table it is.
    pub fn extension(&self) -> &str {
        &self.extension
    }

    /// What is wrong with it, in the words of the type's deserializer.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ExtensionConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[extensions.{}] is not valid: {}", self.extension, self.message)
    }
}

impl std::error::Error for ExtensionConfigError {}

/// `extension`'s table in `config`, as `T`. Always the extension the service was declared under, so
/// one extension can never read another's.
pub(super) fn read<T: DeserializeOwned>(config: &AppConfig, extension: &str) -> Result<T, ExtensionConfigError> {
    config.config.extension(extension).map_err(|error| ExtensionConfigError {
        extension: extension.to_owned(),
        // The deserializer puts the key on a line of its own; one line reads better in a notification.
        message: error.to_string().lines().map(str::trim).collect::<Vec<_>>().join(" "),
    })
}
