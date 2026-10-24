//! A driver's persistent integration, separate from a terminal's temporary reporting connection.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Locations chosen by the caller. Drivers must not read process-global home/config variables.
#[derive(Clone, Debug)]
pub struct IntegrationContext {
    /// Shared host helper registered by integrations that use a native MCP connection.
    pub reporter: super::reporting::ReportingHelper,
    /// The user's home directory.
    pub home: PathBuf,
    /// Persistent directory reserved for this driver by the installer.
    pub directory: PathBuf,
    /// Current environment, including native programs' config-directory overrides.
    pub environment: BTreeMap<String, String>,
}

impl IntegrationContext {
    /// A program-specific environment override, or a path relative to the supplied home.
    pub fn location(&self, variable: &str, relative: &str) -> PathBuf {
        self.environment.get(variable).filter(|value| !value.is_empty())
            .map_or_else(|| self.home.join(relative), PathBuf::from)
    }
}

/// What explicit installation adds. The shared installer owns filesystem changes and recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentIntegration {
    /// Version of this driver's bundled integration.
    pub version: String,
    /// Files and native configuration entries needed by the integration.
    pub resources: Vec<IntegrationResource>,
}

/// One owned file or a single registration in an otherwise user-owned document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IntegrationResource {
    /// A standalone integration file. Existing unowned files are never replaced.
    File {
        /// Absolute destination selected by the driver.
        path: PathBuf,
        /// Bundled file bytes.
        content: Vec<u8>,
    },
    /// Append exactly one entry without replacing the other entries in the array.
    JsonArrayEntry {
        /// User-owned native configuration file.
        path: PathBuf,
        /// Object keys leading to the array, without dotted-key parsing.
        keys: Vec<String>,
        /// Exactly one entry; existing equal entries remain user-owned.
        value: serde_json::Value,
    },
    /// Append a hook to an existing inline TOML hook source, keeping one native representation.
    TomlArrayEntry {
        /// Existing native TOML configuration file.
        path: PathBuf,
        /// Table keys leading to the array or array of tables.
        keys: Vec<String>,
        /// Exactly one native hook registration.
        value: serde_json::Value,
    },
    /// A native feature or Heca enable switch. Uninstall restores its previous value.
    TomlBool {
        /// User-owned configuration file.
        path: PathBuf,
        /// Table keys ending in the boolean setting.
        keys: Vec<String>,
        /// Installed value; uninstall restores the previous value or removes the added key.
        value: bool,
    },
    /// One owned TOML value. Existing unequal content is never overwritten or adopted.
    TomlEntry {
        /// User-owned configuration file.
        path: PathBuf,
        /// Table keys ending in the owned setting.
        keys: Vec<String>,
        /// Installed scalar, array or table.
        value: serde_json::Value,
    },
}

impl IntegrationResource {
    /// The document or standalone file this resource belongs to.
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::File { path, .. } | Self::JsonArrayEntry { path, .. } | Self::TomlArrayEntry { path, .. }
                | Self::TomlBool { path, .. } | Self::TomlEntry { path, .. } => path,
        }
    }
}
