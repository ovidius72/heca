//! Session-local reporting through stdio MCP or native command hooks.

mod command;
mod records;
mod server;

use std::{collections::BTreeMap, path::{Path, PathBuf}};

use serde_json::{Value, json};

use super::{AgentError, AgentState};

pub(crate) const FLAG: &str = "--heca-agent-report";
pub(crate) const ENDPOINT: &str = "HECA_AGENT_REPORT";
pub(crate) const DRIVER: &str = "HECA_AGENT_DRIVER";
pub(crate) const EVENTS: &str = "HECA_AGENT_EVENTS";

/// The host executable and the configured report limit, shared by all reporting adapters.
#[derive(Clone, Debug)]
pub struct ReportingHelper {
    /// Host that handles Heca's private helper invocation before starting its normal CLI.
    pub executable: PathBuf,
    /// Maximum bytes in an incoming request or an outgoing report.
    pub max_report_bytes: usize,
}

impl ReportingHelper {
    /// MCP tool used by installed lifecycle hooks. The adapter supplies native event names.
    pub const TOOL: &str = "report_status";

    fn executable(&self) -> Result<&str, AgentError> {
        let executable = self.executable.to_str()
            .ok_or_else(|| AgentError::Report("reporting executable must be UTF-8".into()))?;
        if self.max_report_bytes == 0 {
            return Err(AgentError::Report("reporting limit must be positive".into()));
        }
        Ok(executable)
    }

    /// A shell command that reads native JSON from stdin and reports only session metadata.
    /// It is inactive without this driver's current Heca reporting environment. It emits no
    /// hook output and never changes the native program's permission decision.
    pub fn command_hook(&self, driver: &str, event: &str) -> Result<String, AgentError> {
        let executable = self.executable()?;
        Ok([executable, FLAG, &self.max_report_bytes.to_string(), driver, event].into_iter()
            .map(super::session::quote).collect::<Vec<_>>().join(" "))
    }

    /// A stdio MCP server registration. Empty reporting values keep an installed helper inactive
    /// outside Heca, even when the native program's background server inherited old values.
    pub fn mcp_server(&self, endpoint: Option<&Path>, reports: &BTreeMap<String, AgentState>) -> Result<Value, AgentError> {
        let executable = self.executable()?;
        let mut variables: BTreeMap<String, String> = [ENDPOINT, DRIVER, EVENTS].into_iter()
            .map(|key| (key.into(), String::new())).collect();
        if let Some(endpoint) = endpoint {
            let endpoint = endpoint.to_str().ok_or_else(|| AgentError::Report("report endpoint must be UTF-8".into()))?;
            let driver = reports.values().next().map_or("", |report| report.driver.as_str());
            if reports.values().any(|report| report.driver != driver) {
                return Err(AgentError::Report("one reporting connection cannot mix drivers".into()));
            }
            let events = serde_json::to_string(reports).map_err(|error| AgentError::Report(error.to_string()))?;
            if events.len() > self.max_report_bytes {
                return Err(AgentError::Report("report configuration exceeds max_report_bytes".into()));
            }
            variables.insert(ENDPOINT.into(), endpoint.into());
            variables.insert(DRIVER.into(), driver.into());
            variables.insert(EVENTS.into(), events);
        }
        Ok(json!({"command": executable, "args": [FLAG, self.max_report_bytes.to_string()], "env": variables}))
    }
}

pub(crate) fn run(arguments: Vec<std::ffi::OsString>) -> Result<i32, AgentError> {
    let Some(limit) = arguments.first() else {
        return Err(AgentError::Report("incomplete private reporting invocation".into()));
    };
    let limit = limit.to_str().and_then(|value| value.parse::<usize>().ok()).filter(|value| *value > 0)
        .ok_or_else(|| AgentError::Report("invalid reporting limit".into()))?;
    match arguments.as_slice() {
        [_] => server::run(std::io::stdin().lock(), std::io::stdout().lock(), limit)?,
        [_, driver, event] => {
            let driver = driver.to_str().ok_or_else(|| AgentError::Report("driver must be UTF-8".into()))?;
            let event = event.to_str().ok_or_else(|| AgentError::Report("event must be UTF-8".into()))?;
            command::run(std::io::stdin().lock(), driver, event, limit)?;
        }
        _ => return Err(AgentError::Report("incomplete private reporting invocation".into())),
    }
    Ok(0)
}
