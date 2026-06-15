//! Chrome metrics and central application constants.
//!
//! UI dimensions, timing defaults, layout proportions, and render parameters
//! that were previously scattered as magic numbers across the codebase.

use heca_core::layout::types::{Point, Rectangle, Size};
use heca_core::layout::ColumnWidth;
use std::time::Duration;

// ── Chrome metrics ──

/// Default tab bar height in logical pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 32.0;
/// Default status bar height in logical pixels.
pub const DEFAULT_STATUS_BAR_HEIGHT: f32 = 24.0;
/// Default collapsed sidebar width in logical pixels.
pub const DEFAULT_COLLAPSED_SIDEBAR_WIDTH: f32 = 40.0;
/// Minimum sidebar width to be considered expanded (for rendering decisions).
pub const SIDEBAR_EXPANDED_THRESHOLD: f32 = 80.0;

// ── Timing ──

/// Prefix mode auto-exit timeout (ms). After this time with no key, prefix mode cancels.
pub const PREFIX_TIMEOUT: Duration = Duration::from_millis(500);
/// Target frame interval (~60 FPS).
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

// ── Layout defaults ──

/// Default width proportion for newly created columns.
pub(crate) const DEFAULT_COLUMN_PROPORTION: f64 = 0.5;
/// Helper to get the default ColumnWidth for new columns.
pub const fn default_column_width() -> ColumnWidth {
    ColumnWidth::Proportion(DEFAULT_COLUMN_PROPORTION)
}

// ── Pane name overlay ──

/// Font size factor for the pane name overlay (fraction of min(width, height)).
pub const PANE_NAME_SIZE_FACTOR: f32 = 0.25;
/// Minimum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MIN: f32 = 24.0;
/// Maximum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MAX: f32 = 72.0;

// ── Chrome text ──

/// Default font size for chrome text (tab bar, status bar).
pub const CHROME_TEXT_SIZE: f32 = 14.0;

// ── Mouse ──

/// Distance from content area edge that triggers edge scrolling (logical pixels).
pub const EDGE_SCROLL_TRIGGER: f32 = 80.0;

/// Layout configuration for chrome elements around the pane area.
#[derive(Clone, Copy, Debug)]
pub struct ChromeConfig {
    pub tab_bar_height: f32,
    pub status_bar_height: f32,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
}

impl ChromeConfig {
    /// Compute the rectangle available for pane content, given a window size.
    /// Chrome occupies the outer edges; panes get the center.
    pub fn content_rect(&self, window_width: f32, window_height: f32) -> Rectangle {
        let x = self.left_sidebar_width;
        let y = self.tab_bar_height;
        let w = window_width
            - self.left_sidebar_width.min(window_width)
            - self
                .right_sidebar_width
                .min(window_width - self.left_sidebar_width);
        let h = window_height - self.tab_bar_height - self.status_bar_height;
        Rectangle::new(Point::new(x as f64, y as f64), Size::new(w as f64, h as f64))
    }
}

// ── Grid-UI chrome scene builder ──────────────────────────────────────────────

use crate::app_state::SidebarItemState;
use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree};
use heca_core::layout::PaneId;
use heca_grid_ui::builders::{LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    ActiveMarker, Badge, DockFrame, Flex, Glyph, Icon, IconButton, Label, Pane, Row, Surface,
};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
use std::cell::Cell;
use std::rc::Rc;

/// Sink a pane card writes its id into when clicked. `Some(id)` = that card fired.
pub(crate) type SidebarClickSink = Rc<Cell<Option<PaneId>>>;

/// Host-owned sinks the chrome widgets write into when activated, so the app can read
/// the result after dispatching a pointer event into the retained tree. Lives in
/// `AppState` (survives tree rebuilds), cloned into widget callbacks. Grows as later
/// slices add targeting (F4.4) / drag (F4.5).
pub(crate) struct ChromeSinks {
    /// `Some(pane_id)` = that pane card was clicked → focus it (F4.2).
    pub click: SidebarClickSink,
    /// `Some(ws_idx)` = that workspace header was toggled → flip its collapsed state (F4.3).
    pub ws_toggle: Rc<Cell<Option<usize>>>,
}

