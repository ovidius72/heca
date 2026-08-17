//! [`KeyHint`] — a **generic keyboard-hint overlay** for any actionable component.
//!
//! Many chrome interactions are *keyboard-centric picks*: a prefix (move / swap /
//! focus-select, a jump mode, a command palette) lights up **letters over the
//! things you can act on**, and you press the letter to choose one. That overlay
//! is the same everywhere — a glowing keycap drawn on top of a target — so it
//! lives here as a reusable wrapper rather than being baked into any one widget
//! (rail icon cells, content-area panes, list rows, palette items all reuse it).
//!
//! `KeyHint` wraps a single child and is **transparent**: it isn't focusable and
//! it routes events/ticks straight through (the default [`Component`] container
//! behaviour), so the wrapped widget stays clickable/focusable exactly as before.
//! It only *adds paint*: while its host-owned [`Signal<Option<String>>`] is
//! `Some`, it stamps that text as a keycap over the child; when `None`, it paints
//! nothing extra. Because the signal is written by the **host**, mouse, keyboard,
//! and RPC all drive it identically (the chrome plan's read-via-signals /
//! write-via-actions rule) — matching `heca`'s existing `candidates` flow.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::{Direction, Length};
use heca_core::layout::{Point, Rectangle, Size};

/// Where the keycap sits over the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum HintPlacement {
    /// Centered along the top edge — for compact square targets (rail icon cells).
    #[default]
    TopCenter,
    /// Centered over the whole target — for large targets (content-area panes).
    Center,
    /// Vertically centered, pinned to the **right edge** — for wide list rows
    /// (sidebar pane/column cards) where a right-aligned keycap keeps the row's
    /// label readable.
    CenterRight,
    /// Pinned to the **top-right** — for tall targets (e.g. a workspace dock) whose
    /// header row sits at the top: right-aligned like a list row, but anchored to the
    /// top edge rather than the target's vertical center. Pair with
    /// [`offset_y`](KeyHint::offset_y) to drop it onto the header line.
    TopRight,
    /// Pinned just inside the **top-left**, centred within a shallow band from the target's top
    /// edge — for a large target (a card in the exposé, a content pane) where the letter should
    /// stay out of the way of what the target shows and never sit on its border.
    ///
    /// This is what the universal picker drew for every large target before the letters became the
    /// widget's own to place, and it is the reason it exists as a variant: the rule was real, it
    /// just lived in a host paint pass where no call site could ask for it (F003/P082/T427).
    TopLeft,
}

/// **How a widget's hint keycap is drawn.**
///
/// These four knobs were private fields on [`KeyHint`], which is why a letter could only be drawn
/// by wrapping a widget in one. They live on [`Base`] now, so the framework draws the cap for *any*
/// widget carrying a letter and a widget places its own (F003/P082/T431).
#[derive(Clone, Copy, Debug, Default)]
pub struct HintStyle {
    /// Where the cap sits over the target.
    pub placement: HintPlacement,
    /// Explicit cap font size (logical px); otherwise derived from the resolved font.
    pub size: Option<f32>,
    /// Cap colour override; defaults to the theme `accent`, glow included.
    pub color: Option<Color>,
    /// Extra vertical nudge applied after placement — positive moves it down.
    pub offset_y: f64,
}

