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
//! Hover progress animates over time via [`Component::tick`]. Glow and border
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
use crate::color::Color;
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::effects::Flash;
use crate::font::MONO_ADVANCE_RATIO;
use crate::reactive::{Signal, SignalGet};
use crate::scene::{Border, Glow};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::{Glyph, Icon, Label};
use heca_core::layout::{Point, Rectangle, Size};

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

/// Reference padding (logical px) at [`WidgetSize::Large`]; smaller sizes scale it
/// down by [`WidgetSize::pad_scale`] (tighter than the font at `Small`). The font
/// scales centrally, so the box stays balanced at every size.
const BASE_PAD: f32 = 10.0;
/// Gap between composed content items (icon ↔ label …), as a fraction of the resolved font, so the
/// cluster breathes proportionally at every size instead of at a fixed px.
///
/// `0.5` = **half a character** at the button's own font: the text advance is
/// [`MONO_ADVANCE_RATIO`] (`0.6`) of the font size, so this is a hair under one character width —
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
/// makes the button [`ViewNode`]-realizable (the host mapper attaches the node's children here) and
/// extensible by composition rather than by adding hand-drawn extras.
///
/// # Size and color come from the tree, not from arithmetic
/// - **Size**: the button does not compute its width. It hugs its content (`Auto` + padding), so
///   taffy measures whatever it holds — at any depth. The [size variant](LayoutExt::size) cascades
///   to the content automatically (see [`Style::size_explicit`](crate::style::Style::size_explicit)).
/// - **Color**: the button publishes one state-derived color per frame via
///   [`PaintCx::with_content_color`], and unstyled children ([`Label`], [`Icon`]) pick it up. That
///   is how composed content animates with the hover sweep and fades when disabled, while a child
///   with its own color (e.g. `Badge::danger`) keeps it.
///
/// # One control, one target
/// The button is a single click target and — via
/// [`Base::focus_barrier`](crate::component::Base::focus_barrier) — a single Tab stop, whatever it
/// contains. An interactive child would render but never receive its own clicks or focus.
///
/// **Disabled look.** When `disabled`, the chrome drops to the theme `muted` tone and the content
/// color fades to `muted` at [`DISABLED_CONTENT_ALPHA`], which reads on every variant — including
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
    /// Overrides the hue this button reads in. See [`tone`](Self::tone).
    tone: Option<Color>,
    /// **The glyph this button was given**, if any. Recorded when [`icon`](Self::icon) inserts it,
    /// so a container can ask what a button *is* without taking its content apart —
    /// [`ButtonGroup`](super::ButtonGroup) does, to build the row a collapsed button becomes.
    glyph: Option<Glyph>,
}