impl ChromeSinks {
    pub(crate) fn new() -> Self {
        Self {
            click: Rc::new(Cell::new(None)),
            ws_toggle: Rc::new(Cell::new(None)),
        }
    }

    fn clear(&self) {
        self.click.set(None);
        self.ws_toggle.set(None);
    }
}

/// What a dispatched sidebar click resolved to.
pub(crate) enum ChromeClick {
    /// A pane card was clicked → focus this pane.
    Pane(PaneId),
    /// A workspace header was clicked → toggle this workspace's collapsed state.
    WorkspaceToggle(usize),
    /// Click landed on empty space / a non-interactive area.
    None,
}

/// Leading glyph for a pane card. **Dynamic-ready seam:** today every pane hosts a
/// terminal, so this is always [`Glyph::Terminal`]; later it becomes a function of
/// the process/app running in the pane (editor, shell, browser, …).
fn pane_glyph(_pane_name: &str) -> Glyph {
    Glyph::Terminal
}

/// A single pane **card**, styled like the showcase PANES rows: a state-tinted
/// background + radius, an active accent bar, a leading (dynamic) icon and the pane
/// name. The showcase card's trailing status badge + multi-segment git-branch `Tag`
/// are intentionally omitted — that data (process status / branch / change info)
/// isn't on `SidebarPaneEntry` yet; this is the composition seam for it. Display-only
/// for now — interaction is wired with the F4 shared-state layer.
fn pane_card(pane: &SidebarPaneEntry, theme: &GuiTheme, sink: SidebarClickSink) -> Row {
    let active = pane.state == SidebarItemState::Active;
    let tint = if active { theme.accent } else { theme.foreground };
    let pane_id = pane.pane_id;
    Row::new()
        .background(tint.with_alpha(if active { 30 } else { 12 }))
        .radius(theme.control_radius())
        .padding(6.0)
        .marker(ActiveMarker::Bar)
        .active(active)
        // On click/Enter the card records its pane id in the host sink; the app reads
        // it after dispatch and focuses that pane (read-via-signal / write-via-action).
        .on_activate(move || sink.set(Some(pane_id)))
        .child(
            Flex::row()
                .align(Align::Center)
                .gap(8.0)
                .child(Icon::new(pane_glyph(&pane.name)).size(16.0).color(tint))
                .child(Label::new(pane.name.clone()).color(if active {
                    theme.accent
                } else {
                    theme.foreground
                })),
        )
}

/// One **column**, rendered compactly: a full-height left **marker bar** + the
/// column's stacked pane cards — no per-column header row (columns are spatial
/// groupings whose only user-facing job is to be a move/swap target + drag handle).
/// The bar is the **seam** for that: the move/swap letter target and future DnD
/// drag handle attach here (wired with the F4 shared-state layer). The bar brightens
/// to the accent when the column holds the active pane. `Align::Stretch` (the Flex
/// default) makes the fixed-width bar span the height of the pane stack.
fn column_view(c: &SidebarColEntry, theme: &GuiTheme, sink: &SidebarClickSink) -> Flex {
    let active = c.panes.iter().any(|p| p.state == SidebarItemState::Active);
    let bar_color = if active {
        theme.accent
    } else {
        theme.accent.with_alpha(90)
    };
    let mut panes = Flex::column().gap(3.0).grow(1.0);
    for pane in &c.panes {
        panes = panes.child(pane_card(pane, theme, sink.clone()));
    }
    Flex::row()
        .gap(6.0)
        .child(
            Surface::new()
                .width(Length::Px(3.0))
                .radius(1.5)
                .background(bar_color),
        )
        .child(panes)
}

