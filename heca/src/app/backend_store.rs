//! **The one owner of terminal processes.**
//!
//! A terminal process is what runs behind a [`Terminal`](crate::chrome::terminal::Terminal): a
//! shell or a command, on a PTY. Nothing outside this store holds one. Ids are issued here, once,
//! and are never reused; a pane's terminal is found through the pane it was started for, so the
//! readers that ask "the backend of this pane" keep their question while the identity underneath
//! is the store's.
//!
//! **There is one door in: [`ensure`](BackendStore::ensure).** It starts a process or, if the pane
//! already has one, keeps it. A process lives until its pane is closed ([`kill_for_pane`]) or heca
//! ends — rebuilding a tree, or removing the component that shows it, never touches it. (The word
//! is "terminal process", never "session": a session is an instance of heca, F012.)

use crate::chrome::terminal::TerminalId;
use heca_core::backend::PaneBackend;
use heca_core::layout::PaneId;
use std::collections::HashMap;
use std::path::PathBuf;

/// What to run in a terminal. Fixed at start: a running terminal's program and folder do not change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Program {
    /// The user's own shell.
    Shell,
    /// A command, run under the user's shell so job control and rc behave as they do in a pane the
    /// user opened.
    Command(String),
}

/// **Everything that decides what a terminal process is**, as plain data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TerminalSpec {
    pub(crate) program: Program,
    /// The folder to start in; `None` is heca's own.
    pub(crate) cwd: Option<PathBuf>,
    /// The grid it starts with, `(columns, rows)`. A terminal on screen reports its real one.
    pub(crate) grid: (usize, usize),
}

/// A terminal process could not be started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpawnError(pub(crate) String);

impl std::fmt::Display for SpawnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What [`BackendStore::ensure`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ensured {
    /// A process was started.
    Spawned(TerminalId),
    /// The pane already had one, and it keeps running. `differs` says the spec asked for was not the
    /// one it is running — which is reported once, never acted on.
    Kept { id: TerminalId, differs: bool },
}

#[cfg(test)]
impl Ensured {
    pub(crate) fn id(self) -> TerminalId {
        match self {
            Self::Spawned(id) | Self::Kept { id, .. } => id,
        }
    }
}

struct Process {
    backend: Box<dyn PaneBackend>,
    spec: TerminalSpec,
}

/// The terminal processes, and which pane each was started for.
pub struct BackendStore {
    processes: HashMap<TerminalId, Process>,
    by_pane: HashMap<PaneId, TerminalId>,
    /// The next id to issue. Only ever goes up.
    next: u64,
}

impl BackendStore {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            by_pane: HashMap::new(),
            next: 1,
        }
    }

    /// **Start a terminal process for `pane`, or keep the one it has.** The only way a process
    /// begins.
    ///
    /// `make` builds the process and is only called when one is needed, so a pane that already has
    /// a terminal costs nothing here. A failed spawn leaves the store as it was.
    pub(crate) fn ensure(
        &mut self,
        pane: PaneId,
        spec: TerminalSpec,
        make: impl FnOnce(&TerminalSpec) -> Result<Box<dyn PaneBackend>, SpawnError>,
    ) -> Result<Ensured, SpawnError> {
        if let Some(&id) = self.by_pane.get(&pane) {
            let differs = self.processes.get(&id).is_some_and(|p| p.spec != spec);
            return Ok(Ensured::Kept { id, differs });
        }
        let backend = make(&spec)?;
        let id = TerminalId(self.next);
        self.next += 1;
        self.processes.insert(id, Process { backend, spec });
        self.by_pane.insert(pane, id);
        Ok(Ensured::Spawned(id))
    }

    /// The terminal process started for `pane`.
    pub(crate) fn terminal_of(&self, pane: PaneId) -> Option<TerminalId> {
        self.by_pane.get(&pane).copied()
    }

    /// The pane `id` was started for.
    pub(crate) fn pane_of(&self, id: TerminalId) -> Option<PaneId> {
        self.by_pane
            .iter()
            .find_map(|(pane, t)| (*t == id).then_some(*pane))
    }

    /// **End the terminal process started for `pane`** — the pane was closed. The only way one ends
    /// before heca does.
    pub fn kill_for_pane(&mut self, pane: PaneId) -> bool {
        match self.by_pane.remove(&pane) {
            Some(id) => self.processes.remove(&id).is_some(),
            None => false,
        }
    }

    /// End the terminal processes of all of these panes — a column or a workspace was deleted.
    pub fn kill_all(&mut self, panes: impl IntoIterator<Item = PaneId>) {
        for pane in panes {
            self.kill_for_pane(pane);
        }
    }

    /// Get a mutable reference to a pane's backend.
    pub fn get_mut(&mut self, pane: PaneId) -> Option<&mut dyn PaneBackend> {
        let id = self.by_pane.get(&pane)?;
        Some(self.processes.get_mut(id)?.backend.as_mut())
    }

    /// Get an immutable reference to a pane's backend.
    pub fn get(&self, pane: PaneId) -> Option<&dyn PaneBackend> {
        let id = self.by_pane.get(&pane)?;
        Some(self.processes.get(id)?.backend.as_ref())
    }

    /// Iterate over all backends mutably (e.g. for per-frame polling).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn PaneBackend>> {
        self.processes.values_mut().map(|p| &mut p.backend)
    }

    /// Collect pane IDs whose backends have exited and should be closed.
    pub fn pane_ids_to_close(&self) -> Vec<PaneId> {
        self.by_pane
            .iter()
            .filter_map(|(pane, id)| {
                self.processes
                    .get(id)
                    .is_some_and(|p| p.backend.should_close())
                    .then_some(*pane)
            })
            .collect()
    }
}

