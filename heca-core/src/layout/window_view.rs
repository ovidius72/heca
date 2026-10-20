//! **What one window sees** of the layout — and nothing else.
//!
//! The session holds the content every window shares: which workspaces, columns and panes exist.
//! Where a window is looking at that content is its own: the workspace it shows, the animation
//! between two workspaces, how far each workspace is scrolled, how big the window is. Two windows
//! on one session each hold a [`WindowView`] and move independently.
//!
//! The layout reads and moves a view through [`Layout`](super::Layout) and
//! [`LayoutMut`](super::LayoutMut); nothing outside this crate has to know what is in one, so
//! nothing in it is public: a caller outside `heca-core` only ever holds a [`WindowView`] and
//! hands it to the session.

use std::collections::HashMap;

use super::animation::{Animated, SwipeTracker};
use super::motion::Motion;
use super::types::{ColumnId, PaneId, Point, Rectangle, Size, WorkspaceId};
use super::view_offset::ViewOffset;

/// Workspace switching state.
#[derive(Debug, Clone, Default)]
pub enum WorkspaceSwitch {
    #[default]
    None,
    /// Animated transition between workspaces.
    Animation {
        from_idx: usize,
        to_idx: usize,
        progress: Animated<f64>,
    },
    /// Gesture-driven switch (touchpad swipe).
    Gesture {
        tracker: SwipeTracker,
        current_idx: f64,
    },
}

/// What a window sees of **one** workspace's scrolling strip: how far it is scrolled, which column
/// has the focus, and the box and scale it is laid out in.
#[derive(Debug, Clone)]
pub struct ScrollView {
    /// Horizontal scroll offset, relative to the active column.
    pub(crate) offset: ViewOffset,
    /// Index of the active column.
    pub(crate) active_column: usize,
    /// Whether to activate the previous column on removal, and the offset to restore.
    pub(crate) activate_prev_on_removal: Option<f64>,
    /// The box the columns are laid out in.
    pub(crate) area: Rectangle,
    /// Scale factor.
    pub(crate) scale: f64,
    /// What is sliding in this workspace on this window's screen.
    pub(crate) motion: Motion,
    /// **Which pane this window has focused in each column.** By id, not row, so adding, removing
    /// or reordering panes never renumbers it. A column with no entry — or one whose focused pane
    /// is gone — falls back to its first pane. The focused pane is per window (F012, decision
    /// 463ccd07), so it lives here and not in the shared [`Column`](super::column::Column).
    pub(crate) active_pane: HashMap<ColumnId, PaneId>,
}

impl ScrollView {
    /// A view that has not been scrolled, looking at the first column.
    pub fn new(area: Rectangle, scale: f64) -> Self {
        Self {
            offset: ViewOffset::Static(0.0),
            active_column: 0,
            activate_prev_on_removal: None,
            area,
            scale,
            motion: Motion::default(),
            active_pane: HashMap::new(),
        }
    }

    /// The pane this window has focused in the column `id`, or `None` when it has focused none.
    /// The caller resolves it against the column's current panes, falling back to the first.
    pub(crate) fn focused_pane(&self, id: ColumnId) -> Option<PaneId> {
        self.active_pane.get(&id).copied()
    }

    /// Focus pane `pane` in column `id` for this window.
    pub(crate) fn focus_pane(&mut self, id: ColumnId, pane: PaneId) {
        self.active_pane.insert(id, pane);
    }

    /// Forget the focus of every column `keep` does not keep — columns the content no longer has.
    pub(crate) fn retain_columns(&mut self, keep: impl Fn(ColumnId) -> bool) {
        self.active_pane.retain(|id, _| keep(*id));
    }
}

/// Everything one window holds of the layout: the workspace it shows, the switch animation, its
/// size and scale, and a [`ScrollView`] for each workspace it has looked at.
#[derive(Debug, Clone)]
pub struct WindowView {
    /// The shown workspace, by id. `None` until the window is told which to show — it falls back
    /// to the first. An id, not an index, so a workspace another window removes never leaves this
    /// one pointing at the wrong workspace.
    pub(crate) active_workspace: Option<WorkspaceId>,
    /// The animation between two workspaces, if one is running.
    pub(crate) switch: WorkspaceSwitch,
    viewport: Size,
    scale: f64,
    scrolls: HashMap<WorkspaceId, ScrollView>,
    /// What a workspace this window has not looked at yet is seen as: unscrolled, at the first
    /// column. Reads fall back to it so they never have to create anything.
    blank: ScrollView,
}

impl WindowView {
    /// A window of `viewport` size at `scale`, showing the first workspace.
    pub fn new(viewport: Size, scale: f64) -> Self {
        Self {
            active_workspace: None,
            switch: WorkspaceSwitch::None,
            viewport,
            scale,
            scrolls: HashMap::new(),
            blank: ScrollView::new(Self::area_of(viewport), scale),
        }
    }

    /// The window's size (full window, before any chrome).
    pub(crate) fn viewport(&self) -> Size {
        self.viewport
    }

    /// The window's scale factor.
    pub(crate) fn scale(&self) -> f64 {
        self.scale
    }

    /// What this window sees of workspace `ws`.
    pub(crate) fn scroll(&self, ws: WorkspaceId) -> &ScrollView {
        self.scrolls.get(&ws).unwrap_or(&self.blank)
    }

    /// What this window sees of workspace `ws`, made on first use.
    pub(crate) fn scroll_mut(&mut self, ws: WorkspaceId) -> &mut ScrollView {
        let blank = &self.blank;
        self.scrolls.entry(ws).or_insert_with(|| blank.clone())
    }

    /// Forget every workspace `keep` does not keep: they were removed from the session.
    pub(crate) fn forget_all_but(&mut self, keep: impl Fn(WorkspaceId) -> bool) {
        self.scrolls.retain(|id, _| keep(*id));
    }

    /// The window was resized. This records the new size and what a workspace not yet looked at
    /// will be laid out in; the workspaces already seen are re-laid out by
    /// [`LayoutMut::update_viewport`](super::LayoutMut::update_viewport), which has to read the old
    /// area first to carry the floating panes across — so it is the only way to resize a window.
    pub(super) fn set_viewport(&mut self, size: Size) {
        self.viewport = size;
        self.blank.area = Self::area_of(size);
    }

    fn area_of(size: Size) -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), size)
    }
}
