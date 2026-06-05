use crate::app_state::SidebarItemState;
use crate::input::WmAction;
use heca_core::layout::session::Session;

/// A flat item in the sidebar navigation list.
/// Built from the tree, skipping collapsed items.
#[derive(Debug, Clone)]
pub enum SidebarItem {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: u64 },
    FloatingPane { pane_id: u64, ws_idx: usize },
}

impl SidebarItem {
    /// Returns the workspace index for Workspace, Column, and FloatingPane variants.
    /// For Pane, returns None (use `cursor_workspace_index()` for tree search).
    pub fn workspace_idx(&self) -> Option<usize> {
        match self {
            SidebarItem::Workspace { ws_idx } => Some(*ws_idx),
            SidebarItem::Column { ws_idx, .. } => Some(*ws_idx),
            SidebarItem::FloatingPane { ws_idx, .. } => Some(*ws_idx),
            SidebarItem::Pane { .. } => None,
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
    pub name: String,
    pub collapsed: bool,
    pub panes: Vec<SidebarPaneEntry>,
}

/// A workspace entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarWsEntry {
    pub ws_idx: usize,
    pub name: String,
    pub collapsed: bool,
    pub state: SidebarItemState,
    pub columns: Vec<SidebarColEntry>,
    pub floating_panes: Vec<SidebarPaneEntry>,
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
    /// Button hitboxes for [+w], [+c], [+p] buttons (set during render).
    pub button_hitboxes: Vec<SidebarButtonHitbox>,
}

/// Hitbox for a sidebar button.
#[derive(Debug, Clone)]
pub struct SidebarButtonHitbox {
    pub action: WmAction,
    /// Optional workspace index — used to switch workspace before dispatching.
    pub ws_idx: Option<usize>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl SidebarTree {
    pub fn new() -> Self {
        Self {
            workspaces: Vec::new(),
            cursor: 0,
            item_count: 0,
            scroll_offset: 0,
            flat_items: Vec::new(),
            button_hitboxes: Vec::new(),
        }
    }

    /// Rebuild the tree from the current session state.
    /// `last_visited_pane_per_ws` provides per-workspace last-visited pane IDs
    /// for toggle highlighting (Prefix+i).
    pub fn rebuild(
        &mut self,
        session: &Session,
        last_visited_ws_idx: Option<usize>,
        focused_pane: Option<u64>,
        last_visited_pane_per_ws: &[Option<u64>],
    ) {
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

            let last_visited_in_ws = last_visited_pane_per_ws.get(ws_idx).copied().flatten();

            let mut ws_entry = SidebarWsEntry {
                ws_idx,
                name: ws
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1)),
                collapsed: false,
                state,
                columns: Vec::new(),
                floating_panes: Vec::new(),
            };

            let cols_to_show: Vec<usize> = if ws.scrolling.columns.is_empty() {
                Vec::new()
            } else {
                (0..ws.scrolling.columns.len()).collect()
            };

            for &col_idx in &cols_to_show {
                if let Some(col) = ws.scrolling.columns.get(col_idx) {
                    let _is_active_col = col_idx == ws.scrolling.active_column_idx && is_active;
                    let mut col_entry = SidebarColEntry {
                        col_idx,
                        name: col
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Col {}", col_idx + 1)),
                        collapsed: false,
                        panes: Vec::new(),
                    };

                    for pane in col.panes.iter() {
                        let is_active_pane = Some(pane.id.0) == focused_pane;
                        let is_visited_pane =
                            !is_active_pane && Some(pane.id.0) == last_visited_in_ws;
                        col_entry.panes.push(SidebarPaneEntry {
                            pane_id: pane.id.0,
                            name: pane.title.clone(),
                            state: if is_active_pane {
                                SidebarItemState::Active
                            } else if is_visited_pane {
                                SidebarItemState::Visited
                            } else {
                                SidebarItemState::None
                            },
                        });
                    }

                    ws_entry.columns.push(col_entry);
                }
            }

            for float in &ws.floating_panes {
                let is_active_float = Some(float.pane.id.0) == focused_pane;
                let is_visited_float =
                    !is_active_float && Some(float.pane.id.0) == last_visited_in_ws;
                ws_entry.floating_panes.push(SidebarPaneEntry {
                    pane_id: float.pane.id.0,
                    name: float.pane.title.clone(),
                    state: if is_active_float {
                        SidebarItemState::Active
                    } else if is_visited_float {
                        SidebarItemState::Visited
                    } else {
                        SidebarItemState::None
                    },
                });
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
            self.flat_items.push(SidebarItem::Workspace {
                ws_idx: ws_entry.ws_idx,
            });

            if !ws_entry.collapsed {
                for col_entry in &ws_entry.columns {
                    self.flat_items.push(SidebarItem::Column {
                        ws_idx: ws_entry.ws_idx,
                        col_idx: col_entry.col_idx,
                    });

                    if !col_entry.collapsed {
                        for pane_entry in &col_entry.panes {
                            self.flat_items.push(SidebarItem::Pane {
                                pane_id: pane_entry.pane_id,
                            });
                        }
                    }
                }

                for float_entry in &ws_entry.floating_panes {
                    self.flat_items.push(SidebarItem::FloatingPane {
                        pane_id: float_entry.pane_id,
                        ws_idx: ws_entry.ws_idx,
                    });
                }
            }
        }

