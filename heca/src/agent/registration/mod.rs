//! A native embedding adds a driver through its existing extension handle.

use std::{cell::RefCell, sync::Arc};

use heca_config::settings::{AgentProgramConfig, AgentsConfig};
use heca_core::agent::AgentDrivers;
use crate::chrome::StartupQueue;

use super::*;

thread_local! {
    static QUEUE: StartupQueue<ConfiguredDriver> = const { StartupQueue::new() };
    static ADDED: RefCell<AgentDrivers> = RefCell::new(AgentDrivers::new());
}

impl crate::Extension {
    /// Add a terminal agent implementation. Its identity is scoped to this extension.
    ///
    /// A program built on Heca calls this before `heca::run()`. Recognition, setup and reports
    /// use the same contract as the built-in programs; no private registry is needed.
    pub fn agent_driver(&self, driver: impl AgentDriver + 'static) {
        let Some(id) = self.full("agent driver", driver.id()) else { return };
        let scoped = ConfiguredDriver {
            id,
            inner: Arc::new(driver),
            config: None,
        };
        if let Err(late) = QUEUE.with(|queue| queue.add(scoped)) {
            crate::chrome::warn_author(format!("[heca] agent driver '{}' was added after the app started and is refused", late.id));
        }
    }
}

#[cfg(test)]
mod tests;

struct ConfiguredDriver {
    id: String,
    inner: Arc<dyn AgentDriver>,
    config: Option<Box<AgentProgramConfig>>,
}

impl AgentDriver for ConfiguredDriver {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str {
        self.config.as_ref().and_then(|config| config.name.as_deref()).unwrap_or_else(|| self.inner.name())
    }
    fn icon(&self) -> Option<AgentIcon> {
        self.config.as_ref().and_then(|config| config.icon.as_ref()).map(AgentIcon::file)
            .or_else(|| self.inner.icon())
    }
    fn programs(&self) -> &[String] {
        self.config.as_ref().and_then(|config| config.executables.as_deref()).unwrap_or_else(|| self.inner.programs())
    }
    fn recognizes(&self, program: &str) -> bool {
        self.config.as_ref().and_then(|config| config.executables.as_ref()).map_or_else(
            || self.inner.recognizes(program),
            |executables| executables.iter().any(|name| name == heca_core::agent::program_name(program)),
        )
    }
    fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> { self.inner.prepare(context) }
    fn integration(&self, context: &IntegrationContext) -> Result<Option<AgentIntegration>, AgentError> {
        self.inner.integration(context)
    }
    fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        let Some(mut report) = self.inner.read_status(input)? else { return Ok(None) };
        // Validate before scoping: an implementation cannot claim another one's identity.
        if report.driver != self.inner.id() {
            return Err(AgentError::Report(format!("report identity differs from {}", self.inner.id())));
        }
        report.driver.clone_from(&self.id);
        report.name = self.name().to_owned();
        Ok(Some(report))
    }
}

pub(super) fn append(drivers: &mut AgentDrivers, config: &AgentsConfig) {
    append_with(drivers, config, false);
}

pub(super) fn append_available(drivers: &mut AgentDrivers, config: &AgentsConfig) {
    append_with(drivers, config, true);
}

fn append_with(drivers: &mut AgentDrivers, config: &AgentsConfig, include_disabled: bool) {
    let declared = QUEUE.with(StartupQueue::take);
    let added = ADDED.with(|added| {
        let mut added = added.borrow_mut();
        for driver in declared {
            if let Err(error) = added.register(driver) {
                crate::chrome::warn_author(format!("[heca] agent registration: {error}"));
            }
        }
        added.clone()
    });
    for driver in added.iter() {
        let setting = config.programs.get(driver.id());
        if !include_disabled && setting.is_none_or(|setting| setting.enabled != Some(true)) { continue }
        let driver = ConfiguredDriver {
            id: driver.id().to_owned(), inner: driver.clone(), config: setting.cloned().map(Box::new),
        };
        if let Err(error) = drivers.register(driver) {
            crate::chrome::warn_author(format!("[heca] agent registration: {error}"));
        }
    }
}
