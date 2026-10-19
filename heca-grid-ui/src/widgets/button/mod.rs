//! [`Button`] — an interactive surface whose look is driven by a [`ButtonVariant`]
//! and the shared [`WidgetSize`](crate::style::WidgetSize) (GridCN/shadcn model),
//! with an **animated** hover that differs per variant:
//!
//! | Variant | Hover behavior |
//! |---------|----------------|
//! | Primary (default) | solid border + accent fill that **sweeps bottom→top** with a glow |
//! | Secondary | subtle `muted` fill + `muted` border (theme-consistent — never rides on `surface`); both firm toward `foreground` on hover (no extra hover glow) |
//! | Destructive | like default but red, **fades in** (no sweep) |
//! | Outline | dim border → accent, faint fill + glow |
//! | Ghost | no border/bg at rest → **opaque bg + border fade in**, glowing with the fade |
//! | Link | text only → **underline** appears |
//!
//! Hover progress animates over time via [`Component::tick`](crate::component::Component::tick). Glow and border
//! are toggleable (`.glow(bool)`, `.bordered(bool)`).
//!
//! **Rest glow.** Every bordered variant (Primary / Secondary / Destructive / Outline)
//! carries a faint theme-driven halo **at rest** — intensity from
//! `interaction.control_rest_glow`, tone from the variant (danger for Destructive) —
//! so the control shows the neon identity before any hover/focus and the `glow_size`
//! setting visibly scales it. Ghost/Link are surface-less at rest (nothing to halo);
//! Ghost's fading-in hover surface glows with it. Disabled controls never halo.
//!
//! **Disabled look.** When `disabled`, a button drops its vivid accent/danger chrome to the
//! theme `muted` tone and draws its label in `muted` at a reduced alpha
//! (`DISABLED_CONTENT_ALPHA`). This reads clearly as inactive on **every** variant —
//! including the transparent Ghost/Link, where a background scrim would be invisible — and is
//! fully theme-driven (no hardcoded colours). Disabled buttons are also inert and unfocusable.

use crate::builders::{LayoutExt, Parent};
use crate::component::Base;
use crate::effects::Flash;
use crate::widgets::Glyph;

mod builders;
mod component;

/// Seconds for a full hover transition.
const HOVER_DURATION: f32 = 0.10;
/// Hover glow spread radius (px) — how far the halo reaches (bigger = wider).
/// Matches the other controls' hover halos (Select/Toggle 16, Checkbox 14);
/// the previous 30 made a hovered button visibly out of family (user-reported).
const GLOW_RADIUS: f32 = 16.0;
/// Hover glow peak intensity — how bright (smaller = thinner/fainter).
const GLOW_INTENSITY: f32 = 0.12;
/// REST glow spread radius (px) — deliberately much tighter than the hover halo
/// so a resting button reads like every other control (Input/Select ≈ 12), not
/// like a hovered one.
const REST_GLOW_RADIUS: f32 = 12.0;
/// Opacity of the `Secondary` variant's **fill** — a faint `theme.muted` tint. Using `muted`
/// (a foreground-family token that always contrasts the surface and never equals it) instead of
/// `theme.surface` gives Secondary a consistent "subtly filled" identity on **every** theme:
/// `surface` sits near `background` on some themes (fill vanishes) and can even equal `theme.border`
/// on others (border vanishes). A widget-level constant like `DISABLED_CONTENT_ALPHA`; promote to a
/// theme token if it needs per-theme tuning.
const SECONDARY_FILL_ALPHA: u8 = 36;
/// Opacity of a **disabled** button's label. It is drawn in the theme `muted` tone at this
/// reduced alpha so the disabled state reads clearly on every variant — including the
/// transparent Ghost/Link, whose *enabled* rest label is already `muted` (so only the lowered
/// alpha distinguishes disabled from a normal low-emphasis button).
const DISABLED_CONTENT_ALPHA: f32 = 0.38;

/// Visual variant of a [`Button`] (GridCN/shadcn set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum ButtonVariant {
    /// Accent border; fill sweeps in from the bottom on hover.
    #[default]
    Primary,
    /// Muted surface; border firms up on hover (no glow).
    Secondary,
    /// Danger colors; fades in on hover.
    Destructive,
    /// Dim outline that brightens to accent on hover.
    Outline,
    /// No chrome until hover (bg + border fade in).
    Ghost,
    /// Text only; underline appears on hover.
    Link,
}

