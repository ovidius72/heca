use crate::color::Color;
use crate::theme::{GlowLevel, Intensity, Theme};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════════
//  Vibrancy
// ═══════════════════════════════════════════════════════════════════════════════

/// OS backdrop-blur material (the *desktop-behind-the-window* frost).
///
/// This is **not portable and not numeric**: macOS maps each variant to an
/// `NSVisualEffectMaterial`; Windows uses Acrylic/Mica (best-effort); Linux is a
/// no-op. The window/surface layer applies it once after window creation when it
/// is not [`Vibrancy::None`]. For a *portable, numeric* blur, use the in-app blur
/// amount instead (`AppearanceConfig::blur`). Serialised `snake_case` in TOML
/// (e.g. `vibrancy = "hud_window"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Vibrancy {
    /// No OS backdrop material (default) — disables vibrancy entirely.
    #[default]
    None,
    /// `NSVisualEffectMaterialSidebar` — sidebar/panel tint.
    Sidebar,
    /// `NSVisualEffectMaterialHUDWindow` — dark floating HUD panel.
    HudWindow,
    /// `NSVisualEffectMaterialUnderWindowBackground` — blurred desktop behind the window.
    UnderWindowBackground,
    /// `NSVisualEffectMaterialPopover` — popover / tooltip material.
    Popover,
    /// `NSVisualEffectMaterialMenu` — menu bar / dropdown material.
    Menu,
    /// `NSVisualEffectMaterialFullScreenUI` — full-screen overlay material.
    FullScreenUi,
    /// `NSVisualEffectMaterialWindowBackground` — generic window background.
    WindowBackground,
}

/// A segment shown in the pane info bar (left side), in config order. Serialised
/// `snake_case` in TOML (e.g. `pane_title_segments = ["location", "app_name"]`).
/// A segment with no data for a pane (e.g. git outside a repo) is skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneSegment {
    /// Working directory (home-relative path).
    Location,
    /// Resolved program/app name (process catalog).
    AppName,
    /// Git branch (hidden outside a repo).
    GitBranch,
    /// Git change counts `+A ~M -D` (hidden when clean / outside a repo).
    GitStatus,
}

/// An action button shown in the pane info bar (right side), in config order.
/// Serialised `snake_case` (e.g. `pane_title_actions = ["split", "close"]`). Each
/// maps to an existing window-manager action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneAction {
    /// Split the pane.
    Split,
    /// Move the pane left.
    MoveLeft,
    /// Move the pane right.
    MoveRight,
    /// Close the pane.
    Close,
    /// Toggle zoom (maximise) for the pane's column.
    Zoom,
    /// Toggle floating for the pane.
    Float,
}

/// Frame decoration style for a container surface (panes, sidebar). Maps onto the
/// grid-ui `PaneFrame` in the app layer. Serialised `snake_case` in TOML (e.g.
/// `pane_border_style = "bracketed"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BorderStyle {
    /// No border — background fill only.
    None,
    /// A clean continuous border.
    Bordered,
    /// The accent corner-bracket reticle (bright rounded corners + a dimmed line).
    Bracketed,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Default-value helpers (used by serde attributes on AppearanceConfig)
// ═══════════════════════════════════════════════════════════════════════════════

fn default_transparency() -> u8 {
    0
}

fn default_blur() -> u8 {
    0
}

fn default_terminal_transparency() -> u8 {
    0
}

fn default_terminal_floating_transparency() -> u8 {
    0
}

fn default_terminal_floating_blur() -> u8 {
    0
}
fn default_terminal_show_scrollbar() -> ScrollbarVisibility {
    ScrollbarVisibility::WhenNeeded
}

fn default_terminal_show_scrolled_up_badge() -> bool {
    true
}

/// Default terminal ligatures: `true` (coding-font ligatures like `->`/`=>`/`!=`
/// render). Set `false` to disable `calt`/`liga`/`clig` for the terminal font.
fn default_terminal_ligatures() -> bool {
    true
}

/// Default hyperlink decoration: a straight underline (plus the link color).
fn default_terminal_hyperlink_style() -> HyperlinkStyle {
    HyperlinkStyle::Underline
}

fn default_vibrancy() -> Vibrancy {
    Vibrancy::None
}

/// Default z=0 background blur amount: `0` (off — gradient drawn un-blurred).
fn default_background_blur() -> u8 {
    0
}

/// Default z=0 background transparency: `0` (opaque z=0 — clean cross-platform
/// frost out of the box; the user opts into translucency).
fn default_background_transparency() -> u8 {
    0
}

fn default_pane_title_segments() -> Vec<PaneSegment> {
    vec![PaneSegment::Location, PaneSegment::AppName]
}

fn default_pane_title_actions() -> Vec<PaneAction> {
    // Move-left/right are intentionally omitted from the default bar — panes are
    // already movable with the mouse (drag). `MoveLeft`/`MoveRight` remain valid
    // config values for users who want them. Default = split + close.
    vec![PaneAction::Split, PaneAction::Close]
}

/// Maximum in-app blur radius in logical px, at `blur = 100`.
const MAX_BLUR_PX: f32 = 48.0;

/// Sidebar width bounds (logical px); the configured `sidebar_width` is clamped to
/// this range. Default when unset.
pub const MIN_SIDEBAR_WIDTH: f32 = 160.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 560.0;
const DEFAULT_SIDEBAR_WIDTH: f32 = 300.0;

