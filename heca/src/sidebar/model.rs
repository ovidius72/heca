use crate::app_state::SidebarItemState;
use crate::input::WmAction;
use heca_core::layout::session::Session;
use std::collections::HashMap;

/// Precise location of a pane within the sidebar tree.
/// Built during `sync_from_session()` for O(1) lookup during render.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaneTreeLocation {
    pub ws_idx: usize,
    /// `Some(col_idx)` for tiled panes, `None` for floating panes.
    pub col_idx: Option<usize>,
    /// Index within `columns[col_idx].panes` or `floating_panes`.
    pub pane_idx: usize,
}

/// A flat item in the sidebar navigation list.
/// Built from the tree, skipping collapsed items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarItemKind {
    Workspace,
    Column,
    Pane,
    FloatingPane,
}

#[derive(Debug, Clone)]
pub enum SidebarItem {
    Workspace { ws_idx: usize },
    Column { ws_idx: usize, col_idx: usize },
    Pane { pane_id: u64 },
    FloatingPane { pane_id: u64, ws_idx: usize },
}

impl SidebarItem {
    /// Returns the kind of this sidebar item.
    pub fn kind(&self) -> SidebarItemKind {
        match self {
            SidebarItem::Workspace { .. } => SidebarItemKind::Workspace,
            SidebarItem::Column { .. } => SidebarItemKind::Column,
            SidebarItem::Pane { .. } => SidebarItemKind::Pane,
            SidebarItem::FloatingPane { .. } => SidebarItemKind::FloatingPane,
        }
    }

    /// Returns whether this row can be selected/highlighted by the cursor.
    pub fn is_selectable(&self) -> bool {
        !matches!(self, SidebarItem::FloatingPane { .. })
    }

    /// Returns whether this row can be expanded/collapsed (shows a disclosure symbol).
    pub fn is_expandable(&self) -> bool {
        matches!(self, SidebarItem::Workspace { .. } | SidebarItem::Column { .. })
    }

