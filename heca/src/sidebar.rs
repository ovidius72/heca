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
}

impl SidebarItem {
    /// Returns the workspace index for Workspace and Column variants.
    /// For Pane, returns None (use `cursor_workspace_index()` for tree search).
    pub fn workspace_idx(&self) -> Option<usize> {
        match self {
            SidebarItem::Workspace { ws_idx } => Some(*ws_idx),
            SidebarItem::Column { ws_idx, .. } => Some(*ws_idx),
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
                    let _is_active_col = col_idx == ws.scrolling.active_column_idx && is_active;
                    let mut col_entry = SidebarColEntry {
                        col_idx,
                        collapsed: false,
                        panes: Vec::new(),
                    };

                    for pane in col.panes.iter() {
                        let is_active_pane = Some(pane.id.0) == focused_pane;
                        let is_visited_pane = !is_active_pane && Some(pane.id.0) == last_visited_in_ws;
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

    /// Move selection up, keeping cursor visible.
    pub fn cursor_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
        self.scroll_to_cursor();
    }

    /// Move selection down, keeping cursor visible.
    pub fn cursor_down(&mut self) {
        if self.cursor + 1 < self.item_count {
            self.cursor += 1;
        }
        self.scroll_to_cursor();
    }

    /// Ensure the cursor is within the visible scroll area.
    fn scroll_to_cursor(&mut self) {
        let visible_lines = 20; // rough estimate; renderer computes exact
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + visible_lines {
            self.scroll_offset = self.cursor.saturating_sub(visible_lines - 1);
        }
    }

    /// Returns true if the item at `idx` is visible in collapsed sidebar mode.
    /// Columns are hidden; only Workspace and Pane items are shown.
    fn is_visible_collapsed(&self, idx: usize) -> bool {
        self.flat_items.get(idx).is_some_and(|item| {
            !matches!(item, SidebarItem::Column { .. })
        })
    }

    /// Move selection up, skipping invisible Column items (for collapsed sidebar nav).
    pub fn cursor_up_collapsed(&mut self) {
        loop {
            if self.cursor == 0 {
                break;
            }
            self.cursor -= 1;
            if self.is_visible_collapsed(self.cursor) {
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
            if self.is_visible_collapsed(self.cursor) {
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
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx) {
                            col_entry.collapsed = !col_entry.collapsed;
                        }
                }
                SidebarItem::Pane { .. } => {
                    // Panes are leaves — no expand/collapse.
                }
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
                        && ws_entry.collapsed {
                            ws_entry.collapsed = false;
                            self.rebuild_flat_items();
                            self.clamp_cursor();
                        }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx)
                            && col_entry.collapsed {
                                col_entry.collapsed = false;
                                self.rebuild_flat_items();
                                self.clamp_cursor();
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
                        self.clamp_cursor();
                    }
                }
                SidebarItem::Column { ws_idx, col_idx } => {
                    if let Some(ws_entry) = self.workspaces.get_mut(*ws_idx)
                        && let Some(col_entry) = ws_entry.columns.get_mut(*col_idx) {
                            col_entry.collapsed = true;
                            self.rebuild_flat_items();
                            self.clamp_cursor();
                        }
                }
                SidebarItem::Pane { .. } => {
                    // Panes are leaves — no-op.
                }
            }
        }
    }

    /// Get the item at cursor, if any.
    pub fn current_item(&self) -> Option<&SidebarItem> {
        self.flat_items.get(self.cursor)
    }



    /// Get the workspace index of the item at the current cursor position.
    /// Works for Workspace, Column, and Pane items.
    pub fn cursor_workspace_index(&self) -> Option<usize> {
        self.flat_items.get(self.cursor).and_then(|item| match item {
            SidebarItem::Workspace { ws_idx } => Some(*ws_idx),
            SidebarItem::Column { ws_idx, .. } => Some(*ws_idx),
            SidebarItem::Pane { pane_id } => {
                // Search workspaces for this pane
                self.workspaces.iter().position(|ws| {
                    ws.columns.iter().any(|col| {
                        col.panes.iter().any(|p| p.pane_id == *pane_id)
                    })
                })
            }
        })
    }
}