/// Keycap font size as a fraction of the wrapped component's resolved font.
const HINT_FONT_MUL: f32 = 1.05;
/// Horizontal / vertical padding inside the keycap, in fractions of the hint font.
const PAD_X_FRAC: f32 = 0.42;
const PAD_Y_FRAC: f32 = 0.22;
/// Inset of a `TopCenter` keycap from the target's top edge (logical px).
const TOP_INSET: f64 = 2.0;
/// Inset of a `CenterRight` keycap from the target's right edge (logical px).
const RIGHT_INSET: f64 = 6.0;
/// Keycap font for a **compact** target, as a multiple of the inherited font — smaller, so the chip
/// stays proportional to the little button it captions.
const HINT_COMPACT_MUL: f32 = 0.85;
/// A target no bigger than this on **both** axes cannot hold a cap without hiding its own content,
/// so the cap goes beside it. In units of the inherited font, so it follows a font change.
const HINT_COMPACT_MAX_MUL: f32 = 2.4;
/// The air between a compact target and its caption.
const HINT_ADJACENT_GAP: f64 = 2.0;
/// The band from a large target's top edge that a [`HintPlacement::TopLeft`] cap is centred in, in
/// units of the inherited font — roughly one list row, so a tall card gets its letter on the top
/// line rather than floating in the middle of the picture.
const HINT_BAND_MUL: f32 = 2.8;
/// Inset from a target's left edge, so the cap sits just inside it rather than on its border.
const LEFT_INSET: f64 = 2.0;
/// Per-glyph advance estimate (fraction of font) for sizing the keycap to its text.
const GLYPH_ADVANCE_FRAC: f32 = 0.62;
/// Keycap glow intensity (scaled by the theme `glow_size`) — soft, not blazing.
const KEYCAP_GLOW: f32 = 0.45;

/// Size of the keycap chip for `text` at `font` (logical px) — the exact sizing
/// [`KeyHint`] uses. Exposed so hosts can stamp a standalone keycap over targets
/// that are not part of a component tree (e.g. terminal hyperlink spans) without
/// duplicating the formula.
pub fn keycap_size(font: f32, text: &str) -> Size {
    keycap_size_cells(font, text.chars().count().max(1))
}

/// [`keycap_size`] for a **Nerd Font** keycap — one square cell, whatever the glyph.
pub fn keycap_size_nf(font: f32) -> Size {
    keycap_size_cells(font, 1)
}

/// The chip size for `cells` character cells of content — the one formula both callers share.
fn keycap_size_cells(font: f32, cells: usize) -> Size {
    let font64 = font as f64;
    let pad_x = (font * PAD_X_FRAC) as f64;
    let pad_y = (font * PAD_Y_FRAC) as f64;
    let w =
        (cells as f64 * (font * GLYPH_ADVANCE_FRAC) as f64 + 2.0 * pad_x).max(font64 + 2.0 * pad_y);
    let h = font64 + 2.0 * pad_y;
    Size::new(w, h)
}

/// Visual style of a keycap chip painted by [`paint_keycap`]. The caller picks a
/// semantic variant; [`paint_keycap`] owns the styling (all values from the theme).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeycapVariant {
    /// Solid glowing chip — opaque base + accent tint + dark glyph. The default
    /// hint/pick look, stamped over arbitrary content (terminal hyperlink spans,
    /// pick targets), where the chip must stay legible against whatever is beneath
    /// it.
    #[default]
    Filled,
    /// A quiet [`Tag`](super::Tag)-style chip: subtle fill + accent border, **no glow**,
    /// accent glyph. For keycaps shown on an already dark, host-owned surface (e.g. the
    /// context-menu quick-pick inside the menu panel) where a glowing halo would be too
    /// heavy and a crisp outline reads better.
    Bordered,
}

/// Paint a standalone keycap — a glowing chip with a centered dark glyph — at `cap`
/// with `text`. The shared hint visual, factored out of [`KeyHint`] so it can be
/// stamped directly into a scene over targets that are not widgets (e.g. terminal
/// hyperlink spans, via the host's overlay pass) or reused by other overlays (the
/// context-menu quick-pick). `color` overrides the default theme `accent` for both
/// fill and glow; `variant` picks [`KeycapVariant::Filled`] (content overlays) or
/// [`KeycapVariant::Bordered`] (keycaps on an owned menu/panel surface).
pub fn paint_keycap(
    cx: &mut PaintCx,
    cap: Rectangle,
    text: &str,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    if text.is_empty() {
        return;
    }
    paint_keycap_content(cx, cap, KeycapContent::Text(text), font, color, variant);
}