/// Build the **WorkspacesContainer** content — the workspace tree mounted inside the
/// sidebar shell (see `heca-sidebar-design-spec`). Each workspace is a `.frameless()`
/// [`DockFrame`] (header count [`Badge`] = total panes); its columns are compact
/// [`column_view`]s (left marker bar + pane cards, no "Col N" header rows — those ate
/// the sidebar for no user value). Pure projection of the [`SidebarTree`].
fn build_workspaces_container(tree: &SidebarTree, theme: &GuiTheme, sinks: &ChromeSinks) -> Flex {
    let mut col = Flex::column().gap(6.0).grow(1.0);
    for ws in &tree.workspaces {
        let pane_count =
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len();
        let active_ws = ws.state == SidebarItemState::Active;
        let badge = if active_ws {
            Badge::accent(pane_count.to_string())
        } else {
            Badge::neutral(pane_count.to_string())
        };
        // The header toggle records the workspace in the toggle sink; the app flips
        // its collapsed state (canonical) and the tree rebuilds (F4.3).
        let ws_idx = ws.ws_idx;
        let ws_sink = sinks.ws_toggle.clone();
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless()
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!ws.collapsed)
            .on_toggle(move |_| ws_sink.set(Some(ws_idx)))
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        // Columns stacked with a clear gap between them (the gap + bar mark each
        // column); panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(8.0);
        for c in &ws.columns {
            cols = cols.child(column_view(c, theme, &sinks.click));
        }
        for float in &ws.floating_panes {
            cols = cols.child(pane_card(float, theme, sinks.click.clone()));
        }
        dock = dock.child(cols);
        col = col.child(dock);
    }
    col
}

/// Build the LEFT sidebar **SHELL**: a full-height, bracket-framed, frosted panel
/// occupying the *whole* left column — a header (collapse toggle) over the body. Per
/// the chrome plan (F5, `pluggable-chrome-plugin-plan.md` §2.1) the sidebar is a
/// *shell*; the [`build_workspaces_container`] tree is mounted into the body as the
/// first container (display-only until the F4 shared-state layer wires interaction).
fn build_sidebar_shell(
    tree: &SidebarTree,
    left_w: f32,
    sidebar_h: f32,
    theme: &GuiTheme,
    sinks: &ChromeSinks,
) -> Pane {
    // Header: a sidebar glyph + a collapse toggle pushed to the right. The toggle is
    // inert until interaction is wired (it becomes an action against shared state).
    let header = Flex::row()
        .align(Align::Center)
        .gap(6.0)
        .padding_xy(2.0, 2.0)
        .child(Icon::new(Glyph::Sidebar).size(16.0).color(theme.muted))
        .child(Flex::row().grow(1.0))
        .child(IconButton::new(
            Icon::new(Glyph::CaretRight).size(14.0).color(theme.muted),
        ));

    Pane::new()
        .width(Length::Px(left_w))
        .height(Length::Px(sidebar_h))
        .padding(10.0)
        .gap(8.0)
        .background(theme.surface)
        .child(header)
        .child(build_workspaces_container(tree, theme, sinks))
}

/// Assemble the chrome root widget tree (no layout/paint): a transparent tab band,
/// a middle row hosting the (optional) full-height sidebar shell + a transparent
/// content spacer, and the opaque status bar at the bottom. Returns the concrete
/// [`Flex`] so it can be **retained** across frames (see [`RetainedChrome`]).
fn chrome_root(
    w: f32,
    h: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    status: &str,
    side_bg: Color,
    fg: Color,
    sidebar: Option<Pane>,
) -> Flex {
    let middle_h = (h - tab_bar_height - status_bar_height).max(0.0);

    // Middle row: the full-height sidebar shell (when expanded) + a transparent
    // spacer over the content area (panes are drawn by the hand-drawn path under
    // this scene). The shell sizes its own width/height.
    let mut middle = Flex::row()
        .width(Length::Px(w))
        .height(Length::Px(middle_h));
    if let Some(shell) = sidebar {
        middle = middle.child(shell);
    }
    middle = middle.child(Flex::row().grow(1.0));

    Flex::column()
        .width(Length::Px(w))
        .height(Length::Px(h))
        // Transparent tab band — the hand-drawn tab bar paints underneath.
        .child(
            Flex::row()
                .width(Length::Px(w))
                .height(Length::Px(tab_bar_height)),
        )
        .child(middle)
        .child(
            Surface::row()
                .width(Length::Px(w))
                .height(Length::Px(status_bar_height))
                .background(side_bg)
                .radius(0.0)
                .align(Align::Center)
                .padding_xy(8.0, 0.0)
                .child(Label::new(status).font_size(CHROME_TEXT_SIZE).color(fg)),
        )
}