/// Hit-test the sidebar to find which flat item (if any) is under `mouse_y`.
///
/// * `sidebar_top` — Y coordinate of the sidebar's top edge
/// * `sidebar_height` — total height of the sidebar content area
/// * `sidebar_width` — current width (used to decide expanded vs collapsed)
/// * `mouse_y` — the mouse cursor's Y coordinate
///
/// Returns the flat item index, or `None` if the click missed all items.
pub fn sidebar_hit_test(
    tree: &SidebarTree,
    sidebar_top: f32,
    sidebar_height: f32,
    sidebar_width: f32,
    mouse_y: f32,
) -> Option<usize> {
    if mouse_y < sidebar_top || mouse_y > sidebar_top + sidebar_height {
        return None;
    }

    let is_collapsed = sidebar_width < 80.0;
    let relative_y = mouse_y - (sidebar_top + 4.0);
    if relative_y < 0.0 {
        return None;
    }

    // Account for the [+w] button row at the top.
    let adjusted_y = relative_y - BTN_ROW_HEIGHT;
    if adjusted_y < 0.0 {
        return None; // Clicked on the button row itself.
    }

    let line_index = (adjusted_y / ITEM_HEIGHT) as usize;

    if is_collapsed {
        // In collapsed mode columns are invisible; map visible line to flat idx.
        let mut visible_line = 0usize;
        for (fi, item) in tree.flat_items.iter().enumerate() {
            if matches!(item, SidebarItem::Column { .. }) {
                continue;
            }
            if visible_line == line_index {
                return Some(fi);
            }
            visible_line += 1;
        }
    } else {
        let visible_lines = (sidebar_height / ITEM_HEIGHT) as usize;
        let fi = tree.scroll_offset + line_index;
        if line_index < visible_lines && fi < tree.flat_items.len() {
            return Some(fi);
        }
    }

    None
}