/// Default width of affordance outlines (focus ring + selection) when
/// `focus_border_width` is unset — kept visible regardless of the global border.
const DEFAULT_FOCUS_BORDER_WIDTH: f32 = 1.5;

// ═══════════════════════════════════════════════════════════════════════════════
//  Per-surface appearance (nested `[appearance.terminal/pane/sidebar]` tables)
// ═══════════════════════════════════════════════════════════════════════════════

/// How OSC 8 hyperlinks are decorated in terminal panes. The link **color** is a
/// separate knob (`hyperlink_color`, default `theme.accent`); this picks the
/// decoration on top of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HyperlinkStyle {
    /// No special rendering — links look like normal text.
    None,
    /// Recolor only (no line).
    Color,
    /// Recolor + straight underline.
    #[default]
    Underline,
    /// Recolor + wavy undercurl.
    Undercurl,
}

/// When the terminal scrollback scrollbar should be shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScrollbarVisibility {
    /// Always show the scrollbar when the pane has scrollable history.
    Always,
    /// Show only while the viewport is scrolled away from the live bottom.
    #[default]
    WhenNeeded,
    /// Never show the scrollbar.
    Never,
}

/// Terminal **content** surface translucency/blur (`[appearance.terminal]`). The
/// pane *frame* (border/radius/gap) lives in [`PaneAppearance`] — this owns only
/// the see-through-ness of the terminal surface itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TerminalAppearance {
    /// Tiled terminal-pane transparency, `0..=100` (`0` opaque, `100`
    /// see-through). Surface alpha composited over the z=0 background.
    #[serde(default = "default_terminal_transparency")]
    pub transparency: u8,
    /// Floating terminal-pane transparency, `0..=100` (`0` opaque). Independent of
    /// the tiled `transparency`, so floating panes can stay readable while tiled
    /// panes are frosted. Default `0` (opaque).
    #[serde(default = "default_terminal_floating_transparency")]
    pub floating_transparency: u8,
    /// Floating terminal-pane real-blur strength, `0..=100` (`0` = off).
    /// Independent of the z=0 `background_blur`. Default `0` (no blur).
    #[serde(default = "default_terminal_floating_blur")]
    pub floating_blur: u8,
    /// Scrollbar visibility for panes with host-managed scrollback.
    /// `always | when_needed | never`. Default `when_needed`.
    #[serde(default = "default_terminal_show_scrollbar")]
    pub show_scrollbar: ScrollbarVisibility,
    /// Whether to show the "N lines above" badge while scrolled up.
    /// Default `true`.
    #[serde(default = "default_terminal_show_scrolled_up_badge")]
    pub show_scrolled_up_badge: bool,
    /// Coding-font ligatures (`->`, `=>`, `!=`, …) in terminal panes. When
    /// `false`, `calt`/`liga`/`clig` are disabled in the terminal shaping path so
    /// each character renders standalone. Default `true`. Terminal font only —
    /// UI/chrome text is unaffected.
    #[serde(default = "default_terminal_ligatures")]
    pub ligatures: bool,
    /// Decoration for OSC 8 hyperlinks (`none | color | underline | undercurl`).
    /// Default `underline`. Pairs with `hyperlink_color`.
    #[serde(default = "default_terminal_hyperlink_style")]
    pub hyperlink_style: HyperlinkStyle,
    /// Hyperlink color. `None` → `theme.accent`. Applied for every style except
    /// `none`.
    #[serde(default)]
    pub hyperlink_color: Option<Color>,
}

impl Default for TerminalAppearance {
    fn default() -> Self {
        Self {
            transparency: default_terminal_transparency(),
            floating_transparency: default_terminal_floating_transparency(),
            floating_blur: default_terminal_floating_blur(),
            show_scrollbar: default_terminal_show_scrollbar(),
            show_scrolled_up_badge: default_terminal_show_scrolled_up_badge(),
            ligatures: default_terminal_ligatures(),
            hyperlink_style: default_terminal_hyperlink_style(),
            hyperlink_color: None,
        }
    }
}

/// Pane frame + info-bar appearance (`[appearance.pane]`). Every border field is
/// optional; when unset it inherits the global `[appearance]` default, which in
/// turn falls back to the theme. (Surface → global → theme.)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PaneAppearance {
    /// Frame style. `None` → [`BorderStyle::Bordered`]. `none | bordered | bracketed`.
    #[serde(default)]
    pub border_style: Option<BorderStyle>,
    /// Border width (logical px). `None` → global `border_width` → theme. Clamped `[0, 10]`.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// Inactive/unfocused border color. `None` → global `border_color` → `theme.border` @50%.
    #[serde(default)]
    pub border_color: Option<Color>,
    /// Corner radius (logical px). `None` → global `border_radius` → theme. Clamped `[0, 20]`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Focused/active border color. `None` → `theme.accent`.
    #[serde(default)]
    pub active_border_color: Option<Color>,
    /// Floating-pane border color (all floating panes, so they read as a distinct
    /// layer). `None` → `theme.float_accent`.
    #[serde(default)]
    pub floating_border_color: Option<Color>,
    /// Gap between tiled panes (logical px). `None` → 8.0.
    #[serde(default)]
    pub gap: Option<f32>,
    /// Internal padding (content inset from the border, logical px). `None` → theme
    /// `pane_padding`. Clamped `[0, 20]`.
    #[serde(default)]
    pub padding: Option<f32>,
    /// Info-bar segments (left), in order. Empty hides the left side. See [`PaneSegment`].
    #[serde(default = "default_pane_title_segments")]
    pub title_segments: Vec<PaneSegment>,
    /// Info-bar action buttons (right), in order. Empty hides the right side. See [`PaneAction`].
    #[serde(default = "default_pane_title_actions")]
    pub title_actions: Vec<PaneAction>,
}

