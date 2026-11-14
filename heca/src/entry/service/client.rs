//! **The window's side of a service** — what a component reads, kept in step with the server.
//!
//! The extension says how a snapshot and a delta change its value; everything else is here: the
//! revision and server run each value was built from, the fresh snapshot asked for when a delta
//! does not follow, and the reset when the window changes session.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, SignalWith, signal};

use crate::server::wire::service::{ServiceUpdate, ServiceUpdateKind};

use super::{Service, Update};

/// **What a service shows this window**, as a reactive value. Copy it into any component; reading
/// it while building or painting redraws that component when it changes.
pub struct ServiceState<C: 'static> {
    value: Signal<C>,
}

impl<C: 'static> Clone for ServiceState<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: 'static> Copy for ServiceState<C> {}

impl<C: 'static> ServiceState<C> {
    pub(super) fn new(initial: C) -> Self {
        Self { value: signal(initial) }
    }

    /// Read it without copying it out.
    pub fn with<R>(&self, read: impl FnOnce(&C) -> R) -> R {
        self.value.with(read)
    }

    /// A copy of it.
    pub fn get(&self) -> C
    where
        C: Clone,
    {
        self.value.get()
    }
}

/// Whether an update could be applied.
enum Applied {
    Done,
    /// It did not follow what this window holds: ask for a snapshot.
    OutOfStep,
}

/// How the extension folds a snapshot or delta into its window-side value.
type ApplyFn<S, C> = Rc<dyn Fn(&mut C, Update<<S as Service>::Snapshot, <S as Service>::Delta>)>;

/// One service's window-side state, whatever its types.
trait Entry {
    fn reset(&self);
    fn apply(&self, update: ServiceUpdate) -> Applied;
}

struct Typed<S: Service, C: 'static> {
    name: String,
    state: ServiceState<C>,
    initial: C,
    apply: ApplyFn<S, C>,
    /// The server run and revision the value was built from; `None` before the first snapshot.
    held: Cell<Option<(u64, u64)>>,
}

impl<S: Service, C: Clone + 'static> Typed<S, C> {
    fn refuse(&self, error: impl std::fmt::Display) {
        crate::notification::Notification::danger(format!("Service {} sent something unreadable", self.name))
            .body(error.to_string())
            .dedup_key(format!("service:{}:unreadable", self.name))
            .send();
    }
}

impl<S: Service, C: Clone + 'static> Entry for Typed<S, C> {
    fn reset(&self) {
        self.held.set(None);
        self.state.value.set(self.initial.clone());
    }

    fn apply(&self, update: ServiceUpdate) -> Applied {
        if update.schema_version != S::SCHEMA_VERSION {
            self.refuse(format!(
                "it speaks version {} and this window version {}",
                update.schema_version,
                S::SCHEMA_VERSION
            ));
            return Applied::Done;
        }
        match update.kind {
            ServiceUpdateKind::Snapshot(value) => match serde_json::from_value(value) {
                Ok(snapshot) => {
                    let mut fresh = self.initial.clone();
                    (self.apply)(&mut fresh, Update::Snapshot(snapshot));
                    self.state.value.set(fresh);
                    self.held.set(Some((update.server_epoch, update.revision)));
                }
                Err(error) => {
                    self.refuse(error);
                    self.reset();
                }
            },
            ServiceUpdateKind::Delta { base_revision, value } => {
                if self.held.get() != Some((update.server_epoch, base_revision)) {
                    return Applied::OutOfStep;
                }
                match serde_json::from_value(value) {
                    Ok(delta) => {
                        self.state.value.update(|current| (self.apply)(current, Update::Delta(delta)));
                        self.held.set(Some((update.server_epoch, update.revision)));
                    }
                    Err(error) => {
                        self.refuse(error);
                        return Applied::OutOfStep;
                    }
                }
            }
            ServiceUpdateKind::Removed => self.reset(),
        }
        Applied::Done
    }
}

thread_local! {
    /// Every service's window-side state, by name. Thread-local like the declarations.
    static ENTRIES: RefCell<HashMap<String, Rc<dyn Entry>>> = RefCell::new(HashMap::new());
}

pub(super) fn register<S: Service, C: Clone + 'static>(
    name: &str,
    state: ServiceState<C>,
    initial: C,
    apply: ApplyFn<S, C>,
) {
    let entry = Typed::<S, C> { name: name.to_owned(), state, initial, apply, held: Cell::new(None) };
    ENTRIES.with(|entries| entries.borrow_mut().insert(name.to_owned(), Rc::new(entry)));
}

/// Apply what the server sent this window. `reset` first when the window changed session or left
/// one, so nothing of the old one survives. `resync` is told each service that fell out of step.
pub(crate) fn apply_updates(reset: bool, updates: Vec<ServiceUpdate>, mut resync: impl FnMut(&str)) {
    let entries: Vec<_> = ENTRIES.with(|entries| entries.borrow().values().cloned().collect());
    if reset {
        for entry in &entries { entry.reset(); }
    }
    for update in updates {
        let Some(entry) = ENTRIES.with(|entries| entries.borrow().get(&update.service).cloned()) else { continue };
        let service = update.service.clone();
        if let Applied::OutOfStep = entry.apply(update) { resync(&service); }
    }
}

#[cfg(test)]
pub(super) fn reset() {
    ENTRIES.with(|entries| entries.borrow_mut().clear());
}