/// Check if a mouse position hits any sidebar button.
/// Returns the button if hit, None otherwise.
pub fn sidebar_button_hit_test(
    tree: &SidebarTree,
    mouse_x: f32,
    mouse_y: f32,
) -> Option<(usize, WmAction)> {
    for (i, hitbox) in tree.button_hitboxes.iter().enumerate() {
        if mouse_x >= hitbox.x && mouse_x <= hitbox.x + hitbox.width
            && mouse_y >= hitbox.y && mouse_y <= hitbox.y + hitbox.height
        {
            return Some((i, hitbox.action.clone()));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Sidebar rendering functions
// ---------------------------------------------------------------------------

use heca_renderer::primitive::PrimitiveRenderer;
use heca_renderer::text::TextRenderer;

const ITEM_HEIGHT: f32 = 24.0;
const INDENT_WS: f32 = 8.0;
const INDENT_COL: f32 = 26.0;
const INDENT_PANE: f32 = 44.0;
const BTN_SIZE: f32 = 20.0;
const BTN_PAD_X: f32 = 2.0;
const BTN_RADIUS: f32 = 4.0;
const BTN_ROW_HEIGHT: f32 = ITEM_HEIGHT; // [+w] button row at top

/// Render the expanded sidebar tree (width >= 80px).
/// If `candidates` is provided, pane letters are shown during PaneSelect/PaneSwap.
// Each param is a distinct render input; grouping would hurt call-site readability.
#[allow(clippy::too_many_arguments)]
pub fn render_sidebar_expanded(
    tree: &mut SidebarTree,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    is_sidebar_nav: bool,
    accent: [f32; 4],
    foreground: [f32; 4],
    cursor_bg: [f32; 4],
    visited_color: [f32; 4],
    candidates: Option<&[(char, u64)]>,
    focused_pane: Option<u64>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    // Flat index being hovered during drag (for highlight).
    drag_hover_fi: Option<usize>,
    // Flat index of the item being dragged (for drag source visual effect).
    drag_source_fi: Option<usize>,
    // Drag source colors from theme.
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    // Hovered button index for hover effect.
    hovered_btn_idx: Option<usize>,
    // Font sizes from theme.
    label_font_size: f32,
    button_font_size: f32,
) {
    // Clear and collect button hitboxes.
    tree.button_hitboxes.clear();

    let scroll = tree.scroll_offset;
    let mut line_y = y + 4.0;
    let visible_lines = (height / ITEM_HEIGHT) as usize;
    let mut drawn = 0usize;

    // [+w] button at the top of the sidebar.
    {
        let btn_x = x + width - BTN_SIZE - BTN_PAD_X - 4.0;
        let btn_y = line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0;
        let btn_idx = tree.button_hitboxes.len();
        let is_hov = hovered_btn_idx == Some(btn_idx);
        let (bg, brd, tc) = if is_hov {
            ([foreground[0], foreground[1], foreground[2], 0.3], [foreground[0], foreground[1], foreground[2], 0.7], foreground)
        } else {
            ([foreground[0], foreground[1], foreground[2], 0.12], [foreground[0], foreground[1], foreground[2], 0.35], foreground)
        };
        primitive_renderer.draw_rounded_rect(btn_x, btn_y, BTN_SIZE, BTN_SIZE, bg, brd, 1.0, BTN_RADIUS);
        let tw = 2.0 * button_font_size * 0.55;
        text_renderer.queue_text("+w", btn_x + (BTN_SIZE - tw) / 2.0, btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0, button_font_size, tc);
        tree.button_hitboxes.push(SidebarButtonHitbox { action: WmAction::CreateWorkspace, ws_idx: None, x: btn_x, y: btn_y, width: BTN_SIZE, height: BTN_SIZE });
    }
    line_y += ITEM_HEIGHT;
    drawn += 1;

    for (fi, flat_item) in tree.flat_items.iter().enumerate() {
        if fi < scroll {
            continue;
        }
        if drawn >= visible_lines {
            break;
        }

        let is_cursor = fi == tree.cursor && is_sidebar_nav;
        let (indent, label, is_ws) = match flat_item {
            SidebarItem::Workspace { ws_idx } => {
                if let Some(ws) = tree.workspaces.get(*ws_idx) {
                    let arrow = if ws.collapsed { "▶ " } else { "▼ " };
                    (INDENT_WS, format!("{}{}", arrow, ws.name), true)
                } else {
                    (INDENT_WS, format!("WS {}", ws_idx), true)
                }
            }
            SidebarItem::Column { ws_idx: _, col_idx } => {
                (INDENT_COL, format!("Col {}", col_idx + 1), false)
            }
            SidebarItem::Pane { pane_id } => {
                let mut label = pane_name_short(*pane_id, &tree.workspaces);
                if let Some(cands) = candidates
                    && focused_pane != Some(*pane_id)
                    && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == *pane_id) {
                        label = format!("[{}] {}", ch, label);
                    }
                (INDENT_PANE, label, false)
            }
        };

        // Cursor background (sidebar nav mode)
        if is_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
        }

        // Drag hover highlight (lighter accent)
        if drag_hover_fi == Some(fi) {
            let mut drag_bg = accent;
            drag_bg[3] = 0.25; // more transparent than cursor
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
        }

        // Drag source effect — the item being dragged gets a themed background + border.
        if drag_source_fi == Some(fi) {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
            primitive_renderer.draw_border(x + 1.0, line_y + 1.0, width - 2.0, ITEM_HEIGHT - 2.0, drag_source_border, 1.0);
        }

        // Determine color based on state
        let color = if is_cursor {
            accent
        } else if is_ws {
            // Look up workspace state
            if let Some(ws) = flat_item.workspace_idx()
                .and_then(|wi| tree.workspaces.get(wi))
            {
                match ws.state {
                    crate::app_state::SidebarItemState::Active => accent,
                    crate::app_state::SidebarItemState::Visited => visited_color,
                    crate::app_state::SidebarItemState::None => foreground,
                }
            } else {
                foreground
            }
        } else {
            // Check if it's an active pane
            if let SidebarItem::Pane { pane_id } = flat_item {
                if let Some(ws) = tree.workspaces.iter().find(|w| {
                    w.columns.iter().any(|c| c.panes.iter().any(|p| p.pane_id == *pane_id))
                }) {
                    if let Some(col) = ws.columns.iter().find(|c| c.panes.iter().any(|p| p.pane_id == *pane_id)) {
                        if let Some(pane) = col.panes.iter().find(|p| p.pane_id == *pane_id) {
                            match pane.state {
                                crate::app_state::SidebarItemState::Active => accent,
                                crate::app_state::SidebarItemState::Visited => visited_color,
                                crate::app_state::SidebarItemState::None => foreground,
                            }
                        } else { foreground }
                    } else { foreground }
                } else { foreground }
            } else { foreground }
        };

        let text_x = x + indent;
        let text_y = line_y + (ITEM_HEIGHT - label_font_size) / 2.0 + 2.0;
        text_renderer.queue_text(&label, text_x, text_y, label_font_size, color);

        // Buttons on the right side of each item.
        let btn_x = x + width - BTN_SIZE - BTN_PAD_X - 4.0;
        let btn_y = line_y + (ITEM_HEIGHT - BTN_SIZE) / 2.0;

        if let SidebarItem::Workspace { ws_idx } = flat_item {
            let btn_idx = tree.button_hitboxes.len();
            let is_hov = hovered_btn_idx == Some(btn_idx);
            let (bg, brd, tc) = if is_hov {
                ([foreground[0], foreground[1], foreground[2], 0.3], [foreground[0], foreground[1], foreground[2], 0.7], foreground)
            } else {
                ([foreground[0], foreground[1], foreground[2], 0.12], [foreground[0], foreground[1], foreground[2], 0.35], foreground)
            };
            primitive_renderer.draw_rounded_rect(btn_x, btn_y, BTN_SIZE, BTN_SIZE, bg, brd, 1.0, BTN_RADIUS);
            let tw = 2.0 * button_font_size * 0.55;
            text_renderer.queue_text("+c", btn_x + (BTN_SIZE - tw) / 2.0, btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0, button_font_size, tc);
            tree.button_hitboxes.push(SidebarButtonHitbox { action: WmAction::SplitHorizontal, ws_idx: Some(*ws_idx), x: btn_x, y: btn_y, width: BTN_SIZE, height: BTN_SIZE });
        }

        if let SidebarItem::Column { ws_idx, col_idx: _ } = flat_item {
            let btn_idx = tree.button_hitboxes.len();
            let is_hov = hovered_btn_idx == Some(btn_idx);
            let (bg, brd, tc) = if is_hov {
                ([foreground[0], foreground[1], foreground[2], 0.3], [foreground[0], foreground[1], foreground[2], 0.7], foreground)
            } else {
                ([foreground[0], foreground[1], foreground[2], 0.12], [foreground[0], foreground[1], foreground[2], 0.35], foreground)
            };
            primitive_renderer.draw_rounded_rect(btn_x, btn_y, BTN_SIZE, BTN_SIZE, bg, brd, 1.0, BTN_RADIUS);
            let tw = 2.0 * button_font_size * 0.55;
            text_renderer.queue_text("+p", btn_x + (BTN_SIZE - tw) / 2.0, btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0, button_font_size, tc);
            tree.button_hitboxes.push(SidebarButtonHitbox { action: WmAction::SplitVertical, ws_idx: Some(*ws_idx), x: btn_x, y: btn_y, width: BTN_SIZE, height: BTN_SIZE });
        }

        if let SidebarItem::Pane { pane_id } = flat_item {
            let btn_idx = tree.button_hitboxes.len();
            let is_hov = hovered_btn_idx == Some(btn_idx);
            let (bg, brd, tc) = if is_hov {
                ([0.9, 0.3, 0.3, 0.4], [0.9, 0.3, 0.3, 0.8], [0.95, 0.4, 0.4, 1.0])
            } else {
                ([0.9, 0.3, 0.3, 0.15], [0.9, 0.3, 0.3, 0.4], [0.9, 0.3, 0.3, 0.9])
            };
            primitive_renderer.draw_rounded_rect(btn_x, btn_y, BTN_SIZE, BTN_SIZE, bg, brd, 1.0, BTN_RADIUS);
            let tw = 1.0 * button_font_size * 0.55;
            text_renderer.queue_text("-", btn_x + (BTN_SIZE - tw) / 2.0, btn_y + (BTN_SIZE - button_font_size) / 2.0 + 2.0, button_font_size, tc);
            tree.button_hitboxes.push(SidebarButtonHitbox { action: WmAction::ClosePaneById { pane_id: *pane_id }, ws_idx: None, x: btn_x, y: btn_y, width: BTN_SIZE, height: BTN_SIZE });
        }

        line_y += ITEM_HEIGHT;
        drawn += 1;
    }
}

/// Render the collapsed sidebar (activity strip at 40px width).
/// When `is_sidebar_nav` is true, the cursor line is highlighted so the user
/// can navigate even in collapsed mode.
/// `candidates` are shown as pane letters during PaneSwap / PaneSelect.
// Each param is a distinct render input; grouping would hurt call-site readability.
#[allow(clippy::too_many_arguments)]
pub fn render_sidebar_collapsed(
    tree: &mut SidebarTree,
    x: f32,
    y: f32,
    width: f32,
    _height: f32,
    is_sidebar_nav: bool,
    accent: [f32; 4],
    foreground: [f32; 4],
    visited_color: [f32; 4],
    cursor_bg: [f32; 4],
    candidates: Option<&[(char, u64)]>,
    focused_pane: Option<u64>,
    text_renderer: &mut TextRenderer,
    primitive_renderer: &mut PrimitiveRenderer,
    // Flat index being hovered during drag (for highlight).
    drag_hover_fi: Option<usize>,
    // Flat index of the item being dragged (for drag source visual effect).
    drag_source_fi: Option<usize>,
    // Drag source colors from theme.
    drag_source_bg: [f32; 4],
    drag_source_border: [f32; 4],
    // Hovered button index (unused in collapsed — no buttons).
    _hovered_btn_idx: Option<usize>,
    // Font sizes from theme.
    label_font_size: f32,
    _button_font_size: f32,
) {
    let font_size = label_font_size;
    let activity_bar_w = 4.0;
    let text_x = x + activity_bar_w + 4.0; // 4px gap after activity bar
    let mut line_y = y + 4.0;

    // Track flat item index so we can match the cursor position.
    // flat_items ordering: Workspace, [Column, [Pane...]...]...
    let mut flat_idx = 0usize;

    for ws in &tree.workspaces {
        let is_ws_cursor = is_sidebar_nav && flat_idx == tree.cursor;
        flat_idx += 1; // consume Workspace item

        // Activity bar
        let bar_color = match ws.state {
            crate::app_state::SidebarItemState::Active => accent,
            crate::app_state::SidebarItemState::Visited => visited_color,
            crate::app_state::SidebarItemState::None => [0.0; 4], // transparent
        };

        // Count lines for this workspace
        let section_lines = 1 + if ws.collapsed { 0 } else {
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>()
        };
        let section_height = section_lines as f32 * ITEM_HEIGHT;

        // Draw activity bar (full height of section)
        if bar_color[3] > 0.0 {
            primitive_renderer.draw_rect(x, line_y, activity_bar_w, section_height, bar_color);
        }

        // Cursor highlight
        if is_ws_cursor {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
        }

        // Drag hover highlight (workspace)
        if drag_hover_fi == Some(flat_idx - 1) {
            let mut drag_bg = accent;
            drag_bg[3] = 0.25;
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
        }

        // Drag source effect for workspace items.
        if drag_source_fi == Some(flat_idx - 1) {
            primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
            primitive_renderer.draw_border(x + 1.0, line_y + 1.0, width - 2.0, ITEM_HEIGHT - 2.0, drag_source_border, 1.0);
        }

        // Workspace number/identifier (first 2 chars)
        let ws_label = ws.name.chars().take(2).collect::<String>();
        let ws_color = if is_ws_cursor {
            accent
        } else {
            match ws.state {
                crate::app_state::SidebarItemState::Active => accent,
                crate::app_state::SidebarItemState::Visited => visited_color,
                crate::app_state::SidebarItemState::None => foreground,
            }
        };
        let ws_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
        text_renderer.queue_text(&ws_label, text_x, ws_text_y, font_size, ws_color);
        line_y += ITEM_HEIGHT;

        // Pane letters
        if !ws.collapsed {
            for col in &ws.columns {
                flat_idx += 1; // consume Column item (invisible in collapsed mode)
                for pane in &col.panes {
                    let is_pane_cursor = is_sidebar_nav && flat_idx == tree.cursor;
                    flat_idx += 1; // consume Pane item

                    if is_pane_cursor {
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, cursor_bg);
                    }

                    // Drag hover highlight (pane or column)
                    if drag_hover_fi == Some(flat_idx) {
                        let mut drag_bg = accent;
                        drag_bg[3] = 0.25;
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_bg);
                    }

                    // Drag source effect for pane items.
                    if drag_source_fi == Some(flat_idx) {
                        primitive_renderer.draw_rect(x, line_y, width, ITEM_HEIGHT, drag_source_bg);
                        primitive_renderer.draw_border(x + 1.0, line_y + 1.0, width - 2.0, ITEM_HEIGHT - 2.0, drag_source_border, 1.0);
                    }

                    let mut pane_char = pane.name.chars().next()
                        .unwrap_or('?').to_string();
                    // Show candidate letter during PaneSwap / PaneSelect
                    // but NOT for the focused pane — no need to swap with yourself.
                    if let Some(cands) = candidates
                        && focused_pane != Some(pane.pane_id)
                        && let Some((ch, _)) = cands.iter().find(|(_, pid)| *pid == pane.pane_id)
                    {
                        pane_char = ch.to_string();
                    }

                    let pane_color = if is_pane_cursor {
                        accent
                    } else {
                        match pane.state {
                            crate::app_state::SidebarItemState::Active => accent,
                            crate::app_state::SidebarItemState::Visited => visited_color,
                            crate::app_state::SidebarItemState::None => foreground,
                        }
                    };
                    let pane_text_y = line_y + (ITEM_HEIGHT - font_size) / 2.0;
                    text_renderer.queue_text(&pane_char, text_x, pane_text_y, font_size, pane_color);
                    line_y += ITEM_HEIGHT;
                }
            }
        }
    }
}

