//! **The one owner of terminal processes.**
//!
//! A terminal process is what runs behind a [`Terminal`](crate::chrome::terminal::Terminal): a
//! shell or a command, on a PTY. Nothing outside this store holds one. Ids are issued here, once,
//! and are never reused; a pane's terminal is found through the pane it was started for, so the
//! readers that ask "the backend of this pane" keep their question while the identity underneath
//! is the store's.
//!
//! **There is one door in: [`ensure`](BackendStore::ensure).** It starts a process or, if its
//! [`TerminalOwner`] already has one, keeps it. An owner is a pane, or a name an extension gave a
//! terminal it placed (`demo.shell`). A process lives until its owner ends it ([`kill_for_pane`],
//! [`kill`](BackendStore::kill)) or heca ends — rebuilding a tree, removing the component that
//! shows it, or closing the dock it was in never touches it. (The word is "terminal process", never
//! "session": a session is an instance of heca, F012.)

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

/// **Who a terminal process was started for** — what [`ensure`](BackendStore::ensure) keeps one
/// process per.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum TerminalOwner {
    /// A pane: closing the pane ends it.
    Pane(PaneId),
    /// A terminal an extension placed, by its qualified name (`demo.shell`): the owner half comes
    /// from the extension's handle, so two extensions cannot reach each other's. Nothing but an
    /// explicit kill, or heca ending, stops it.
    Named(String),
}

impl From<PaneId> for TerminalOwner {
    fn from(pane: PaneId) -> Self {
        Self::Pane(pane)
    }
}

/// The terminal processes, and who each was started for.
///
/// **An id is the identity; a process is what runs behind it, and may not exist yet.** An owner is
/// given its id the first time anyone asks ([`id_for`](BackendStore::id_for)), so the component that
/// shows a terminal has a stable name from the frame it is built, before — or without — a process.
pub struct BackendStore {
    processes: HashMap<TerminalId, Process>,
    by_owner: HashMap<TerminalOwner, TerminalId>,
    /// The next id to issue. Only ever goes up.
    next: u64,
}

