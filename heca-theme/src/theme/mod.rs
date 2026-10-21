//! Unified theme types for heca.
//!
//! A [`Theme`] is the palette + effect configuration shared by all rendering
//! layers — grid-ui widgets, chrome, sidebar, and terminal panes.
//! The default theme is the dark, cyan-accented Tron skin ([`Theme::grid_tron`]).

use crate::color::Color;
use serde::{Deserialize, Serialize};

mod defaults;
mod effects;
mod interaction;
mod input;

pub use effects::{FrameStyle, GlowLevel, Intensity, Shadow};
pub use interaction::InteractionAlphas;
pub use input::InputStyle;

use defaults::*;

// ═══════════════════════════════════════════════════════════════════════════════
//  Theme
// ═══════════════════════════════════════════════════════════════════════════════

/// Small controls (inputs, selects, checkboxes, chips) round at this fraction of
/// the base [`Theme::border_radius`], so one global radius scales every widget together.
const CONTROL_RADIUS_FRAC: f32 = 0.5;
/// How far the derived focus-outline color is shifted from a tone toward the theme's
/// `foreground` when `focus_ring` is unset. Because `foreground` is light on dark themes and dark
/// on light themes, this brightens the ring on dark themes and darkens it on light ones — so it
/// separates from the widget's own accent border either way (no hardcoded light/dark, no fixed
/// white/black).
const FOCUS_RING_CONTRAST_FACTOR: f32 = 0.35;
const SIDEBAR_BG_DARKEN_FACTOR: f32 = 0.05;
const TOP_BOTTOM_PANE_BG_DARKEN_FACTOR: f32 = 0.10;
/// How much the derived z=0 gradient *bottom* color darkens
/// [`Theme::background`] toward black when `background_gradient_bottom` is unset
/// (subtle vertical depth). Bundled themes set explicit gradient colors, so this
/// is only a safety net for custom themes missing the field.
///
/// `0.08` is a visual sweet-spot: deep enough to give the blurred gradient
/// visible vertical depth, shallow enough not to read as a separate color band
/// (which the blur would smear awkwardly). Tuned to match the subtle depth of
/// the existing chrome-token darkening factors.
const GRADIENT_BOTTOM_DARKEN_FACTOR: f32 = 0.08;

