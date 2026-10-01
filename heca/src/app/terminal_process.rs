//! **Starting a terminal.** The one place the app asks for a terminal process: it gathers the
//! settings, handles a folder that is not there, and goes through the store's one door
//! ([`BackendStore::ensure`](crate::app::backend_store::BackendStore::ensure)).
//!
//! "Terminal process", never "session": a session is an instance of heca (F012).

use crate::app::backend_factory::{LaunchSettings, launch, terminal_grid_for_workspace};
use crate::app::backend_store::{Ensured, Program, SpawnError, TerminalSpec};
use crate::app_state::AppState;
use crate::notification::Notification;
use heca_core::layout::PaneId;

impl TerminalSpec {
    /// The user's shell, sized for workspace `ws_idx`.
    pub(crate) fn shell_in_workspace(state: &AppState, ws_idx: usize) -> Self {
        Self {
            program: Program::Shell,
            cwd: None,
            grid: terminal_grid_for_workspace(state, ws_idx),
        }
    }

    /// A command, sized for workspace `ws_idx`.
    pub(crate) fn command_in_workspace(
        state: &AppState,
        ws_idx: usize,
        command: &str,
        cwd: Option<&str>,
    ) -> Self {
        Self {
            program: Program::Command(command.to_string()),
            cwd: cwd.map(std::path::PathBuf::from),
            grid: terminal_grid_for_workspace(state, ws_idx),
        }
    }
}

impl AppState {
    /// **Start the user's shell for `pane`**, sized for workspace `ws_idx` — what every new pane
    /// does. A shell always starts (a failed PTY falls back to an inert pane), so there is nothing
    /// for the caller to handle.
    pub(crate) fn start_shell_in(&mut self, pane: PaneId, ws_idx: usize) {
        let spec = TerminalSpec::shell_in_workspace(self, ws_idx);
        if let Err(err) = self.start_terminal(pane, spec) {
            eprintln!("[heca] could not start a shell for pane {}: {err}", pane.0);
        }
    }

    /// **Start the terminal for `pane`, or keep the one it has.**
    ///
    /// A folder that is not there is not the same problem for both kinds of program. A *command*
    /// that cannot start in it is an error the caller reports. A *shell* is still worth giving the
    /// user: it starts in the home folder, and the user is told once.
    pub(crate) fn start_terminal(
        &mut self,
        pane: PaneId,
        mut spec: TerminalSpec,
    ) -> Result<Ensured, SpawnError> {
        if matches!(spec.program, Program::Shell)
            && let Some(dir) = spec.cwd.as_ref().filter(|dir| !dir.is_dir())
        {
            Notification::warning(format!(
                "Folder {} doesn't exist, starting in your home folder",
                dir.display()
            ))
            .dedup_key(format!("terminal.cwd:{}", dir.display()))
            .send();
            spec.cwd = dirs::home_dir();
        }
        let settings = LaunchSettings::of(self);
        let ensured = ensure_with(&mut self.backends, pane, spec, &settings)?;
        if let Ensured::Kept { differs: true, .. } = ensured {
            Notification::warning("A terminal was already running for this pane")
                .body("It keeps running as it was; the new program or folder was not applied.")
                .dedup_key(format!("terminal.spec:{}", pane.0))
                .send();
        }
        Ok(ensured)
    }
}

/// The store's door, with the app's launch settings behind it.
fn ensure_with(
    backends: &mut crate::app::backend_store::BackendStore,
    pane: PaneId,
    spec: TerminalSpec,
    settings: &LaunchSettings,
) -> Result<Ensured, SpawnError> {
    backends.ensure(pane, spec, |spec| launch(settings, spec))
}
