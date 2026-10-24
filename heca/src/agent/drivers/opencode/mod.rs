//! OpenCode discovers a local CLI plugin. No plugin-list override or temporary plugin is needed.

use super::*;

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("opencode", config, AgentIcon::svg(include_str!("../assets/icons/opencode.svg")), environment, integration)
}

fn integration(driver: &Driver, context: &IntegrationContext) -> Result<AgentIntegration, AgentError> {
    let directory = if let Some(path) = context.environment.get("OPENCODE_CONFIG_DIR") {
        std::path::PathBuf::from(path)
    } else {
        context.location("XDG_CONFIG_HOME", ".config").join("opencode")
    };
    bundled(driver, vec![IntegrationResource::File {
        path: directory.join("plugins/heca-status/tui.ts"),
        content: include_bytes!("../assets/opencode.ts").to_vec(),
    }])
}
