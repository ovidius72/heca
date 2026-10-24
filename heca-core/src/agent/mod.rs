//! The shared contract for programs that report work status.
//!
//! A driver describes recognition, per-run setup and status decoding. The host asks the same
//! contract for built-in programs and additions from another crate. Process status remains in
//! [`crate::runtime::ProcessStatus`]; an agent report never replaces it.

mod driver;
mod model;
mod icon;
mod registry;
mod integration;
mod channel;
mod session;
mod watcher;
pub mod reporting;
pub mod launcher;

pub use driver::{AgentDriver, AgentInput, AgentLaunchContext, AgentSetup, program_name};
pub use model::{AgentError, AgentState, AgentStatus, TerminalNotification};
pub use icon::AgentIcon;
pub use registry::AgentDrivers;
pub use integration::{AgentIntegration, IntegrationContext, IntegrationResource};
pub use session::AgentOptions;
pub(crate) use session::{AgentSession, quote};

#[cfg(test)]
pub(crate) mod tests;