/// Palette + effect tokens for the entire application.
///
/// This is the single source of truth for all theming. Grid-ui widgets read it
/// via `cx.theme()`, the config system loads it from `.toml` files, and the
/// renderer uses it for chrome/terminal colors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    /// Text-field layout and caret settings, shared by every input widget.
    #[serde(default)]
    pub input: InputStyle,
    pub name: String,

    // ── Core palette ──
    pub background: Color,
    #[serde(default = "default_surface")]
    pub surface: Color,
    pub foreground: Color,
    #[serde(default = "default_muted")]
    pub muted: Color,
    pub border: Color,
    pub accent: Color,
    #[serde(default = "default_glow_color")]
    pub glow: Color,
    /// **The panel a selected thing sits on** — what *is selected*, as opposed to the accent ring,
    /// which says *where the cursor is*.
    ///
    /// The two used to be one colour, and they are drawn on the same rectangle, so the moment the
    /// cursor arrived on the selected row the pair became indistinguishable — in the exposé,
    /// moving onto the focused pane left no way to tell where the focus was. Trying a second *hue*
    /// for the cursor did not fix it either: in every shipped theme `foreground`, `accent` and
    /// `glow` are one family (mocha and grid_tron both set `glow = accent`; each foreground is a
    /// pale tint of it), so the two marks stayed variations on each other.
    ///
    /// So they differ in **kind**, not in hue: selection is a **filled panel**, the cursor is a
    /// **glowing ring**. One can sit inside the other and both stay readable.
    ///
    /// `None` derives it — [`effective_selected_background`](Self::effective_selected_background)
    /// — so a user's own theme gets a coherent selection without naming one.
    #[serde(default)]
    pub selected_background: Option<Color>,
    /// **The row a back-and-forth binding would return to** (`prefix+i`) — "you were just here".
    ///
    /// A colour, not an alpha, for the same reason every other token here is one: a theme author
    /// picks what they can see. `None` derives it — see
    /// [`effective_previous_background`](Self::effective_previous_background).
    #[serde(default)]
    pub previous_background: Option<Color>,
    /// **The frame of the workspace you are in.** A container hint, so it sits far closer to the
    /// surface than a selected row does — a whole frame at a row's strength lifts every row inside
    /// it and swallows the marks within.
    #[serde(default)]
    pub workspace_active_background: Option<Color>,
    /// **The frame of the workspace `prefix+Shift+i` would return to.** Fainter again than the
    /// active frame: it answers *where would I land*, not *where am I*.
    #[serde(default)]
    pub workspace_previous_background: Option<Color>,
    /// **The card `prefix+i` would return to** — the exposé's "you were just here", at card scale.
    ///
    /// The third scale, because a card is neither: bigger than a row, so a row's tint shouts on it;
    /// far smaller than a workspace frame, so a frame's tint vanishes on it. Both were tried on the
    /// map and both were wrong, in opposite directions. `None` derives it — see
    /// [`effective_card_previous_background`](Self::effective_card_previous_background).
    #[serde(default)]
    pub card_previous_background: Option<Color>,
    #[serde(default)]
    pub shadow: Shadow,
    #[serde(default = "default_danger")]
    pub danger: Color,
    #[serde(default = "default_success")]
    pub success: Color,
    #[serde(default = "default_warning")]
    pub warning: Color,

    // ── Chrome geometry ──
    pub border_radius: f32,
    pub border_width: f32,
    #[serde(default = "default_pane_padding")]
    pub pane_padding: f32,

    // ── Chrome background tokens ──
    #[serde(default)]
    pub left_sidebar_background: Option<Color>,
    #[serde(default)]
    pub right_sidebar_background: Option<Color>,
    #[serde(default)]
    pub top_bottom_pane_background: Option<Color>,

    // ── z=0 background gradient (compositor-blur refactor) ──
    /// Optional top color of the z=0 vertical gradient. `None` → falls back to
    /// [`Theme::background`] (see [`Theme::effective_background_gradient_top`]).
    #[serde(default)]
    pub background_gradient_top: Option<Color>,
    /// Optional bottom color of the z=0 vertical gradient. `None` → falls back to
    /// a slightly darkened [`Theme::background`] (see
    /// [`Theme::effective_background_gradient_bottom`]).
    #[serde(default)]
    pub background_gradient_bottom: Option<Color>,

    // ── Effect tokens ──
    #[serde(default)]
    pub glow_size: GlowLevel,
    #[serde(default)]
    pub intensity: Intensity,
    #[serde(default = "default_true")]
    pub show_focus_border: bool,
    /// Frame decoration drawn around an **overlay panel** (dialog, dropdown,
    /// context menu, command palette) by the shared overlay panel chrome.
    /// Defaults to [`FrameStyle::Bracketed`] — the accent corner reticle that
    /// matches `Pane`/`DockFrame`. Set it in a theme (`overlay_frame = "bordered"`)
    /// or per-user via `[appearance] overlay_border_style` to get a plain edge
    /// (`bordered`) or no frame at all (`none`). The widget's own accent border and
    /// glow are separate — this token only governs the shared frame.
    #[serde(default)]
    pub overlay_frame: FrameStyle,
    /// Optional color of the keyboard **focus outline** — the thin ring drawn just *outside* a
    /// focused widget (see [`effective_focus_ring`](Self::effective_focus_ring) /
    /// [`PaintCx::focus_ring`](../heca_grid_ui/struct.PaintCx.html)). `None` → the accent shifted
    /// toward `foreground` for contrast, which stays legible on both dark and light themes. Set it
    /// in a theme (`focus_ring = "#rrggbb"`) to override the default/accent focus color; tonal
    /// rings that aren't the accent (e.g. a destructive button's `danger` ring) always derive via
    /// [`focus_ring_tone`](Self::focus_ring_tone) and are not overridden by this token.
    #[serde(default)]
    pub focus_ring: Option<Color>,
    /// Optional colour of the **picker's letters** — the keycaps `prefix+/` and the pane / column /
    /// workspace picks stamp over their targets. `None` → the accent.
    ///
    /// **One colour for every letter in the app**, exactly as `hint_font_size` is one size: a letter
    /// is chrome the framework draws *over* a target, not part of the target, so it must not take
    /// the colour of whatever it happens to land on. Set it in a theme (`hint_color = "#rrggbb"`)
    /// when the accent is hard to pick out against your panes.
    ///
    /// A host may still tint one *kind* of target differently — workspace picks are drawn in
    /// `warning` so they read apart from pane picks — through
    /// [`HintExt::hint_color`](../heca_grid_ui/builders/trait.HintExt.html), which
    /// overrides this per widget.
    #[serde(default)]
    pub hint_color: Option<Color>,
    #[serde(default = "default_icon_secondary_alpha")]
    pub icon_secondary_alpha: f32,
    /// Opacity (`0.0..=1.0`) of the **active-region wash** — the faint accent
    /// overlay a `DockFrame` paints over itself when marked active (e.g. the
    /// active workspace in the sidebar). Theme/config-driven, not baked into the
    /// widget, so the active highlight flips in place via the bound signal.
    #[serde(default = "default_active_wash_alpha")]
    pub active_wash_alpha: f32,
    /// Falloff radius (logical px, before the theme's `glow_size` scales it) of the halo around the
    /// **active pane** — and the card that stands for it in the exposé. One token so the map and
    /// the app cannot drift.
    #[serde(default = "default_active_glow_radius")]
    pub active_glow_radius: f32,
    /// Opacity (`0.0..=1.0`) of the **picture of a carried widget** that follows the pointer while
    /// it is dragged — over everything, so what it is carried onto still shows through.
    #[serde(default = "default_drag_image_alpha")]
    pub drag_image_alpha: f32,
    /// Colour of the **line a carried pane can be dropped on** — the border between two stacked
    /// panes and the top and bottom of a column. `None` is the theme's accent.
    #[serde(default)]
    pub drag_edge_color: Option<Color>,
    /// Colour of that line where a drop **would land now** (the one under the pointer), so it
    /// stands apart from the rest. `None` is the theme's warning colour.
    #[serde(default)]
    pub drag_edge_target_color: Option<Color>,
    /// Thickness (logical px) of that line when a drag is over it; at rest it is half as thick.
    #[serde(default = "default_drag_edge_width")]
    pub drag_edge_width: f32,
    /// How big that picture is next to the widget it shows (`1.0` = life size), shrunk toward the
    /// point it was picked up by so it keeps following the pointer.
    #[serde(default = "default_drag_image_scale")]
    pub drag_image_scale: f32,
    /// Strength (`0.0..=1.0`) of that halo.
    #[serde(default = "default_active_glow_strength")]
    pub active_glow_strength: f32,
    /// Opacity (`0.0..=1.0`) of a sidebar/list **card's resting background** tint
    /// (e.g. each pane card). Kept very low so a card reads as a subtle raised
    /// surface rather than a filled block. Theme/config-driven.
    #[serde(default = "default_card_background_alpha")]
    pub card_background_alpha: f32,
    /// Blur radius, in **logical px**, of the frosted backdrop behind a layer that asks for one
    /// (the exposé). `0.0` = flat: whatever is behind shows through sharp.
    ///
    /// Theme-owned for the same reason [`InteractionAlphas::scrim`] is: the treatment of what sits
    /// *behind* an overlay is a design decision about the surface, not an ambient effect the user
    /// switches on. It is deliberately **not** `appearance.blur`, whose contract is that every
    /// appearance default is off — an overview whose backdrop is a flat fill is a different screen
    /// rather than a lens over this one, so this one is on by default and tuned here.
    #[serde(default = "default_overlay_frost_radius")]
    pub overlay_frost_radius: f32,

    /// Interaction-state alpha tokens (hover / active / border / tonal-fill /
    /// scrim …). Theme-owned so the whole UI's interaction feel is tuned in one
    /// place, not per-widget constants. `#[serde(default)]` → existing theme TOMLs
    /// (which don't list them) inherit [`InteractionAlphas::default`].
    #[serde(default)]
    pub interaction: InteractionAlphas,

    // ── Float pane colors ──
    #[serde(default = "default_float_bg")]
    pub float_background: Color,
    #[serde(default = "default_float_accent")]
    pub float_accent: Color,
    #[serde(default = "default_float_focus")]
    pub float_focus: Color,

    // ── Drag-and-drop colors ──
    #[serde(default = "default_drag_ghost_bg", alias = "sidebar_drag_ghost_bg")]
    pub drag_ghost_bg: Color,
    #[serde(default = "default_drag_ghost_fg", alias = "sidebar_drag_ghost_fg")]
    pub drag_ghost_fg: Color,
    #[serde(default = "default_drag_source_bg", alias = "sidebar_drag_source_bg")]
    pub drag_source_bg: Color,
    #[serde(
        default = "default_drag_source_border",
        alias = "sidebar_drag_source_border"
    )]
    pub drag_source_border: Color,
    #[serde(default = "default_drop_target_bg")]
    pub drop_target_bg: Color,
    #[serde(default = "default_drop_target_border")]
    pub drop_target_border: Color,
    #[serde(default = "default_drop_insertion")]
    pub drop_insertion: Color,

    // ── Sidebar font sizes ──
    #[serde(default = "default_sidebar_label_font_size")]
    pub sidebar_label_font_size: f32,
    #[serde(default = "default_sidebar_button_font_size")]
    pub sidebar_button_font_size: f32,

    // ── Terminal ──
    #[serde(default)]
    pub terminal_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_background: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_background: Option<Color>,
    #[serde(default)]
    pub terminal_cursor_border: Option<Color>,
    #[serde(default)]
    pub terminal_selection_foreground: Option<Color>,
    #[serde(default)]
    pub terminal_selection_background: Option<Color>,
    #[serde(default)]
    pub terminal_ansi: Option<[Color; 8]>,
    #[serde(default)]
    pub terminal_brights: Option<[Color; 8]>,
}

