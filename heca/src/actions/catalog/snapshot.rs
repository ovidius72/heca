//! **Which actions this run of the server has** — the names, as a value a service can keep.

use std::sync::Arc;

use super::ActionCatalog;

/// **The names of the actions this run of the server has**, taken when its services start.
///
/// It holds names only: no handlers, and no way back to the catalog or the registry. It is a list
/// of what exists, never a grant — whether an action may run is still decided by the action policy
/// when it is asked. It is not saved: a server that starts again takes a new one.
///
/// It can be kept and sent to another thread.
#[derive(Clone, Debug)]
pub struct ActionCatalogSnapshot {
    /// Sorted, so a name is found without a walk.
    names: Arc<[String]>,
}

impl ActionCatalogSnapshot {
    pub(super) fn of(catalog: &ActionCatalog) -> Self {
        let mut names: Vec<String> = catalog.all().map(|meta| meta.name.clone()).collect();
        names.sort_unstable();
        Self {
            names: names.into(),
        }
    }

    /// Whether an action of this name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.names
            .binary_search_by(|have| have.as_str().cmp(name))
            .is_ok()
    }

    /// Every action's name, in alphabetical order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }
}

impl ActionCatalog {
    /// The names of every action in this catalog, as a value that can be kept.
    pub fn snapshot(&self) -> ActionCatalogSnapshot {
        ActionCatalogSnapshot::of(self)
    }
}