impl Default for PaneAppearance {
    fn default() -> Self {
        Self {
            border_style: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            active_border_color: None,
            floating_border_color: None,
            gap: None,
            padding: None,
            title_segments: default_pane_title_segments(),
            title_actions: default_pane_title_actions(),
        }
    }
}

/// Sidebar shell appearance (`[appearance.sidebar]`). Border fields unset →
/// global `[appearance]` default → theme. (Surface → global → theme.)
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SidebarAppearance {
    /// Frame style. `None` → [`BorderStyle::Bracketed`]. `none | bordered | bracketed`.
    #[serde(default)]
    pub border_style: Option<BorderStyle>,
    /// Border width (logical px). `None` → global `border_width` → theme. Clamped `[0, 10]`.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// Border color (the `bordered` frame). `None` → global `border_color` → `theme.border`.
    #[serde(default)]
    pub border_color: Option<Color>,
    /// Corner radius (logical px). `None` → global `border_radius` → theme. Clamped `[0, 40]`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Shell background fill. `None` → the theme-derived sidebar surface color. The
    /// window/chrome transparency still applies on top.
    #[serde(default)]
    pub background_color: Option<Color>,
    /// Gap between the sidebar and the content area (logical px). `None` → 12.0.
    #[serde(default)]
    pub gap: Option<f32>,
    /// Sidebar width (logical px). `None` → 300; clamped to `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`.
    #[serde(default)]
    pub width: Option<f32>,
}

// ═══════════════════════════════════════════════════════════════════════════════
//  AppearanceConfig
// ═══════════════════════════════════════════════════════════════════════════════

