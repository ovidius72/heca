use std::{collections::BTreeMap, path::PathBuf};

use super::{AgentError, AgentIntegration, AgentState, IntegrationContext, TerminalNotification};

/// Resources supplied by the terminal owner for one program's integration.
#[derive(Clone, Debug)]
pub struct AgentLaunchContext {
    /// Shared host helper for session-local reporting connections.
    pub reporter: super::reporting::ReportingHelper,
    /// Private directory for this run's reporting resources.
    pub directory: PathBuf,
    /// Program's working directory.
    pub cwd: PathBuf,
    /// Private status endpoint, when the program uses a side channel.
    pub endpoint: Option<PathBuf>,
    /// Environment of the command at invocation, after the user's shell configuration and exports.
    pub environment: BTreeMap<String, String>,
}

/// Per-run additions to a program launch. The caller keeps its original arguments after these.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AgentSetup {
    /// Additional arguments before the user's arguments.
    pub arguments: Vec<String>,
    /// Environment additions for this run only.
    pub environment: BTreeMap<String, String>,
}

/// Report input, borrowed while the driver reads it.
#[derive(Clone, Copy, Debug)]
pub enum AgentInput<'a> {
    /// Notification observed in the program's terminal output.
    Notification(&'a TerminalNotification),
    /// One complete message received through the program's private side channel.
    SideChannel(&'a [u8]),
}

/// One contract for recognition, launch setup and status reports.
///
/// Built-in implementations and additions from another crate implement this same trait.
/// The host never switches on a program name. The driver's setup may write only in the private
/// directory it is given; it must not replace the user's settings.
pub trait AgentDriver: Send + Sync {
    /// Stable driver identity, used in reports and configuration.
    fn id(&self) -> &str;

    /// Configured display name.
    fn name(&self) -> &str;

    /// Optional logo. Built-in and added drivers use the same image metadata contract.
    fn icon(&self) -> Option<super::AgentIcon> { None }

    /// Executable names recognized by this implementation.
    fn programs(&self) -> &[String];

    /// Whether this implementation recognizes a program. Paths are reduced to their basename.
    fn recognizes(&self, program: &str) -> bool {
        let name = program_name(program);
        self.programs().iter().any(|candidate| candidate == name)
    }

    /// Describe a persistent integration without changing any files.
    ///
    /// Return `None` when this driver needs no persistent installation. Only an explicit
    /// installer operation may apply the returned resources; terminal setup never does so.
    ///
    /// # Errors
    /// Return a configuration error when the native installation cannot be described.
    fn integration(&self, _context: &IntegrationContext) -> Result<Option<AgentIntegration>, AgentError> {
        Ok(None)
    }

    /// Prepare the program's temporary reporting connection and environment.
    ///
    /// # Errors
    /// Return a launch or configuration error without editing personal configuration.
    fn prepare(&self, context: &AgentLaunchContext) -> Result<AgentSetup, AgentError>;

    /// Read a status report. Return `None` for unrelated input; do not infer a state from it.
    ///
    /// # Errors
    /// Return a report error for malformed input that belongs to this driver.
    fn read_status(&self, input: AgentInput<'_>) -> Result<Option<AgentState>, AgentError>;
}

/// Executable basename used by recognition and configured executable aliases.
pub fn program_name(program: &str) -> &str {
    std::path::Path::new(program).file_name().and_then(|name| name.to_str()).unwrap_or(program)
}
