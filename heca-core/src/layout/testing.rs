//! Fixtures for tests of the layout, and of anything built on it: a thing and the view one window has of it, held together
//! the way a window holds them, with `m()` for the moves and `r()` for the reads.

use std::ops::{Deref, DerefMut};

use super::scrolling::{ScrollingMut, ScrollingRef};
use super::types::{LayoutOptions, Rectangle, SessionId, Size, WorkspaceId};
use super::{
    Layout, LayoutMut, ScrollView, ScrollingSpace, Session, WindowView, Workspace, WorkspaceMut,
};

/// A scrolling space and one window's view of it.
pub struct Seen {
    /// The content.
    pub space: ScrollingSpace,
    /// The window's view of it.
    pub view: ScrollView,
}

impl Seen {
    /// An empty space with a view of `area` at `scale`.
    pub fn new(area: Rectangle, scale: f64, options: LayoutOptions) -> Self {
        Self {
            space: ScrollingSpace::new(options),
            view: ScrollView::new(area, scale),
        }
    }

    /// The moves.
    pub fn m(&mut self) -> ScrollingMut<'_> {
        self.space.through_mut(&mut self.view)
    }

    /// The reads.
    pub fn r(&self) -> ScrollingRef<'_> {
        self.space.through(&self.view)
    }
}

impl Deref for Seen {
    type Target = ScrollingSpace;

    fn deref(&self) -> &ScrollingSpace {
        &self.space
    }
}

impl DerefMut for Seen {
    fn deref_mut(&mut self) -> &mut ScrollingSpace {
        &mut self.space
    }
}

/// A workspace and one window's view of it.
pub struct Shown {
    /// The content.
    pub ws: Workspace,
    /// The window's view of it.
    pub view: ScrollView,
}

impl Shown {
    /// An empty workspace laid out in `size`.
    pub fn new(size: Size) -> Self {
        Self {
            ws: Workspace::new(WorkspaceId(0), LayoutOptions::default()),
            view: ScrollView::new(Rectangle::from_size(size), 1.0),
        }
    }

    /// The moves.
    pub fn m(&mut self) -> WorkspaceMut<'_> {
        self.ws.through_mut(&mut self.view)
    }
}

impl Deref for Shown {
    type Target = Workspace;

    fn deref(&self) -> &Workspace {
        &self.ws
    }
}

impl DerefMut for Shown {
    fn deref_mut(&mut self) -> &mut Workspace {
        &mut self.ws
    }
}

/// A session and one window's view of it.
pub struct Windowed {
    /// The content every window shares.
    pub session: Session,
    /// This window's view of it.
    pub view: WindowView,
}

impl Windowed {
    /// A session with one empty workspace, in a window of `size` at `scale`.
    pub fn new(size: Size, scale: f64) -> Self {
        Self {
            session: Session::new(SessionId(1), LayoutOptions::default()),
            view: WindowView::new(size, scale),
        }
    }

    /// A window of `size` on a session of the given shape: one workspace per entry, each a list of
    /// columns, each a list of pane ids (the first one makes the column, the others stack under
    /// it). Workspace 0 is shown.
    pub fn with_shape(shape: &[&[&[u64]]]) -> Self {
        let mut window = Self::new(Size::new(1000.0, 800.0), 1.0);
        for _ in 1..shape.len() {
            window.m().add_workspace();
        }
        for (idx, columns) in shape.iter().enumerate() {
            window.show(idx);
            for panes in *columns {
                let mut ids = panes.iter();
                let Some(first) = ids.next() else { continue };
                window.m().add_pane(super::Pane::new(super::PaneId(*first), "p"), None, true);
                let col = window.l().workspace(idx).map_or(0, |ws| ws.scrolling.columns.len() - 1);
                for id in ids {
                    let pane = super::Pane::new(super::PaneId(*id), "p");
                    window.ws().scroll_mut().add_pane_to_column(col, None, pane, false);
                }
            }
        }
        window.show(0);
        window
    }

    /// The reads.
    pub fn l(&self) -> Layout<'_> {
        self.session.through(&self.view)
    }

    /// The moves.
    pub fn m(&mut self) -> LayoutMut<'_> {
        self.session.through_mut(&mut self.view)
    }

    /// Show workspace `idx` (no animation) — the state of a window that is already there.
    pub fn show(&mut self, idx: usize) {
        self.view.active_workspace = idx;
    }

    /// Put workspace `idx` at a given scroll: `offset` from the active column, with column
    /// `active_column` focused — the state of a window that has been scrolled.
    pub fn scrolled(&mut self, idx: usize, offset: f64, active_column: usize) {
        let id = self.session.workspaces[idx].id;
        let scroll = self.view.scroll_mut(id);
        scroll.offset = super::ViewOffset::Static(offset);
        scroll.active_column = active_column;
    }

    /// The workspace the window shows, moved by it.
    pub fn ws(&mut self) -> WorkspaceMut<'_> {
        let ws = &mut self.session.workspaces[self.view.active_workspace];
        let scroll = self.view.scroll_mut(ws.id);
        ws.through_mut(scroll)
    }
}
