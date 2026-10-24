//! Agent reporting, presentation and sound defaults. Project overrides use the shared loader.

use std::{collections::BTreeMap, path::PathBuf};

use serde::{Deserialize, Serialize};

use super::schema::declare_settings;

/// Open presentation table shared by config and status-line consumers.
pub type AgentStatuses = BTreeMap<String, AgentStatusConfig>;

declare_settings! {
    defaults: ["agents"];

    /// `[settings.agents]` — status integrations for terminal programs.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct AgentsConfig {
        /// Connect enabled programs to their installed reporting integration in Heca terminals.
        pub enabled: bool,
        /// Maximum size of one incoming report, in bytes.
        pub max_report_bytes: usize,
        /// Maximum number of reports waiting for the terminal owner.
        pub channel_capacity: usize,
        /// Maximum wait for a program's per-invocation setup.
        pub launch_timeout_ms: u64,
        /// Built-in driver metadata and lifecycle-event mappings, keyed by driver id.
        pub programs: BTreeMap<String, AgentProgramConfig>,
        /// Open status vocabulary: plugins and projects may add more entries.
        pub statuses: AgentStatuses,
        /// Sound playback, disabled until the user enables it and supplies sound files.
        pub sounds: AgentSoundsConfig,
    }
}

/// Per-program metadata. Driver code interprets its own lifecycle event names.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentProgramConfig {
    pub enabled: Option<bool>,
    pub name: Option<String>,
    pub executables: Option<Vec<String>>,
    /// Optional local SVG/bitmap mark, overriding the registered driver's logo.
    pub icon: Option<PathBuf>,
    /// Native events to report. Unset uses the report map; empty disables reporting.
    pub enabled_events: Option<Vec<String>>,
    /// Program-native event (optionally `event:matcher`) to an open status id.
    #[serde(default)]
    pub reports: BTreeMap<String, String>,
}

/// Presentation for one status. Unlisted statuses keep their reported identifier and program icon.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentStatusConfig {
    pub label: Option<String>,
    /// A name from the shared glyph catalog.
    pub icon: Option<String>,
    /// Shared semantic theme tone: accent, muted, success, warning or danger.
    pub tone: Option<String>,
    /// File or registered asset to play on entering this status. Absent/disabled means silence.
    pub sound: Option<crate::sound::Sound>,
}

declare_settings! {
    defaults: ["agents", "sounds"];

    /// Playback settings shared by built-ins and additional drivers.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct AgentSoundsConfig {
        pub enabled: bool,
        pub volume: f32,
        pub queue_capacity: usize,
    }
}
