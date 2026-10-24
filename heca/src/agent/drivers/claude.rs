//! Claude's installed command hooks forward native metadata through the shared report helper.

use super::*;

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("claude", config, AgentIcon::svg(include_str!("assets/icons/claude.svg")), environment, integration)
}

fn integration(driver: &Driver, context: &IntegrationContext) -> Result<AgentIntegration, AgentError> {
    let path = context.location("CLAUDE_CONFIG_DIR", ".claude").join("settings.json");
    bundled(driver, hooks::registrations(driver, path, &context.reporter)?)
}
