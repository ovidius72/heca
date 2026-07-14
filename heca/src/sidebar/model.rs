use crate::app_state::SidebarItemState;
use crate::chrome::SidebarSelection;
use crate::input::WmAction;
use heca_core::layout::{PaneId, session::Session};

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
    Pane { pane_id: PaneId },
    FloatingPane { pane_id: PaneId, ws_idx: usize },
}

impl SidebarItem {
    /// This row as the stable [`SidebarSelection`] the chrome store holds.
    ///
    /// The two types say the same thing at different altitudes: `SidebarItem` is a row in
    /// a rebuilt-every-frame projection, `SidebarSelection` is `Copy` state that outlives
    /// the rebuild and crosses the host boundary (event payload, store value, and — later
    /// — RPC/plugin reads).
    pub fn selection(&self) -> SidebarSelection {
        match *self {
            SidebarItem::Workspace { ws_idx } => SidebarSelection::Workspace { ws_idx },
            SidebarItem::Column { ws_idx, col_idx } => {
                SidebarSelection::Column { ws_idx, col_idx }
            }
            SidebarItem::Pane { pane_id } => SidebarSelection::Pane { pane_id },
            SidebarItem::FloatingPane { pane_id, ws_idx } => {
                SidebarSelection::FloatingPane { pane_id, ws_idx }
            }
        }
    }

    /// Returns the kind of this sidebar item.
    pub fn kind(&self) -> SidebarItemKind {
        match self {
            SidebarItem::Workspace { .. } => SidebarItemKind::Workspace,
            SidebarItem::Column { .. } => SidebarItemKind::Column,
            SidebarItem::Pane { .. } => SidebarItemKind::Pane,
            SidebarItem::FloatingPane { .. } => SidebarItemKind::FloatingPane,
        }
    }

}

/// A pane entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarPaneEntry {
    pub pane_id: PaneId,
    /// Process-derived title (the program hint, used for icon resolution + as the
    /// display name when no custom override is set).
    pub name: String,
    /// User-set override name (from rename); when present it wins over the process name.
    pub custom_name: Option<String>,
    /// Active/visited projection of the pane. Populated by `sync_from_session` and
    /// asserted by tests; the in-app expanded sidebar reads active-state from
    /// `chrome_state` instead, and this field is the model's own projection that the
    /// planned `WorkspacesContainerProvider` (plugin-task-10) will consume.
    #[allow(dead_code)]
    pub state: SidebarItemState,
}

/// A column entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarColEntry {
    pub col_idx: usize,
    pub collapsed: bool,
    pub panes: Vec<SidebarPaneEntry>,
}

/// A workspace entry in the sidebar tree.
#[derive(Debug, Clone)]
pub struct SidebarWsEntry {
    pub ws_idx: usize,
    pub name: String,
    pub collapsed: bool,
    /// Active/visited projection (see [`SidebarPaneEntry::state`]).
    #[allow(dead_code)]
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
    pub fn sync_from_session(
        &mut self,
        session: &Session,
        last_visited_ws_idx: Option<usize>,
        focused_pane: Option<PaneId>,
        last_visited_pane_per_ws: &[Option<PaneId>],
    ) {
        // Preserve column collapse across rebuild. Workspace collapse is owned by
        // `chrome_state.collapsed_ws` now — callers re-apply it via `apply_ws_collapsed`
        // after sync (so it defaults to expanded here).
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

            // Defaults to expanded; chrome_state's collapse set is applied post-sync.
            let collapsed = false;

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
                        collapsed: col_collapsed,
                        panes: Vec::with_capacity(col.panes.len()),
                    };

