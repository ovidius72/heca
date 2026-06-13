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

use heca_grid_ui::{Color, Component, LayoutEngine, PaintCx, Scene};
use heca_grid_ui::builders::{LayoutExt, Parent, StyleExt};
use heca_grid_ui::style::{Align, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Flex, Label, Surface};

/// Build a chrome [`Scene`] containing just the status bar, positioned at its real
/// screen rect. Pure: takes plain values so it can be unit-tested without wgpu/AppState.
fn status_bar_scene(
    w: f32,
    h: f32,
    status_bar_height: f32,
    status: &str,
    side_bg: Color,
    fg: Color,
) -> Scene {
    let mut theme = GuiTheme::grid_tron();
    theme.foreground = fg;
    theme.surface = side_bg;
    theme.border = side_bg;

    let mut root = Flex::column()
        .width(Length::Px(w))
        .height(Length::Px(h))
        .child(
            // Transparent spacer that pushes the status row to the bottom.
            Flex::column()
                .width(Length::Px(w))
                .height(Length::Px(h - status_bar_height)),
        )
        .child(
            Surface::row()
                .width(Length::Px(w))
                .height(Length::Px(status_bar_height))
                .background(side_bg)
                .align(Align::Center)
                .padding_xy(8.0, 0.0)
                .child(Label::new(status).font_size(CHROME_TEXT_SIZE).color(fg)),
        );

    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut root, Size::new(w as f64, h as f64));
    {
        let mut cx = PaintCx::new(&mut scene, &theme)
            .with_viewport(Size::new(w as f64, h as f64));
        root.paint(&mut cx);
    }
    scene
}

/// Read app state and produce a chrome [`Scene`] containing the status bar.
/// Pure projection: reads `state`, returns a `Scene`, mutates nothing.
pub(crate) fn build_chrome_scene(state: &crate::app_state::AppState) -> Scene {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys.width as f32 / scale;
    let h = phys.height as f32 / scale;

    // Mirror the special-case from render.rs exactly.
    let side_bg = if state.theme.name == "Catppuccin Mocha" {
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

    status_bar_scene(w, h, DEFAULT_STATUS_BAR_HEIGHT, &status, side_bg, fg)
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
    fn status_bar_scene_emits_text() {
        use heca_grid_ui::{Color, DrawCommand};
        let scene = super::status_bar_scene(
            800.0,
            600.0,
            24.0,
            "2 panes | foo | NORMAL",
            Color::new(17, 17, 27, 255),
            Color::new(200, 200, 200, 255),
        );
        assert!(!scene.is_empty(), "scene should not be empty");
        assert!(
            scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(..))),
            "scene should contain at least one Text draw command",
        );
    }
}