/// [`paint_keycap`] for a **Nerd Font** glyph — a shift/command/escape key drawn as its own picture
/// rather than spelled out (`NfGlyph`).
///
/// A separate entry point rather than a `&str` a caller could pass a glyph char in: the codepoint
/// has to be shaped with the Nerd Font family, and that is a property of the *run*, not of the
/// string. Passing the char to [`paint_keycap`] would shape it with the UI face, which does not have
/// it — a silent empty box.
pub fn paint_keycap_nf(
    cx: &mut PaintCx,
    cap: Rectangle,
    glyph: super::NfGlyph,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    paint_keycap_content(cx, cap, KeycapContent::Nf(glyph), font, color, variant);
}

/// What is drawn inside a keycap chip: a string in the UI face, or a Nerd Font glyph.
#[derive(Clone, Copy)]
enum KeycapContent<'a> {
    Text(&'a str),
    Nf(super::NfGlyph),
}

impl KeycapContent<'_> {
    /// Draw the content centered in `cap`, in the face it belongs to.
    fn paint(self, cx: &mut PaintCx, cap: Rectangle, color: Color, font: f32) {
        match self {
            KeycapContent::Text(t) => {
                cx.text(cap, t, color, font, TextAlign::Center, TextStyle::BOLD)
            }
            KeycapContent::Nf(g) => {
                if let Some(c) = g.char() {
                    cx.nf_icon(cap, &c.to_string(), color, font);
                }
            }
        }
    }

}

/// One cap of a keyboard chord: a **Nerd Font key glyph** where the key has a picture (shift,
/// command, escape, the arrows), the literal text where it does not (`h`, `]`, `F5`).
///
/// Two cases and not one, because the two are drawn in different faces — see [`paint_keycap_nf`].
/// A caller builds a chord as a `Vec<KeyCap>` (`[λ] [⇧] [e]`) and the widget draws the chips.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyCap {
    /// A key with a glyph in the embedded Nerd Font.
    Nf(super::NfGlyph),
    /// A key spelled out — a letter, a digit, a punctuation key, or a named key with no glyph.
    Text(String),
}

impl KeyCap {
    /// The chip size for this cap at `font`.
    pub fn size(&self, font: f32) -> Size {
        match self {
            KeyCap::Nf(_) => keycap_size_nf(font),
            KeyCap::Text(t) => keycap_size(font, t),
        }
    }

    /// Draw this cap at `cap`, in the face it belongs to.
    pub fn paint(
        &self,
        cx: &mut PaintCx,
        cap: Rectangle,
        font: f32,
        color: Option<Color>,
        variant: KeycapVariant,
    ) {
        match self {
            KeyCap::Nf(g) => paint_keycap_nf(cx, cap, *g, font, color, variant),
            KeyCap::Text(t) => paint_keycap(cx, cap, t, font, color, variant),
        }
    }
}

/// The shared chip: the surface (per [`KeycapVariant`]) plus whatever sits in it.
fn paint_keycap_content(
    cx: &mut PaintCx,
    cap: Rectangle,
    content: KeycapContent<'_>,
    font: f32,
    color: Option<Color>,
    variant: KeycapVariant,
) {
    let (accent, glow_c, background, ctrl_radius, keycap_alpha) = {
        let t = cx.theme();
        (
            t.colors.accent,
            t.colors.glow,
            t.colors.background,
            t.colors.control_radius(),
            t.colors.interaction.keycap,
        )
    };
    let keycap_c = color.unwrap_or(accent);
    let radius = ctrl_radius.min((cap.size.h / 2.0) as f32);
    match variant {
        KeycapVariant::Filled => {
            let keycap_glow = color.unwrap_or(glow_c);
            // Opaque base (carrying the glow) so the chip never lets underlying content bleed
            // through — a keycap stamped over an icon/glyph (e.g. a drag handle or toolbar icon)
            // must stay legible, not show a ghost of what's beneath it. Over a dark surface this
            // matches the old translucent look; over content it hides it.
            cx.rect(
                cap,
                background,
                None,
                radius,
                Some(Glow {
                    color: keycap_glow,
                    radius: 6.0,
                    intensity: KEYCAP_GLOW,
                }),
            );
            // Accent tint on top of the opaque base, then the dark bold glyph for contrast.
            cx.rect(cap, keycap_c.with_alpha(keycap_alpha), None, radius, None);
            content.paint(cx, cap, background, font);
        }
        KeycapVariant::Bordered => {
            // Outline-only chip: **no fill** (empty interior) + a full-strength **accent** border
            // (the `keycap_c` colour at full alpha, so it reads as the theme accent — not a washed
            // tint), no glow, accent glyph. Matches the showcase KeyHint (no background). For
            // keycaps on an already-dark owned surface (a menu panel).
            let border = cx.border(keycap_c);
            cx.rect(cap, Color::TRANSPARENT, border, radius, None);
            content.paint(cx, cap, keycap_c, font);
        }
    }
}