/// Read-only appearance contract shared by all rendering layers — none owns it.
///
/// Layout: app-wide knobs live at the top (`[appearance]`); per-surface chrome
/// lives in three nested sub-tables — [`TerminalAppearance`] (`[appearance.terminal]`),
/// [`PaneAppearance`] (`[appearance.pane]`), [`SidebarAppearance`]
/// (`[appearance.sidebar]`).
///
/// App-wide controls:
/// - `transparency` — how see-through the app is (`0` opaque, `100` fully
///   transparent). Portable (window/surface alpha).
/// - `blur` — the **in-app** frosted-glass blur behind translucent panels
///   (palette/sidebar). Portable (our own GPU pass, F3). `0` = off.
/// - `vibrancy` — the **OS backdrop** material (blurs the desktop *behind* the
///   window). Not numeric, not portable. [`Vibrancy::None`] = off.
/// - `background_*` — the z=0 frosted gradient layer panes composite over.
/// - `glow_size` / `intensity` — effect tokens (override the theme).
/// - `border_width` / `border_color` / `border_radius` — global border defaults
///   every surface inherits; `focus_border_width` — affordance-outline width.
///
/// Inheritance for surface border fields: surface value → global `[appearance]`
/// default → theme. Missing `[appearance]` (and any sub-table) falls back to
/// these defaults (everything off → identical to an opaque app).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppearanceConfig {
    /// Window/app transparency amount, `0..=100` (`0` opaque, `100` see-through).
    #[serde(default = "default_transparency")]
    pub transparency: u8,

    /// In-app frosted blur amount, `0..=100` (`0` = off). Portable GPU pass (F3);
    /// distinct from the OS `vibrancy` backdrop.
    #[serde(default = "default_blur")]
    pub blur: u8,

    /// OS backdrop material ([`Vibrancy::None`] = off). Platform-dependent.
    #[serde(default = "default_vibrancy")]
    pub vibrancy: Vibrancy,

    // ── z=0 background layer (compositor-blur refactor) ──
    /// z=0 background gradient *top* color override. `None` → inherits
    /// `theme.background_gradient_top` (which itself falls back to
    /// `theme.background`). Set in config.toml to override the theme.
    #[serde(default)]
    pub background_gradient_top: Option<Color>,
    /// z=0 background gradient *bottom* color override. `None` → inherits
    /// `theme.background_gradient_bottom` (which falls back to a slightly
    /// darkened `theme.background`).
    #[serde(default)]
    pub background_gradient_bottom: Option<Color>,
    /// z=0 background **blur** amount, `0..=100` (`0` = off, gradient drawn
    /// un-blurred). Portable heca-owned wgpu blur (independent of OS
    /// `vibrancy`). Drives `background_blur_radius()`.
    #[serde(default = "default_background_blur")]
    pub background_blur: u8,
    /// z=0 background **transparency**, `0..=100` (`0` = opaque z=0 — the
    /// default, for clean cross-platform frost; `100` = fully see-through,
    /// showing the desktop behind the gradient). Drives `background_alpha()`.
    #[serde(default = "default_background_transparency")]
    pub background_transparency: u8,

    // ── Effect tokens (glow + scanlines) ──
    // Both override the theme's effect token when set; `None` inherits from
    // the theme. They are independent dimensions: `glow_size` owns glow
    // presence + radius + strength; `intensity` owns scanline/CRT overlay
    // opacity only (it does NOT affect glow).
    /// Glow halo level override. `None` → inherits `theme.glow_size`. Values:
    /// `none | thin | medium | large`. Drives glow presence, halo radius, and
    /// strength.
    #[serde(default)]
    pub glow_size: Option<GlowLevel>,
    /// Scanline/CRT overlay intensity override. `None` → inherits
    /// `theme.intensity`. Values: `off | low | medium | heavy`. Drives
    /// scanline-overlay opacity only; does **not** affect glow.
    #[serde(default)]
    pub intensity: Option<Intensity>,

    // ── Global border defaults (every surface inherits these when its own field
    //    is unset; each in turn falls back to the theme). ──
    /// **Global** decorative border width (logical px). The app-wide BORDER
    /// control: the chrome and every surface (`pane`/`sidebar`) inherit it when
    /// their own `border_width` is unset. `None` → the theme value.
    #[serde(default)]
    pub border_width: Option<f32>,
    /// **Global** decorative border color — the `bordered` frame color inherited by
    /// the chrome and surfaces when their own `border_color` is unset. `None` → the
    /// theme's (subtle) border color. (The `bracketed` reticle uses the theme accent.)
    #[serde(default)]
    pub border_color: Option<Color>,
    /// **Global** corner radius (logical px) inherited by surfaces when their own
    /// `border_radius` is unset. `None` → the theme `border_radius`.
    #[serde(default)]
    pub border_radius: Option<f32>,
    /// Width (logical px) of the **affordance** outlines — the keyboard focus ring
    /// and the selected-item highlight. Independent of the decorative border width,
    /// so focus/selection stay visible even with borders off. `None` → `1.5`.
    #[serde(default)]
    pub focus_border_width: Option<f32>,

    // ── Per-surface sub-tables ──
    /// Terminal **content** translucency/blur (`[appearance.terminal]`).
    #[serde(default)]
    pub terminal: TerminalAppearance,
    /// Pane frame + info-bar appearance (`[appearance.pane]`).
    #[serde(default)]
    pub pane: PaneAppearance,
    /// Sidebar shell appearance (`[appearance.sidebar]`).
    #[serde(default)]
    pub sidebar: SidebarAppearance,
}

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
        1.0 - (self.transparency.min(100) as f32) / 100.0
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
            (self.opacity() * 0.7).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// In-app blur radius in logical px (`0.0` = off). Scales `blur` 0..100 to
    /// `0..=MAX_BLUR_PX`. Read by the in-app blur pass (F3).
    pub fn blur_radius(&self) -> f32 {
        (self.blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
    }

    /// Terminal pane surface opacity in `0.0..=1.0`
    /// (`[appearance.terminal] transparency = 0` → `1.0` opaque).
    pub fn terminal_opacity(&self) -> f32 {
        1.0 - (self.terminal.transparency.min(100) as f32) / 100.0
    }

    /// Floating terminal-pane surface opacity in `0.0..=1.0`
    /// (`[appearance.terminal] floating_transparency = 0` → `1.0` opaque).
    /// Independent from the tiled `terminal_opacity()` so floating panes can stay readable.
    pub fn terminal_floating_opacity(&self) -> f32 {
        1.0 - (self.terminal.floating_transparency.min(100) as f32) / 100.0
    }

    /// Floating terminal-pane real-blur radius in logical px (`0.0` =
    /// off). Independent from the z=0 `background_blur`.
    pub fn terminal_floating_blur_radius(&self) -> f32 {
        (self.terminal.floating_blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
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
        (self.background_blur.min(100) as f32) / 100.0 * MAX_BLUR_PX
    }

    /// z=0 background opacity in `0.0..=1.0` (`background_transparency = 0` →
    /// `1.0` opaque; `100` → `0.0` fully transparent). The z=0 blit into the
    /// scene uses this alpha.
    pub fn background_alpha(&self) -> f32 {
        (1.0 - (self.background_transparency.min(100) as f32) / 100.0).clamp(0.0, 1.0)
    }

    // ── Effect token resolvers ──
    // Config.toml `[appearance]` overrides take precedence; `None` inherits
    // from the theme automatically.

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

    // ── Pane chrome resolvers ──
    // Config.toml `[appearance]` overrides take precedence; `None` inherits
    // from the theme automatically.

    /// Effective **global** decorative border width: config `border_width`
    /// override → theme `border_width`, clamped to `[0, 10]`. Used by the chrome /
    /// sidebar and as the fallback for panes. The app-wide equivalent of the
    /// showcase BORDER control.
    pub fn effective_border_width(&self, theme: &Theme) -> f32 {
        self.border_width.unwrap_or(theme.border_width).clamp(0.0, 10.0)
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
            .clamp(0.0, 10.0)
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
            .clamp(0.0, 20.0)
    }

    /// Effective active pane border color. Config override → theme `accent`.
    pub fn effective_pane_active_border_color(&self, theme: &Theme) -> Color {
        self.pane.active_border_color.unwrap_or(theme.accent)
    }

    /// Effective floating pane border color (applies to all floating panes,
    /// active or inactive, so they read as a distinct layer). Config override →
    /// `theme.float_accent`.
    pub fn effective_pane_floating_border_color(&self, theme: &Theme) -> Color {
        self.pane.floating_border_color.unwrap_or(theme.float_accent)
    }

    /// Effective gap between tiled panes (logical px). Config override → 8.0.
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
        let gap = self.pane.gap.unwrap_or(8.0);
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
        self.pane.padding.unwrap_or(theme.pane_padding).clamp(0.0, 20.0)
    }

    /// Effective sidebar gap. Config override → 12.0.
    pub fn effective_sidebar_gap(&self, _theme: &Theme) -> f32 {
        self.sidebar.gap.unwrap_or(12.0)
    }

    /// Effective sidebar width (logical px), clamped to
    /// `[MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH]`. Config override → 300.
    pub fn effective_sidebar_width(&self) -> f32 {
        self.sidebar
            .width
            .unwrap_or(DEFAULT_SIDEBAR_WIDTH)
            .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH)
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
            .clamp(0.0, 10.0)
    }

    /// Effective sidebar-shell border color: `[appearance.sidebar] border_color` →
    /// global `border_color` → theme `border`. Used by the `bordered` sidebar frame.
    pub fn effective_sidebar_border_color(&self, theme: &Theme) -> Color {
        self.sidebar
            .border_color
            .or(self.border_color)
            .unwrap_or(theme.border)
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
            .clamp(0.0, 40.0)
    }

    /// Effective affordance-outline width (focus ring + selection highlight).
    /// Config override → `1.5`, clamped to `[0, 10]`. Independent of the
    /// decorative border width so focus/selection stay visible at `border_width = 0`.
    pub fn effective_focus_border_width(&self) -> f32 {
        self.focus_border_width
            .unwrap_or(DEFAULT_FOCUS_BORDER_WIDTH)
            .clamp(0.0, 10.0)
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            transparency: default_transparency(),
            blur: default_blur(),
            vibrancy: default_vibrancy(),
            background_gradient_top: None,
            background_gradient_bottom: None,
            background_blur: default_background_blur(),
            background_transparency: default_background_transparency(),
            glow_size: None,
            intensity: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            focus_border_width: None,
            terminal: TerminalAppearance::default(),
            pane: PaneAppearance::default(),
            sidebar: SidebarAppearance::default(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_all_off() {
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.transparency, 0);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.terminal.transparency, 0);
        assert_eq!(cfg.terminal.floating_transparency, 0);
        assert_eq!(cfg.terminal.floating_blur, 0);
        assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::WhenNeeded);
        assert!(cfg.terminal.show_scrolled_up_badge);
        assert_eq!(cfg.vibrancy, Vibrancy::None);
        assert_eq!(cfg.background_blur, 0);
        assert_eq!(cfg.background_transparency, 0);
        assert_eq!(cfg.background_gradient_top, None);
        assert_eq!(cfg.background_gradient_bottom, None);
        assert_eq!(cfg.glow_size, None);
        assert_eq!(cfg.intensity, None);
        assert!(!cfg.is_transparent());
        assert!((cfg.opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.blur_radius()).abs() < f32::EPSILON);
        assert!((cfg.terminal_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);
        // z=0 defaults to opaque (background_transparency = 0).
        assert!((cfg.background_alpha() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.background_blur_radius()).abs() < f32::EPSILON);
        assert_eq!(cfg.os_vibrancy(), None);
    }

    #[test]
    fn pane_info_bar_defaults_to_location_and_app_with_split_close() {
        let cfg = AppearanceConfig::default();
        assert_eq!(
            cfg.pane.title_segments,
            vec![PaneSegment::Location, PaneSegment::AppName]
        );
        // Move-left/right are omitted by default (mouse drag already moves panes).
        assert_eq!(
            cfg.pane.title_actions,
            vec![PaneAction::Split, PaneAction::Close]
        );
        assert!(cfg.pane_info_bar_visible());
    }

    #[test]
    fn pane_info_segments_and_actions_parse_snake_case() {
        let cfg: AppearanceConfig = toml::from_str(
            "[pane]\ntitle_segments = [\"location\", \"app_name\", \"git_branch\", \"git_status\"]\ntitle_actions = [\"split\", \"close\"]",
        )
        .unwrap();
        assert_eq!(
            cfg.pane.title_segments,
            vec![
                PaneSegment::Location,
                PaneSegment::AppName,
                PaneSegment::GitBranch,
                PaneSegment::GitStatus
            ]
        );
        assert_eq!(
            cfg.pane.title_actions,
            vec![PaneAction::Split, PaneAction::Close]
        );
    }

    #[test]
    fn empty_pane_info_bar_is_not_visible() {
        let cfg: AppearanceConfig =
            toml::from_str("[pane]\ntitle_segments = []\ntitle_actions = []").unwrap();
        assert!(!cfg.pane_info_bar_visible());
    }

    #[test]
    fn amounts_map_to_derived_values() {
        let cfg = AppearanceConfig {
            transparency: 25,
            blur: 50,
            terminal: TerminalAppearance {
                transparency: 40,
                ..Default::default()
            },
            vibrancy: Vibrancy::Sidebar,
            ..Default::default()
        };
        assert!((cfg.opacity() - 0.75).abs() < 1e-6);
        assert!(cfg.is_transparent());
        assert!((cfg.blur_radius() - 24.0).abs() < 1e-6); // 50% of 48px
        assert!((cfg.terminal_opacity() - 0.6).abs() < 1e-6);
        assert_eq!(cfg.os_vibrancy(), Some(Vibrancy::Sidebar));
    }

    #[test]
    fn z0_background_knobs_map_to_derived_values() {
        let cfg = AppearanceConfig {
            background_blur: 50,
            background_transparency: 25,
            ..Default::default()
        };
        // blur 50 → 50% of MAX_BLUR_PX (24.0).
        assert!((cfg.background_blur_radius() - 24.0).abs() < 1e-6);
        // transparency 25 → alpha 0.75.
        assert!((cfg.background_alpha() - 0.75).abs() < 1e-6);

        // Over-cap clamps.
        let over = AppearanceConfig {
            background_blur: 200,
            background_transparency: 200,
            ..Default::default()
        };
        assert!((over.background_blur_radius() - MAX_BLUR_PX).abs() < 1e-6);
        assert!((over.background_alpha() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn z0_gradient_default_resolves_to_theme_colors() {
        let mocha = crate::theme::catppuccin_mocha();
        let cfg = AppearanceConfig::default();
        // No config override → inherits the theme's explicit TOML gradient
        // colors (mocha ships them).
        let top = cfg.effective_background_gradient_top(&mocha);
        let bottom = cfg.effective_background_gradient_bottom(&mocha);
        assert_eq!(top, mocha.background_gradient_top.unwrap());
        assert_eq!(bottom, mocha.background_gradient_bottom.unwrap());
        assert_ne!(top, bottom);
    }

    #[test]
    fn z0_gradient_config_override_wins_over_theme() {
        let mocha = crate::theme::catppuccin_mocha();
        let cfg = AppearanceConfig {
            background_gradient_top: Some(Color::new(1, 2, 3, 255)),
            background_gradient_bottom: Some(Color::new(4, 5, 6, 255)),
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_background_gradient_top(&mocha),
            Color::new(1, 2, 3, 255)
        );
        assert_eq!(
            cfg.effective_background_gradient_bottom(&mocha),
            Color::new(4, 5, 6, 255)
        );
    }

    #[test]
    fn z0_gradient_unset_theme_fields_fall_back_to_derived() {
        let mocha = crate::theme::catppuccin_mocha();
        let mut bare = mocha.clone();
        bare.background_gradient_top = None;
        bare.background_gradient_bottom = None;
        let cfg = AppearanceConfig::default();
        // top = background; bottom = darker(background) (≠ background).
        assert_eq!(
            cfg.effective_background_gradient_top(&bare),
            bare.background
        );
        assert_ne!(
            cfg.effective_background_gradient_bottom(&bare),
            bare.background
        );
    }

    #[test]
    fn effect_token_defaults_unset() {
        let cfg = AppearanceConfig::default();
        assert_eq!(cfg.glow_size, None);
        assert_eq!(cfg.intensity, None);
    }

    #[test]
    fn effect_token_resolvers_inherit_theme_when_unset() {
        let mocha = crate::theme::catppuccin_mocha();
        let cfg = AppearanceConfig::default();
        // mocha ships glow_size = GlowLevel::None (the "no glow" variant),
        // intensity = Intensity::Off. (Not to be confused with cfg.glow_size,
        // the Option<GlowLevel> field, which is Option::None for a default config.)
        assert_eq!(cfg.effective_glow_size(&mocha), mocha.glow_size);
        assert_eq!(cfg.effective_intensity(&mocha), mocha.intensity);

        // grid_tron ships glow_size = Medium, intensity = Medium.
        let grid_tron = crate::theme::load("grid_tron");
        assert_eq!(cfg.effective_glow_size(&grid_tron), grid_tron.glow_size);
        assert_eq!(cfg.effective_intensity(&grid_tron), grid_tron.intensity);
    }

    #[test]
    fn effect_token_config_override_wins_over_theme() {
        let mocha = crate::theme::catppuccin_mocha();
        // mocha defaults: glow_size = None, intensity = Off.
        let cfg = AppearanceConfig {
            glow_size: Some(GlowLevel::Large),
            intensity: Some(Intensity::Heavy),
            ..Default::default()
        };
        assert_eq!(cfg.effective_glow_size(&mocha), GlowLevel::Large);
        assert_eq!(cfg.effective_intensity(&mocha), Intensity::Heavy);
        // The theme value is untouched — override is read-only at resolve time.
        assert_eq!(mocha.glow_size, GlowLevel::None);
        assert_eq!(mocha.intensity, Intensity::Off);
    }

    #[test]
    fn effect_tokens_parse_snake_case() {
        // Tests the serde contract of the re-exported heca-theme types
        // (`GlowLevel`/`Intensity` derive Deserialize with `#[serde(rename_all =
        // "snake_case")]`) at the config boundary where TOML values flow into
        // `AppearanceConfig`. If the enum serde representation changes in
        // `heca-theme`, this test breaks — which is the intended signal here.
        #[derive(Deserialize)]
        struct Wrapper {
            glow_size: GlowLevel,
            intensity: Intensity,
        }
        let w: Wrapper = toml::from_str(
            r#"glow_size = "large"
intensity = "heavy""#,
        )
        .expect("effect tokens should parse snake_case");
        assert_eq!(w.glow_size, GlowLevel::Large);
        assert_eq!(w.intensity, Intensity::Heavy);

        let off: Wrapper = toml::from_str(
            r#"glow_size = "none"
intensity = "off""#,
        )
        .expect("off values should parse");
        assert_eq!(off.glow_size, GlowLevel::None);
        assert_eq!(off.intensity, Intensity::Off);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let cfg: AppearanceConfig =
            toml::from_str("transparency = 30\n[terminal]\ntransparency = 15\n")
                .expect("partial appearance toml should parse");
        assert_eq!(cfg.transparency, 30);
        assert_eq!(cfg.blur, 0);
        assert_eq!(cfg.terminal.transparency, 15);
        assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::WhenNeeded);
        assert!(cfg.terminal.show_scrolled_up_badge);
        assert_eq!(cfg.vibrancy, Vibrancy::None);
    }

    #[test]
    fn terminal_scrollbar_visibility_parses_snake_case() {
        let cfg: AppearanceConfig = toml::from_str(
            "[terminal]\nshow_scrollbar = \"always\"\n",
        )
        .expect("terminal show_scrollbar should parse");
        assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::Always);

        let cfg: AppearanceConfig = toml::from_str(
            "[terminal]\nshow_scrollbar = \"never\"\n",
        )
        .expect("terminal show_scrollbar should parse never");
        assert_eq!(cfg.terminal.show_scrollbar, ScrollbarVisibility::Never);
    }

    #[test]
    fn terminal_scrolled_up_badge_setting_parses() {
        let cfg: AppearanceConfig =
            toml::from_str("[terminal]\nshow_scrolled_up_badge = false\n")
                .expect("terminal show_scrolled_up_badge should parse");
        assert!(!cfg.terminal.show_scrolled_up_badge);

        let cfg: AppearanceConfig =
            toml::from_str("[terminal]\nshow_scrolled_up_badge = true\n")
                .expect("terminal show_scrolled_up_badge should parse true");
        assert!(cfg.terminal.show_scrolled_up_badge);
    }

    #[test]
    fn vibrancy_parses_snake_case() {
        #[derive(Deserialize)]
        struct Wrapper {
            vibrancy: Vibrancy,
        }

        let w: Wrapper = toml::from_str(r#"vibrancy = "hud_window""#)
            .expect("hud_window should parse to Vibrancy::HudWindow");
        assert_eq!(w.vibrancy, Vibrancy::HudWindow);
        let n: Wrapper =
            toml::from_str(r#"vibrancy = "none""#).expect("none should parse to Vibrancy::None");
        assert_eq!(n.vibrancy, Vibrancy::None);
    }

    #[test]
    fn config_without_appearance_section_uses_defaults() {
        use crate::loader::Config;

        let cfg: Config = toml::from_str(
            r#"
[settings]
theme = "mocha"
"#,
        )
        .expect("config without [appearance] should parse");

        assert_eq!(cfg.appearance, AppearanceConfig::default());
    }

    #[test]
    fn floating_terminal_knobs_are_independent_of_tiled() {
        // Tiled translucent while floating defaults to opaque.
        let cfg = AppearanceConfig {
            terminal: TerminalAppearance {
                transparency: 95,
                floating_transparency: 0,
                floating_blur: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!((cfg.terminal_opacity() - 0.05).abs() < 1e-6);
        assert!((cfg.terminal_floating_opacity() - 1.0).abs() < f32::EPSILON);
        assert!((cfg.terminal_floating_blur_radius()).abs() < f32::EPSILON);

        // Floating knobs can be set independently of the tiled ones.
        let cfg = AppearanceConfig {
            terminal: TerminalAppearance {
                floating_transparency: 50,
                floating_blur: 25,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!((cfg.terminal_floating_opacity() - 0.5).abs() < 1e-6);
        assert!((cfg.terminal_floating_blur_radius() - 12.0).abs() < 1e-6); // 25% of 48px
    }

    #[test]
    fn floating_border_color_resolves_config_then_float_accent() {
        let mocha = crate::theme::catppuccin_mocha();

        // Default: no config override -> theme.float_accent.
        let cfg = AppearanceConfig::default();
        assert_eq!(
            cfg.effective_pane_floating_border_color(&mocha),
            mocha.float_accent
        );

        // Config override wins.
        let cfg = AppearanceConfig {
            pane: PaneAppearance {
                floating_border_color: Some(Color::new(1, 2, 3, 255)),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_pane_floating_border_color(&mocha),
            Color::new(1, 2, 3, 255)
        );
    }

    #[test]
    fn border_style_parses_snake_case_and_resolves_defaults() {
        let cfg: AppearanceConfig = toml::from_str(
            "[pane]\nborder_style = \"bracketed\"\n[sidebar]\nborder_style = \"none\"",
        )
        .expect("border styles should parse snake_case");
        assert_eq!(cfg.pane.border_style, Some(BorderStyle::Bracketed));
        assert_eq!(cfg.sidebar.border_style, Some(BorderStyle::None));
        assert_eq!(cfg.effective_pane_border_style(), BorderStyle::Bracketed);
        assert_eq!(cfg.effective_sidebar_border_style(), BorderStyle::None);

        // Defaults match the current app look: panes bordered, sidebar bracketed.
        let dflt = AppearanceConfig::default();
        assert_eq!(dflt.effective_pane_border_style(), BorderStyle::Bordered);
        assert_eq!(dflt.effective_sidebar_border_style(), BorderStyle::Bracketed);
    }

    #[test]
    fn sidebar_border_width_resolves_config_then_global_then_theme() {
        let mocha = crate::theme::catppuccin_mocha();

        // Unset → falls back to the global border_width (here the theme value).
        let dflt = AppearanceConfig::default();
        assert_eq!(
            dflt.effective_sidebar_border_width(&mocha),
            dflt.effective_border_width(&mocha)
        );

        // Global border_width set, sidebar unset → sidebar inherits the global.
        let cfg = AppearanceConfig {
            border_width: Some(4.0),
            ..Default::default()
        };
        assert_eq!(cfg.effective_sidebar_border_width(&mocha), 4.0);

        // Sidebar-specific override wins over the global, and is clamped to [0, 10].
        let cfg = AppearanceConfig {
            border_width: Some(4.0),
            sidebar: SidebarAppearance {
                border_width: Some(99.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(cfg.effective_sidebar_border_width(&mocha), 10.0);
    }

    #[test]
    fn sidebar_background_and_radius_resolve_config_then_fallback() {
        let mocha = crate::theme::catppuccin_mocha();
        let theme_surface = Color::new(9, 9, 9, 255);

        // Unset → the theme-derived surface passed in / the theme radius.
        let dflt = AppearanceConfig::default();
        assert_eq!(
            dflt.effective_sidebar_background_color(theme_surface),
            theme_surface
        );
        assert_eq!(
            dflt.effective_sidebar_border_radius(&mocha),
            mocha.border_radius
        );

        // Config override wins (radius clamped to [0, 40]).
        let cfg = AppearanceConfig {
            sidebar: SidebarAppearance {
                background_color: Some(Color::new(1, 2, 3, 255)),
                border_radius: Some(99.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            cfg.effective_sidebar_background_color(theme_surface),
            Color::new(1, 2, 3, 255)
        );
        assert_eq!(cfg.effective_sidebar_border_radius(&mocha), 40.0);
    }

    #[test]
    fn global_border_width_drives_chrome_and_pane_fallback() {
        let theme = crate::theme::Theme::default();
        // Unset: global + pane both fall back to the theme width.
        let dflt = AppearanceConfig::default();
        assert_eq!(dflt.effective_border_width(&theme), theme.border_width);
        assert_eq!(dflt.effective_pane_border_width(&theme), theme.border_width);

        // Global override drives both chrome and the pane fallback.
        let g = AppearanceConfig { border_width: Some(3.0), ..Default::default() };
        assert_eq!(g.effective_border_width(&theme), 3.0);
        assert_eq!(g.effective_pane_border_width(&theme), 3.0);

        // pane.border_width overrides the global for panes only.
        let p = AppearanceConfig {
            border_width: Some(3.0),
            pane: PaneAppearance {
                border_width: Some(1.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(p.effective_border_width(&theme), 3.0);
        assert_eq!(p.effective_pane_border_width(&theme), 1.0);
    }

    #[test]
    fn focus_border_width_resolves_and_clamps() {
        // Unset: focus defaults to 1.5 (visible regardless of the decorative border).
        let dflt = AppearanceConfig::default();
        assert!((dflt.effective_focus_border_width() - 1.5).abs() < f32::EPSILON);

        // Explicit value passes through; out-of-range clamps to [0, 10].
        let cfg = AppearanceConfig {
            focus_border_width: Some(99.0),
            ..Default::default()
        };
        assert_eq!(cfg.effective_focus_border_width(), 10.0);
    }

    #[test]
    fn pane_chrome_values_are_clamped_to_safe_ranges() {
        let theme = crate::theme::Theme::default();

        // Explicit config values above the cap clamp down into range.
        let over = AppearanceConfig {
            pane: PaneAppearance {
                border_width: Some(999.0),
                border_radius: Some(88.0),
                padding: Some(999.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(over.effective_pane_border_width(&theme), 10.0);
        assert_eq!(over.effective_pane_border_radius(&theme), 20.0);
        assert_eq!(over.effective_pane_padding(&theme), 20.0);

        // Negative values clamp to the lower bound (0).
        let under = AppearanceConfig {
            pane: PaneAppearance {
                border_width: Some(-5.0),
                border_radius: Some(-2.0),
                padding: Some(-1.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(under.effective_pane_border_width(&theme), 0.0);
        assert_eq!(under.effective_pane_border_radius(&theme), 0.0);
        assert_eq!(under.effective_pane_padding(&theme), 0.0);

        // Defaults (None) fall back to the theme then clamp — theme defaults
        // (border_width 1.0, border_radius 6.0) are already in range, and pane
        // padding defaults to 4.0.
        let dflt = AppearanceConfig::default();
        assert_eq!(dflt.effective_pane_border_width(&theme), theme.border_width);
        assert_eq!(
            dflt.effective_pane_border_radius(&theme),
            theme.border_radius
        );
        assert_eq!(dflt.effective_pane_padding(&theme), 4.0);

        // In-range values pass through unchanged.
        let mid = AppearanceConfig {
            pane: PaneAppearance {
                border_width: Some(4.0),
                border_radius: Some(15.0),
                padding: Some(8.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(mid.effective_pane_border_width(&theme), 4.0);
        assert_eq!(mid.effective_pane_border_radius(&theme), 15.0);
        assert_eq!(mid.effective_pane_padding(&theme), 8.0);
    }

    #[test]
    fn pane_gap_floored_at_border_width_only_when_borders_visible() {
        let theme = crate::theme::Theme::default();

        // Borders ON (default style "bordered"): gap 0 floors up to the border
        // width so the two adjacent pane borders don't overlap on the shared edge.
        let zero = AppearanceConfig {
            pane: PaneAppearance {
                gap: Some(0.0),
                border_width: Some(3.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(zero.effective_pane_gap(&theme), 3.0);

        // A gap wider than the border is left untouched.
        let wide = AppearanceConfig {
            pane: PaneAppearance {
                gap: Some(12.0),
                border_width: Some(3.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(wide.effective_pane_gap(&theme), 12.0);

        // border_style = "none": no border is drawn, so gap 0 stays truly flush.
        let flush = AppearanceConfig {
            pane: PaneAppearance {
                gap: Some(0.0),
                border_style: Some(BorderStyle::None),
                border_width: Some(3.0),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(flush.effective_pane_gap(&theme), 0.0);
    }
}
