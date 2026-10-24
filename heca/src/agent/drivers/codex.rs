//! Persistent native hooks use the current session's MCP connection, including on a shared server.

use std::collections::BTreeMap;
use serde_json::json;
use super::*;

const SERVER: &str = "heca_status";

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("codex", config, AgentIcon::svg(include_str!("assets/icons/codex.svg")), prepare, integration)
}

fn prepare(driver: &Driver, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError> {
    let mut setup = environment(driver, context)?;
    let server = context.reporter.mcp_server(context.endpoint.as_deref(), &driver.report_map())?;
    let mut setting = toml_edit::ser::to_document(&json!({"server": server}))
        .map_err(|error| AgentError::Report(error.to_string()))?;
    let value = setting.remove("server").and_then(|item| item.into_value().ok())
        .ok_or_else(|| AgentError::Report("cannot encode session reporting connection".into()))?;
    setup.arguments = vec!["-c".into(), format!("mcp_servers.{SERVER}={value}")];
    Ok(setup)
}

fn integration(driver: &Driver, context: &IntegrationContext) -> Result<AgentIntegration, AgentError> {
    let directory = context.location("CODEX_HOME", ".codex");
    let mut resources = Vec::new();
    for native in &driver.native_events {
        let (event, matcher) = native.split_once(':').map_or((native.as_str(), None), |(event, matcher)| (event, Some(matcher)));
        // Native MCP hooks do not support SessionEnd. The terminal owner clears status on exit.
        if event == "SessionEnd" { continue }
        let mut input = json!({"event": native, "session_id": "${session_id}", "model": "${model}"});
        if event != "SessionStart" && event != "Notification" {
            input["turn_id"] = json!("${turn_id}");
        }
        let mut value = json!({"hooks": [{
            "type": "mcp_tool", "server": SERVER, "tool": ReportingHelper::TOOL, "input": input,
        }]});
        if let Some(matcher) = matcher { value["matcher"] = json!(matcher); }
        resources.push(IntegrationResource::JsonArrayEntry {
            path: directory.join("hooks.json"), keys: vec!["hooks".into(), event.into()], value,
        });
    }
    // Keep whichever representation this config layer already uses. Creating hooks.json beside
    // inline hooks would make Codex load both sources and warn on every start.
    match std::fs::read_to_string(directory.join("config.toml")) {
        Ok(text) => {
            let config: toml_edit::DocumentMut = text.parse()
                .map_err(|error: toml_edit::TomlError| AgentError::Report(error.to_string()))?;
            if config.contains_key("hooks") {
                for resource in &mut resources {
                    if let IntegrationResource::JsonArrayEntry { keys, value, .. } = resource {
                        *resource = IntegrationResource::TomlArrayEntry {
                            path: directory.join("config.toml"), keys: keys.clone(), value: value.clone(),
                        };
                    }
                }
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    resources.push(IntegrationResource::TomlEntry {
        path: directory.join("config.toml"), keys: vec!["mcp_servers".into(), SERVER.into()],
        value: context.reporter.mcp_server(None, &BTreeMap::new())?,
    });
    resources.push(IntegrationResource::TomlBool {
        path: directory.join("config.toml"), keys: vec!["features".into(), "hooks".into()], value: true,
    });
    bundled(driver, resources)
}