impl Default for Theme {
    fn default() -> Self {
        Self::grid_tron()
    }
}

impl Theme {
    /// **The panel a selected row/card sits on**, explicit or derived.
    ///
    /// Derived, it is the theme's own `surface` carried a fifth of the way toward its `accent`:
    /// far enough from the surface to read as *lit*, far enough from the accent that the cursor's
    /// ring still stands out on top of it, and — because it is built out of the theme's own two
    /// colours — coherent in a palette nobody here has seen. A theme that art-directs its selection
    /// sets the field and this returns that instead.
    pub fn effective_selected_background(&self) -> Color {
        self.selected_background
            .unwrap_or_else(|| self.surface.lerp(self.accent, SELECTED_LIFT))
    }

    /// The "you were just here" row, explicit or derived — the selected panel carried most of the
    /// way back toward the surface, so the two read as one family at two strengths.
    pub fn effective_previous_background(&self) -> Color {
        self.previous_background
            .unwrap_or_else(|| self.surface.lerp(self.accent, PREVIOUS_LIFT))
    }

    /// The current workspace's frame, explicit or derived. **Container-scale**: a frame covers
    /// every row inside it, so it lifts far less than a row does or the marks within it vanish.
    pub fn effective_workspace_active_background(&self) -> Color {
        self.workspace_active_background
            .unwrap_or_else(|| self.surface.lerp(self.accent, WORKSPACE_ACTIVE_LIFT))
    }

