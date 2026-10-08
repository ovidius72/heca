//! Sizes the window works out for itself: bar heights, sidebar widths, font sizes, opacity.

use super::*;

impl AppState {
    /// Effective tab-bar (top bar) height: the default when shown, `0.0` when
    /// hidden (`[settings] show_top_bar` is where it starts). Chrome layout/hit-testing read this
    /// so a hidden bar reclaims its space everywhere consistently.
    pub fn tab_bar_height(&self) -> f32 {
        if self
            .chrome_state
            .is_visible(crate::chrome::RegionId::TopBar)
        {
            self.appearance.effective_top_bar_height()
        } else {
            0.0
        }
    }

    /// Effective status-bar (bottom bar) height: the default when shown, `0.0`
    /// when hidden via `[settings] show_bottom_bar`.
    pub fn status_bar_height(&self) -> f32 {
        if self
            .chrome_state
            .is_visible(crate::chrome::RegionId::BottomBar)
        {
            self.appearance.effective_bottom_bar_height()
        } else {
            0.0
        }
    }

    /// Effective left-sidebar width for layout/hit-testing: `0.0` while the region is hidden,
    /// otherwise the (resizable) width. There is no icon rail — see `docs/sidebar-provider-modes.md`.
    pub fn left_sidebar_width(&self) -> f32 {
        if self
            .chrome_state
            .is_visible(crate::chrome::RegionId::LeftSidebar)
        {
            self.chrome_state.left_size()
        } else {
            0.0
        }
    }

    /// Effective right-sidebar width for layout/hit-testing: `0.0` while the region is hidden.
    pub fn right_sidebar_width(&self) -> f32 {
        if self
            .chrome_state
            .is_visible(crate::chrome::RegionId::RightSidebar)
        {
            self.chrome_state.right_size()
        } else {
            0.0
        }
    }
}

impl AppState {
    /// The effective **terminal** global font size: the configured terminal size
    /// plus the app zoom offset, clamped. This is the base every pane starts from
    /// before its own per-pane offset is applied.
    pub fn app_font_size(&self) -> f32 {
        (self.font_config.size.terminal + self.app_font_zoom).clamp(
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MIN,
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MAX,
        )
    }

    /// The effective **chrome/UI** font size: the configured UI size plus the same
    /// app zoom offset, clamped. Scales the sidebar, tabs, and status bar together
    /// with the terminals so the app zoom is truly app-wide.
    pub fn app_ui_font_size(&self) -> f32 {
        (self.font_config.size.ui + self.app_font_zoom).clamp(
            crate::app::terminal_metrics::APP_UI_FONT_SIZE_MIN,
            crate::app::terminal_metrics::APP_UI_FONT_SIZE_MAX,
        )
    }

    /// The effective terminal font size for a specific pane: the configured size
    /// plus the global zoom offset plus that pane's per-pane offset, clamped to the
    /// supported range. Both the PTY cell fit and the rendered glyph size derive
    /// from this single value so they never disagree.
    pub fn effective_terminal_font_size(&self, pane_id: PaneId) -> f32 {
        self.terminal_font_size(Some(pane_id))
    }

    /// The terminal font size for a terminal that may or may not belong to a pane: a pane has its own
    /// zoom on top of the global one, any other terminal follows the global.
    pub fn terminal_font_size(&self, pane: Option<PaneId>) -> f32 {
        let pane_offset = pane
            .and_then(|pane| self.pane_font_zoom.get(&pane))
            .copied()
            .unwrap_or(0.0);
        (self.font_config.size.terminal + self.app_font_zoom + pane_offset).clamp(
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MIN,
            crate::app::terminal_metrics::TERMINAL_FONT_SIZE_MAX,
        )
    }

    /// The base cell size a pane's PTY grid is fitted to, honoring per-pane zoom.
    /// Falls back to the global `terminal_cell_size` for panes with no override.
    pub fn pane_base_cell_size(&self, pane_id: PaneId) -> (f32, f32) {
        self.pane_cell_override
            .get(&pane_id)
            .copied()
            .unwrap_or(self.terminal_cell_size)
    }

    /// Terminal pane surface opacity, derived from the shared appearance contract.
    ///
    /// Returns `1.0` (opaque) when `terminal_transparency = 0`, and the
    /// terminal-specific `terminal_opacity()` otherwise. This is the alpha used
    /// for the translucent pane surface fill drawn over the frosted backdrop.
    ///
    /// In the z=0 model, the tiled frost is the background layer showing through
    /// the translucent terminal surface, so this is simply the surface alpha
    /// used when compositing the terminal over z=0. See `render_frame`.
    pub fn terminal_surface_opacity(&self) -> f32 {
        if self.appearance.terminal.transparency > 0 {
            self.appearance.terminal_opacity()
        } else {
            1.0
        }
    }

    /// Floating terminal-pane surface opacity, mirroring
    /// [`terminal_surface_opacity`](Self::terminal_surface_opacity) but driven by
    /// `terminal_floating_transparency`. Default `0` -> `1.0` (opaque, readable)
    /// so floating panes stay solid while tiled panes are frosted.
    pub fn terminal_floating_surface_opacity(&self) -> f32 {
        if self.appearance.terminal.floating_transparency > 0 {
            self.appearance.terminal_floating_opacity()
        } else {
            1.0
        }
    }
}
