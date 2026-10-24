//! Terminal agent reporting. Added drivers use the same contract as built-in programs.
//!
//! Implement [`AgentDriver`], then register it before running Heca:
//! `heca::extension("demo").agent_driver(MyDriver::new())`.

mod drivers;
mod registration;
pub mod integration;
pub(crate) mod sounds;

pub use heca_core::agent::{
    AgentDriver, AgentError, AgentInput, AgentLaunchContext, AgentOptions, AgentSetup, AgentState,
    AgentStatus, TerminalNotification, AgentIntegration, IntegrationContext, IntegrationResource, AgentIcon,
};
pub use heca_core::agent::reporting::ReportingHelper;

/// The machine's integration operations, including drivers declared by an embedding.
///
/// Constructing this service does not install or enable anything. CLI and settings callers
/// explicitly call `list`, `inspect` or `apply` on the returned value.
pub fn integrations(config: &heca_config::settings::AgentsConfig) -> std::io::Result<integration::Integrations> {
    let missing = || std::io::Error::other("cannot locate the user's Heca integration directories");
    let context = IntegrationContext {
        reporter: ReportingHelper { executable: std::env::current_exe()?, max_report_bytes: config.max_report_bytes },
        home: dirs::home_dir().ok_or_else(missing)?,
        directory: heca_config::state_file::data_path("integrations").ok_or_else(missing)?,
        environment: std::env::vars_os().filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?))).collect(),
    };
    Ok(integrations_in(config, context, heca_config::loader::user_config_path().ok_or_else(missing)?))
}

/// The same service with explicit locations, for embeddings and isolated installation tests.
pub fn integrations_in(
    config: &heca_config::settings::AgentsConfig,
    context: IntegrationContext,
    config_file: std::path::PathBuf,
) -> integration::Integrations {
    let mut drivers = drivers::available(config);
    registration::append_available(&mut drivers, config);
    integration::Integrations::new(drivers, context, config_file)
}

pub(crate) fn options(config: &heca_config::settings::AgentsConfig) -> Option<AgentOptions> {
    let mut drivers = drivers::configured(config);
    registration::append(&mut drivers, config);
    if !config.enabled { drivers = heca_core::agent::AgentDrivers::new(); }
    let launcher = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            crate::chrome::warn_author(format!("[heca] cannot locate the agent launcher: {error}"));
            return None;
        }
    };
    Some(AgentOptions {
        drivers,
        max_report_bytes: config.max_report_bytes,
        channel_capacity: config.channel_capacity,
        launch_timeout: std::time::Duration::from_millis(config.launch_timeout_ms),
        launcher,
    })
}