/// Helper to find a pane's display name from the tree.
fn pane_name_short(pane_id: u64, workspaces: &[SidebarWsEntry]) -> String {
    for ws in workspaces {
        for col in &ws.columns {
            for pane in &col.panes {
                if pane.pane_id == pane_id {
                    return pane.name.clone();
                }
            }
        }
    }
    format!("Pane {}", pane_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{
        Pane as LayoutPane, PaneId,
        session::Session, types::{SessionId, Size},
    };

    fn make_test_session() -> (Session, Vec<u64>) {
        let viewport = Size::new(1280.0, 800.0);
        let mut session = Session::new(SessionId(1), viewport, 2.0, heca_core::layout::types::LayoutOptions::default());

        // Create 3 panes in the first workspace
        let _ids: Vec<u64> = (1..=4).map(|i| {
            let pane = LayoutPane::new(PaneId(i), format!("Pane{}", i));
            let id = pane.id.0;
            session.add_pane(pane, None, true);
            id
        }).collect();

        // Add a second workspace with 1 pane
        let wa = session.active_workspace().map(|ws| {
            let r = ws.scrolling.working_area;
            heca_core::layout::types::Rectangle::new(r.loc, r.size)
        }).unwrap_or_else(|| {
            heca_core::layout::types::Rectangle::new(
                heca_core::layout::types::Point::new(0.0, 0.0),
                viewport,
            )
        });
        session.add_workspace(wa);
        let pane5 = LayoutPane::new(PaneId(5), "Pane5");
        let _id5 = pane5.id.0;
        session.add_pane(pane5, None, true);

        (session, vec![1, 2, 3, 4, 5])
    }

    #[test]
    fn test_tree_rebuild() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        tree.rebuild(&session, None, Some(1), &[]);

        // Should have 2 workspaces
        assert_eq!(tree.workspaces.len(), 2, "should have 2 workspaces");

        // WS 0 should be active (the one with panes)
        assert_eq!(tree.workspaces[0].state, SidebarItemState::Active, "WS 0 should be active");
        assert_eq!(tree.workspaces[1].state, SidebarItemState::None, "WS 1 should be none (not yet visited)");

        // WS 0 should have some columns with panes
        let ws0 = &tree.workspaces[0];
        assert!(!ws0.columns.is_empty(), "WS 0 should have columns");
        assert!(!ws0.collapsed, "WS 0 should not be collapsed by default");
    }

    #[test]
    fn test_tree_flat_items() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Flat items should contain workspaces, columns, and panes
        assert!(!tree.flat_items.is_empty(), "flat items should not be empty");

        // First item should be a workspace
        match &tree.flat_items[0] {
            SidebarItem::Workspace { .. } => {}
            other => panic!("first flat item should be Workspace, got {:?}", other),
        }

        // Item count should match flat_items.len()
        assert_eq!(tree.item_count, tree.flat_items.len(), "item_count should match");
    }

    #[test]
    fn test_cursor_movement() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        assert_eq!(tree.cursor, 0, "cursor starts at 0");

        tree.cursor_down();
        assert_eq!(tree.cursor, 1, "cursor moves down to 1");

        tree.cursor_up();
        assert_eq!(tree.cursor, 0, "cursor moves up back to 0");

        // Move to end, then past end should clamp
        for _ in 0..tree.item_count + 5 {
            tree.cursor_down();
        }
        assert_eq!(tree.cursor, tree.item_count - 1, "cursor clamps at last item");

        // Move past start should clamp at 0
        for _ in 0..103 {
            tree.cursor_up();
        }
        assert_eq!(tree.cursor, 0, "cursor clamps at first item");
    }

    #[test]
    fn test_expand_collapse_workspace() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Initially not collapsed
        assert!(!tree.workspaces[0].collapsed, "WS 0 should not be collapsed initially");

        // Move cursor to workspace 0 and toggle expand
        tree.cursor = 0;
        tree.toggle_expand();

        // Should now be collapsed
        assert!(tree.workspaces[0].collapsed, "WS 0 should be collapsed after toggle");

        // Flat items should have fewer items (children hidden)
        let collapsed_count = tree.flat_items.len();

        // Toggle again to expand
        tree.toggle_expand();
        assert!(!tree.workspaces[0].collapsed, "WS 0 should be expanded after second toggle");
        assert!(tree.flat_items.len() > collapsed_count, "flat items should increase after expand");
    }

    #[test]
    fn test_visited_tracking() {
        let (mut session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        // Simulate visiting workspace 1 (switch to it)
        session.switch_to_workspace(1);

        // Rebuild with last_visited_ws_idx = 0 (WS 0 was visited before)
        tree.rebuild(&session, Some(0), Some(5), &[]);

        // WS 1 should be active (current)
        assert_eq!(tree.workspaces[1].state, SidebarItemState::Active, "WS 1 should be active after switch");
        // WS 0 should be visited
        assert_eq!(tree.workspaces[0].state, SidebarItemState::Visited, "WS 0 should be visited");
        // WS 2 (doesn't exist) is none
    }

    #[test]
    fn test_rebuild_clears_previous() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();

        tree.rebuild(&session, None, Some(1), &[]);
        let first_count = tree.flat_items.len();

        // Rebuild again — should be same result
        tree.rebuild(&session, None, Some(1), &[]);
        assert_eq!(tree.flat_items.len(), first_count, "rebuild should produce same result");

        // Cursor should be clamped if it was out of bounds
        tree.cursor = 9999;
        tree.rebuild(&session, None, Some(1), &[]);
        assert!(tree.cursor < tree.flat_items.len(), "cursor should be clamped after rebuild");
    }

    #[test]
    fn test_empty_session() {
        let viewport = Size::new(1280.0, 800.0);
        let session = Session::new(SessionId(1), viewport, 2.0, heca_core::layout::types::LayoutOptions::default());
        let mut tree = SidebarTree::new();

        tree.rebuild(&session, None, None, &[]);

        // Even an empty session has at least 1 workspace (the initial one)
        assert!(!tree.workspaces.is_empty(), "should have at least 1 workspace");
        // But it may have no panes
    }

    #[test]
    fn test_cursor_down_collapsed_skips_columns() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Start at first workspace (index 0). In collapsed mode cursor_down
        // should skip the Column item and land on the first Pane.
        tree.cursor = 0;
        tree.cursor_down_collapsed();
        assert!(
            matches!(tree.flat_items[tree.cursor], SidebarItem::Pane { .. }),
            "cursor_down_collapsed should skip Column and land on Pane, got {:?}",
            tree.flat_items[tree.cursor]
        );
    }

    #[test]
    fn test_cursor_up_collapsed_skips_columns() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Put cursor on the first Pane item after its Column.
        // cursor_up_collapsed should skip the Column and land on Workspace.
        let first_pane_idx = tree.flat_items.iter().position(|i| {
            matches!(i, SidebarItem::Pane { .. })
        }).expect("should have a pane");
        tree.cursor = first_pane_idx;
        tree.cursor_up_collapsed();
        assert!(
            matches!(tree.flat_items[tree.cursor], SidebarItem::Workspace { .. }),
            "cursor_up_collapsed should skip Column and land on Workspace, got {:?}",
            tree.flat_items[tree.cursor]
        );
    }

    #[test]
    fn test_toggle_expand_clamps_cursor() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Place cursor deep inside workspace 0 (e.g. on a pane).
        let ws0_last_idx = tree.flat_items.iter().enumerate().rposition(|(_, i)| {
            matches!(i, SidebarItem::Workspace { ws_idx } if *ws_idx == 0)
                || matches!(i, SidebarItem::Column { ws_idx, .. } if *ws_idx == 0)
                || matches!(i, SidebarItem::Pane { pane_id } if *pane_id <= 4)
        }).expect("should have ws0 items");
        tree.cursor = ws0_last_idx;

        // Collapse workspace 0 — its children disappear.
        tree.workspaces[0].collapsed = true;
        tree.rebuild_flat_items();
        tree.clamp_cursor();

        // clamp_cursor() only bounds-checks; cursor may end up on a later workspace.
        // The invariant is that cursor must be valid after collapse.
        assert!(
            tree.cursor < tree.flat_items.len(),
            "cursor must be valid after collapse: cursor={} len={}",
            tree.cursor,
            tree.flat_items.len()
        );
    }

    #[test]
    fn test_column_expand_collapse() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Find first Column item in flat list.
        let col_idx = tree.flat_items.iter().position(|i| {
            matches!(i, SidebarItem::Column { .. })
        }).expect("should have a column");
        tree.cursor = col_idx;

        // Collapse the column.
        tree.toggle_expand();
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(tree.workspaces[ws_idx].columns[c].collapsed, "column should be collapsed");
        }

        // Expand it back.
        tree.toggle_expand();
        if let SidebarItem::Column { ws_idx, col_idx: c } = tree.flat_items[tree.cursor] {
            assert!(!tree.workspaces[ws_idx].columns[c].collapsed, "column should be expanded");
        }
    }

    #[test]
    fn test_sidebar_hit_test_expanded() {
        let (session, _ids) = make_test_session();
        let tree = SidebarTree::new();
        // Rebuild into a fresh tree (cursor at 0)
        let mut tree = tree;
        tree.rebuild(&session, None, Some(1), &[]);

        // Click on first item line (just below top padding).
        let fi = sidebar_hit_test(&tree, 32.0, 400.0, 200.0, 36.0 + 4.0 + 2.0);
        assert_eq!(fi, Some(0), "click on first line should hit flat item 0");

        // Click above sidebar should miss.
        assert_eq!(sidebar_hit_test(&tree, 32.0, 400.0, 200.0, 10.0), None);
    }

    #[test]
    fn test_sidebar_hit_test_collapsed() {
        let (session, _ids) = make_test_session();
        let mut tree = SidebarTree::new();
        tree.rebuild(&session, None, Some(1), &[]);

        // Collapsed mode (width < 80). Click on second visible line.
        // In collapsed mode visible lines are: WS, Pane, Pane... (columns hidden).
        let fi = sidebar_hit_test(&tree, 32.0, 400.0, 40.0, 36.0 + 4.0 + 2.0 + ITEM_HEIGHT);
        // Second visible line should be the first Pane (skipping the Column).
        assert_eq!(fi, Some(2), "second visible line in collapsed mode should be first Pane (flat idx 2)");
    }
}