    /// Returns the workspace index for Workspace, Column, and FloatingPane variants.
    /// For Pane, returns None.
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
    /// Maps pane_id → precise tree location for O(1) render lookups.
    pub pane_id_to_entry: HashMap<u64, PaneTreeLocation>,
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
            pane_id_to_entry: HashMap::new(),
            button_hitboxes: Vec::new(),
        }
    }

    /// Rebuild the tree from the current session state.
    /// `last_visited_pane_per_ws` provides per-workspace last-visited pane IDs
    /// for toggle highlighting (Prefix+i).
    pub fn sync_from_session(
        &mut self,
        session: &Session,
        last_visited_ws_idx: Option<usize>,
        focused_pane: Option<u64>,
        last_visited_pane_per_ws: &[Option<u64>],
    ) {
        // Preserve collapsed state across rebuild.
        let prev_ws_collapsed: std::collections::HashMap<usize, bool> = self
            .workspaces
            .iter()
            .map(|w| (w.ws_idx, w.collapsed))
            .collect();
        let prev_col_collapsed: std::collections::HashMap<(usize, usize), bool> = self
            .workspaces
            .iter()
            .flat_map(|w| {
                w.columns
                    .iter()
                    .map(move |c| ((w.ws_idx, c.col_idx), c.collapsed))
            })
            .collect();

        self.workspaces.clear();
        self.flat_items.clear();
        self.pane_id_to_entry.clear();

        self.workspaces = Vec::with_capacity(session.workspaces.len());

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

            let collapsed = prev_ws_collapsed.get(&ws_idx).copied().unwrap_or(false);

            let mut ws_entry = SidebarWsEntry {
                ws_idx,
                name: ws
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1)),
                collapsed,
                state,
                columns: Vec::with_capacity(ws.scrolling.columns.len()),
                floating_panes: Vec::with_capacity(ws.floating_panes.len()),
            };

            let cols_to_show: Vec<usize> = if ws.scrolling.columns.is_empty() {
                Vec::new()
            } else {
                (0..ws.scrolling.columns.len()).collect()
            };

            for &col_idx in &cols_to_show {
                if let Some(col) = ws.scrolling.columns.get(col_idx) {
                    let _is_active_col = col_idx == ws.scrolling.active_column_idx && is_active;
                    let col_collapsed = prev_col_collapsed
                        .get(&(ws_idx, col_idx))
                        .copied()
                        .unwrap_or(false);
                    let mut col_entry = SidebarColEntry {
                        col_idx,
                        name: col
                            .name
                            .clone()
                            .unwrap_or_else(|| format!("Col {}", col_idx + 1)),
                        collapsed: col_collapsed,
                        panes: Vec::with_capacity(col.panes.len()),
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

        self.sync_flat_items();
        self.clamp_cursor();
    }

    /// Rebuild the flat navigation list from the tree (skipping collapsed items).
    pub fn sync_flat_items(&mut self) {
        self.flat_items.clear();
        self.pane_id_to_entry.clear();

        for ws_entry in &self.workspaces {
            let ws_idx = ws_entry.ws_idx;
            self.flat_items.push(SidebarItem::Workspace {
                ws_idx: ws_entry.ws_idx,
            });

            if !ws_entry.collapsed {
                for (col_idx, col_entry) in ws_entry.columns.iter().enumerate() {
                    self.flat_items.push(SidebarItem::Column {
                        ws_idx: ws_entry.ws_idx,
                        col_idx: col_entry.col_idx,
                    });

                    if !col_entry.collapsed {
                        for (pane_idx, pane_entry) in col_entry.panes.iter().enumerate() {
                            self.pane_id_to_entry.insert(
                                pane_entry.pane_id,
                                PaneTreeLocation {
                                    ws_idx,
                                    col_idx: Some(col_idx),
                                    pane_idx,
                                },
                            );
                            self.flat_items.push(SidebarItem::Pane {
                                pane_id: pane_entry.pane_id,
                            });
                        }
                    }
                }

                for (pane_idx, float_entry) in ws_entry.floating_panes.iter().enumerate() {
                    self.pane_id_to_entry.insert(
                        float_entry.pane_id,
                        PaneTreeLocation {
                            ws_idx,
                            col_idx: None,
                            pane_idx,
                        },
                    );
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
            .is_some_and(|item| item.is_selectable())
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
            item.kind() != SidebarItemKind::Column
                && item.kind() != SidebarItemKind::FloatingPane
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

    fn pane_location(&self, pane_id: u64) -> Option<(usize, Option<usize>)> {
        for ws_entry in &self.workspaces {
            for col_entry in &ws_entry.columns {
                if col_entry.panes.iter().any(|pane| pane.pane_id == pane_id) {
                    return Some((ws_entry.ws_idx, Some(col_entry.col_idx)));
                }
            }
            if ws_entry
                .floating_panes
                .iter()
                .any(|pane| pane.pane_id == pane_id)
            {
                return Some((ws_entry.ws_idx, None));
            }
        }
        None
    }

    fn workspace_flat_index(&self, ws_idx: usize) -> Option<usize> {
        self.flat_items
            .iter()
            .position(|item| matches!(item, SidebarItem::Workspace { ws_idx: item_ws } if *item_ws == ws_idx))
    }

    fn column_flat_index(&self, ws_idx: usize, col_idx: usize) -> Option<usize> {
        self.flat_items.iter().position(|item| {
            matches!(
                item,
                SidebarItem::Column {
                    ws_idx: item_ws,
                    col_idx: item_col,
                } if *item_ws == ws_idx && *item_col == col_idx
            )
        })
    }

    fn current_item_in_workspace(&self, ws_idx: usize) -> bool {
        match self.current_item() {
            Some(SidebarItem::Workspace { ws_idx: item_ws }) => *item_ws == ws_idx,
            Some(SidebarItem::Column { ws_idx: item_ws, .. }) => *item_ws == ws_idx,
            Some(SidebarItem::FloatingPane { ws_idx: item_ws, .. }) => *item_ws == ws_idx,
            Some(SidebarItem::Pane { pane_id }) => self
                .pane_location(*pane_id)
                .is_some_and(|(item_ws, _)| item_ws == ws_idx),
            None => false,
        }
    }

    fn current_item_in_column(&self, ws_idx: usize, col_idx: usize) -> bool {
        match self.current_item() {
            Some(SidebarItem::Column {
                ws_idx: item_ws,
                col_idx: item_col,
            }) => *item_ws == ws_idx && *item_col == col_idx,
            Some(SidebarItem::Pane { pane_id }) => self
                .pane_location(*pane_id)
                .is_some_and(|(item_ws, item_col)| item_ws == ws_idx && item_col == Some(col_idx)),
            _ => false,
        }
    }

    pub fn toggle_workspace_collapsed(&mut self, ws_idx: usize) {
        let move_cursor_to_parent = self.current_item_in_workspace(ws_idx)
            && !matches!(self.current_item(), Some(SidebarItem::Workspace { ws_idx: item_ws }) if *item_ws == ws_idx);
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx) {
            ws_entry.collapsed = !ws_entry.collapsed;
            let now_collapsed = ws_entry.collapsed;
            self.sync_flat_items();
            if move_cursor_to_parent
                && now_collapsed
                && let Some(parent_idx) = self.workspace_flat_index(ws_idx)
            {
                self.cursor = parent_idx;
            }
            self.clamp_cursor();
        }
    }

    pub fn expand_workspace(&mut self, ws_idx: usize) {
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx)
            && ws_entry.collapsed
        {
            ws_entry.collapsed = false;
            self.sync_flat_items();
            self.clamp_cursor();
        }
    }

    pub fn collapse_workspace(&mut self, ws_idx: usize) {
        let move_cursor_to_parent = self.current_item_in_workspace(ws_idx)
            && !matches!(self.current_item(), Some(SidebarItem::Workspace { ws_idx: item_ws }) if *item_ws == ws_idx);
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx) {
            ws_entry.collapsed = true;
            self.sync_flat_items();
            if move_cursor_to_parent
                && let Some(parent_idx) = self.workspace_flat_index(ws_idx)
            {
                self.cursor = parent_idx;
            }
            self.clamp_cursor();
        }
    }

    pub fn toggle_column_collapsed(&mut self, ws_idx: usize, col_idx: usize) {
        let move_cursor_to_parent = self.current_item_in_column(ws_idx, col_idx)
            && !matches!(self.current_item(), Some(SidebarItem::Column { ws_idx: item_ws, col_idx: item_col }) if *item_ws == ws_idx && *item_col == col_idx);
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx)
            && let Some(col_entry) = ws_entry.columns.get_mut(col_idx)
        {
            col_entry.collapsed = !col_entry.collapsed;
            let now_collapsed = col_entry.collapsed;
            self.sync_flat_items();
            if move_cursor_to_parent
                && now_collapsed
                && let Some(parent_idx) = self.column_flat_index(ws_idx, col_idx)
            {
                self.cursor = parent_idx;
            }
            self.clamp_cursor();
        }
    }

    pub fn expand_column(&mut self, ws_idx: usize, col_idx: usize) {
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx)
            && let Some(col_entry) = ws_entry.columns.get_mut(col_idx)
            && col_entry.collapsed
        {
            col_entry.collapsed = false;
            self.sync_flat_items();
            self.clamp_cursor();
        }
    }

    pub fn collapse_column(&mut self, ws_idx: usize, col_idx: usize) {
        let move_cursor_to_parent = self.current_item_in_column(ws_idx, col_idx)
            && !matches!(self.current_item(), Some(SidebarItem::Column { ws_idx: item_ws, col_idx: item_col }) if *item_ws == ws_idx && *item_col == col_idx);
        if let Some(ws_entry) = self.workspaces.get_mut(ws_idx)
            && let Some(col_entry) = ws_entry.columns.get_mut(col_idx)
        {
            col_entry.collapsed = true;
            self.sync_flat_items();
            if move_cursor_to_parent
                && let Some(parent_idx) = self.column_flat_index(ws_idx, col_idx)
            {
                self.cursor = parent_idx;
            }
            self.clamp_cursor();
        }
    }

    /// Toggle expand/collapse of the item under cursor.
    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        self.toggle_workspace_collapsed(ws_idx);
                    }
                }
                SidebarItemKind::Column => {
                    if let SidebarItem::Column { ws_idx, col_idx } = item {
                        self.toggle_column_collapsed(ws_idx, col_idx);
                    }
                }
                SidebarItemKind::Pane | SidebarItemKind::FloatingPane => {}
            }
        }
    }

    /// Expand the item under cursor (recurse into children).
    pub fn expand(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        self.expand_workspace(ws_idx);
                    }
                }
                SidebarItemKind::Column => {
                    if let SidebarItem::Column { ws_idx, col_idx } = item {
                        self.expand_column(ws_idx, col_idx);
                    }
                }
                SidebarItemKind::Pane | SidebarItemKind::FloatingPane => {}
            }
        }
    }

    /// Collapse the item under cursor.
    pub fn collapse(&mut self) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        self.collapse_workspace(ws_idx);
                    }
                }
                SidebarItemKind::Column => {
                    if let SidebarItem::Column { ws_idx, col_idx } = item {
                        self.collapse_column(ws_idx, col_idx);
                    }
                }
                SidebarItemKind::Pane | SidebarItemKind::FloatingPane => {}
            }
        }
    }

    /// Fast pane-entry lookup using the pre-built `pane_id_to_entry` map.
    /// Returns `None` if the pane_id is not in the tree.
    pub fn pane_entry(&self, pane_id: u64) -> Option<&SidebarPaneEntry> {
        let loc = self.pane_id_to_entry.get(&pane_id)?;
        let ws = self.workspaces.get(loc.ws_idx)?;
        match loc.col_idx {
            Some(ci) => ws.columns.get(ci)?.panes.get(loc.pane_idx),
            None => ws.floating_panes.get(loc.pane_idx),
        }
    }

    /// Get the item at cursor, if any.
    pub fn current_item(&self) -> Option<&SidebarItem> {
        self.flat_items.get(self.cursor)
    }
}
