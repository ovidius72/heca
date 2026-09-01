//! **Chrome drag identities** — what a drag id in this app refers to (F003/P082/T427 split).
//!
//! The drag framework is domain-neutral: an id is an opaque `usize`. This is the app-side map that
//! gives one meaning, so a kind round-trips through `drag::source_at` / `resolve_at` without
//! anything trusting a raw number.

use std::collections::HashMap;

use heca_core::layout::PaneId;

/// What one of this component's own row names means. The drag framework is domain-neutral — it
/// hands back the **name the row declared about itself** (`"pane:7"`) and knows nothing else — so
/// this is the app-side map that gives one meaning. A key is never parsed: the component that wrote
/// it is the only thing that may say what it is, exactly as `Provider::context_path` answers for
/// the right-click menu.
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

/// What each of this component's row names means, recorded as the tree is built and read back when
/// a drag starts or lands. Lives on [`RetainedChrome`]; rebuilt with the tree.
///
/// It used to hand out an opaque index and the widget carried that instead of its own name, so a
/// row said who it was twice — and a plugin's row could say it neither time, because the list of
/// draggable surfaces was a closed enum in our source. The widget layer now carries the name only;
/// what a name *means* stays here, where the component that wrote it lives.
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