/// Layout + paint a (retained) chrome root tree into a [`Scene`] at the window size.
/// Re-run every frame; cheap and creates no signals (those live in the retained tree).
pub(crate) fn paint_chrome_root(root: &mut Flex, w: f32, h: f32, theme: &GuiTheme) -> Scene {
    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(root, Size::new(w as f64, h as f64));
    {
        let mut cx =
            PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        root.paint(&mut cx);
    }
    scene
}

/// Test helper: build + layout + paint in one shot. Runtime uses the retained tree
/// ([`build_chrome_root`] + [`paint_chrome_root`]) instead.
#[cfg(test)]
fn chrome_scene(
    w: f32,
    h: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    status: &str,
    side_bg: Color,
    fg: Color,
    theme: &GuiTheme,
    sidebar: Option<Pane>,
) -> Scene {
    let mut root = chrome_root(
        w,
        h,
        tab_bar_height,
        status_bar_height,
        status,
        side_bg,
        fg,
        sidebar,
    );
    paint_chrome_root(&mut root, w, h, theme)
}

/// `(status-bar bg, sidebar-shell bg, foreground)` for the chrome, honoring the
/// transparency setting: the sidebar shell is deliberately *less* frosted than the
/// rest of the chrome (the main scrolling area stays fully transparent).
fn chrome_colors(state: &crate::app_state::AppState) -> (Color, Color, Color) {
    // Mirror render.rs's hand-drawn chrome base color exactly.
    let base = if state.theme.name == "Catppuccin Mocha" {
        Color::new(17, 17, 27, 255)
    } else {
        Color::new(243, 244, 248, 255)
    };
    let side_bg = base.with_alpha((state.appearance.chrome_opacity() * 255.0).round() as u8);
    let sidebar_alpha = (0.5 + 0.5 * state.appearance.opacity()).clamp(0.0, 1.0);
    let sidebar_bg = base.with_alpha((sidebar_alpha * 255.0).round() as u8);
    let fg = {
        let c = state.theme.foreground;
        Color::new(c.r, c.g, c.b, c.a)
    };
    (side_bg, sidebar_bg, fg)
}

/// Bridge the app theme into a GuiTheme for the chrome (frosted sidebar surface +
/// app accent/border/radius). Cheap (no signals) — rebuilt each frame to paint the
/// retained tree.
pub(crate) fn chrome_gui_theme(state: &crate::app_state::AppState) -> GuiTheme {
    let (_, sidebar_bg, fg) = chrome_colors(state);
    let mut theme = GuiTheme::grid_tron();
    theme.foreground = fg;
    theme.surface = sidebar_bg;
    theme.accent = {
        let c = state.theme.accent;
        Color::new(c.r, c.g, c.b, c.a)
    };
    theme.border = {
        let c = state.theme.border;
        Color::new(c.r, c.g, c.b, c.a)
    };
    theme.radius = state.theme.border_radius;
    theme
}

/// The status-bar text projection (`N panes | focus | MODE…`).
fn chrome_status(state: &crate::app_state::AppState) -> String {
    let pane_count = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.columns.iter().map(|c| c.panes.len()).sum::<usize>())
        .unwrap_or(0);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.title.as_str())
        .unwrap_or("—");
    let (mode_str, rename_hint) = crate::app::render::status_mode_parts(&state.input_mode);
    format!("{} panes | {} | {}{}", pane_count, focus_title, mode_str, rename_hint)
}