/// A transparent wrapper that carries a hint letter on behalf of a **region** — a group of widgets,
/// or something that is not a widget you can put a builder on.
///
/// It no longer *draws* the letter: [`paint_child`](crate::component::paint_child) does that for
/// every widget carrying one, so a widget can be pickable on its own and this is a decorator you
/// reach for when there is nothing to hang the declaration on (F003/P082/T431).
pub struct KeyHint {
    base: Base,
}

#[heca_grid_ui_macros::props]
impl KeyHint {

    /// **What a pick of this letter does.**
    ///
    /// ```
    /// use heca_grid_ui::prelude::*;
    /// use heca_grid_ui::widgets::KeyHint;
    ///
    /// # let row_id = 7u64;
    /// # fn cursor_to(_: u64) {}
    /// let row = KeyHint::new(Row::new().child(Label::new("nvim")))
    ///     .on_hint(move || cursor_to(row_id));
    /// ```
    ///
    /// **It goes on the wrapper, not on every widget.** Being pickable is something you opt a
    /// region into — you were already wrapping it to show the letter — so `Label::on_hint` is a
    /// method that never has to exist, and you can read off the tree what is reachable. (A context
    /// menu is the other shape on purpose: a menu is *about* a widget, so it is a slot any widget
    /// carries; a hint is *aimed at* a region you chose to make reachable.)
    ///
    /// **It replaces an id and a registry.** A hint target used to be `hints.register(intent)`
    /// followed by `.hint_target(id)` — three things a caller had to know (that a registry exists,
    /// that they must pre-register, and a host-private intent type), and a plugin could construct
    /// none of them. That made `prefix+/` a shipped feature a plugin could only have a
    /// second-class version of, which RULE ZERO in `AGENTS.md` forbids. The framework collects
    /// these out of the tree ([`hint::collect_hints`](crate::hint::collect_hints)), hands out the
    /// letters, draws them, and runs this one when it is picked.
    ///
    /// **A pick is not a click.** They are different gestures and a region may answer them
    /// differently: heca's sidebar row activates the pane on a click and *stays in the sidebar* on
    /// a hint. Pointing one intent at both is what made `prefix+/` leave the sidebar.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a closure")]
    pub fn on_hint(mut self, f: impl Fn() + 'static) -> Self {
        self.base.hint = Some(Box::new(f));
        self
    }
    /// Wrap `child`. Bind the hint text with [`hint`](KeyHint::hint).
    pub fn new(child: impl Component + 'static) -> Self {
        Self::wrap(Box::new(child))
    }

    /// Wrap an **already-boxed** subtree — what a dynamically built tree is (a chrome provider's
    /// render seam, `realize` output), where the concrete widget type is not known at the call
    /// site. Mirrors [`Parent::child_boxed`](crate::builders::Parent::child_boxed).
    pub fn new_boxed(child: Box<dyn Component>) -> Self {
        Self::wrap(child)
    }