#[heca_grid_ui_macros::props]
impl Button {
    /// An **empty** primary button — no content. Compose it with [`child`](Parent::child) /
    /// [`icon`](Self::icon) / [`content_boxed`](Self::content_boxed).
    ///
    /// [`new`](Self::new) is the common case (a single label); this is the entry point when the
    /// content is a tree.
    pub fn empty() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.one_click_target = true; // and one click target (Base::one_click_target)
        // One control = one Tab stop: focus never descends into composed content.
        base.focus_barrier = true;
        // Content is laid out as a centered row; padding/gap derive from the size variant in
        // `remeasure`, and the variant itself is set via `LayoutExt::size`.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Center;
        let mut button = Self {
            base,
            variant: ButtonVariant::Primary,
            show_glow: true,
            show_border: true,
            progress: 0.0,
            flash: Flash::new(),
            on_click: None,
            active: false,
            icon_only: false,
            label: String::new(),
            tone: None,
            glyph: None,
        };
        button.remeasure();
        button
    }

    /// A primary button showing `label` — sugar for [`empty`](Self::empty) plus a bold
    /// [`Label`] child.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        let mut b = Self::empty().child(Label::new(label.clone()).bold(true));
        b.label = label;
        b
    }

    /// Prepend a leading [`Icon`] — sugar for a child, so `Button::new("Save").icon(Glyph::Check)`
    /// holds `[Icon, Label]`. The icon inherits the button's state color and size variant.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self.base.children.insert(0, Box::new(Icon::new(glyph)));
        self
    }

    /// **The button's own click, shareable.**
    ///
    /// A container that has to present this button as something else — a
    /// [`ButtonGroup`](super::ButtonGroup) turning it into a menu row when there is no room for it —
    /// runs *this*, so the two are not two paths over one action. It is a shared handle rather than
    /// an owned box for exactly that reason.
    pub fn click_handler(&self) -> Option<std::rc::Rc<dyn Fn()>> {
        self.on_click.clone()
    }

    /// **What this button's icon is**, or `None` when it has only words.
    ///
    /// Read by a container that has to render the button as something else — a
    /// [`ButtonGroup`](super::ButtonGroup) building the menu row a collapsed button becomes. It
    /// asks rather than reaching into `children`, so the answer cannot depend on how the content
    /// happens to be composed.
    pub fn glyph(&self) -> Option<Glyph> {
        self.glyph
    }

    /// **The words this button carries.** Every constructor takes them, so a button always has
    /// some — which is what makes a [`ButtonGroup`](super::ButtonGroup) collapse readable. Answered
    /// from its own field, so it is the same whether the words are being shown or not.
    pub fn label(&self) -> String {
        self.label.clone()
    }

    /// Append an already-boxed component — the seam for a subtree built by a mapper
    /// (`realize(&ViewNode)` returns `Box<dyn Component>`, which is not itself `Component` and so
    /// cannot go through [`Parent::child`]). Mirrors `Dialog::body_boxed`.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn content_boxed(mut self, content: Box<dyn Component>) -> Self {
        self.base.children.push(content);
        self
    }

    /// Convenience constructors, one per variant.
    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label)
    }
    pub fn secondary(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Secondary)
    }
    pub fn destructive(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Destructive)
    }
    pub fn outline(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Outline)
    }
    pub fn ghost(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Ghost)
    }
    pub fn link(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Link)
    }

    /// Mark the button as **held on** (toggled). When `true` it paints a persistent tone-tinted
    /// wash and a firm border under whatever its variant draws — the held version of a hover frame,
    /// matching the [`Toggle`](super::Toggle) on-state — so it reads as an active *status* rather
    /// than a button that merely exists. Hover and press still layer on top, so an engaged button
    /// still brightens under the cursor.
    ///
    /// **The same state, and the same tokens, as [`IconButton::active`](super::IconButton::active)**
    /// — a pane's zoom and float buttons look identical whether they are drawn as icons or as
    /// icons with words, which is what lets a [`ButtonGroup`](super::ButtonGroup) hold them.
    #[heca_grid_ui_macros::prop]
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// **Which variant this button is**, so a container can tell one that was left at the default
    /// from one that named its own — see [`ButtonGroup::variant`](super::ButtonGroup::variant).
    pub fn variant_of(&self) -> ButtonVariant {
        self.variant
    }

    /// **The hue this button reads in**, overriding what its variant would use — its content, its
    /// hover wash and its held-on frame.
    ///
    /// The same builder [`IconButton::tone`](super::IconButton::tone) has, and for the same reason:
    /// a button can be *about* something dangerous without being drawn as a boxed destructive
    /// control. In a row of quiet ghost buttons, a `Destructive` variant is the odd one out — it
    /// carries a border the others do not — where a ghost button in the danger hue reads as a cue.
    #[heca_grid_ui_macros::prop]
    pub fn tone(mut self, c: Color) -> Self {
        self.tone = Some(c);
        self
    }

    /// **Show only the icon, keeping the words.** The label is not drawn and takes no space, and
    /// the button becomes square — the horizontal room a `Button` reserves exists for text, so a
    /// button with none is a button that is too wide by exactly that much.
    ///
    /// The words are still *carried*: they are what it says on hover, and what its row reads when a
    /// [`ButtonGroup`](super::ButtonGroup) moves it into a menu. Nothing is lost by turning them
    /// off, which is what makes this the cheapest thing a group can give up.
    ///
    /// A button with no icon ignores it — hiding the words would leave an empty box.
    #[heca_grid_ui_macros::prop]
    pub fn icon_only(mut self, on: bool) -> Self {
        Component::set_icon_only(&mut self, on);
        self
    }

    /// Set the variant.
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Set an explicit font size — overrides the inherited theme font + size scale.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.visual.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Enable or disable the glow — both the hover glow and the faint theme
    /// rest glow (`interaction.control_rest_glow`). Default: enabled.
    #[heca_grid_ui_macros::prop]
    pub fn glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Show or hide the border (default: shown for bordered variants).
    #[heca_grid_ui_macros::prop]
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.show_border = enabled;
        self
    }

    /// Set the click callback.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self.base.activatable = true; // and pickable — a letter runs this (Base::activatable)
        self
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.base.pointer.hovered
    }


    /// Border that eases from semi-opaque (rest) to solid (hover) by `p`. The
    /// stroke `width` is the theme's `border_width` (so `border_width == 0` means
    /// no border, like every other surface).
    fn animated_border(&self, c: Color, p: f32, width: f32, rest_border: f32) -> Option<Border> {
        if !self.show_border || width <= 0.0 {
            return None;
        }
        let a = rest_border + (255.0 - rest_border) * p.clamp(0.0, 1.0);
        Some(Border {
            color: c.with_alpha(a.round() as u8),
            width,
        })
    }

    /// Paint the composed content (the children) under the button's **state color**.
    ///
    /// The button never draws its content and never touches its children: it publishes one
    /// theme-derived color for this frame, and unstyled children ([`Label`]/[`Icon`]) inherit it
    /// (see [`PaintCx::with_content_color`]). Because the button repaints every frame while its
    /// hover progress eases, the value changes per frame and the content animates with it — with
    /// no knowledge of hover, and no per-child wiring. A child with its own color keeps it.
    ///
    /// A disabled button fades the content to `muted` — the one universal, theme-driven "inactive"
    /// cue that reads on every variant, including transparent Ghost/Link where a scrim would not.
    fn paint_content(&self, cx: &mut PaintCx, color: Color) {
        let color = if self.base.disabled.get_untracked() {
            cx.theme()
                .colors
                .muted
                .with_alpha((DISABLED_CONTENT_ALPHA * 255.0).round() as u8)
        } else {
            color
        };
        cx.with_content_color(color, |cx| {
            for child in &self.base.children {
                crate::component::paint_child(child.as_ref(), cx);
            }
        });
    }

    /// The content box: the bounds minus the horizontal padding — i.e. the strip the children
    /// occupy. Used by the [`Link`](ButtonVariant::Link) underline, which therefore spans the real
    /// content whatever it is, instead of a width faked from a character count.
    fn content_box(&self) -> Rectangle {
        let b = self.base.bounds;
        let pad = self.base.style.layout.padding_x.unwrap_or(self.base.style.layout.padding) as f64;
        Rectangle::new(
            Point::new(b.loc.x + pad, b.loc.y),
            Size::new((b.size.w - 2.0 * pad).max(0.0), b.size.h),
        )
    }

    /// A fill rising from the bottom by fraction `p`, with a glow (the
    /// bottom-to-top sweep).
    fn paint_rising_fill(&self, cx: &mut PaintCx, fill: Color, glow: Color, p: f32, radius: f32) {
        let b = self.base.bounds;
        let fh = b.size.h * p as f64;
        let rect = Rectangle::new(
            Point::new(b.loc.x, b.loc.y + b.size.h - fh),
            Size::new(b.size.w, fh),
        );
        let g = self.show_glow.then_some(Glow {
            color: glow,
            radius: GLOW_RADIUS,
            intensity: GLOW_INTENSITY,
        });
        cx.rect(rect, fill, None, radius, g);
    }

    /// A thin underline beneath the content, spanning the [content box](Self::content_box).
    fn paint_underline(&self, cx: &mut PaintCx, color: Color) {
        let content = self.content_box();
        let y = content.loc.y + content.size.h / 2.0 + (self.base.font as f64 * 0.5);
        cx.rect(
            Rectangle::new(
                Point::new(content.loc.x, y),
                Size::new(content.size.w, 1.5),
            ),
            color,
            None,
            0.0,
            None,
        );
    }
}

