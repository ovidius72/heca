//! **Chrome drag identities** — what a drag id in this app refers to
//!
//! The drag framework is domain-neutral: an id is an opaque `usize`. This is the app-side map that
//! gives one meaning, so a kind round-trips through `drag::source_at` / `resolve_at` without
//! anything trusting a raw number.

use std::collections::HashMap;

use heca_core::layout::PaneId;

/// What one of this component's own row names means. The drag framework is domain-neutral — it
/// hands back the **name the row declared about itself** (`"pane:7"`) and knows nothing else — so
/// this is the app-side map that gives one meaning. A key is never parsed: the component that wrote
/// it is the only thing that may say what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChromeDragItem {
    /// A pane card (drag source + drop target).
    Pane(PaneId),
    /// A column [`MarkerGroup`](heca_grid_ui::widgets::MarkerGroup), addressed positionally (drag source + drop target).
    Column { ws: usize, col: usize },
    /// A workspace frame (drop target only — drop a column here to move it
    /// into that workspace).
    Workspace { ws: usize },
    /// A mounted dock, by its container id — the seat the host puts round it (drag source when it
    /// is movable, and a place to land beside).
    Dock(String),
    /// The space between two docks in a region: a dock dropped there lands after `after`.
    DockGap { after: String },
}

/// What each of this component's row names means, recorded as the tree is built and read back when
/// a drag starts or lands. Lives on [`RetainedChrome`](crate::chrome::RetainedChrome); rebuilt with the tree.
///
/// The widget carries only its own name; what a name *means* stays here, where the component that
/// wrote it lives, so a plugin's row is named the same way.
#[derive(Default, Clone, Debug)]
pub(crate) struct DragItemRegistry {
    items: HashMap<String, ChromeDragItem>,
}

impl DragItemRegistry {
    /// Record what one of this component's row names refers to.
    pub(crate) fn register(&mut self, key: impl Into<String>, item: ChromeDragItem) {
        self.items.insert(key.into(), item);
    }

    /// What a name means (`None` when it is not one this build wrote).
    pub(crate) fn get(&self, key: &str) -> Option<&ChromeDragItem> {
        self.items.get(key)
    }

    /// Everything recorded, for tests.
    #[cfg(test)]
    pub(crate) fn items(&self) -> Vec<ChromeDragItem> {
        self.items.values().cloned().collect()
    }
}