    fn wrap(child: Box<dyn Component>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (overlay positions off them).
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        // Column direction so the single child stretches to the wrapper's full width
        // (cross-axis, default `Align::Stretch`). This keeps the wrapper transparent
        // to a stretching parent: a wide list row fills its column instead of
        // shrinking to content width, while a hugged square target is unaffected.
        base.style.layout.direction = Direction::Column;
        base.children.push(child);
        Self { base }
    }

    /// Bind the **host-owned** hint signal. The app sets `Some(letter)` when a
    /// pick/jump mode opens (from key **or** RPC) and clears it on exit.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn hint(mut self, hint: Signal<Option<String>>) -> Self {
        self.base.hint_label = hint;
        self
    }

    /// The hint signal (e.g. to set/clear it directly).
    ///
    /// It **is** [`Base::hint_label`], not a second signal beside it: the universal picker offers a
    /// letter through the base slot and a host mode (pane-select, swap) sets the same one, so the
    /// two cannot show different letters over one target.
    pub fn hint_signal(&self) -> Signal<Option<String>> {
        self.base.hint_label
    }

    /// Where the keycap sits over the target (default [`HintPlacement::TopCenter`]).
    #[heca_grid_ui_macros::prop]
    pub fn placement(mut self, placement: HintPlacement) -> Self {
        self.base.hint_style.placement = placement;
        self
    }

    /// Explicit keycap font size in logical px (overrides the font-derived size).
    #[heca_grid_ui_macros::prop]
    pub fn size(mut self, px: f32) -> Self {
        self.base.hint_style.size = Some(px);
        self
    }

    /// Override the keycap color (default: theme `accent`). The glow follows it too.
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, c: Color) -> Self {
        self.base.hint_style.color = Some(c);
        self
    }

    /// Nudge the keycap down by `px` logical pixels after placement (positive = down).
    /// Use it to drop a `TopCenter` cap from a tall target's top edge onto its header
    /// row (e.g. align with a workspace dock's title).
    #[heca_grid_ui_macros::prop]
    pub fn offset_y(mut self, px: f64) -> Self {
        self.base.hint_style.offset_y = px;
        self
    }

}

/// The cap font for a target of `bounds` at inherited `font`, under `style`.
fn hint_font(font: f32, style: &HintStyle, bounds: Rectangle) -> f32 {
    if style.size.is_none() && is_compact(bounds, font) {
        // A small target gets a small cap, or the chip is wider than the thing it labels.
        return font * HINT_COMPACT_MUL;
    }
    style.size.unwrap_or(font * HINT_FONT_MUL)
}

/// **Is this target too small to hold a keycap without hiding what it labels?**
///
/// An icon button is; a card, a row or a dock is not. The threshold is in units of the
/// inherited font rather than pixels, so it follows a font-size change like everything else.
///
/// The compact case is the *widget's* business, and that is the point: it used to live in a host
/// paint pass that measured every target's bounds and chose a placement for it. A host deciding how
/// a widget it has never seen should look is the same mistake as a host deciding where that widget
/// paints (F003/P082/T427).
fn is_compact(bounds: Rectangle, font: f32) -> bool {
    let max = (font * HINT_COMPACT_MAX_MUL) as f64;
    bounds.size.w > 0.0 && bounds.size.w <= max && bounds.size.h <= max
}

