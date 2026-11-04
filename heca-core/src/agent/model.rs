use std::fmt;

use serde::{Deserialize, Serialize};

/// An open status identifier. Programs and configuration may introduce more identifiers.
///
/// This is deliberately not an enum: adding a status must not require changing the host.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentStatus(String);

impl AgentStatus {
    /// Keep the identifier supplied by a driver or configuration.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The configuration/event identifier, without a presentation label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A complete reported state, independent of a window or renderer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentState {
    /// Original launcher inputs, attached by the host before integration setup is added.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<crate::backend::ProcessLaunch>,
    /// Driver that supplied the report.
    pub driver: String,
    /// Display name supplied by that driver's configured program metadata.
    pub name: String,
    /// Open identifier resolved through the status configuration for presentation and sounds.
    pub status: AgentStatus,
    /// Native lifecycle event that produced this report, when supplied by the integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Task title explicitly supplied by the program or launcher.
    pub task: Option<String>,
    /// Optional detail about the current activity.
    pub detail: Option<String>,
    /// Program-provided turn identity, when available.
    pub turn_id: Option<String>,
    /// Mark supplied by the registered driver, resolved by the shared image widget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<super::AgentIcon>,
    /// Native model name, when explicitly reported by the program.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Native session identity. Distinct from the terminal pane's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Native session name/title. The UI uses the pane name until one is available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_title: Option<String>,
}

impl AgentState {
    /// A report with no task, detail or turn identity.
    pub fn new(
        driver: impl Into<String>,
        name: impl Into<String>,
        status: AgentStatus,
    ) -> Self {
        Self {
            launch: None,
            driver: driver.into(),
            name: name.into(),
            status,
            event: None,
            task: None,
            detail: None,
            turn_id: None,
            icon: None,
            model: None,
            session_id: None,
            session_title: None,
        }
    }

    /// Identify the native event without coupling the driver to a window's presentation.
    pub fn event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Preserve omitted metadata only across reports for the same explicitly identified session.
    pub(crate) fn retain_session(mut self, previous: Option<&Self>) -> Self {
        if let Some(previous) = previous
            && self.driver == previous.driver
            && self.session_id.as_ref().is_some_and(|id| !id.is_empty())
            && self.session_id == previous.session_id
        {
            if self.model.is_none() { self.model.clone_from(&previous.model); }
            if self.session_title.is_none() { self.session_title.clone_from(&previous.session_title); }
            if self.launch.is_none() { self.launch.clone_from(&previous.launch); }
        }
        self
    }
}

/// A standard terminal notification, before a driver decides whether it is its own report.
/// Ordinary notifications are not automatically agent state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerminalNotification {
    /// OSC command number, such as 9, 99 or 777.
    pub code: u16,
    /// Payload after the command number's first separator.
    pub payload: String,
}

/// A failed registration, launch preparation or report.
#[derive(Debug)]
pub enum AgentError {
    /// Driver identifiers must be nonempty and unique within their owner.
    Driver(String),
    /// A driver could not prepare its per-run integration.
    Launch(std::io::Error),
    /// A report was malformed or claimed a different driver.
    Report(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Driver(id) => write!(f, "invalid or duplicate driver: {id}"),
            Self::Launch(error) => error.fmt(f),
            Self::Report(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Launch(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AgentError {
    fn from(error: std::io::Error) -> Self {
        Self::Launch(error)
    }
}
