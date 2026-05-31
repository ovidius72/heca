use crate::app_state::SidebarItemState;
use heca_core::layout::session::Session;

/// A flat item in the sidebar navigation list.
/// Built from the tree, skipping collapsed items.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: u64 },
}

impl SidebarItem {
    pub fn workspace_idx(&self) -> Option<usize> {
        match self {
            SidebarItem::Workspace { ws_idx } => Some(*ws_idx),
            SidebarItem::Column { ws_idx, .. } => Some(*ws_idx),
            SidebarItem::Pane { pane_id: _ } => None, // need to find via session
        }
    }
}

/// A pane entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarPaneEntry {
    pub pane_id: u64,
    pub name: String,
    pub state: SidebarItemState,
}

/// A column entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarColEntry {
    pub col_idx: usize,
    pub collapsed: bool,
    pub panes: Vec<SidebarPaneEntry>,
}

impl SidebarColEntry {
    pub fn visible_pane_count(&self) -> usize {
        if self.collapsed { 0 } else { self.panes.len() }
    }
}

/// A workspace entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarWsEntry {
    pub ws_idx: usize,
    pub name: String,
    pub collapsed: bool,
    pub state: SidebarItemState,
    pub columns: Vec<SidebarColEntry>,
}

impl SidebarWsEntry {
    /// Number of visible items (columns + panes) under this workspace.
    pub fn visible_child_count(&self) -> usize {
        if self.collapsed {
            return 0;
        }
        let mut count = 0;
        for col in &self.columns {
            count += 1; // the column header
            count += col.visible_pane_count();
        }
        count
    }
}

/// The sidebar tree model — mirrors the session's layout hierarchy.
#[derive(Debug, Clone)]
pub struct SidebarTree {
    pub workspaces: Vec<SidebarWsEntry>,
    /// Index into the flat item list for cursor navigation.
    pub cursor: usize,
    /// Total number of selectable items in the flat list.
    pub item_count: usize,
    /// Scroll offset for the tree view.
    pub scroll_offset: usize,
    /// Flat navigation list (built from tree, skipping collapsed items).
    pub flat_items: Vec<SidebarItem>,
}

impl SidebarTree {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            cursor: 0,
            item_count: 0,
            scroll_offset: 0,
            flat_items: Vec::new(),
        }
    }

    /// Rebuild the tree from the current session state.
    pub fn rebuild(&mut self, session: &Session, last_visited_ws_idx: Option<usize>, focused_pane: Option<u64>) {
        self.workspaces.clear();
        self.flat_items.clear();

        for (ws_idx, ws) in session.workspaces.iter().enumerate() {
            let is_active = ws_idx == session.active_workspace_idx;
            let state = if is_active {
                SidebarItemState::Active
            } else if Some(ws_idx) == last_visited_ws_idx {
                SidebarItemState::Visited
            } else {
                SidebarItemState::None
            };

            let mut ws_entry = SidebarWsEntry {
                ws_idx,
                name: ws.name.clone().unwrap_or_else(|| format!("Workspace {}", ws_idx + 1)),
                collapsed: false,
                state,
                columns: Vec::new(),
            };

            // Add scrolling columns
            let cols_to_show: Vec<usize> = if ws.scrolling.columns.is_empty() {
                Vec::new()
            } else {
                (0..ws.scrolling.columns.len()).collect()
            };

            for &col_idx in &cols_to_show {
                if let Some(col) = ws.scrolling.columns.get(col_idx) {
                    let is_active_col = col_idx == ws.scrolling.active_column_idx && is_active;
                    let mut col_entry = SidebarColEntry {
                        col_idx,
                        collapsed: false,
                        panes: Vec::new(),
                    };

                    for (_, pane) in col.panes.iter().enumerate() {
                        let is_active_pane = Some(pane.id.0) == focused_pane;
                        col_entry.panes.push(SidebarPaneEntry {
                            pane_id: pane.id.0,
                            name: pane.title.clone(),
                            state: if is_active_pane {
                                SidebarItemState::Active
                            } else {
                                SidebarItemState::None
                            },
                        });
                    }

                    ws_entry.columns.push(col_entry);
                }
            }

            self.workspaces.push(ws_entry);
        }

        self.rebuild_flat_items();
        self.clamp_cursor();
    }

    /// Rebuild the flat navigation list from the tree (skipping collapsed items).
    pub fn rebuild_flat_items(&mut self) {
        self.flat_items.clear();

        for ws_entry in &self.workspaces {
            self.flat_items.push(SidebarItem::Workspace { ws_idx: ws_entry.ws_idx });

            if !ws_entry.collapsed {
                for col_entry in &ws_entry.columns {
                    self.flat_items.push(SidebarItem::Column {
                        ws_idx: ws_entry.ws_idx,
                        col_idx: col_entry.col_idx,
                    });

                    if !col_entry.collapsed {
                        for pane_entry in &col_entry.panes {
                            self.flat_items.push(SidebarItem::Pane { pane_id: pane_entry.pane_id });
                        }
                    }
                }
            }
        }

        self.item_count = self.flat_items.len();
    }

    /// Clamp cursor to valid range.
    fn clamp_cursor(&mut self) {
        if self.flat_items.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.flat_items.len() {
            self.cursor = self.flat_items.len() - 1;
        }
    }

    /// Move selection up.
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move selection down.
    pub fn cursor_down(&mut self) {
        if self.cursor + 1 < self.item_count {
            self.cursor += 1;
        }
    }

    /// Toggle expand/collapse of the item under cursor.
    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor) {
            match item {
                SidebarItem::Workspace { ws_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        ws_entry.collapsed = !ws_entry.collapsed;
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        if let Some(col_entry) = ws_entry.columns.get_mut(*col_idx) {
                            col_entry.collapsed = !col_entry.collapsed;
                        }
                    }
                }
                SidebarItem::Pane { .. } => {
                    // Panes are leaves — no expand/collapse.
                }
            }
            self.rebuild_flat_items();
        }
    }

    /// Expand the item under cursor (recurse into children).
    pub fn expand(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor) {
            match item {
                SidebarItem::Workspace { ws_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        if ws_entry.collapsed {
                            ws_entry.collapsed = false;
                            self.rebuild_flat_items();
                        }
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        if let Some(col_entry) = ws_entry.columns.get_mut(*col_idx) {
                            if col_entry.collapsed {
                                col_entry.collapsed = false;
                                self.rebuild_flat_items();
                            }
                        }
                    }
                }
                SidebarItem::Pane { .. } => {
                    // Panes are leaves — nothing to expand.
                }
            }
        }
    }

    /// Collapse the item under cursor.
    pub fn collapse(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor) {
            match item {
                SidebarItem::Workspace { ws_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        ws_entry.collapsed = true;
                        self.rebuild_flat_items();
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx) {
                        if let Some(col_entry) = ws_entry.columns.get_mut(*col_idx) {
                            col_entry.collapsed = true;
                            self.rebuild_flat_items();
                        }
                    }
                }
                SidebarItem::Pane { .. } => {
                    // Panes are leaves — collapse up: collapse the parent column.
                    // Find the parent column and collapse it.
                    // This requires traversing the tree — we'll just no-op for now.
                }
            }
        }
    }

    /// Get the item at cursor, if any.
    pub fn current_item(&self) -> Option<&SidebarItem> {
        self.flat_items.get(self.cursor)
    }
}