impl Button {
    /// The button's one action: flash, then run `on_click`. Every way of pressing it — pointer,
    /// keyboard, or a caller invoking [`Component::activate`] — goes through here, so they cannot
    /// drift apart and none of them has to know how the others work.
    fn fire(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_click {
            f();
        }
    }
}

impl Component for Button {
    /// A caller with only a `dyn Component` can press this button — no synthetic keypress, no
    /// focus required, because an addressed call is not an event competing for a target.
    /// Its own [`icon_only`](Self::icon_only), reachable through `dyn Component` so a container
    /// short of room can ask without knowing what its children are.
    fn set_icon_only(&mut self, on: bool) {
        let on = on && self.glyph.is_some();
        if self.icon_only == on {
            return;
        }
        self.icon_only = on;
        if on {
            // ⚠️ **Taken out of the tree, not hidden inside it.**
            //
            // `display: none` on the label leaves the button's box answered differently by the two
            // passes taffy makes — it reported its full height and was then placed as though it had
            // almost none, so the button (and anything holding it) sat low and hung out of its row.
            // Reproduced with a plain `Flex` holding one icon-only button, which is how it was found
            // (Antonio, driving, 2026-09-03). A child that is not there has no such disagreement.
            self.base.children.retain(|c| c.text_summary().is_none());
        } else if !self.label.is_empty() {
            self.base
                .children
                .push(Box::new(Label::new(self.label.clone()).bold(true)));
        }
        self.remeasure();
    }

