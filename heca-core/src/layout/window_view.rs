//! **What one window sees** of the layout — and nothing else.
//!
//! The session holds the content every window shares: which workspaces, columns and panes exist.
//! Where a window is looking at that content is its own: the workspace it shows, the animation
//! between two workspaces, how far each workspace is scrolled, how big the window is. Two windows
//! on one session each hold a [`WindowView`] and move independently (F012, decision 2c249a29).
//!
//! The layout reads and moves a view through [`Layout`](super::Layout) and
//! [`LayoutMut`](super::LayoutMut); nothing outside this crate has to know what is in one, so
//! nothing in it is public: a caller outside `heca-core` only ever holds a [`WindowView`] and
//! hands it to the session.

use std::collections::HashMap;

use super::animation::{Animated, SwipeTracker};
use super::types::{Point, Rectangle, Size, WorkspaceId};
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
        }
    }
}

/// Everything one window holds of the layout: the workspace it shows, the switch animation, its
/// size and scale, and a [`ScrollView`] for each workspace it has looked at.
#[derive(Debug, Clone)]
pub struct WindowView {
    /// Index of the shown workspace.
    pub(crate) active_workspace: usize,
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
            active_workspace: 0,
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

    /// Forget workspace `ws`: it was removed from the session.
    pub(crate) fn forget(&mut self, ws: WorkspaceId) {
        self.scrolls.remove(&ws);
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
