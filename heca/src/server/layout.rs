//! **The layout as the server may use it** — by index and by id, and nothing that depends on what a
//! window is showing.
//!
//! A window shows one workspace and has one column active in it; the server must not ask. It is
//! told where its asker is ([`Asker`](super::Asker)) and addresses everything else by position or
//! id. This type is the whole of what a handler can reach, so "the server does not read the view"
//! is a fact the compiler checks: there is no `active_workspace`, no switching and no viewport
//! here to call.

use heca_core::layout::{
    Added, LayoutMut, Moved, PaneId, Removed, ScrollingSpace, Session, WorkspaceMut,
};

/// The session, moved by index and by id.
pub(crate) struct ServerLayout<'a>(LayoutMut<'a>);

macro_rules! delegate {
    ($($(#[$doc:meta])* fn $name:ident(&mut self $(, $arg:ident: $ty:ty)* $(,)?) -> $ret:ty;)*) => {
        $($(#[$doc])* pub(crate) fn $name(&mut self $(, $arg: $ty)*) -> $ret {
            self.0.$name($($arg),*)
        })*
    };
}

impl<'a> ServerLayout<'a> {
    pub(crate) fn new(layout: LayoutMut<'a>) -> Self {
        Self(layout)
    }

    /// The shared content, to read.
    pub(crate) fn session(&self) -> &Session {
        self.0.reader().session()
    }

    /// Workspace `idx`'s columns, to change with **no view**: the content and nothing a window
    /// looks through. A change made here is shown to a window by the fact it reports.
    pub(crate) fn columns_mut(&mut self, idx: usize) -> Option<&mut ScrollingSpace> {
        self.0.columns_mut(idx)
    }

    /// Workspace `idx`, to change.
    pub(crate) fn workspace_mut(&mut self, idx: usize) -> Option<WorkspaceMut<'_>> {
        self.0.workspace_mut(idx)
    }

    delegate! {
        /// The next identity from the session's one counter.
        fn next_id(&mut self) -> u64;
        fn rename_pane(&mut self, pane: PaneId, name: &str) -> bool;
        fn rename_workspace(&mut self, ws: usize, name: &str) -> bool;
        fn rename_column(&mut self, ws: usize, col: usize, name: &str) -> bool;
        fn remove_pane_anywhere(&mut self, pane: PaneId) -> Option<Removed>;
        fn remove_column_with_panes(&mut self, ws: usize, col: usize) -> Option<Removed>;
        fn remove_workspace_with_panes(&mut self, ws: usize) -> Option<Removed>;
        fn move_pane_to_column(&mut self, pane: PaneId, dst_col: usize) -> Option<Moved>;
        fn move_pane_to_new_column(&mut self, pane: PaneId) -> Option<Moved>;
        fn move_pane_to_workspace(
            &mut self,
            pane: PaneId,
            dst_ws: usize,
            dst_col: usize,
            join: bool,
        ) -> Option<Moved>;
        fn move_column_to_workspace(
            &mut self,
            src_ws: usize,
            col: usize,
            dst_ws: usize,
        ) -> Option<Moved>;
        fn move_column(
            &mut self,
            src_ws: usize,
            src_col: usize,
            dst_ws: usize,
            dst_idx: usize,
            activate: bool,
        ) -> Option<Moved>;
        fn take_pane_into(&mut self, pane: PaneId, dst_ws: usize, activate: bool) -> Option<Moved>;
        fn place_pane(
            &mut self,
            pane: PaneId,
            dst_ws: usize,
            col: usize,
            row: Option<usize>,
        ) -> Option<Moved>;
        fn swap_panes(&mut self, a: PaneId, b: PaneId) -> bool;
        fn swap_columns_between(
            &mut self,
            a_ws: usize,
            a_col: usize,
            b_ws: usize,
            b_col: usize,
        ) -> bool;
        fn add_pane_to_column(&mut self, ws: usize, col: usize) -> Option<Added>;
        fn add_column(&mut self, ws: usize) -> Option<Added>;
    }
}