                    for pane in col.panes.iter() {
                        let is_active_pane = Some(pane.id) == focused_pane;
                        let is_visited_pane =
                            !is_active_pane && Some(pane.id) == last_visited_in_ws;
                        col_entry.panes.push(SidebarPaneEntry {
                            pane_id: pane.id,
                            name: pane.title.clone(),
                            custom_name: pane.custom_name.clone(),
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
                let is_active_float = Some(float.pane.id) == focused_pane;
                let is_visited_float =
                    !is_active_float && Some(float.pane.id) == last_visited_in_ws;
                ws_entry.floating_panes.push(SidebarPaneEntry {
                    pane_id: float.pane.id,
                    name: float.pane.title.clone(),
                    custom_name: float.pane.custom_name.clone(),
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

    /// The row under the nav cursor, as the **stable, `Copy`
    /// [`SidebarSelection`]** the chrome store holds — `None` when the tree is empty.
    ///
    /// This is the projection *out* of the tree: `cursor` is a positional index into
    /// `flat_items`, which is rebuilt from the session on every layout change, so it is
    /// not something another consumer (the host, RPC, a plugin) could hold onto. The
    /// selection names the thing itself.
    pub fn selection(&self) -> Option<SidebarSelection> {
        self.current_item().map(SidebarItem::selection)
    }

    /// Project the **canonical** selection from the chrome store back onto the cursor —
    /// the selection counterpart of [`apply_ws_collapsed`](Self::apply_ws_collapsed),
    /// and for the same reason: `sync_from_session` rebuilds `flat_items` from scratch,
    /// so the positional cursor has to be re-derived from the state that outlives the
    /// rebuild.
    ///
    /// Because the store is the source of truth, this is also what makes the selection
    /// **drivable from outside**: an RPC or a plugin that writes
    /// [`set_nav_selection`](crate::chrome::WorkspacesContainerState::set_nav_selection)
    /// moves the cursor here.
    ///
    /// `None` leaves the cursor alone (nothing selected is not a request to move), and a
    /// selection whose row no longer exists — its pane closed, its workspace deleted —
    /// leaves the cursor where it was, clamped into range, rather than silently jumping
    /// somewhere arbitrary.
    pub fn apply_nav_selection(&mut self, selection: Option<SidebarSelection>) {
        let Some(selection) = selection else {
            return;
        };
        if let Some(idx) = self
            .flat_items
            .iter()
            .position(|item| item.selection() == selection)
        {
            self.cursor = idx;
            self.scroll_to_cursor();
        } else {
            self.clamp_cursor();
        }
    }

    fn is_navigable(&self, idx: usize) -> bool {
        // The nav cursor stops only on panes and workspace headers — never on
        // columns (landing on a column reads as a "jump into nothing") nor floating
        // panes. Workspace headers stay selectable so a collapsed workspace can be
        // re-expanded with `l`.
        self.flat_items.get(idx).is_some_and(|item| {
            matches!(item, SidebarItem::Pane { .. } | SidebarItem::Workspace { .. })
        })
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

    fn pane_location(&self, pane_id: PaneId) -> Option<(usize, Option<usize>)> {
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
        self.flat_items.iter().position(
            |item| matches!(item, SidebarItem::Workspace { ws_idx: item_ws } if *item_ws == ws_idx),
        )
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
            Some(SidebarItem::Column {
                ws_idx: item_ws, ..
            }) => *item_ws == ws_idx,
            Some(SidebarItem::FloatingPane {
                ws_idx: item_ws, ..
            }) => *item_ws == ws_idx,
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

    /// Project the canonical workspace-collapse set (`chrome_state.collapsed_ws`)
    /// into the nav model: refresh the `WsEntry.collapsed` mirror from `set`, rebuild
    /// the flat list, and adjust the cursor (when `changed_ws` just became collapsed
    /// and the cursor was inside it, move it to the workspace header). Collapse is
    /// owned by `chrome_state`; this only mirrors it for navigation/rendering.
    pub fn apply_ws_collapsed(
        &mut self,
        set: &std::collections::HashSet<usize>,
        changed_ws: Option<usize>,
    ) {
        // Decide the cursor move BEFORE the flat list changes (indices shift on sync).
        let move_cursor_to_parent = changed_ws.is_some_and(|ws_idx| {
            set.contains(&ws_idx)
                && self.current_item_in_workspace(ws_idx)
                && !matches!(self.current_item(), Some(SidebarItem::Workspace { ws_idx: item_ws }) if *item_ws == ws_idx)
        });
        for ws_entry in &mut self.workspaces {
            ws_entry.collapsed = set.contains(&ws_entry.ws_idx);
        }
        self.sync_flat_items();
        if move_cursor_to_parent
            && let Some(ws_idx) = changed_ws
            && let Some(parent_idx) = self.workspace_flat_index(ws_idx)
        {
            self.cursor = parent_idx;
        }
        self.clamp_cursor();
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

    /// Toggle expand/collapse of the item under cursor. Workspace collapse is owned
    /// by `chrome_state`; columns stay in the tree.
    pub fn toggle_expand(&mut self, chrome_state: &crate::chrome::WorkspacesContainerState) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        chrome_state.toggle_ws_collapsed(ws_idx);
                        let set = chrome_state.with_collapsed_ws(|s| s.clone());
                        self.apply_ws_collapsed(&set, Some(ws_idx));
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
    pub fn expand(&mut self, chrome_state: &crate::chrome::WorkspacesContainerState) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        chrome_state.set_ws_collapsed(ws_idx, false);
                        let set = chrome_state.with_collapsed_ws(|s| s.clone());
                        self.apply_ws_collapsed(&set, Some(ws_idx));
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
    pub fn collapse(&mut self, chrome_state: &crate::chrome::WorkspacesContainerState) {
        if let Some(item) = self.flat_items.get(self.cursor).cloned() {
            match item.kind() {
                SidebarItemKind::Workspace => {
                    if let SidebarItem::Workspace { ws_idx } = item {
                        chrome_state.set_ws_collapsed(ws_idx, true);
                        let set = chrome_state.with_collapsed_ws(|s| s.clone());
                        self.apply_ws_collapsed(&set, Some(ws_idx));
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

    /// Get the item at cursor, if any.
    pub fn current_item(&self) -> Option<&SidebarItem> {
        self.flat_items.get(self.cursor)
    }
}