/// Reference padding (logical px) at [`WidgetSize::Large`](crate::style::WidgetSize::Large); smaller sizes scale it
/// down by [`WidgetSize::pad_scale`](crate::style::WidgetSize::pad_scale) (tighter than the font at `Small`). The font
/// scales centrally, so the box stays balanced at every size.
const BASE_PAD: f32 = 10.0;
/// Gap between composed content items (icon ↔ label …), as a fraction of the resolved font, so the
/// cluster breathes proportionally at every size instead of at a fixed px.
///
/// `0.5` = **half a character** at the button's own font: the text advance is
/// [`MONO_ADVANCE_RATIO`](crate::font::MONO_ADVANCE_RATIO) (`0.6`) of the font size, so this is a hair under one character width —
/// the same optical spacing a space would give between an icon and the word after it, without the
/// pair drifting apart. It also keeps the gap consistent with the horizontal padding, which adds
/// exactly one character per side.
const GAP_RATIO: f32 = 0.5;

fn alpha(p: f32) -> u8 {
    (p.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A clickable button. Its look comes from its [`ButtonVariant`]; its **content is composed from
/// child components**, not drawn by the button.
///
/// # Content is children
/// The button paints only its own *chrome* (fill, border, hover sweep, press flash, focus ring —
/// all from the [`Theme`](crate::theme::Theme)) and lets the layout engine place its children,
/// which paint themselves. So a button can hold anything: a label, an icon + a label, or an
/// arbitrary tree.
///
/// ```ignore
/// Button::destructive("Delete").icon(Glyph::Trash)   // sugar → children [Icon, Label]
///
/// Button::empty()                                     // arbitrary tree, any depth
///     .variant(ButtonVariant::Destructive)
///     .child(Flex::column().gap(4.0)
///         .child(Flex::row().gap(6.0)
///             .child(Icon::new(Glyph::Trash))
///             .child(Label::new("Delete")))
///         .child(Label::new("Ctrl+D")))
/// ```
///
/// The convenience forms are **sugar that builds those same children at construction time** —
/// there is no separate "simple mode": one child vector, one layout, one paint path. This is what
/// makes the button [`ViewNode`](heca_view::ViewNode)-realizable (the host mapper attaches the
/// node's children here) and extensible by composition rather than by adding hand-drawn extras.
///
/// # Size and color come from the tree, not from arithmetic
/// - **Size**: the button does not compute its width. It hugs its content (`Auto` + padding), so
///   taffy measures whatever it holds — at any depth. The [size variant](LayoutExt::size) cascades
///   to the content automatically (see
///   [`Layout::size_explicit`](crate::style::Layout::size_explicit)).
/// - **Color**: the button publishes one state-derived color per frame via
///   [`PaintCx::with_content_color`](crate::component::PaintCx::with_content_color), and unstyled children ([`Label`](crate::widgets::Label), [`Icon`](crate::widgets::Icon)) pick it up. That
///   is how composed content animates with the hover sweep and fades when disabled, while a child
///   with its own color (e.g. `Badge::danger`) keeps it.
///
/// # One control, one target
/// The button is a single click target and — via
/// [`Base::focus_barrier`](crate::component::Base::focus_barrier) — a single Tab stop, whatever it
/// contains. An interactive child would render but never receive its own clicks or focus.
///
/// **Disabled look.** When `disabled`, the chrome drops to the theme `muted` tone and the content
/// color fades to `muted` at `DISABLED_CONTENT_ALPHA`, which reads on every variant — including
/// the transparent Ghost/Link, where a background scrim would be invisible.
pub struct Button {
    base: Base,
    variant: ButtonVariant,
    show_glow: bool,
    show_border: bool,
    /// Animated hover amount, 0.0 (rest) → 1.0 (hovered). The *state* is
    /// [`Base::hovered`](crate::component::Base::hovered), kept by the pointer router; this is
    /// only how far the visual has eased toward it.
    progress: f32,
    /// Press flash effect (brightens on press, fades out).
    flash: Flash,
    on_click: Option<std::rc::Rc<dyn Fn()>>,
    /// **Held on** — a toggled *status*, not a transient hover. See [`active`](Self::active).
    active: bool,
    /// Showing only its icon. See [`icon_only`](Self::icon_only).
    icon_only: bool,
    /// **The words, kept whether or not they are being shown.** Held here rather than read back out
    /// of the children, because showing only the icon **removes** the label from the tree — see
    /// [`icon_only`](Self::icon_only).
    label: String,
    /// Overrides the hue this button reads in. See [`accent`](crate::builders::ToneExt::accent).
    /// **This button is about something destructive.** Set by the `Destructive` variant, which
    /// carries the whole look — hue and frame together — and a container never strips either.
    /// What it decides beyond the variant's own painting is the hue of the held-on frame, so an
    /// engaged destructive button reads in danger rather than in the accent.
    dangerous: bool,
    /// **The glyph this button was given**, if any. Recorded when [`icon`](Self::icon) inserts it,
    /// so a container can ask what a button *is* without taking its content apart —
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) does, to build the row a collapsed button becomes.
    glyph: Option<Glyph>,
}

impl LayoutExt for Button {}
/// Content is children: `.child(..)` appends any component (sugar like [`Button::new`] /
/// [`Button::icon`] builds those same children).
impl Parent for Button {}

#[cfg(test)]
mod tests;
