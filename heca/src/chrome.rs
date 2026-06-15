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
use heca_grid_ui::builders::{LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    ActiveMarker, Badge, DockFrame, Flex, Glyph, Icon, IconButton, Label, Pane, Row, Surface,
};
use heca_grid_ui::{Color, Component, LayoutEngine, PaintCx, Scene};

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
fn pane_card(pane: &SidebarPaneEntry, theme: &GuiTheme) -> Row {
    let active = pane.state == SidebarItemState::Active;
    let tint = if active { theme.accent } else { theme.foreground };
    Row::new()
        .background(tint.with_alpha(if active { 30 } else { 12 }))
        .radius(theme.control_radius())
        .padding(6.0)
        .marker(ActiveMarker::Bar)
        .active(active)
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
fn column_view(c: &SidebarColEntry, theme: &GuiTheme) -> Flex {
    let active = c.panes.iter().any(|p| p.state == SidebarItemState::Active);
    let bar_color = if active {
        theme.accent
    } else {
        theme.accent.with_alpha(90)
    };
    let mut panes = Flex::column().gap(3.0).grow(1.0);
    for pane in &c.panes {
        panes = panes.child(pane_card(pane, theme));
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
fn build_workspaces_container(tree: &SidebarTree, theme: &GuiTheme) -> Flex {
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
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless()
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!ws.collapsed)
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
            cols = cols.child(column_view(c, theme));
        }
        for float in &ws.floating_panes {
            cols = cols.child(pane_card(float, theme));
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
fn build_sidebar_shell(tree: &SidebarTree, left_w: f32, sidebar_h: f32, theme: &GuiTheme) -> Pane {
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
        .child(build_workspaces_container(tree, theme))
}

/// Build a full-window chrome [`Scene`]: a transparent tab band, a middle row
/// hosting the (optional) full-height sidebar shell beside a transparent content
/// spacer, and the opaque status bar at the bottom. Pure: takes plain values + a
/// prebuilt sidebar so it can be unit-tested without wgpu/AppState.
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

    let mut root = Flex::column()
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
        );

    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut root, Size::new(w as f64, h as f64));
    {
        let mut cx = PaintCx::new(&mut scene, theme)
            .with_viewport(Size::new(w as f64, h as f64));
        root.paint(&mut cx);
    }
    scene
}

/// Read app state and produce a full-window chrome [`Scene`] (tab band, expanded
/// LEFT sidebar, status bar). Pure projection: reads `state`, returns a `Scene`,
/// mutates nothing. The collapsed sidebar rail is still hand-drawn in `render.rs`.
pub(crate) fn build_chrome_scene(state: &crate::app_state::AppState, chrome: ChromeConfig) -> Scene {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys.width as f32 / scale;
    let h = phys.height as f32 / scale;

    // Mirror the special-case from render.rs exactly.
    let side_bg_base = if state.theme.name == "Catppuccin Mocha" {
        Color::new(
            (0.067_f32 * 255.0).round() as u8,
            (0.067_f32 * 255.0).round() as u8,
            (0.106_f32 * 255.0).round() as u8,
            255,
        )
    } else {
        Color::new(
            (0.953_f32 * 255.0).round() as u8,
            (0.957_f32 * 255.0).round() as u8,
            (0.973_f32 * 255.0).round() as u8,
            255,
        )
    };
    // Frosted chrome: translucent status-bar background when the window is
    // transparent (mirrors render.rs's chrome_alpha for the hand-drawn chrome).
    let side_bg = side_bg_base.with_alpha((state.appearance.chrome_opacity() * 255.0).round() as u8);
    // The sidebar SHELL is deliberately LESS transparent than the rest of the
    // chrome (the main scrolling area stays fully transparent): blend the base
    // opacity halfway toward solid so the shell reads as a panel, not a hole.
    let sidebar_alpha = (0.5 + 0.5 * state.appearance.opacity()).clamp(0.0, 1.0);
    let sidebar_bg = side_bg_base.with_alpha((sidebar_alpha * 255.0).round() as u8);

    let pane_count = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.scrolling
                .columns
                .iter()
                .map(|c| c.panes.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.title.as_str())
        .unwrap_or("—");
    let (mode_str, rename_hint) = crate::app::render::status_mode_parts(&state.input_mode);
    let status = format!(
        "{} panes | {} | {}{}",
        pane_count, focus_title, mode_str, rename_hint
    );

    let fg = {
        let c = state.theme.foreground;
        Color::new(c.r, c.g, c.b, c.a)
    };

    // Bridge the app theme into a GuiTheme: frosted sidebar surface, app accent +
    // border (the sidebar's brackets/markers read from these), app radius.
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

    let left_w = chrome.left_sidebar_width;
    let sidebar = if left_w >= SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_h = (h - DEFAULT_TAB_BAR_HEIGHT - DEFAULT_STATUS_BAR_HEIGHT).max(0.0);
        Some(build_sidebar_shell(
            &state.sidebar_tree,
            left_w,
            sidebar_h,
            &theme,
        ))
    } else {
        None
    };

    chrome_scene(
        w,
        h,
        DEFAULT_TAB_BAR_HEIGHT,
        DEFAULT_STATUS_BAR_HEIGHT,
        &status,
        side_bg,
        fg,
        &theme,
        sidebar,
    )
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
        // The shell is [header, WorkspacesContainer]; the container hosts a dock per
        // workspace (so the tree's text is visible inside the shell).
        let shell = super::build_sidebar_shell(&tree, 280.0, 600.0, &theme);
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