/// A **retained** chrome tree + the signature of the state that produced it. The
/// tree is rebuilt only when [`chrome_signature`] changes; otherwise it is just
/// re-laid-out and painted each frame. This keeps the widget signals alive across
/// frames (no per-frame signal churn) and gives a live tree to dispatch events into
/// (F4.2). The collapsed sidebar rail is still hand-drawn in `render.rs`.
pub(crate) struct RetainedChrome {
    pub(crate) root: Flex,
    pub(crate) sig: u64,
}

/// Build the chrome root tree from app state (the expensive part — creates the
/// widget tree and its signals). Call only when [`chrome_signature`] changes.
pub(crate) fn build_chrome_root(state: &crate::app_state::AppState, chrome: ChromeConfig) -> Flex {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys.width as f32 / scale;
    let h = phys.height as f32 / scale;
    let (side_bg, _sidebar_bg, fg) = chrome_colors(state);
    let theme = chrome_gui_theme(state);
    let status = chrome_status(state);

    let left_w = chrome.left_sidebar_width;
    let sidebar = if left_w >= SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_h = (h - DEFAULT_TAB_BAR_HEIGHT - DEFAULT_STATUS_BAR_HEIGHT).max(0.0);
        Some(build_sidebar_shell(
            &state.sidebar_tree,
            left_w,
            sidebar_h,
            &theme,
            &state.chrome_sinks,
        ))
    } else {
        None
    };

    chrome_root(
        w,
        h,
        DEFAULT_TAB_BAR_HEIGHT,
        DEFAULT_STATUS_BAR_HEIGHT,
        &status,
        side_bg,
        fg,
        sidebar,
    )
}

