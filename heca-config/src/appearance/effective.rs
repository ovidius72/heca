//! **What wins for each setting**: the value in `config.toml`, else the loaded theme's, else the
//! built-in default — and the limits each is kept within.

use super::*;

impl AppearanceConfig {
    /// Whether the pane info bar shows at all (any segment or action configured).
    /// When false, the pane reserves no extra top padding for it.
    pub fn pane_info_bar_visible(&self) -> bool {
        !self.pane.title_segments.is_empty() || !self.pane.title_actions.is_empty()
    }
}

impl AppearanceConfig {
    /// Background opacity in `0.0..=1.0` (`transparency = 0` → `1.0` opaque).
    pub fn opacity(&self) -> f32 {
        opacity_of(self.transparency)
    }

    /// Whether the window/surface should be created transparent.
    pub fn is_transparent(&self) -> bool {
        self.transparency > 0
    }

    /// Chrome panel (sidebar/status/tab) background opacity. A bit more
    /// see-through than the global `opacity()` so panels read as *frosted*
    /// (vibrancy showing through) rather than a flat dark tint. `1.0` (opaque)
    /// when the window isn't transparent.
    pub fn chrome_opacity(&self) -> f32 {
        if self.is_transparent() {
            (self.opacity() * CHROME_OPACITY_SHARE).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// In-app blur radius in logical px (`0.0` = off). Scales `blur` 0..100 to
    /// `0..=MAX_BLUR_PX`. Read by the in-app blur pass (F3).
    pub fn blur_radius(&self) -> f32 {
        blur_px(self.blur)
    }

    /// Terminal pane surface opacity in `0.0..=1.0`
    /// (`[appearance.terminal] transparency = 0` → `1.0` opaque).
    pub fn terminal_opacity(&self) -> f32 {
        opacity_of(self.terminal.transparency)
    }

    /// Floating terminal-pane surface opacity in `0.0..=1.0`
    /// (`[appearance.terminal] floating_transparency = 0` → `1.0` opaque).
    /// Independent from the tiled `terminal_opacity()` so floating panes can stay readable.
    pub fn terminal_floating_opacity(&self) -> f32 {
        opacity_of(self.terminal.floating_transparency)
    }

    /// Floating terminal-pane real-blur radius in logical px (`0.0` =
    /// off). Independent from the z=0 `background_blur`.
    pub fn terminal_floating_blur_radius(&self) -> f32 {
        blur_px(self.terminal.floating_blur)
    }

    /// The OS backdrop material to apply, or `None` when disabled.
    pub fn os_vibrancy(&self) -> Option<Vibrancy> {
        (self.vibrancy != Vibrancy::None).then_some(self.vibrancy)
    }

    // ── z=0 background layer resolvers ──

    /// Effective z=0 gradient *top* color: config override →
    /// `theme.effective_background_gradient_top()` (theme field →
    /// `theme.background`).
    pub fn effective_background_gradient_top(&self, theme: &Theme) -> Color {
        self.background_gradient_top
            .unwrap_or_else(|| theme.effective_background_gradient_top())
    }

    /// Effective z=0 gradient *bottom* color: config override →
    /// `theme.effective_background_gradient_bottom()` (theme field →
    /// darkened `theme.background`).
    pub fn effective_background_gradient_bottom(&self, theme: &Theme) -> Color {
        self.background_gradient_bottom
            .unwrap_or_else(|| theme.effective_background_gradient_bottom())
    }

    /// z=0 background blur radius in logical px (`0.0` = off). Scales
    /// `background_blur` 0..100 to `0..=MAX_BLUR_PX`. The caller converts to
    /// physical px (`* scale_factor`) before passing to the GPU blur (the
    /// compositor scene texture is framebuffer-sized).
    pub fn background_blur_radius(&self) -> f32 {
        blur_px(self.background_blur)
    }

    /// z=0 background opacity in `0.0..=1.0` (`background_transparency = 0` →
    /// `1.0` opaque; `100` → `0.0` fully transparent). The z=0 blit into the
    /// scene uses this alpha.
    pub fn background_alpha(&self) -> f32 {
        opacity_of(self.background_transparency)
    }

    // ── Effect token resolvers ──
    // Config.toml `[appearance]` overrides take precedence; `None` inherits
    // from the theme automatically.

    /// Effective picker letter colour: config override → `theme.hint_color` → the accent. One
    /// colour for every keycap in the app; a host may still tint one *kind* of target apart with
    /// the per-widget `hint_color` builder.
    pub fn effective_hint_color(&self, theme: &Theme) -> Color {
        self.hint_color
            .unwrap_or_else(|| theme.effective_hint_color())
    }

    /// Effective glow halo level: config override → `theme.glow_size`. Owns
    /// glow presence + radius + strength.
    pub fn effective_glow_size(&self, theme: &Theme) -> GlowLevel {
        self.glow_size.unwrap_or(theme.glow_size)
    }

    /// Effective scanline/CRT overlay intensity: config override →
    /// `theme.intensity`. Drives scanline-overlay opacity only (not glow).
    pub fn effective_intensity(&self, theme: &Theme) -> Intensity {
        self.intensity.unwrap_or(theme.intensity)
    }

    /// Effective keyboard focus-outline visibility: config override →
    /// `theme.show_focus_border`. `false` removes the focus ring everywhere.
    pub fn effective_show_focus_border(&self, theme: &Theme) -> bool {
        self.show_focus_border.unwrap_or(theme.show_focus_border)
    }

    /// Effective **overlay panel** frame style: config override →
    /// `theme.overlay_frame` (themes default `bracketed`). Governs the shared
    /// frame the overlay panel chrome draws around dialogs / dropdowns / context
    /// menus / the command palette — not each widget's own accent border or glow.
    pub fn effective_overlay_border_style(&self, theme: &Theme) -> FrameStyle {
        self.overlay_border_style
            .map(FrameStyle::from)
            .unwrap_or(theme.overlay_frame)
    }

    // ── Pane chrome resolvers ──
    // Config.toml `[appearance]` overrides take precedence; `None` inherits
    // from the theme automatically.

    /// Effective **global** decorative border width: config `border_width`
    /// override → theme `border_width`, clamped to `[0, 10]`. Used by the chrome /
    /// sidebar and as the fallback for panes. The app-wide equivalent of the
    /// showcase BORDER control.
    pub fn effective_border_width(&self, theme: &Theme) -> f32 {
        self.border_width
            .unwrap_or(theme.border_width)
            .clamp(0.0, MAX_BORDER_WIDTH)
    }

    /// Effective **global** decorative border color: config `border_color`
    /// override → theme `border`. Used by the chrome / sidebar `bordered` frame.
    pub fn effective_border_color(&self, theme: &Theme) -> Color {
        self.border_color.unwrap_or(theme.border)
    }

    /// Effective **global** corner radius: config `border_radius` override → theme
    /// `border_radius`. The fallback surfaces inherit when their own radius is unset.
    pub fn effective_border_radius(&self, theme: &Theme) -> f32 {
        self.border_radius.unwrap_or(theme.border_radius)
    }

    /// Effective pane border width: `[appearance.pane] border_width` → global
    /// `border_width` → theme, clamped to `[0, 10]`.
    pub fn effective_pane_border_width(&self, theme: &Theme) -> f32 {
        self.pane
            .border_width
            .unwrap_or_else(|| self.effective_border_width(theme))
            .clamp(0.0, MAX_BORDER_WIDTH)
    }

    /// Effective inactive pane border color: `[appearance.pane] border_color` →
    /// global `border_color` → theme `border` at 50% alpha.
    pub fn effective_pane_border_color(&self, theme: &Theme) -> Color {
        self.pane
            .border_color
            .or(self.border_color)
            .unwrap_or_else(|| theme.border.with_alpha(128))
    }

    /// Effective pane corner radius: `[appearance.pane] border_radius` → global
    /// `border_radius` → theme, clamped to `[0, 20]`. Higher radii make the rounded
    /// content-clip (stencil) eat into terminal content at the corners; capping
    /// keeps the clip gentle so cells/text aren't cut off.
    pub fn effective_pane_border_radius(&self, theme: &Theme) -> f32 {
        self.pane
            .border_radius
            .unwrap_or_else(|| self.effective_border_radius(theme))
            .clamp(0.0, MAX_PANE_RADIUS)
    }

    /// Effective active pane border color. Config override → theme `accent`.
    pub fn effective_pane_active_border_color(&self, theme: &Theme) -> Color {
        self.pane.active_border_color.unwrap_or(theme.accent)
    }

    /// Effective floating pane border color (applies to all floating panes,
    /// active or inactive, so they read as a distinct layer). Config override →
    /// `theme.float_accent`.
    pub fn effective_pane_floating_border_color(&self, theme: &Theme) -> Color {
        self.pane
            .floating_border_color
            .unwrap_or(theme.float_accent)
    }

    /// Effective gap between tiled panes (logical px). Config override → [`DEFAULT_PANE_GAP`].
    ///
    /// **Border-overlap floor:** when the pane border is visible (style ≠ `none`
    /// and width > 0), neighbouring panes each draw a full border on their shared
    /// edge, so a gap smaller than the border width makes the two borders overlap
    /// (and bleed onto each other). The gap is therefore floored at the effective
    /// pane border width whenever borders are on — a configured `gap = 0` becomes
    /// exactly one border width, yielding a clean single divider instead of an
    /// overlap. With `border_style = "none"` the configured gap is used as-is
    /// (`gap = 0` gives truly flush panes).
    pub fn effective_pane_gap(&self, theme: &Theme) -> f32 {
        let gap = self.pane.gap.unwrap_or(DEFAULT_PANE_GAP);
        if self.effective_pane_border_style() != BorderStyle::None {
            let border = self.effective_pane_border_width(theme);
            if border > 0.0 {
                return gap.max(border);
            }
        }
        gap
    }

    /// Effective pane internal padding (content inset from the pane border).
    /// Config override → `theme.pane_padding` (4.0 for mocha), clamped to
    /// `[0, 20]`. Snug by default now that the rounded content-clip (stencil)
    /// handles corners — a small straight-edge gap no longer overflows.
    pub fn effective_pane_padding(&self, theme: &Theme) -> f32 {
        self.pane
            .padding
            .unwrap_or(theme.pane_padding)
            .clamp(0.0, MAX_PANE_PADDING)
    }

    /// Effective sidebar gap. Config override → [`DEFAULT_SIDEBAR_GAP`].
    pub fn effective_sidebar_gap(&self, _theme: &Theme) -> f32 {
        self.sidebar.gap.unwrap_or(DEFAULT_SIDEBAR_GAP)
    }

    /// Effective sidebar width (logical px), clamped to
    /// `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`. Config override → 300.
    pub fn effective_sidebar_width(&self) -> f32 {
        self.sidebar
            .width
            .unwrap_or(DEFAULT_SIDEBAR_WIDTH)
            .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
    }

    /// Effective top-bar height (logical px). Config override → 32, clamped `[16, 96]`.
    pub fn effective_top_bar_height(&self) -> f32 {
        self.top_bar_height
            .unwrap_or(DEFAULT_TOP_BAR_HEIGHT)
            .clamp(MIN_BAR_HEIGHT, MAX_BAR_HEIGHT)
    }

    /// Effective bottom-bar height (logical px). Config override → 24, clamped `[16, 96]`.
    pub fn effective_bottom_bar_height(&self) -> f32 {
        self.bottom_bar_height
            .unwrap_or(DEFAULT_BOTTOM_BAR_HEIGHT)
            .clamp(MIN_BAR_HEIGHT, MAX_BAR_HEIGHT)
    }

    /// The size (logical px) of the name drawn over a dragged pane of `pane_min_side` px across:
    /// the configured share of it, kept within the configured `[min, max]`.
    pub fn effective_drag_label_size(&self, pane_min_side: f32) -> f32 {
        let min = self
            .pane
            .drag_label_min_size
            .unwrap_or(DEFAULT_DRAG_LABEL_MIN_SIZE);
        let max = self
            .pane
            .drag_label_max_size
            .unwrap_or(DEFAULT_DRAG_LABEL_MAX_SIZE)
            .max(min);
        let factor = self
            .pane
            .drag_label_size_factor
            .unwrap_or(DEFAULT_DRAG_LABEL_SIZE_FACTOR);
        (pane_min_side * factor).clamp(min, max)
    }

    // ── Border style / affordance resolvers ──

    /// Effective terminal-pane frame style. Config override → [`BorderStyle::Bordered`]
    /// (the current default look).
    pub fn effective_pane_border_style(&self) -> BorderStyle {
        self.pane.border_style.unwrap_or(BorderStyle::Bordered)
    }

    /// Effective sidebar-shell frame style. Config override → [`BorderStyle::Bracketed`]
    /// (the current default look).
    pub fn effective_sidebar_border_style(&self) -> BorderStyle {
        self.sidebar.border_style.unwrap_or(BorderStyle::Bracketed)
    }

    /// Effective sidebar-shell border width: `[appearance.sidebar] border_width` →
    /// global `border_width` → theme, clamped to `[0, 10]`.
    pub fn effective_sidebar_border_width(&self, theme: &Theme) -> f32 {
        self.sidebar
            .border_width
            .unwrap_or_else(|| self.effective_border_width(theme))
            .clamp(0.0, MAX_BORDER_WIDTH)
    }

    /// Effective sidebar-shell background color: `[appearance.sidebar]
    /// background_color` → the theme-derived sidebar surface color passed in.
    pub fn effective_sidebar_background_color(&self, theme_surface: Color) -> Color {
        self.sidebar.background_color.unwrap_or(theme_surface)
    }

    /// Effective sidebar-shell corner radius: `[appearance.sidebar] border_radius`
    /// → global `border_radius` → theme, clamped to `[0, 40]`.
    pub fn effective_sidebar_border_radius(&self, theme: &Theme) -> f32 {
        self.sidebar
            .border_radius
            .unwrap_or_else(|| self.effective_border_radius(theme))
            .clamp(0.0, MAX_SIDEBAR_RADIUS)
    }

    /// Effective affordance-outline width (focus ring + selection highlight).
    /// Config override → `1.5`, clamped to `[0, 10]`. Independent of the
    /// decorative border width so focus/selection stay visible at `border_width = 0`.
    pub fn effective_focus_border_width(&self) -> f32 {
        self.focus_border_width
            .unwrap_or(DEFAULT_FOCUS_BORDER_WIDTH)
            .clamp(0.0, MAX_BORDER_WIDTH)
    }
}

/// A 0–100 transparency as an opacity share: `0` is solid, `100` fully clear.
fn opacity_of(transparency: u8) -> f32 {
    1.0 - (transparency.min(100) as f32) / 100.0
}

/// A 0–100 blur amount as a radius in px, up to [`MAX_BLUR_PX`].
fn blur_px(amount: u8) -> f32 {
    (amount.min(100) as f32) / 100.0 * MAX_BLUR_PX
}