/// The keycap rect for `text` within the target `b`, per `style`.
fn keycap_rect(b: Rectangle, font: f32, style: &HintStyle, text: &str, viewport: Size) -> Rectangle {
    let Size { w, h } = keycap_size(hint_font(font, style, b), text);
    // **A target too small to cover goes beside it, not over it.** Centred just below, so the
    // icon stays fully visible with its letter as a caption; flipped above when below would
    // fall off the bottom of the viewport.
    if style.size.is_none() && is_compact(b, font) {
        let x = b.loc.x + (b.size.w - w) / 2.0;
        let below = b.loc.y + b.size.h + HINT_ADJACENT_GAP;
        let y = if below + h <= viewport.h {
            below
        } else {
            b.loc.y - h - HINT_ADJACENT_GAP
        };
        return Rectangle::new(Point::new(x, y + style.offset_y), Size::new(w, h));
    }
    let (x, y) = match style.placement {
        HintPlacement::TopCenter => (b.loc.x + (b.size.w - w) / 2.0, b.loc.y + TOP_INSET),
        HintPlacement::Center => (
            b.loc.x + (b.size.w - w) / 2.0,
            b.loc.y + (b.size.h - h) / 2.0,
        ),
        HintPlacement::CenterRight => (
            b.loc.x + b.size.w - w - RIGHT_INSET,
            b.loc.y + (b.size.h - h) / 2.0,
        ),
        HintPlacement::TopRight => (b.loc.x + b.size.w - w - RIGHT_INSET, b.loc.y + TOP_INSET),
        HintPlacement::TopLeft => {
            // Centred within a shallow band from the top edge, so a tall card gets its letter
            // on the top line rather than floating in the middle of the picture.
            let band = b.size.h.min((font * HINT_BAND_MUL) as f64);
            (b.loc.x + LEFT_INSET, b.loc.y + (band - h) / 2.0)
        }
    };
    Rectangle::new(Point::new(x, y + style.offset_y), Size::new(w, h))
}

/// **Draw `c`'s hint letter, if it is carrying one.** Called by
/// [`paint_child`](crate::component::paint_child) for *every* widget, right after the widget has
/// painted itself — which is what lets any widget be pickable without being wrapped in a
/// [`KeyHint`] (F003/P082/T431).
///
/// The two rules that were learned the hard way, and are the reason this is one function rather
/// than something each widget does:
///
/// - **The viewport is the PAINT context's**, not `Base::viewport`. The latter is written per
///   *tree* by the layout pass, so inside a pane header it is the header's own box — a few dozen
///   pixels tall. Reading it there made every compact cap "not fit below", flip above, and get
///   clipped by the pane frame (Antonio, driving, 2026-08-14).
/// - **Into the OVERLAY band**, so nothing painted after this widget covers its letter. Drawing it
///   inline puts the cap at this widget's position in the paint order and every sibling drawn later
///   sits on top: a top-bar button's letter disappeared under the sidebar frame beside it, and a
///   card's under the next card (same session). `with_overlay` keeps the cap in **this widget's
///   own scene**, nested exactly as deep as the widget is — the property the whole fix rests on,
///   since a plugin's overlay-nested picker then lands above its own content with nothing
///   host-side to teach.
pub(crate) fn paint_hint_label(c: &dyn Component, cx: &mut PaintCx) {
    let base = c.base();
    if !base.visible.get_untracked() {
        return;
    }
    let Some(text) = base.hint_label.get_untracked() else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let style = base.hint_style;
    let cap = keycap_rect(base.bounds, base.font, &style, &text, cx.viewport());
    let font = hint_font(base.font, &style, base.bounds);
    cx.with_overlay(|cx| {
        paint_keycap(cx, cap, &text, font, style.color, KeycapVariant::Filled);
    });
}

impl Component for KeyHint {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Just the wrapped, still-interactive child. **The letter is not drawn here** — `paint_child`
    /// draws it for every widget that carries one, this wrapper included, so being pickable stopped
    /// depending on being wrapped (F003/P082/T431).
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for KeyHint {}
impl StyleExt for KeyHint {}
impl Parent for KeyHint {}

#[cfg(test)]
mod tests {
    use super::keycap_size;

    #[test]
    fn keycap_size_is_positive_and_grows_with_text() {
        let one = keycap_size(13.0, "a");
        assert!(one.w > 0.0 && one.h > 0.0);
        // A wider label needs a wider chip; height is text-length independent.
        let many = keycap_size(13.0, "abc");
        assert!(many.w > one.w);
        assert_eq!(many.h, one.h);
    }

    #[test]
    fn keycap_size_scales_with_font() {
        let small = keycap_size(10.0, "a");
        let large = keycap_size(20.0, "a");
        assert!(large.w > small.w);
        assert!(large.h > small.h);
    }
}
