//! A server for tests: no window, no event loop, and terminals that start no process.

use std::time::Duration;

use heca_config::programs::ProgramsConfig;
use heca_core::backend::{FakeBackend, PaneBackend};

use super::ServerState;
use crate::app::backend_factory::{LaunchSettings, TerminalSettings};
use crate::app::backend_store::{BackendStore, SpawnError, TerminalSpec};
use crate::notification::NotificationRuntime;

/// A terminal that starts no process, the size its spec asks for.
fn no_process(
    _: &LaunchSettings<'_>,
    spec: &TerminalSpec,
) -> Result<Box<dyn PaneBackend>, SpawnError> {
    Ok(Box::new(FakeBackend::new(spec.grid.0, spec.grid.1)))
}

/// The default terminal settings.
fn terminal_settings() -> TerminalSettings {
    TerminalSettings {
        agents: None,
        theme: Default::default(),
        shell_integration: false,
        scrollback: 1000,
        scroll_animations: false,
        link_detection: false,
        images: false,
    }
}

impl ServerState {
    /// A server delivering notifications as `mode` says, whose terminals start no process.
    pub(crate) fn for_tests(mode: heca_config::settings::NotificationSystem) -> Self {
        Self::new(
            BackendStore::new(),
            NotificationRuntime::with_capacity(50, Duration::from_millis(4000), mode, 5),
            ProgramsConfig::default(),
            terminal_settings(),
            None,
        )
        .launching_with(no_process)
    }
}
