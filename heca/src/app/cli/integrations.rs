//! Text/JSON presentation over the same installer that embeddings and settings use.

use super::{Answer, Invocation};
use crate::agent::integration::{
    InstallationState, InstallationStatus, IntegrationOperation, Integrations,
};

fn service() -> Result<Integrations, String> {
    Integrations::machine_local().map_err(|error| format!("integration settings: {error}\n"))
}

fn program(inv: &Invocation) -> Result<&str, String> {
    inv.get("program")
        .ok_or_else(|| "missing required argument 'program'\n".into())
}

pub(super) fn apply(inv: &Invocation, operation: IntegrationOperation) -> Answer {
    let status = service()?
        .apply(program(inv)?, operation)
        .map_err(|error| format!("integration: {error}\n"))?;
    // **A running heca takes the change at once**: installing switches the agent on in the config
    // file, and a server that never re-reads it would give no pane the agent's launcher.
    let reloaded = reload_running();
    let mut answer = render(&status, inv.json)?;
    if reloaded && !inv.json {
        answer.push_str("The running heca re-read its settings.\n");
    }
    Ok(answer)
}

#[cfg(unix)]
fn reload_running() -> bool {
    crate::server::socket::reload_running()
}

// A Windows window runs its server in its own process (no background server to tell), and reads
// the file when it starts.
#[cfg(not(unix))]
fn reload_running() -> bool {
    false
}

pub(super) fn inspect(inv: &Invocation) -> Answer {
    let status = service()?
        .inspect(program(inv)?)
        .map_err(|error| format!("integration: {error}\n"))?;
    render(&status, inv.json)
}

pub(super) fn list(inv: &Invocation) -> Answer {
    let statuses = service()?
        .list()
        .map_err(|error| format!("integrations: {error}\n"))?;
    if inv.json {
        return json(&statuses);
    }
    Ok(statuses.iter().map(text).collect::<String>())
}

fn json(value: &impl serde::Serialize) -> Answer {
    serde_json::to_string_pretty(value)
        .map(|text| text + "\n")
        .map_err(|error| format!("cannot format integrations: {error}\n"))
}

fn render(status: &InstallationStatus, as_json: bool) -> Answer {
    if as_json {
        json(status)
    } else {
        Ok(text(status))
    }
}

fn text(status: &InstallationStatus) -> String {
    let state = match status.state {
        InstallationState::NotInstalled => "not installed",
        InstallationState::Unmanaged => "unmanaged",
        InstallationState::Installed => "installed",
        InstallationState::Outdated => "update available",
        InstallationState::Modified => "modified",
        InstallationState::Incomplete => "incomplete",
    };
    let version = status.installed_version.as_deref().unwrap_or("none");
    let mut text = format!(
        "{} ({}): {state}; installed {version}, bundled {}\n",
        status.name, status.driver, status.bundled_version
    );
    for issue in &status.issues {
        text.push_str(&format!(
            "  {}: {}\n",
            issue.path.display(),
            issue.description
        ));
    }
    text
}