impl Default for BackendStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::backend::FakeBackend;

    fn spec(program: Program) -> TerminalSpec {
        TerminalSpec {
            program,
            cwd: None,
            grid: (80, 24),
        }
    }

    fn fake(_: &TerminalSpec) -> Result<Box<dyn PaneBackend>, SpawnError> {
        Ok(Box::new(FakeBackend::new(80, 24)))
    }

    #[test]
    fn ensure_starts_a_process_once_and_keeps_it() {
        let mut store = BackendStore::new();
        let pane = PaneId(1);
        let first = store
            .ensure(pane, spec(Program::Shell), fake)
            .expect("spawns");
        let Ensured::Spawned(id) = first else {
            panic!("the first ensure starts one, got {first:?}");
        };
        // A second ensure for the pane builds nothing and keeps the first.
        let again = store
            .ensure(pane, spec(Program::Shell), |_| {
                panic!("a pane that has a terminal must not spawn another")
            })
            .expect("keeps");
        assert_eq!(again, Ensured::Kept { id, differs: false });
        assert_eq!(store.terminal_of(pane), Some(id));
    }

    #[test]
    fn ensure_with_a_different_spec_keeps_the_running_one_and_says_so() {
        let mut store = BackendStore::new();
        let pane = PaneId(1);
        let id = store
            .ensure(pane, spec(Program::Shell), fake)
            .expect("spawns")
            .id();
        let other = store
            .ensure(pane, spec(Program::Command("htop".into())), fake)
            .expect("keeps");
        assert_eq!(other, Ensured::Kept { id, differs: true });
    }

    #[test]
    fn a_failed_spawn_leaves_the_store_as_it_was() {
        let mut store = BackendStore::new();
        let pane = PaneId(1);
        let failed = store.ensure(pane, spec(Program::Shell), |_| {
            Err(SpawnError("no pty".into()))
        });
        assert_eq!(failed, Err(SpawnError("no pty".into())));
        assert!(store.terminal_of(pane).is_none() && store.get(pane).is_none());
        // And the next try starts fresh.
        assert!(matches!(
            store.ensure(pane, spec(Program::Shell), fake),
            Ok(Ensured::Spawned(_))
        ));
    }

    #[test]
    fn closing_a_pane_ends_its_process_and_ids_are_never_reused() {
        let mut store = BackendStore::new();
        let (a, b) = (PaneId(1), PaneId(2));
        let first = store.ensure(a, spec(Program::Shell), fake).unwrap().id();
        assert!(store.kill_for_pane(a));
        assert!(store.get(a).is_none() && store.terminal_of(a).is_none());
        assert!(!store.kill_for_pane(a), "already gone");
        // A new terminal — even for the same pane — gets a new id.
        let second = store.ensure(a, spec(Program::Shell), fake).unwrap().id();
        let third = store.ensure(b, spec(Program::Shell), fake).unwrap().id();
        assert!(first != second && second != third && first != third);
    }

    #[test]
    fn a_terminal_is_found_by_its_id_and_by_its_pane() {
        let mut store = BackendStore::new();
        let pane = PaneId(7);
        let id = store.ensure(pane, spec(Program::Shell), fake).unwrap().id();
        assert_eq!(store.pane_of(id), Some(pane));
        assert!(store.get_mut(pane).is_some());
    }
}
