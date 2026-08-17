//! **Chrome drag identities** — what a drag id in this app refers to (F003/P082/T427 split).
//!
//! The drag framework is domain-neutral: an id is an opaque `usize`. This is the app-side map that
//! gives one meaning, so a kind round-trips through `drag::source_at` / `resolve_at` without
//! anything trusting a raw number.

use heca_core::layout::PaneId;
use heca_grid_ui::DragItemId;

/// What a sidebar [`DragItemId`] refers to. The drag framework is domain-neutral
/// (ids are opaque `usize`); this app-side map gives them meaning. `ColumnId` can't
/// be the id directly — it's assigned inconsistently and can collide with a `PaneId`
/// (`scrolling.rs` builds `ColumnId(pane.id.0)`), so kind is decided by this map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChromeDragItem {
    /// A pane card (drag source + drop target).
    Pane(PaneId),
    /// A column [`MarkerGroup`], addressed positionally (drag source + drop target).
    Column { ws: usize, col: usize },
    /// A workspace [`DockFrame`] (drop target only — drop a column here to move it
    /// into that workspace).
    Workspace { ws: usize },
}

/// Build-time registry that hands out dense [`DragItemId`]s (id = push index) and
/// records what each refers to. Lives on [`RetainedChrome`]; rebuilt with the tree.
#[derive(Default, Clone, Debug)]
pub(crate) struct DragItemRegistry {
    items: Vec<ChromeDragItem>,
}

impl DragItemRegistry {
    /// Register a draggable/droppable item and return its freshly-assigned id.
    pub(crate) fn register(&mut self, item: ChromeDragItem) -> DragItemId {
        let id = DragItemId::new(self.items.len());
        self.items.push(item);
        id
    }

    /// Decode an id back to what it refers to (`None` if not from this build).
    pub(crate) fn get(&self, id: DragItemId) -> Option<&ChromeDragItem> {
        self.items.get(id.raw())
    }

    /// All registered items, in id order.
    #[cfg(test)]
    pub(crate) fn items(&self) -> &[ChromeDragItem] {
        &self.items
    }
}

