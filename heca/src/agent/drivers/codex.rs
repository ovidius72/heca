//! Persistent native hooks use the current session's MCP connection, including on a shared server.

use std::collections::BTreeMap;
use serde_json::json;
use super::*;
use super::resume::{Arity::{Flag, Many, One}, Grammar};

/// **Resuming**, from `codex --help` and `codex resume --help` (codex-cli 0.167.2): `codex resume
/// [OPTIONS] [SESSION_ID] [PROMPT]` takes "Session id (UUID) or session name", with the same options
/// as `codex`. The subcommand and id go first, then the user's own options; the prompt, attached
/// images (`-i`, for the initial prompt), `--last`/`--all` (which choose a session) and `--worktree`
/// (a new worktree) are dropped. Any other subcommand (`exec`, `review`, `fork`, ...) is not a
/// conversation to continue.
static GRAMMAR: Grammar = Grammar {
    options: &[
        ("-c", One), ("--config", One), ("--enable", One), ("--disable", One), ("--remote", One),
        ("--remote-auth-token-env", One), ("-m", One), ("--model", One), ("--local-provider", One),
        ("-p", One), ("--profile", One), ("-s", One), ("--sandbox", One), ("-C", One), ("--cd", One),
        ("--add-dir", One), ("-a", One), ("--ask-for-approval", One), ("-i", Many), ("--image", Many),
        ("--last", Flag), ("--all", Flag), ("--include-non-interactive", Flag), ("--strict-config", Flag),
        ("--oss", Flag), ("--approve-for-me", Flag), ("--dangerously-bypass-approvals-and-sandbox", Flag),
        ("--dangerously-bypass-hook-trust", Flag), ("--worktree", Flag), ("--search", Flag),
        ("--no-alt-screen", Flag), ("--no-daemon", Flag), ("-h", Flag), ("--help", Flag), ("-V", Flag), ("--version", Flag),
    ],
    subcommands: &[
        "agents", "exec", "e", "review", "login", "logout", "mcp", "plugin", "app-server", "remote-control", "app",
        "completion", "update", "doctor", "sandbox", "debug", "apply", "a", "queue", "archive", "delete",
        "migrate-rollouts", "migrate-nfs-db", "unarchive", "fork", "cloud", "exec-server", "features", "help",
    ],
    resumes_with: Some("resume"),
    replaced: &["-i", "--image", "--last", "--all", "--include-non-interactive", "--worktree"],
    refused: &["-h", "--help", "-V", "--version"],
    resume: "resume",
    keeps_first_word: false,
};

const SERVER: &str = "heca_status";

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("codex", config, AgentIcon::svg(include_str!("assets/icons/codex.svg")), prepare, integration, Some(&GRAMMAR))
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
