//! **Starting, running and ending terminal processes** — the server's, because the processes are.
//!
//! What a process is started with comes from two places: the configuration, which the server holds
//! ([`TerminalSettings`], set at start and on every reload), and the window that asked — its cell
//! size and the box the terminal will fill, which every request carries. Nothing here reads a
//! window.
//!
//! "Terminal process", never "session": a session is an instance of heca.

use std::path::{Path, PathBuf};

use heca_core::layout::PaneId;

use super::ServerWaker;
use crate::app::backend_factory::{Launch, LaunchSettings, TerminalSettings};
use crate::app::backend_store::{
    BackendStore, Ensured, Program, SpawnError, TerminalOwner, TerminalSpec,
};
use crate::notification::Notification;

/// The terminal processes and what starting one takes, borrowed from the server for one request.
pub(crate) struct Terminals<'a> {
    /// The processes — the one owner of them.
    pub(crate) backends: &'a mut BackendStore,
    /// What the configuration says a process starts with.
    pub(crate) settings: &'a TerminalSettings,
    /// How a process wakes the loop when its output changes. `None` where there is no loop to wake.
    pub(crate) waker: Option<&'a ServerWaker>,
    /// How a process is built.
    pub(crate) launch: Launch,
}

impl Terminals<'_> {
    /// **Start the user's shell for `pane`**, `grid` cells big, for a window whose cells are
    /// `cell` big. A shell always starts (a failed PTY falls back to an inert pane), so there is
    /// nothing for the caller to handle.
    pub(crate) fn start_shell(&mut self, pane: PaneId, grid: (usize, usize), cell: (f32, f32)) {
        let spec = TerminalSpec {
            program: Program::Shell,
            cwd: None,
            grid,
        };
        if let Err(err) = self.start(pane, spec, cell) {
            eprintln!("[heca] could not start a shell for pane {}: {err}", pane.0);
        }
    }

    /// **Start the terminal for `owner`, or keep the one it has**, for a window whose cells are
    /// `cell` big.
    ///
    /// A folder that is not there is not the same problem for both kinds of program. A *command*
    /// that cannot start in it is an error the caller reports. A *shell* is still worth giving the
    /// user: it starts in the home folder, and the user is told once.
    pub(crate) fn start(
        &mut self,
        owner: impl Into<TerminalOwner>,
        mut spec: TerminalSpec,
        cell: (f32, f32),
    ) -> Result<Ensured, SpawnError> {
        let owner = owner.into();
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
        let settings = LaunchSettings {
            terminal: self.settings,
            wake_on_output: self.waker.map(ServerWaker::on_output),
            cell_size: cell,
        };
        let launch = self.launch;
        let ensured = self
            .backends
            .ensure(owner.clone(), spec, |spec| launch(&settings, spec))?;
        if let Ensured::Kept { differs: true, .. } = ensured {
            Notification::warning("A terminal was already running for this pane")
                .body("It keeps running as it was; the new program or folder was not applied.")
                .dedup_key(format!("terminal.spec:{owner:?}"))
                .send();
        }
        Ok(ensured)
    }

    /// End the processes of these panes, which are gone.
    pub(crate) fn end(&mut self, panes: impl IntoIterator<Item = PaneId>) {
        self.backends.kill_all(panes);
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