/// Hit-test a sidebar click by dispatching a pointer-press into the **retained**
/// chrome tree (whose widgets are laid out at their real on-screen bounds), then
/// reading which widget recorded itself in a sink — a pane card ([`ChromeClick::Pane`])
/// or a workspace header ([`ChromeClick::WorkspaceToggle`]). `pos` is in logical
/// window coordinates (same space as the tree layout).
///
/// The dispatched tree is then discarded (rebuilt next frame) so any incidental
/// internal widget state (e.g. a `DockFrame`'s own expanded flag) doesn't persist
/// out of sync with canonical state — the app applies the change to the `SidebarTree`.
pub(crate) fn chrome_dispatch_click(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> ChromeClick {
    state.chrome_sinks.clear();
    if let Some(tree) = state.chrome_tree.as_mut() {
        tree.root.event(&Event::PointerPressed {
            pos: Point::new(pos.0 as f64, pos.1 as f64),
        });
    }
    state.chrome_tree = None;
    if let Some(pane_id) = state.chrome_sinks.click.get() {
        ChromeClick::Pane(pane_id)
    } else if let Some(ws_idx) = state.chrome_sinks.ws_toggle.get() {
        ChromeClick::WorkspaceToggle(ws_idx)
    } else {
        ChromeClick::None
    }
}

fn sidebar_state_tag(s: &SidebarItemState) -> u8 {
    match s {
        SidebarItemState::Active => 0,
        SidebarItemState::Visited => 1,
        SidebarItemState::None => 2,
    }
}

/// Hash of everything the chrome tree displays (window size, theme, status text,
/// sidebar content). When it changes, the retained tree is rebuilt; otherwise the
/// existing tree is reused (re-laid-out + painted only).
pub(crate) fn chrome_signature(state: &crate::app_state::AppState, chrome: ChromeConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hsh = std::collections::hash_map::DefaultHasher::new();
    let phys = state.window.inner_size();
    phys.width.hash(&mut hsh);
    phys.height.hash(&mut hsh);
    state.scale_factor.to_bits().hash(&mut hsh);
    chrome.left_sidebar_width.to_bits().hash(&mut hsh);
    state.theme.name.hash(&mut hsh);
    for c in [state.theme.accent, state.theme.foreground, state.theme.border] {
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    state.theme.border_radius.to_bits().hash(&mut hsh);
    state.appearance.chrome_opacity().to_bits().hash(&mut hsh);
    state.appearance.opacity().to_bits().hash(&mut hsh);
    chrome_status(state).hash(&mut hsh);
    for ws in &state.sidebar_tree.workspaces {
        ws.ws_idx.hash(&mut hsh);
        ws.name.hash(&mut hsh);
        ws.collapsed.hash(&mut hsh);
        sidebar_state_tag(&ws.state).hash(&mut hsh);
        for c in &ws.columns {
            c.col_idx.hash(&mut hsh);
            c.collapsed.hash(&mut hsh);
            for p in &c.panes {
                p.pane_id.0.hash(&mut hsh);
                p.name.hash(&mut hsh);
                sidebar_state_tag(&p.state).hash(&mut hsh);
            }
            u8::MAX.hash(&mut hsh); // column separator in the hash stream
        }
        for p in &ws.floating_panes {
            p.pane_id.0.hash(&mut hsh);
            p.name.hash(&mut hsh);
            sidebar_state_tag(&p.state).hash(&mut hsh);
        }
        u64::MAX.hash(&mut hsh); // workspace separator
    }
    hsh.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_rect_full() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.loc.x, 200.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 880.0);
        assert_eq!(r.size.h, 744.0);
    }

    #[test]
    fn test_content_rect_clamping_when_sidebars_exceed_window() {
        // Both sidebars together exceed the window width.
        // Left sidebar is NOT clamped for x-position, but IS clamped for width calculation.
        // Right sidebar is clamped to remaining space after left sidebar.
        // Width must never go negative.
        let c = ChromeConfig {
            tab_bar_height: 20.0,
            status_bar_height: 10.0,
            left_sidebar_width: 300.0,
            right_sidebar_width: 300.0,
        };
        let r = c.content_rect(500.0, 600.0);
        // x = 300, y = 20
        // left clamped: min(300, 500) = 300
        // right clamped: min(300, 500-300) = min(300, 200) = 200
        // w = 500 - 300 - 200 = 0  (not negative)
        // h = 600 - 20 - 10 = 570
        assert_eq!(r.loc.x, 300.0);
        assert_eq!(r.loc.y, 20.0);
        assert_eq!(r.size.w, 0.0);
        assert_eq!(r.size.h, 570.0);
    }

    #[test]
    fn test_content_rect_no_sidebars() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 0.0,
            right_sidebar_width: 0.0,
        };
        let r = c.content_rect(1024.0, 768.0);
        assert_eq!(r.loc.x, 0.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 1024.0);
        assert_eq!(r.size.h, 712.0);
    }

    #[test]
    fn chrome_scene_emits_status_text() {
        use heca_grid_ui::{Color, DrawCommand};
        let theme = GuiTheme::grid_tron();
        // No sidebar (collapsed) — just the status bar should produce text.
        let scene = super::chrome_scene(
            800.0,
            600.0,
            32.0,
            24.0,
            "2 panes | foo | NORMAL",
            Color::new(17, 17, 27, 255),
            Color::new(200, 200, 200, 255),
            &theme,
            None,
        );
        assert!(!scene.is_empty(), "scene should not be empty");
        assert!(
            scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(..))),
            "scene should contain at least one Text draw command",
        );
    }

    #[test]
    fn sidebar_shell_hosts_header_and_container() {
        use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree, SidebarWsEntry};
        use heca_grid_ui::Component;

        let mut tree = SidebarTree::new();
        tree.workspaces.push(SidebarWsEntry {
            ws_idx: 0,
            name: "ws1".into(),
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![SidebarColEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![SidebarPaneEntry {
                    pane_id: heca_core::layout::PaneId(1),
                    name: "pane1".into(),
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        let theme = GuiTheme::grid_tron();
        let sinks = super::ChromeSinks::new();
        // The shell is [header, WorkspacesContainer]; the container hosts a dock per
        // workspace (so the tree's text is visible inside the shell).
        let shell = super::build_sidebar_shell(&tree, 280.0, 600.0, &theme, &sinks);
        assert_eq!(
            shell.base().children.len(),
            2,
            "sidebar shell must be [header, WorkspacesContainer body]",
        );
        let container = &shell.base().children[1];
        assert!(
            !container.base().children.is_empty(),
            "WorkspacesContainer must host a dock per workspace",
        );
    }
}
