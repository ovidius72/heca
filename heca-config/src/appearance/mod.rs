//! **`[appearance]`** — how heca looks, set in `config.toml` over the loaded theme.
//!
//! | File | What it owns |
//! |---|---|
//! | `styles` | the named choices: vibrancy, border style, link style, scrollbar visibility |
//! | `terminal` | the terminal's look: transparency, scrollbar, ligatures, links, images, bell, search |
//! | `pane` | the pane's look: header parts and buttons |
//! | `sidebar` | the sidebar's look: width, row lines |
//! | `effective` | what wins for each setting: the config, then the theme, then the default |

mod effective;
mod pane;
mod sidebar;
mod styles;
mod terminal;
#[cfg(test)]
mod tests;

pub use pane::{PaneAppearance, default_pane_title_actions, default_pane_title_segments};
pub use sidebar::{SidebarAppearance, default_sidebar_pane_lines};
pub use styles::{BorderStyle, HyperlinkStyle, ScrollbarVisibility, Vibrancy};
pub use terminal::TerminalAppearance;

use crate::color::Color;
use crate::theme::{FrameStyle, GlowLevel, Intensity, Theme};
use serde::{Deserialize, Serialize};

fn default_transparency() -> u8 {
    0
}

fn default_blur() -> u8 {
    0
}

fn default_hint_font_size() -> f32 {
    12.0
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

/// Maximum in-app blur radius in logical px, at `blur = 100`.
const MAX_BLUR_PX: f32 = 48.0;

/// Sidebar width bounds (logical px); the configured `sidebar_width` is clamped to
/// this range. Default when unset.
pub const MIN_SIDEBAR_WIDTH: f32 = 160.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 560.0;
const DEFAULT_SIDEBAR_WIDTH: f32 = 300.0;

/// Bar heights (logical px) when `top_bar_height` / `bottom_bar_height` are unset, and the range a
/// configured height is clamped to — a bar must stay tall enough to hold its text and short enough
/// to leave the panes room.
const DEFAULT_TOP_BAR_HEIGHT: f32 = 32.0;
const DEFAULT_BOTTOM_BAR_HEIGHT: f32 = 24.0;
const MIN_BAR_HEIGHT: f32 = 16.0;
const MAX_BAR_HEIGHT: f32 = 96.0;

/// The name drawn over a pane while it is dragged: its size is this share of the pane's shorter
/// side, kept within `[min, max]` logical px. Defaults when the `drag_label_*` keys are unset.
const DEFAULT_DRAG_LABEL_SIZE_FACTOR: f32 = 0.25;
const DEFAULT_DRAG_LABEL_MIN_SIZE: f32 = 24.0;
const DEFAULT_DRAG_LABEL_MAX_SIZE: f32 = 72.0;

/// Default width of affordance outlines (focus ring + selection) when
/// `focus_border_width` is unset — kept visible regardless of the global border.
pub const DEFAULT_FOCUS_BORDER_WIDTH: f32 = 1.5;
/// The widest any border may be set, in logical px — panes, sidebars and the focus outline alike.
const MAX_BORDER_WIDTH: f32 = 10.0;
/// The roundest a pane's corners may be, in logical px.
const MAX_PANE_RADIUS: f32 = 20.0;
/// The most padding a pane may keep inside its border, in logical px.
const MAX_PANE_PADDING: f32 = 20.0;
/// The roundest the sidebar's corners may be, in logical px.
const MAX_SIDEBAR_RADIUS: f32 = 40.0;
/// The space between panes when `[appearance.pane] gap` is unset, in logical px.
pub const DEFAULT_PANE_GAP: f32 = 8.0;
/// The space between sidebar sections when `[appearance.sidebar] gap` is unset, in logical px.
pub const DEFAULT_SIDEBAR_GAP: f32 = 12.0;
/// Over a transparent window, the chrome is this share of the window's own opacity — a little
/// clearer, so the window's content still reads through it.
const CHROME_OPACITY_SHARE: f32 = 0.7;

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

    /// **The `prefix+/` picker's letter size, in logical px.** One size for every letter in the app.
    ///
    /// Fixed rather than derived from whatever a letter sits on: a size variant scales what a widget
    /// draws as its own content, and a letter is chrome the framework stamps *over* a target — so
    /// taking the target's size gave an emphasized header button a letter a quarter larger than the
    /// pane's own, in the same picker.
    #[serde(default = "default_hint_font_size")]
    pub hint_font_size: f32,
    /// Picker letter **colour** override. `None` → inherits the theme's `hint_color`, which itself
    /// falls back to the accent. Set in `config.toml` to pin one colour whatever theme is loaded.
    ///
    /// The pair with [`hint_font_size`](Self::hint_font_size): size is a setting because it depends
    /// on your display and font, colour is a theme token because it belongs with a palette — and
    /// this override exists for the same reason `glow_size` has one, so a user can differ from every
    /// theme they load without editing each.
    #[serde(default)]
    pub hint_color: Option<Color>,

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
    /// Keyboard focus-outline visibility override. `None` → inherits
    /// `theme.show_focus_border` (themes default `true`). `false` removes the
    /// focus ring on every widget; when `true`/unset the ring still shows only
    /// on **keyboard** focus (Tab/arrows), never on a mouse click
    /// (focus-visible semantics).
    #[serde(default)]
    pub show_focus_border: Option<bool>,
    /// Frame style for **overlay panels** — dialogs, dropdowns, context menus,
    /// the command palette. `None` → inherits `theme.overlay_frame` (themes
    /// default `bracketed`, the accent corner reticle). Set `bordered` for a plain
    /// continuous edge, or `none` for no shared frame at all (each widget's own
    /// accent border and glow are unaffected). `none | bordered | bracketed`.
    #[serde(default)]
    pub overlay_border_style: Option<BorderStyle>,

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

    /// Height of the top bar — the tab bar — in logical px. `None` → 32; clamped `[16, 96]`.
    /// Only used while the bar is shown (`[settings] show_top_bar`).
    #[serde(default)]
    pub top_bar_height: Option<f32>,
    /// Height of the bottom bar — the status bar — in logical px. `None` → 24; clamped `[16, 96]`.
    /// Only used while the bar is shown (`[settings] show_bottom_bar`).
    #[serde(default)]
    pub bottom_bar_height: Option<f32>,
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
    /// The exposé (`[appearance.expose]`).
    #[serde(default)]
    pub expose: ExposeAppearance,
}

/// The exposé's own appearance (`[appearance.expose]`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ExposeAppearance {
    /// Show each pane's working directory (folder icon + home-relative path) on its card in the
    /// exposé. On by default. It used to be `[settings] pane_show_cwd`, which is still read when
    /// this one is not written; once it is written, this one decides.
    #[serde(default)]
    pub show_cwd: Option<bool>,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            transparency: default_transparency(),
            blur: default_blur(),
            vibrancy: default_vibrancy(),
            hint_font_size: default_hint_font_size(),
            background_gradient_top: None,
            background_gradient_bottom: None,
            background_blur: default_background_blur(),
            background_transparency: default_background_transparency(),
            hint_color: None,
            glow_size: None,
            intensity: None,
            show_focus_border: None,
            overlay_border_style: None,
            border_width: None,
            border_color: None,
            border_radius: None,
            focus_border_width: None,
            top_bar_height: None,
            bottom_bar_height: None,
            terminal: TerminalAppearance::default(),
            pane: PaneAppearance::default(),
            sidebar: SidebarAppearance::default(),
            expose: ExposeAppearance::default(),
        }
    }
}
