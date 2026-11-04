//! Configured adapters. The terminal sees only the shared AgentDriver contract.

mod claude;
mod codex;
mod pi;
mod opencode;
mod hooks;

use heca_config::settings::{AgentProgramConfig, AgentsConfig};
use heca_core::agent::AgentDrivers;
use super::*;

type Prepare = fn(&Driver, &AgentLaunchContext) -> Result<AgentSetup, AgentError>;
type Integration = fn(&Driver, &IntegrationContext) -> Result<AgentIntegration, AgentError>;

pub(super) struct Driver {
    id: String,
    name: String,
    icon: AgentIcon,
    programs: Vec<String>,
    reports: std::collections::BTreeMap<String, String>,
    native_events: Vec<String>,
    prepare: Prepare,
    integration: Integration,
}

impl Driver {
    fn configured(id: &str, config: &AgentsConfig, icon: AgentIcon, prepare: Prepare, integration: Integration) -> Option<Self> {
        let AgentProgramConfig {
            enabled: Some(true), name: Some(name), executables: Some(programs), icon: custom_icon, mut reports, enabled_events, ..
        } = config.programs.get(id)?.clone() else { return None };
        let native_events = reports.keys().cloned().collect();
        if let Some(events) = enabled_events {
            for event in &events {
                if !reports.contains_key(event) {
                    crate::chrome::warn_author(format!("[heca] unknown reporting event '{event}' for '{id}'"));
                }
            }
            reports.retain(|event, _| events.contains(event));
        }
        if reports.is_empty() { return None }
        Some(Self { id: id.into(), name, icon: custom_icon.map(AgentIcon::file).unwrap_or(icon), programs, reports, native_events, prepare, integration })
    }

    fn report(&self, status: &str) -> AgentState {
        AgentState::new(&self.id, &self.name, AgentStatus::new(status))
    }

    fn report_map(&self) -> std::collections::BTreeMap<String, AgentState> {
        self.reports.iter().map(|(event, status)| (event.clone(), self.report(status).event(event))).collect()
    }
}

impl AgentDriver for Driver {
    fn id(&self) -> &str { &self.id }
    fn name(&self) -> &str { &self.name }
    fn icon(&self) -> Option<AgentIcon> { Some(self.icon.clone()) }
    fn programs(&self) -> &[String] { &self.programs }
    fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
        (self.prepare)(self, context)
    }
    fn integration(&self, context: &IntegrationContext) -> Result<Option<AgentIntegration>, AgentError> {
        (self.integration)(self, context).map(Some)
    }
    fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError> {
        let bytes = match input {
            AgentInput::Notification(notification) => {
                let Some(payload) = notification.payload.strip_prefix("heca-agent;") else { return Ok(None) };
                payload.as_bytes()
            }
            AgentInput::SideChannel(bytes) => bytes,
        };
        let mut report: AgentState = serde_json::from_slice(bytes)
            .map_err(|error| AgentError::Report(error.to_string()))?;
        if report.driver != self.id { return Ok(None) }
        report.name.clone_from(&self.name);
        Ok(Some(report))
    }
}

fn json(value: &impl serde::Serialize) -> Result<String, AgentError> {
    serde_json::to_string(value).map_err(|error| AgentError::Report(error.to_string()))
}

fn endpoint(context: &AgentLaunchContext) -> Result<String, AgentError> {
    context.endpoint.as_ref().and_then(|path| path.to_str()).map(str::to_owned)
        .ok_or_else(|| AgentError::Report("driver requires a local report file".into()))
}

pub(super) fn configured(config: &AgentsConfig) -> AgentDrivers {
    let mut drivers = AgentDrivers::new();
    for driver in [claude::configured(config), codex::configured(config), pi::configured(config), opencode::configured(config)]
        .into_iter().flatten()
    {
        if let Err(error) = drivers.register(driver) {
            crate::chrome::warn_author(format!("[heca] agent registration: {error}"));
        }
    }
    drivers
}

pub(super) fn available(config: &AgentsConfig) -> AgentDrivers {
    let mut declarations = config.clone();
    for program in declarations.programs.values_mut() {
        program.enabled = Some(true);
        program.enabled_events = None;
    }
    configured(&declarations)
}

fn bundled(driver: &Driver, resources: Vec<IntegrationResource>) -> Result<AgentIntegration, AgentError> {
    let versions: std::collections::BTreeMap<String, String> = serde_json::from_str(include_str!("assets/versions.json"))
        .map_err(|error| AgentError::Report(error.to_string()))?;
    let version = versions.get(&driver.id).ok_or_else(|| AgentError::Report(format!("missing bundled integration version for {}", driver.id)))?;
    Ok(AgentIntegration { version: version.clone(), resources })
}

fn environment(driver: &Driver, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
    let reports = driver.report_map();
    Ok(AgentSetup {
        arguments: Vec::new(), environment: [
            ("HECA_AGENT_REPORT".into(), endpoint(context)?),
            ("HECA_AGENT_DRIVER".into(), driver.id.clone()),
            ("HECA_AGENT_EVENTS".into(), json(&reports)?),
        ].into(),
    })
}

#[cfg(test)]
mod tests;