        self.item_count = self.flat_items.len();
    }

    /// Clamp cursor to valid range.
    pub(crate) fn clamp_cursor(&mut self) {
        if self.flat_items.is_empty() {
            self.cursor = 0;
        } else if self.cursor >= self.flat_items.len() {
            self.cursor = self.flat_items.len() - 1;
        }
    }

    fn is_navigable(&self, idx: usize) -> bool {
        self.flat_items
            .get(idx)
            .is_some_and(|item| !matches!(item, SidebarItem::FloatingPane { .. }))
    }

    /// Move selection up, keeping cursor visible.
    pub fn cursor_up(&mut self) {
        while self.cursor > 0 {
            self.cursor -= 1;
            if self.is_navigable(self.cursor) {
                break;
            }
        }
        self.scroll_to_cursor();
    }

    /// Move selection down, keeping cursor visible.
    pub fn cursor_down(&mut self) {
        while self.cursor + 1 < self.item_count {
            self.cursor += 1;
            if self.is_navigable(self.cursor) {
                break;
            }
        }
        self.scroll_to_cursor();
    }

    /// Ensure the cursor is within the visible scroll area.
    fn scroll_to_cursor(&mut self) {
        let visible_lines = 20;
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor.saturating_sub(visible_lines - 1);
        }
    }

    fn is_navigable_collapsed(&self, idx: usize) -> bool {
        self.flat_items.get(idx).is_some_and(|item| {
            !matches!(
                item,
                SidebarItem::Column { .. } | SidebarItem::FloatingPane { .. }
            )
        })
    }

    /// Move selection up, skipping invisible Column items (for collapsed sidebar nav).
    pub fn cursor_up_collapsed(&mut self) {
        loop {
            if self.cursor == 0 {
                break;
            }
            self.cursor -= 1;
            if self.is_navigable_collapsed(self.cursor) {
                break;
            }
        }
    }

    /// Move selection down, skipping invisible Column items (for collapsed sidebar nav).
    pub fn cursor_down_collapsed(&mut self) {
        loop {
            if self.cursor + 1 >= self.item_count {
                break;
            }
            self.cursor += 1;
            if self.is_navigable_collapsed(self.cursor) {
                break;
            }
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
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx)
                    {
                        col_entry.collapsed = !col_entry.collapsed;
                    }
                }
                SidebarItem::Pane { .. } | SidebarItem::FloatingPane { .. } => {}
            }
            self.rebuild_flat_items();
            self.clamp_cursor();
        }
    }

    /// Expand the item under cursor (recurse into children).
    pub fn expand(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor) {
            match item {
                SidebarItem::Workspace { ws_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && ws_entry.collapsed
                    {
                        ws_entry.collapsed = false;
                        self.rebuild_flat_items();
                        self.clamp_cursor();
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx)
                        && col_entry.collapsed
                    {
                        col_entry.collapsed = false;
                        self.rebuild_flat_items();
                        self.clamp_cursor();
                    }
                }
                SidebarItem::Pane { .. } | SidebarItem::FloatingPane { .. } => {}
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
                        self.clamp_cursor();
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx)
                    {
                        col_entry.collapsed = true;
                        self.rebuild_flat_items();
                        self.clamp_cursor();
                    }
                }
                SidebarItem::Pane { .. } | SidebarItem::FloatingPane { .. } => {}
            }
        }
    }

    /// Get the item at cursor, if any.
    pub fn current_item(&self) -> Option<&SidebarItem> {
        self.flat_items.get(self.cursor)
    }

    /// Get the workspace index of the item at the current cursor position.
    /// Works for Workspace, Column, FloatingPane, and Pane items.
    pub fn cursor_workspace_index(&self) -> Option<usize> {
        self.flat_items
            .get(self.cursor)
            .and_then(|item| match item {
                SidebarItem::Workspace { ws_idx } => Some(*ws_idx),
                SidebarItem::Column { ws_idx, .. } => Some(*ws_idx),
                SidebarItem::FloatingPane { ws_idx, .. } => Some(*ws_idx),
                SidebarItem::Pane { pane_id } => self.workspaces.iter().position(|ws| {
                    ws.columns
                        .iter()
                        .any(|col| col.panes.iter().any(|p| p.pane_id == *pane_id))
                }),
            })
    }
}
