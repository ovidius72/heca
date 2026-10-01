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
use std::path::{Path, PathBuf};

impl TerminalSpec {
    /// `program` in `cwd`, sized for workspace `ws_idx`: the one place a spec's grid is chosen.
    fn in_workspace(
        state: &AppState,
        ws_idx: usize,
        program: Program,
        cwd: Option<PathBuf>,
    ) -> Self {
        Self {
            program,
            cwd,
            grid: terminal_grid_for_workspace(state, ws_idx),
        }
    }

    /// The user's shell, sized for workspace `ws_idx`.
    pub(crate) fn shell_in_workspace(state: &AppState, ws_idx: usize) -> Self {
        Self::in_workspace(state, ws_idx, Program::Shell, None)
    }

    /// A command, sized for workspace `ws_idx`.
    pub(crate) fn command_in_workspace(
        state: &AppState,
        ws_idx: usize,
        command: &str,
        cwd: Option<&str>,
    ) -> Self {
        Self::in_workspace(
            state,
            ws_idx,
            Program::Command(command.to_string()),
            cwd.map(PathBuf::from),
        )
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
        if matches!(spec.program, Program::Shell) {
            let (cwd, missing) = shell_start_dir(spec.cwd.take(), |d| d.is_dir(), dirs::home_dir());
            spec.cwd = cwd;
            if let Some(dir) = missing {
                Notification::warning(format!(
                    "Folder {} doesn't exist, starting in your home folder",
                    dir.display()
                ))
                .dedup_key(format!("terminal.cwd:{}", dir.display()))
                .send();
            }
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

/// **Where a shell starts**, given the folder asked for. A folder that is not there is not worth a
/// dead pane: the shell starts in the home folder instead, and the folder that was missing is handed
/// back so the user can be told once. A command does not get this — it fails instead, because
/// running a program in the wrong folder is worse than not running it.
fn shell_start_dir(
    requested: Option<PathBuf>,
    is_dir: impl Fn(&Path) -> bool,
    home: Option<PathBuf>,
) -> (Option<PathBuf>, Option<PathBuf>) {
    match requested {
        Some(dir) if !is_dir(&dir) => (home, Some(dir)),
        other => (other, None),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_in_a_missing_folder_starts_at_home_and_says_which_folder_was_missing() {
        let home = Some(PathBuf::from("/home/me"));
        let (cwd, missing) = shell_start_dir(Some("/no/such".into()), |_| false, home.clone());
        assert_eq!(cwd, home);
        assert_eq!(missing, Some(PathBuf::from("/no/such")));
    }

    #[test]
    fn a_folder_that_is_there_is_kept_and_no_folder_means_the_shells_own() {
        let home = Some(PathBuf::from("/home/me"));
        let (cwd, missing) = shell_start_dir(Some("/work".into()), |_| true, home.clone());
        assert_eq!((cwd, missing), (Some(PathBuf::from("/work")), None));
        assert_eq!(shell_start_dir(None, |_| false, home), (None, None));
    }
}
