//! **The shared content as plain data a window draws from** (F012, P099/T544 C3).
//!
//! A window attached to a session over the socket has no [`Session`] to read — it draws from this
//! projection the server sends. It carries the *structure* a window needs to lay out and draw
//! (which workspaces, columns and panes exist, their names, widths and height shares) and nothing a
//! window does not: no [`PaneRuntime`](crate::runtime::PaneRuntime) (process / git / agent state
//! travels as events) and no per-window view (that is the client's
//! [`WindowView`](super::window_view::WindowView)). The layout engine runs on the client over a
//! [`Session`] rebuilt from this plus the window's own view; runtime facts arrive separately and
//! update what is drawn, not the geometry.
//!
//! Also the shape P105 saves and restores a session from.

use super::column::{Column, Pane};
use super::scrolling::ScrollingSpace;
use super::session::Session;
use super::types::{ColumnId, ColumnWidth, LayoutOptions, PaneId, SessionId, WorkspaceId};
use super::workspace::{FloatOrigin, FloatingPane, Workspace};

/// Every window, column and pane a session holds, as plain data.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SessionContent {
    pub workspaces: Vec<WorkspaceContent>,
}

/// One workspace: its tiled columns and its floating panes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceContent {
    pub id: WorkspaceId,
    pub name: Option<String>,
    pub columns: Vec<ColumnContent>,
    pub floating: Vec<FloatingContent>,
}

/// One column and the panes stacked in it. `width` + `zoomed` + `full_width` are what the layout
/// engine needs to resolve its on-screen width (see [`Column::resolve_width`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ColumnContent {
    pub id: ColumnId,
    pub name: Option<String>,
    pub width: ColumnWidth,
    /// Zoomed to the viewport (a previous width is held to restore it).
    pub zoomed: bool,
    pub full_width: bool,
    pub panes: Vec<PaneContent>,
}

/// One pane's drawable identity. No runtime: process / git / agent facts travel as events.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PaneContent {
    pub id: PaneId,
    /// The process-derived title; a window shows `name` over it when that is set.
    pub title: String,
    /// The user's rename, which overrides `title` when `Some`.
    pub name: Option<String>,
    /// This pane's share of its column's height, in `(0, 1]`; `None` = auto.
    pub height_share: Option<f64>,
}

/// A floating pane and where it floated out of.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FloatingContent {
    pub pane: PaneContent,
    pub origin: Option<FloatOrigin>,
}

impl Session {
    /// This session's shared content as plain data a window draws from (F012). The per-window view
    /// and every pane's runtime are deliberately absent — they live in the client and in events.
    pub fn content(&self) -> SessionContent {
        SessionContent {
            workspaces: self.workspaces.iter().map(WorkspaceContent::of).collect(),
        }
    }
}

impl WorkspaceContent {
    fn of(ws: &Workspace) -> Self {
        Self {
            id: ws.id,
            name: ws.name.clone(),
            columns: ws.scrolling.columns.iter().map(ColumnContent::of).collect(),
            floating: ws.floating_panes.iter().map(FloatingContent::of).collect(),
        }
    }
}

impl ColumnContent {
    fn of(col: &Column) -> Self {
        Self {
            id: col.id,
            name: col.name.clone(),
            width: col.width,
            zoomed: col.is_zoomed(),
            full_width: col.is_full_width,
            panes: col.panes.iter().map(PaneContent::of).collect(),
        }
    }
}

impl PaneContent {
    fn of(pane: &Pane) -> Self {
        Self {
            id: pane.id,
            title: pane.title.clone(),
            name: pane.custom_name.clone(),
            height_share: pane.height_share,
        }
    }
}

impl FloatingContent {
    fn of(f: &FloatingPane) -> Self {
        Self {
            pane: PaneContent::of(&f.pane),
            origin: f.origin,
        }
    }
}

impl SessionContent {
    /// **Rebuild a drawable [`Session`] from this projection** — a client reconstructing the shared
    /// content to lay out and draw it, for session `id` with the layout `options` from its config.
    /// Every pane's runtime is default (process / git / agent facts arrive as events), and the
    /// per-window view is the client's own.
    pub fn rebuild(&self, id: SessionId, options: LayoutOptions) -> Session {
        let workspaces = self
            .workspaces
            .iter()
            .map(|ws| ws.rebuild(&options))
            .collect();
        Session::from_parts(id, options, workspaces)
    }
}

impl WorkspaceContent {
    fn rebuild(&self, options: &LayoutOptions) -> Workspace {
        let mut scrolling = ScrollingSpace::new(options.clone());
        scrolling.columns = self.columns.iter().map(ColumnContent::rebuild).collect();
        Workspace {
            id: self.id,
            name: self.name.clone(),
            scrolling,
            floating_panes: self.floating.iter().map(FloatingContent::rebuild).collect(),
        }
    }
}

impl ColumnContent {
    fn rebuild(&self) -> Column {
        Column {
            id: self.id,
            name: self.name.clone(),
            panes: self.panes.iter().map(PaneContent::rebuild).collect(),
            width: self.width,
            // The exact pre-zoom width is not carried (a zoomed column draws at the viewport width
            // regardless); a placeholder keeps `is_zoomed()` true so the geometry matches.
            zoom_restore_width: self.zoomed.then_some(self.width),
            is_full_width: self.full_width,
            is_pending_fullscreen: false,
            is_pending_maximized: false,
        }
    }
}

impl PaneContent {
    fn rebuild(&self) -> Pane {
        let mut pane = Pane::new(self.id, self.title.clone());
        pane.custom_name = self.name.clone();
        pane.height_share = self.height_share;
        pane
    }
}

impl FloatingContent {
    fn rebuild(&self) -> FloatingPane {
        FloatingPane {
            pane: self.pane.rebuild(),
            origin: self.origin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::testing::Windowed;
    use crate::layout::types::Size;
    use crate::layout::{Pane, PaneId};

    /// The projection mirrors the session's structure, and round-trips through serde unchanged —
    /// it is what the socket carries to a freshly attached window.
    #[test]
    fn content_mirrors_the_session_and_round_trips() {
        let mut s = Windowed::new(Size::new(1000.0, 800.0), 1.0);
        s.m().add_pane(Pane::new(PaneId(1), "one"), None, true);
        s.m().add_pane(Pane::new(PaneId(2), "two"), None, true);

        let content = s.session.content();
        // One workspace, two panes across its columns, names carried.
        assert_eq!(content.workspaces.len(), 1);
        let ids: Vec<u64> = content.workspaces[0]
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.id.0))
            .collect();
        assert_eq!(ids, vec![1, 2]);

        let json = serde_json::to_string(&content).expect("serialize");
        let back: SessionContent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(content, back, "SessionContent changed across a serde round-trip");
    }

    /// Rebuilding a session from the projection and projecting it again yields the same content —
    /// the client's redrawn copy carries the same structure the server sent.
    #[test]
    fn rebuild_restores_the_same_content() {
        let mut s = Windowed::new(Size::new(1000.0, 800.0), 1.0);
        s.m().add_pane(Pane::new(PaneId(1), "one"), None, true);
        s.m().add_pane(Pane::new(PaneId(2), "two"), None, true);

        let content = s.session.content();
        let rebuilt = content.rebuild(s.session.id, s.session.options.clone());
        assert_eq!(
            rebuilt.content(),
            content,
            "content → rebuild → content must be the identity"
        );
    }
}