    fn activate(&mut self) -> bool {
        self.fire();
        true
    }

    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The button **hugs its content**: `Auto` in both axes, so taffy measures whatever tree it
    /// holds (a label, an icon + label, a multi-line column) and the box follows. Only the padding
    /// and the gap are the button's own, and both derive from the resolved font + size variant, so
    /// the whole affordance scales together.
    ///
    /// Horizontal padding carries one character of breathing room per side on top of the base pad.
    /// That reproduces the old hand-computed width exactly — it was
    /// `(chars + 2) × font × advance + pad × 2`, i.e. the label plus two characters — but now the
    /// *label* is measured by the layout engine instead of by a character count, so the same
    /// spacing holds for content that isn't text at all.
    fn remeasure(&mut self) {
        let fs = self.base.font;
        let pad = BASE_PAD * self.base.size_scale();
        self.base.style.layout.padding = pad;
        // The extra horizontal room is there for **text**. With the words off it is padding around
        // nothing, which is what made a group of icon-only buttons read as too big.
        let side = if self.icon_only {
            pad
        } else {
            pad + fs * MONO_ADVANCE_RATIO
        };
        self.base.style.layout.padding_x = Some(side);
        self.base.style.layout.padding_y = Some(pad);
        self.base.style.layout.gap = fs * GAP_RATIO;
        self.base.style.layout.width = Length::Auto;
        self.base.style.layout.height = Length::Auto;
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // Snapshot theme colors so we can call &mut cx methods afterwards.
        let (
            surface,
            accent,
            glow_c,
            danger,
            foreground,
            muted,
            border_c,
            border_width,
            radius,
            on_accent,
            on_danger,
            ia,
        ) = {
            // **The hue a container published, if it published one** — a button composed inside a
            // severity-toned card follows the card, the way a `Label`'s ink already follows its
            // content colour. `None` unless a container asked, which is everywhere today, so a
            // button on its own is the theme accent exactly as before (F003/P096/T484).
            let tone = cx.control_tone();
            let t = cx.theme();
            let accent = tone.unwrap_or(t.colors.accent);
            (
                t.colors.surface,
                accent,
                tone.unwrap_or(t.colors.glow),
                // **Destructive is not re-toned.** Its colour is what the variant *means*, not
                // decoration a parent may restyle — a Delete inside a warning-toned panel is still
                // a Delete. Same reasoning that keeps `Badge::danger` red inside a coloured parent.
                t.colors.danger,
                t.colors.foreground,
                t.colors.muted,
                t.colors.border,
                t.colors.border_width,
                t.colors.control_radius(),
                // The label on a *filled* button has to contrast with the fill it actually gets,
                // which is the effective hue and not necessarily the theme accent.
                t.colors.on(accent),
                t.colors.on(t.colors.danger),
                t.colors.interaction,
            )
        };
        // A disabled button never hovers/presses: pin progress to the rest state and strip the
        // vivid accent/danger chrome to `muted` so the whole affordance — border and fill, not
        // just the label — reads as inactive on every variant.
        let disabled = self.base.disabled.get_untracked();
        let p = if disabled { 0.0 } else { self.progress.clamp(0.0, 1.0) };
        let (accent, danger) = if disabled { (muted, muted) } else { (accent, danger) };
        // An explicit tone replaces the variant's hue wherever the variant would have used the
        // accent — content, hover wash, held-on frame — without changing which variant this is.
        let accent = if disabled {
            accent
        } else {
            self.tone.unwrap_or(accent)
        };
        let b = self.base.bounds;

        // Faint theme-driven REST glow (`PaintCx::rest_glow`) on the bordered
        // variants, so a control carries the neon identity before any hover/focus and
        // the `glow_size` setting visibly scales it at rest (T011). The tone follows
        // the variant (a destructive button halos in `danger`). Ghost/Link are
        // surface-less at rest and Secondary is the deliberately-quiet variant — none
        // of them halo; a disabled control is flat.
        let base_rest = (self.show_glow && !disabled)
            .then(|| cx.rest_glow(REST_GLOW_RADIUS))
            .flatten();
        let rest_i = base_rest.map_or(0.0, |g| g.intensity);
        let rest_glow = |color: Color| base_rest.map(|g| Glow { color, ..g });

        // **Held on**, drawn once beneath whatever the variant draws — so every variant shows the
        // state the same way and a new variant cannot forget it. The tone follows the ambient
        // control tone exactly as `IconButton`'s does, and the border uses `focus_border_width` so
        // a held button stays legible with decorative borders switched off: this is a status cue,
        // not decoration.
        if self.active && !disabled {
            let tone = cx.control_tone().unwrap_or(accent);
            let line_w = cx.theme().focus_border_width;
            let frame = (line_w > 0.0).then_some(Border {
                color: tone.with_alpha(ia.control_active_border),
                width: line_w,
            });
            let g = self.show_glow.then_some(Glow {
                color: glow_c,
                radius: GLOW_RADIUS,
                intensity: GLOW_INTENSITY,
            });
            cx.rect(b, tone.with_alpha(ia.control_active_fill), frame, radius, g);
        }

        match self.variant {
            ButtonVariant::Primary => {
                cx.rect(
                    b,
                    surface,
                    self.animated_border(accent, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(glow_c),
                );
                if p > 0.0 {
                    self.paint_rising_fill(cx, accent, glow_c, p, radius);
                }
                self.paint_content(cx, accent.lerp(on_accent, p));
            }
            ButtonVariant::Destructive => {
                cx.rect(
                    b,
                    surface,
                    self.animated_border(danger, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(danger),
                );
                if p > 0.0 {
                    let g = self.show_glow.then_some(Glow {
                        color: danger,
                        radius: GLOW_RADIUS,
                        intensity: GLOW_INTENSITY * p,
                    });
                    cx.rect(b, danger.with_alpha(alpha(p)), None, radius, g);
                }
                self.paint_content(cx, danger.lerp(on_danger, p));
            }
            ButtonVariant::Secondary => {
                // Subtle `muted` fill + `muted` border (both firm toward `foreground` on hover) —
                // a theme-consistent identity. `muted` always contrasts the background and is never
                // equal to `surface`/`border`, unlike the old `surface` fill + `theme.border` border
                // (which vanished on themes where surface≈background or border==surface).
                let bc = muted.lerp(foreground, 0.4 * p);
                cx.rect(
                    b,
                    muted.with_alpha(SECONDARY_FILL_ALPHA),
                    self.animated_border(bc, p, border_width, ia.control_rest_border as f32),
                    radius,
                    rest_glow(glow_c),
                );
                self.paint_content(cx, foreground);
            }
            ButtonVariant::Outline => {
                // Hover: vivid accent border, text → primary (accent), lightest glow.
                // At rest the theme rest-glow carries the halo (tight radius); hover
                // blends both radius and intensity over it as `p` rises.
                let fill = accent.with_alpha(alpha(p * 0.1));
                let g = self.show_glow.then_some(Glow {
                    color: glow_c,
                    radius: REST_GLOW_RADIUS + (GLOW_RADIUS * 0.8 - REST_GLOW_RADIUS) * p,
                    intensity: (GLOW_INTENSITY * 0.6 * p).max(rest_i),
                });
                let g = g.filter(|g| g.intensity > 0.0);
                cx.rect(
                    b,
                    fill,
                    self.animated_border(muted.lerp(accent, p), p, border_width, ia.control_rest_border as f32),
                    radius,
                    g,
                );
                self.paint_content(cx, muted.lerp(accent, p));
            }
            ButtonVariant::Ghost => {
                let border = if self.show_border && p > 0.0 && border_width > 0.0 {
                    Some(Border {
                        color: border_c.with_alpha(alpha(p)),
                        width: border_width,
                    })
                } else {
                    None
                };
                // Surface-less at rest (no halo), but the hover surface + border that
                // fade in glow with the fade — like every other hovered control.
                let g = (self.show_glow && !disabled && p > 0.0).then_some(Glow {
                    color: glow_c,
                    radius: GLOW_RADIUS,
                    intensity: GLOW_INTENSITY * p,
                });
                cx.rect(b, surface.with_alpha(alpha(p)), border, radius, g);
                self.paint_content(cx, muted.lerp(foreground, p));
            }
            ButtonVariant::Link => {
                // Color stays constant on hover; press flashes the TEXT (no bg).
                self.paint_content(cx, accent.lerp(foreground, self.flash.amount() * 0.7));
                if p > 0.0 {
                    self.paint_underline(cx, accent.with_alpha(alpha(p)));
                }
            }
        }

        // Press flash — brightening overlay (Link flashes its text above instead).
        // Filled variants need a stronger flash to read over their bright fill.
        if !matches!(self.variant, ButtonVariant::Link) {
            let strength = match self.variant {
                ButtonVariant::Primary | ButtonVariant::Destructive => 0.95,
                _ => 0.6,
            };
            cx.flash(b, self.flash.amount() * strength, radius);
        }

        // No background scrim for the disabled state: it barely shows on transparent variants
        // (Ghost/Link) and keeps the content's bright hue. Instead the muted chrome above +
        // the faded `muted` content color (see `paint_content`) carry the disabled look on
        // every variant.

        // Focus ring — shown whenever the button is focused (not keyboard-only) and enabled. It
        // follows the button's own tone (a destructive button rings in `danger`, not `accent`) so
        // the focus cue matches the widget's border colour instead of clashing with it.
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            // Focus-outline tone (theme-driven, light/dark-aware): the accent case uses the
            // theme's `focus_ring` token or the accent shifted toward `foreground`; a destructive
            // button derives the same shift from its own `danger` tone. Both stay distinct from the
            // widget's border on dark AND light themes.
            let ring = match self.variant {
                ButtonVariant::Destructive => cx.theme().colors.focus_ring_tone(danger),
                _ => cx.theme().colors.effective_focus_ring(),
            };
            cx.focus_ring(b, ring, radius);
        }
    }

    /// Capture, not bubble: this control is **one click target and one Tab stop**
    /// (`Base::focus_barrier`), so its composed content — an `Icon`, a `Label`, anything — must
    /// never see the press first. Handling it before the children is what keeps that true.
    ///
    /// It takes the press and **fires on the click**: the press captures the pointer (so the
    /// release comes here wherever the cursor went) and the click is what a press and a release on
    /// this button *mean*. Dragging off the button before letting go therefore cancels it, which
    /// is what every other control on the machine does — and it costs nothing here, because the
    /// pairing is the framework's.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // Keyboard activation: Space/Enter on the focused button == a click. `dispatch` only
            // offers a raw key to the widget that owns the keyboard, so there is no focus check
            // here to forget — and no way for this button to take a key meant for something else.
            Event::Key {
                key: GridKey::Enter | GridKey::Space,
                pressed: true,
            } => {
                self.fire();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// **The click, after its children have declined it.**
    ///
    /// The press is taken in capture (so composed content can never take it first) and the click
    /// it turns into is delivered to whoever took that press — this control — which is what makes
    /// "one control, one click target" a framework rule rather than something each control
    /// arranges by swallowing events. Bubble, not capture, so an
    /// [`ComponentExt`](crate::builders::ComponentExt) handler registered on this widget gets first
    /// refusal and can take the click with `stop_propagation`.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::Click(_) => {
                self.fire();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;

        // Hover progress eases toward the hovered target.
        let target = if self.base.hovered() && !self.base.disabled.get_untracked() {
            1.0
        } else {
            0.0
        };
        if (self.progress - target).abs() >= 1e-3 {
            let step = dt / HOVER_DURATION;
            self.progress = if self.progress < target {
                (self.progress + step).min(target)
            } else {
                (self.progress - step).max(target)
            };
            animating = true;
        } else {
            self.progress = target;
        }

        // Press flash fades out.
        animating |= self.flash.tick(dt);

        // Composed content animates itself (e.g. a Spinner child): the button drives its own
        // hover/press, the children drive theirs. (The *color* of the content needs no ticking —
        // it is inherited at paint time; see `paint_content`.)
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }

        // Damage just our own rect each frame so the hover/press animation doesn't
        // force a whole-scene redraw (host safety net). Mirrors `Spinner`.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Button {}
/// Content is children: `.child(..)` appends any component (sugar like [`Button::new`] /
/// [`Button::icon`] builds those same children).
impl Parent for Button {}