    /// The **card** back-and-forth would return to, explicit or derived. See
    /// [`card_previous_background`](Self::card_previous_background) for why a card has a scale of
    /// its own.
    pub fn effective_card_previous_background(&self) -> Color {
        self.card_previous_background
            .unwrap_or_else(|| self.surface.lerp(self.accent, CARD_PREVIOUS_LIFT))
    }

    /// The workspace back-and-forth would return to, explicit or derived — fainter than the active
    /// frame.
    pub fn effective_workspace_previous_background(&self) -> Color {
        self.workspace_previous_background
            .unwrap_or_else(|| self.surface.lerp(self.accent, WORKSPACE_PREVIOUS_LIFT))
    }

    /// Corner radius for small controls — a fraction of the base
    /// [`border_radius`](Theme::border_radius).
    pub fn control_radius(&self) -> f32 {
        self.border_radius * CONTROL_RADIUS_FRAC
    }

    fn derived_darker_background(&self, t: f32) -> Color {
        self.background.lerp(Color::rgb(0, 0, 0), t.clamp(0.0, 1.0))
    }

    pub fn effective_left_sidebar_background(&self) -> Color {
        self.left_sidebar_background
            .unwrap_or_else(|| self.derived_darker_background(SIDEBAR_BG_DARKEN_FACTOR))
    }

    pub fn effective_right_sidebar_background(&self) -> Color {
        self.right_sidebar_background
            .unwrap_or_else(|| self.derived_darker_background(SIDEBAR_BG_DARKEN_FACTOR))
    }

    pub fn effective_top_bottom_pane_background(&self) -> Color {
        self.top_bottom_pane_background
            .unwrap_or_else(|| self.derived_darker_background(TOP_BOTTOM_PANE_BG_DARKEN_FACTOR))
    }

    /// A readable text/glyph color to place **on** a tonal fill (`accent`, `danger`,
    /// `success`, …): whichever of the theme's `background` / `foreground` contrasts
    /// more with the fill's luminance. Theme-driven (no hardcoded light/dark) — so a
    /// dark label lands on a bright accent and a light label on a saturated danger.
    pub fn on(&self, fill: Color) -> Color {
        let l = fill.luminance();
        if (l - self.background.luminance()).abs() >= (l - self.foreground.luminance()).abs() {
            self.background
        } else {
            self.foreground
        }
    }

    /// Effective z=0 gradient *top* color: the theme field when set, else
    /// [`Theme::background`] (an unset gradient is a flat bg-color fill — still a
    /// valid frost source once blurred). The app's `[appearance]`
    /// `background_gradient_top` override (if any) takes precedence over this —
    /// see `heca_config::appearance::AppearanceConfig::effective_background_gradient_top`.
    pub fn effective_background_gradient_top(&self) -> Color {
        self.background_gradient_top.unwrap_or(self.background)
    }

