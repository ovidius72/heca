//! Backend storage — wraps the raw `HashMap<PaneId, Box<dyn PaneBackend>>` with
//! a narrow, explicit API so lifecycle logic is not scattered across handlers.

use heca_core::backend::PaneBackend;
use heca_core::layout::PaneId;
use std::collections::HashMap;

/// Typed wrapper around the backend map.
///
/// Exposes only the operations that the rest of the app needs, keeping
/// pane/backend lifecycle ownership explicit.
pub struct BackendStore {
    map: HashMap<PaneId, Box<dyn PaneBackend>>,
}

impl BackendStore {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Insert a backend for the given pane ID.
    pub fn insert_for_pane(&mut self, pane_id: PaneId, backend: Box<dyn PaneBackend>) {
        self.map.insert(pane_id, backend);
    }

    /// Remove and return the backend for the given pane ID, if any.
    pub fn remove_for_pane(&mut self, pane_id: PaneId) -> Option<Box<dyn PaneBackend>> {
        self.map.remove(&pane_id)
    }

    /// Get a mutable reference to a backend.
    pub fn get_mut(&mut self, pane_id: PaneId) -> Option<&mut dyn PaneBackend> {
        match self.map.get_mut(&pane_id) {
            Some(b) => Some(b.as_mut()),
            None => None,
        }
    }

    /// Get an immutable reference to a backend.
    pub fn get(&self, pane_id: PaneId) -> Option<&dyn PaneBackend> {
        match self.map.get(&pane_id) {
            Some(b) => Some(b.as_ref()),
            None => None,
        }
    }

    /// Iterate over all backends mutably (e.g. for per-frame polling).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn PaneBackend>> {
        self.map.values_mut()
    }

    /// Iterate over all backends mutably together with their pane IDs, so
    /// callers can resolve per-pane state (e.g. per-pane font zoom cell sizes).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (PaneId, &mut Box<dyn PaneBackend>)> {
        self.map.iter_mut().map(|(id, backend)| (*id, backend))
    }

    /// Collect pane IDs whose backends have exited and should be closed.
    pub fn pane_ids_to_close(&self) -> Vec<PaneId> {
        self.map
            .iter()
            .filter_map(|(pane_id, backend)| backend.should_close().then_some(*pane_id))
            .collect()
    }

    /// Remove backends for all pane IDs in the given iterator.
    ///
    /// Convenience helper for batch cleanup when deleting a column or workspace.
    pub fn remove_all(&mut self, pane_ids: impl IntoIterator<Item = PaneId>) {
        for pid in pane_ids {
            self.map.remove(&pid);
        }
    }
}

impl Default for BackendStore {
    fn default() -> Self {
        Self::new()
    }
}
