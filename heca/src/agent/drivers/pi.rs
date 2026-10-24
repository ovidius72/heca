//! Pi discovers the installed extension; a launch supplies only its temporary reporting connection.

use super::*;

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("pi", config, AgentIcon::svg(include_str!("assets/icons/pi.svg")), environment, integration)
}

fn integration(driver: &Driver, context: &IntegrationContext) -> Result<AgentIntegration, AgentError> {
    bundled(driver, vec![IntegrationResource::File {
        path: context.location("PI_CODING_AGENT_DIR", ".pi/agent").join("extensions/heca-status.js"),
        content: include_bytes!("assets/pi.mjs").to_vec(),
    }])
}