impl BackendStore {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            by_owner: HashMap::new(),
            next: 1,
        }
    }

    /// **The id of `owner`'s terminal**, issued the first time it is asked for and the same every
    /// time after. Having an id is not having a process: that is [`ensure`](Self::ensure)'s.
    pub(crate) fn id_for(&mut self, owner: impl Into<TerminalOwner>) -> TerminalId {
        let next = &mut self.next;
        *self.by_owner.entry(owner.into()).or_insert_with(|| {
            let id = TerminalId(*next);
            *next += 1;
            id
        })
    }

    /// **Start a terminal process for `owner`, or keep the one it has.** The only way a process
    /// begins.
    ///
    /// `make` builds the process and is only called when one is needed, so an owner that already has
    /// a terminal costs nothing here. A failed spawn leaves no process behind.
    pub(crate) fn ensure(
        &mut self,
        owner: impl Into<TerminalOwner>,
        spec: TerminalSpec,
        make: impl FnOnce(&TerminalSpec) -> Result<Box<dyn PaneBackend>, SpawnError>,
    ) -> Result<Ensured, SpawnError> {
        let id = self.id_for(owner);
        if let Some(running) = self.processes.get(&id) {
            return Ok(Ensured::Kept {
                id,
                differs: running.spec != spec,
            });
        }
        let backend = make(&spec)?;
        self.processes.insert(id, Process { backend, spec });
        Ok(Ensured::Spawned(id))
    }

    /// The id issued to `pane`'s terminal, whether or not a process runs behind it yet.
    pub(crate) fn identity_of(&self, pane: PaneId) -> Option<TerminalId> {
        self.by_owner.get(&TerminalOwner::Pane(pane)).copied()
    }

    /// Whether `id` was issued to an owner that still exists.
    pub(crate) fn is_issued(&self, id: TerminalId) -> bool {
        self.by_owner.values().any(|issued| *issued == id)
    }

    /// The pane `id` was started for — `None` for a terminal no pane owns.
    pub(crate) fn pane_of(&self, id: TerminalId) -> Option<PaneId> {
        self.by_owner.iter().find_map(|(owner, t)| match owner {
            TerminalOwner::Pane(pane) if *t == id => Some(*pane),
            _ => None,
        })
    }

    /// **End the terminal process started for `pane`** — the pane was closed. The only way one ends
    /// before heca does, besides [`kill`](Self::kill).
    pub fn kill_for_pane(&mut self, pane: PaneId) -> bool {
        match self.by_owner.remove(&TerminalOwner::Pane(pane)) {
            Some(id) => self.processes.remove(&id).is_some(),
            None => false,
        }
    }

    /// **End the terminal process `id`**, whoever owns it — an explicit kill. The id is retired
    /// with it: asking for the owner's terminal again gives a new one.
    pub(crate) fn kill(&mut self, id: TerminalId) -> bool {
        self.by_owner.retain(|_, t| *t != id);
        self.processes.remove(&id).is_some()
    }

    /// End the terminal processes of all of these panes — a column or a workspace was deleted.
    pub fn kill_all(&mut self, panes: impl IntoIterator<Item = PaneId>) {
        for pane in panes {
            self.kill_for_pane(pane);
        }
    }

    /// Get a mutable reference to a pane's backend.
    pub fn get_mut(&mut self, pane: PaneId) -> Option<&mut dyn PaneBackend> {
        let id = *self.by_owner.get(&TerminalOwner::Pane(pane))?;
        self.get_mut_by_id(id)
    }

    /// Get an immutable reference to a pane's backend.
    pub fn get(&self, pane: PaneId) -> Option<&dyn PaneBackend> {
        let id = *self.by_owner.get(&TerminalOwner::Pane(pane))?;
        self.get_by_id(id)
    }

    /// The backend of terminal `id`, whoever owns it.
    pub(crate) fn get_mut_by_id(&mut self, id: TerminalId) -> Option<&mut dyn PaneBackend> {
        Some(self.processes.get_mut(&id)?.backend.as_mut())
    }

    /// The backend of terminal `id`, whoever owns it.
    pub(crate) fn get_by_id(&self, id: TerminalId) -> Option<&dyn PaneBackend> {
        Some(self.processes.get(&id)?.backend.as_ref())
    }

    /// Iterate over all backends mutably (e.g. for per-frame polling).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn PaneBackend>> {
        self.processes.values_mut().map(|p| &mut p.backend)
    }

    /// Collect pane IDs whose backends have exited and should be closed.
    pub fn pane_ids_to_close(&self) -> Vec<PaneId> {
        self.by_owner
            .iter()
            .filter_map(|(owner, id)| match owner {
                TerminalOwner::Pane(pane) => self
                    .processes
                    .get(id)
                    .is_some_and(|p| p.backend.should_close())
                    .then_some(*pane),
                // A named terminal's program ending closes no pane: there is none.
                TerminalOwner::Named(_) => None,
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
        assert_eq!(store.identity_of(pane), Some(id));
        assert!(store.get(pane).is_some());
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
        assert!(store.get(pane).is_none());
        let reserved = store.id_for(pane);
        // And the next try starts fresh, under the id the pane already had.
        assert_eq!(
            store.ensure(pane, spec(Program::Shell), fake),
            Ok(Ensured::Spawned(reserved))
        );
    }

    #[test]
    fn closing_a_pane_ends_its_process_and_ids_are_never_reused() {
        let mut store = BackendStore::new();
        let (a, b) = (PaneId(1), PaneId(2));
        let first = store.ensure(a, spec(Program::Shell), fake).unwrap().id();
        assert!(store.kill_for_pane(a));
        assert!(store.get(a).is_none() && store.identity_of(a).is_none());
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

    #[test]
    fn an_owner_has_its_id_before_it_has_a_process() {
        let mut store = BackendStore::new();
        let pane = PaneId(3);
        let id = store.id_for(pane);
        assert_eq!(store.id_for(pane), id, "the same every time");
        assert!(store.get(pane).is_none() && store.get_by_id(id).is_none());
        let spawned = store.ensure(pane, spec(Program::Shell), fake).unwrap().id();
        assert_eq!(
            spawned, id,
            "the process runs behind the id that was already issued"
        );
        assert_eq!(store.identity_of(pane), Some(id));
        assert!(store.get(pane).is_some());
    }

    #[test]
    fn a_named_terminal_is_one_process_that_no_pane_closing_touches() {
        let mut store = BackendStore::new();
        let named = TerminalOwner::Named("demo.shell".into());
        let id = store
            .ensure(named.clone(), spec(Program::Shell), fake)
            .expect("spawns")
            .id();
        // Asked for again — a rebuilt dock, a reopened overlay — it is the same process.
        let again = store
            .ensure(named, spec(Program::Shell), |_| {
                panic!("a named terminal that is running must not spawn another")
            })
            .expect("keeps");
        assert_eq!(again, Ensured::Kept { id, differs: false });
        assert!(store.get_by_id(id).is_some());
        // A pane closing, even the first pane, is not its end; no pane owns it.
        store.kill_for_pane(PaneId(1));
        assert!(store.get_by_id(id).is_some());
        assert_eq!(store.pane_of(id), None);
        assert!(store.pane_ids_to_close().is_empty());
        // Only an explicit kill ends it.
        assert!(store.kill(id));
        assert!(store.get_by_id(id).is_none());
    }

    #[test]
    fn two_extensions_names_never_meet() {
        let mut store = BackendStore::new();
        let a = store
            .ensure(
                TerminalOwner::Named("a.shell".into()),
                spec(Program::Shell),
                fake,
            )
            .unwrap()
            .id();
        let b = store
            .ensure(
                TerminalOwner::Named("b.shell".into()),
                spec(Program::Shell),
                fake,
            )
            .unwrap()
            .id();
        assert_ne!(a, b);
    }
}