    /// Effective z=0 gradient *bottom* color: the theme field when set, else a
    /// slightly darkened [`Theme::background`] (subtle vertical depth). The app's
    /// `[appearance]` `background_gradient_bottom` override (if any) takes
    /// precedence — see
    /// `heca_config::appearance::AppearanceConfig::effective_background_gradient_bottom`.
    pub fn effective_background_gradient_bottom(&self) -> Color {
        self.background_gradient_bottom
            .unwrap_or_else(|| self.derived_darker_background(GRADIENT_BOTTOM_DARKEN_FACTOR))
    }

    /// The keyboard **focus-outline color** for the default (accent) tone — what
    /// [`PaintCx::focus_ring`](../heca_grid_ui/struct.PaintCx.html) draws on every focused widget.
    /// Returns the theme's `focus_ring` token when set, else the accent shifted toward `foreground`
    /// (see [`focus_ring_tone`](Self::focus_ring_tone)) so the ring reads distinct from an accent
    /// border on both dark and light themes.
    pub fn effective_focus_ring(&self) -> Color {
        self.focus_ring
            .unwrap_or_else(|| self.focus_ring_tone(self.accent))
    }

    /// The **picker's letter colour** — the theme's `hint_color` token when set, else the accent.
    ///
    /// Read at the one place the picker's tokens are applied, so every keycap in the app answers to
    /// it and a theme reload moves them all together.
    pub fn effective_hint_color(&self) -> Color {
        self.hint_color.unwrap_or(self.accent)
    }

    /// Derive a focus-outline color from any semantic tone (`accent`, `danger`, …) by shifting it
    /// toward the theme's `foreground`. `foreground` is the theme's high-contrast-against-background
    /// color — light on dark themes, dark on light themes — so this brightens the ring on dark
    /// themes and darkens it on light ones, keeping it separate from the tone's own border in both.
    /// Theme-driven, no hardcoded light/dark (same idiom as [`on`](Self::on)). Use this for tonal
    /// focus rings that aren't the default accent (e.g. a destructive button's `danger` ring); the
    /// `focus_ring` token overrides only the default/accent case via [`effective_focus_ring`](Self::effective_focus_ring).
    pub fn focus_ring_tone(&self, base: Color) -> Color {
        base.lerp(self.foreground, FOCUS_RING_CONTRAST_FACTOR)
    }

    /// The default dark, cyan-accented Tron theme.
    ///
    /// Sourced from the bundled `themes/grid_tron.toml` (embedded at compile
    /// time via `include_str!`) so there is a **single source of truth** — no
    /// parallel hand-maintained Rust color literals to drift from the TOML.
    /// `Theme::default()` returns this. If the TOML ever fails to parse the
    /// `Theme` shape, that is a compile-time-shippable bug and we panic eagerly.
    pub fn grid_tron() -> Self {
        toml::from_str(include_str!("../themes/grid_tron.toml"))
            .expect("bundled grid_tron.toml must parse into Theme")
    }
}

/// How far a derived [selected panel](Theme::effective_selected_background) is carried from the
/// theme's `surface` toward its `accent`.
const SELECTED_LIFT: f32 = 0.42;
/// How far a derived [previous row](Theme::effective_previous_background) sits between the surface
/// and the selected panel.
///
/// **Measured, not judged**: it puts the row at ~1.95:1 against the theme's own surface — a step
/// you can find when you look for it, and a clear one below the selected panel's ~2.3:1. At the
/// 0.24 it shipped with, the same row computed to 1.5:1, which reads as "not much noticeable"
/// because it very nearly is not.
const PREVIOUS_LIFT: f32 = 0.34;
/// A derived **current-workspace frame**. Far below a row's, because a frame is a large area and
/// the same strength would swallow every mark inside it.
const WORKSPACE_ACTIVE_LIFT: f32 = 0.13;
/// A derived **last-visited-workspace frame** — fainter again than the active one.
const WORKSPACE_PREVIOUS_LIFT: f32 = 0.08;
/// A derived **last-visited card** — between a row's mark and a whole frame's, because a card is
/// between the two in size. A row's `0.34` shouts on a surface the size of a pane; a frame's `0.08`
/// disappears on it. This is the value that stays findable at a glance across a grid of cards
/// without competing with the one you are actually on.
const CARD_PREVIOUS_LIFT: f32 = 0.22;

#[cfg(test)]
mod tests;
