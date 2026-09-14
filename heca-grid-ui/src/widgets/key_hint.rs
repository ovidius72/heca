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
    /// **What the cap MEANS, left for the theme to colour.** Overridden by
    /// [`color`](Self::color) when both are set.
    ///
    /// A literal colour cannot be written by a widget that has no theme at build time — which is
    /// every widget, since the theme arrives at paint. So a widget names a tone and the theme
    /// decides the pixels, which is the same rule every other colour in the library follows.
    pub tone: Option<HintTone>,
    /// Extra vertical nudge applied after placement — positive moves it down.
    pub offset_y: f64,
}

/// **What a keycap means**, so the theme can colour it.
///
/// heca reads them: a pane is `Accent`, a workspace `Warning`, a column `Success`, and a
/// structural control — fold this, close that — is `Muted`, because it is not somewhere to go.
/// A caller names the meaning; which pixels that is stays the theme's business and follows a
/// reload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, heca_grid_ui_macros::PropName)]
pub enum HintTone {
    /// The default weight — a target you would navigate to.
    Accent,
    /// A structural control rather than a destination: a fold, a close, a handle.
    Muted,
    /// Reserved for a distinct class of target, so two kinds never read alike.
    Warning,
    /// As `Warning`, a third class.
    Success,
    /// Something destructive.
    Danger,
}

impl HintTone {
    /// The colour, from the theme this frame.
    pub fn resolve(self, theme: &crate::theme::Theme) -> Color {
        match self {
            HintTone::Accent => theme.colors.accent,
            HintTone::Muted => theme.colors.muted,
            HintTone::Warning => theme.colors.warning,
            HintTone::Success => theme.colors.success,
            HintTone::Danger => theme.colors.danger,
        }
    }
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
/// How far a keycap's chip must sit from the theme `background` in luminance for a
/// background-coloured letter to still read on it. Below this the letter takes the contrasting tone
/// instead — see [`keycap_glyph_color`].
const KEYCAP_MIN_GLYPH_CONTRAST: f32 = 0.15;

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
/// **The letter's colour on a filled keycap** — the theme `background`, unless the chip is too close
/// to it to read, in which case the contrasting tone.
///
/// The chip is not `tint`: it is an opaque `background` base with `tint` laid over it at `alpha`, so
/// the fill the eye sees is the blend. Asking about `tint` alone would answer for a colour that is
/// never painted.
///
/// **Why not simply [`Theme::on`] every time.** That is the more contrasty answer and it would
/// change how heca looks: measured on `grid_tron`, the chip lands at luminance 0.24 against a 0.003
/// background and a 0.82 foreground, so `on` returns the *foreground* and the letters would flip
/// from dark-on-accent to light-on-accent. The dark glyph is the design; this only steps in when
/// keeping it would make the letter vanish.
///
/// **Why it steps in at all.** The glyph used to be `background` unconditionally, which is right for
/// an accent far from the background and wrong the moment it is not — a light-background theme had a
/// light glyph on a light chip. `hint_color` then made that reachable on any theme: set the letters
/// near your own background and you get an empty keycap. A colour a user is free to choose cannot
/// carry a legibility rule that only holds for one of its values.
fn keycap_glyph_color(theme: &heca_theme::Theme, tint: Color, alpha: u8) -> Color {
    let chip = theme.background.lerp(tint, alpha as f32 / 255.0);
    let reads_on_background =
        (chip.luminance() - theme.background.luminance()).abs() >= KEYCAP_MIN_GLYPH_CONTRAST;
    if reads_on_background {
        theme.background
    } else {
        theme.on(chip)
    }
}

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
            // Tint on top of the opaque base, then the glyph in whichever of the theme's
            // background/foreground contrasts with the chip — see [`keycap_glyph_color`].
            cx.rect(cap, keycap_c.with_alpha(keycap_alpha), None, radius, None);
            let glyph = {
                let t = cx.theme();
                keycap_glyph_color(&t.colors, keycap_c, keycap_alpha)
            };
            content.paint(cx, cap, glyph, font);
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

    // `on_hint` is **not here any more** (F003/P082/T432). It is
    // [`ComponentExt::on_hint`](crate::builders::ComponentExt::on_hint), on every widget — so a
    // `KeyHint::new(row).on_hint(…)` call still reads exactly the same, and a widget that can carry
    // the declaration itself no longer has to be wrapped to say what a pick does to it.
    //
    // Having it here put the two facts about a target on two different nodes: a row named itself
    // and the wrapper around it carried the pick, while a mounted dock named itself outside and
    // declared the pick within. Which one was on top depended on how the tree was built, so the
    // code matching a letter to an action had to search both up and down — and searching one way
    // only is why sidebar letters kept failing with no error.

    /// Wrap `child`. Bind the hint text with [`hint`](KeyHint::hint).
    pub fn new(child: impl crate::builders::IntoComponent) -> Self {
        Self::wrap(child.into_component())
    }

    fn wrap(child: Box<dyn Component>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (overlay positions off them).
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        // …and transparent to layout as well, or a child sized as a share resolves it against
        // this wrapper and quietly becomes its content size instead.
        crate::component::wrap_transparently(&mut base, child.as_ref());
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

    // ── The four cap knobs ────────────────────────────────────────────────────────────────
    //
    // These are the **wrapper's spelling** of capabilities that live on every widget
    // (`ComponentExt::hint_placement` and friends): the slot they write, `Base::hint_style`, has
    // always been universal, and since F003/P097/T501 so has the way to set it. They delegate
    // rather than repeat the assignment, so there is one writer per field and the two spellings
    // cannot drift — the same arrangement `Tooltip` has with `ComponentExt::tooltip`.
    //
    // Kept because the wrapper itself is kept: for a region that is not a widget you can put a
    // builder on.

    /// Where the keycap sits over the target (default [`HintPlacement::TopCenter`]).
    #[heca_grid_ui_macros::prop]
    pub fn placement(self, placement: HintPlacement) -> Self {
        crate::builders::ComponentExt::hint_placement(self, placement)
    }

    /// Explicit keycap font size in logical px (overrides the font-derived size).
    #[heca_grid_ui_macros::prop]
    pub fn size(self, px: f32) -> Self {
        crate::builders::ComponentExt::hint_size(self, px)
    }

    /// Override the keycap color (default: theme `accent`). The glow follows it too.
    #[heca_grid_ui_macros::prop]
    pub fn color(self, c: Color) -> Self {
        crate::builders::ComponentExt::hint_color(self, c)
    }

    /// Nudge the keycap down by `px` logical pixels after placement (positive = down).
    /// Use it to drop a `TopCenter` cap from a tall target's top edge onto its header
    /// row (e.g. align with a workspace dock's title).
    #[heca_grid_ui_macros::prop]
    pub fn offset_y(self, px: f64) -> Self {
        crate::builders::ComponentExt::hint_offset_y(self, px)
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

/// The keycap rect for `text` within the target `b`, per `style`, **and the rect the cap is
/// allowed to live in** — what [`fit_into_view`] clamps against.
///
/// For every in-target placement the permitted rect is `b` itself. For a **compact** target the
/// cap is placed *outside* `b` on purpose (a caption under the icon, flipped above near the bottom
/// edge), so its permitted rect is `b` grown by the caption band above and below. The branch that
/// chose the placement is the one that knows where the cap may sit — [`fit_into_view`] must not
/// re-derive it from `b` alone, or it drags the caption back onto the icon it captions.
fn keycap_rect(
    b: Rectangle,
    font: f32,
    style: &HintStyle,
    text: &str,
    viewport: Size,
) -> (Rectangle, Rectangle) {
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
        let cap = Rectangle::new(Point::new(x, y + style.offset_y), Size::new(w, h));
        // The permitted area: the target plus a caption band a full cap tall above and below it,
        // unioned with the cap itself so an `offset_y` nudge is never clamped away. `fit_into_view`
        // keeps the cap within *this*, not within `b` — the beside-placement stands.
        let px = b.loc.x.min(cap.loc.x);
        let py = (b.loc.y - h - HINT_ADJACENT_GAP).min(cap.loc.y);
        let pright = (b.loc.x + b.size.w).max(cap.loc.x + w);
        let pbottom = (b.loc.y + b.size.h + HINT_ADJACENT_GAP + h).max(cap.loc.y + h);
        let permitted = Rectangle::new(Point::new(px, py), Size::new(pright - px, pbottom - py));
        return (cap, permitted);
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
    (
        Rectangle::new(Point::new(x, y + style.offset_y), Size::new(w, h)),
        b,
    )
}

/// **Keep the cap inside the part of its row you can actually see.**
///
/// A row half past a sidebar's fold keeps its letter — you can see it, so you can aim at it — but
/// the cap is placed relative to the row's *whole* box, so for a row that is nine tenths below the
/// fold that lands on the dock's frame, outside the sidebar entirely (Antonio, driving, 2026-08-23:
/// *"letter should not overlap the parent DockView"*). The renderer cannot stop it: the cap is drawn
/// into the overlay band, which starts unclipped so a dropdown can escape a scroll region.
///
/// So it is nudged — never cut — into `permitted ∩ clip`, **the visible part of the area its
/// placement is allowed to use**, not merely into the clip: pushing it anywhere in the viewport
/// would park it over the row above, beside that row's own letter, naming something it does not
/// name.
///
/// `permitted` comes from [`keycap_rect`], which knows which placement branch it took: the target
/// box for an in-target cap, the target plus its caption band for a compact target whose cap sits
/// beside it. Clamping against `bounds` alone would undo that beside-placement.
///
/// `None` when the visible sliver is too small to hold the cap. There is nowhere honest to put it
/// then, so nothing is drawn.
fn fit_into_view(
    cap: Rectangle,
    permitted: Rectangle,
    clip: Option<Rectangle>,
) -> Option<Rectangle> {
    let Some(clip) = clip else {
        return Some(cap);
    };
    let room = permitted.intersection(clip)?;
    if room.size.w < cap.size.w || room.size.h < cap.size.h {
        return None;
    }
    let x = cap
        .loc
        .x
        .clamp(room.loc.x, room.loc.x + room.size.w - cap.size.w);
    let y = cap
        .loc
        .y
        .clamp(room.loc.y, room.loc.y + room.size.h - cap.size.h);
    Some(Rectangle::new(Point::new(x, y), cap.size))
}

/// **Draw `c`'s hint letter, if it is carrying one.** Called by
/// [`paint_child`](crate::component::paint_child) for *every* widget, right after the widget has
/// painted itself — which is what lets any widget be pickable without being wrapped in a
/// [`KeyHint`] (F003/P082/T431).
///
/// The rules that were learned the hard way, and are the reason this is one function rather than
/// something each widget does:
///
/// - **The viewport is the PAINT context's**, not `Base::viewport`. The latter is written per
///   *tree* by the layout pass, so inside a pane header it is the header's own box — a few dozen
///   pixels tall. Reading it there made every compact cap "not fit below", flip above, and get
///   clipped by the pane frame (Antonio, driving, 2026-08-14).
/// - **The cap is nudged, never cut.** A row half past a sidebar's fold is still a target — you can
///   see it, so you can aim at it — and its letter is drawn **whole** so it stays readable (Antonio,
///   2026-08-23: *"a half visible pane row should have the letter to peek"*). Clipping it would make
///   it unreadable, so instead it is moved inside the visible part of its own row —
///   [`fit_into_view`] — and dropped when that part is too small to hold it. What stops a cap for a
///   row nobody can see at all is **candidacy**, one level up: [`hint::collect`](crate::hint) drops
///   a target outside its clipping ancestors, so the letter is never handed out.
/// - **The placement says where the cap may live, not the target box.** [`keycap_rect`] returns the
///   permitted rect beside the cap, because a compact target's cap is placed *outside* it on
///   purpose. Re-deriving that from the bounds is what dragged every icon's caption back on top of
///   the icon (F003/P082/T479).
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
    let mut style = base.hint_style;
    // **One size and one colour for every letter in the app**, taken from the picker's own tokens
    // rather than from the widget the letter happens to sit on.
    //
    // A size variant scales what a widget draws as its *own content*; a pane is drawn with its own
    // theme, whose accent differs between the active pane and the rest. A letter is neither — it is
    // chrome the framework stamps over a target, and it belongs to the picker. Read from the target
    // instead, an emphasized header button wore a letter a quarter larger than the pane's own, and
    // in a different colour, in the same picker (Antonio, driving, 2026-09-03).
    let picker_font = cx.theme().hint_font_size;
    style.color = Some(style.color.unwrap_or(cx.theme().hint_color));
    let (cap, permitted) = keycap_rect(base.bounds, picker_font, &style, &text, cx.viewport());
    let Some(cap) = fit_into_view(cap, permitted, cx.clip()) else {
        return;
    };
    let font = hint_font(picker_font, &style, base.bounds);
    cx.with_overlay(|cx| {
        let tint = style
            .color
            .or_else(|| style.tone.map(|t| t.resolve(cx.theme())));
        paint_keycap(cx, cap, &text, font, tint, KeycapVariant::Filled);
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

    use super::{HintStyle, fit_into_view, keycap_rect};
    use heca_core::layout::{Point, Rectangle, Size};

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    /// **Nothing clipping it, nothing to do** — the cap stays exactly where its placement put it.
    #[test]
    fn an_unclipped_cap_is_left_where_it_was_placed() {
        let cap = rect(180.0, 10.0, 20.0, 20.0);
        assert_eq!(fit_into_view(cap, rect(0.0, 0.0, 200.0, 40.0), None), Some(cap));
        // …and so is one whose row is fully inside the clip.
        assert_eq!(
            fit_into_view(cap, rect(0.0, 0.0, 200.0, 40.0), Some(rect(0.0, 0.0, 200.0, 400.0))),
            Some(cap),
        );
    }

    /// **A row nine tenths past the fold keeps its letter, inside the dock** (F003/P082/T438).
    ///
    /// The cap is centred in the row's whole box, which for a row cut to a 25px sliver at the
    /// bottom of a sidebar lands on the dock's frame. It moves up into the sliver instead — and
    /// stays whole, because a cut letter cannot be read.
    #[test]
    fn a_cap_moves_into_the_visible_sliver_of_its_row() {
        let row = rect(0.0, 375.0, 200.0, 40.0); // 25px of it is above the fold at y=400
        let clip = rect(0.0, 0.0, 200.0, 400.0);
        let cap = rect(174.0, 385.0, 20.0, 20.0); // centred in the row: 5px of it hangs out

        let fitted = fit_into_view(cap, row, Some(clip)).expect("25px of row holds a 20px cap");
        assert_eq!(fitted.size, cap.size, "whole, never cut");
        assert!(
            fitted.loc.y + fitted.size.h <= clip.loc.y + clip.size.h,
            "and inside the dock: {fitted:?}",
        );
        assert!(fitted.loc.y >= row.loc.y, "still within its own row, not the one above");
    }

    /// **Too little of the row left to hold a letter, so none is drawn.** Nudging it any further
    /// would park it over the row above, beside that row's own letter, naming something else.
    #[test]
    fn a_cap_with_no_room_left_is_not_drawn() {
        let row = rect(0.0, 392.0, 200.0, 40.0); // only 8px visible
        let clip = rect(0.0, 0.0, 200.0, 400.0);
        let cap = rect(174.0, 402.0, 20.0, 20.0);
        assert_eq!(fit_into_view(cap, row, Some(clip)), None);
    }

    /// **A small icon's caption stays beside it, never on it** (F003/P082/T479).
    ///
    /// A target too small to cover has its letter placed *below* it on purpose, so the icon stays
    /// visible. The clamp used to pull that cap back into the button's own 25px box — dragging the
    /// caption onto the icon it captions, 474 times in one session — because it re-derived the
    /// permitted area from the target alone. The placement now says where the cap may live, and
    /// for a compact target that includes the caption band beside it.
    ///
    /// Measured numbers, from the trace on the real defect: a 26x25 top-bar button inside a
    /// sidebar's clip.
    #[test]
    fn a_compact_target_keeps_its_caption_below_itself() {
        let font = 13.0;
        let button = rect(526.0, 48.0, 26.0, 25.0);
        let clip = rect(316.0, 40.0, 320.0, 728.0);
        let viewport = Size::new(1400.0, 768.0);

        let (cap, permitted) = keycap_rect(button, font, &HintStyle::default(), "w", viewport);
        assert!(
            cap.loc.y >= button.loc.y + button.size.h,
            "placed below the icon it captions: {cap:?}",
        );

        let fitted = fit_into_view(cap, permitted, Some(clip)).expect("the caption band holds it");
        assert_eq!(fitted, cap, "and the clamp leaves it beside the icon, not on it");
    }

    /// **…and flips above only when below would fall off the viewport.**
    ///
    /// The other half of the same rule: the caption band is above *and* below, so a button near
    /// the bottom edge captions itself from above and the clamp leaves that alone too.
    #[test]
    fn a_compact_target_with_no_room_below_captions_itself_from_above() {
        let font = 13.0;
        let button = rect(526.0, 48.0, 26.0, 25.0);
        let clip = rect(316.0, 0.0, 320.0, 728.0);
        let viewport = Size::new(1400.0, 80.0); // nothing fits under the button

        let (cap, permitted) = keycap_rect(button, font, &HintStyle::default(), "w", viewport);
        assert!(
            cap.loc.y + cap.size.h <= button.loc.y,
            "flipped above the icon: {cap:?}",
        );

        let fitted = fit_into_view(cap, permitted, Some(clip)).expect("the caption band holds it");
        assert_eq!(fitted, cap, "and the clamp leaves it there");
    }

    /// **A letter stays readable whatever colour it is given** (Antonio, 2026-09-10: *"what if a
    /// user set the same color of the text?"*).
    ///
    /// `hint_color` is a colour the user picks, so the glyph cannot assume the chip is bright. The
    /// glyph was a constant — always the theme background — which is legible under a bright accent
    /// and invisible the moment the chip is near the background instead.
    #[test]
    fn a_keycap_letter_contrasts_with_the_chip_whatever_colour_it_is_given() {
        use super::keycap_glyph_color;
        let theme = heca_theme::Theme::grid_tron();
        let alpha = theme.interaction.keycap;

        // The dangerous setting: the letters painted the same colour as the background behind them.
        let glyph = keycap_glyph_color(&theme, theme.background, alpha);
        assert_ne!(
            glyph, theme.background,
            "a chip the colour of the background must not carry a background-coloured letter",
        );
        assert_eq!(
            glyph, theme.foreground,
            "it takes the contrasting tone instead"
        );

        // …and the default is unchanged: a bright accent chip still carries the dark glyph.
        assert_eq!(
            keycap_glyph_color(&theme, theme.accent, alpha),
            theme.background,
            "the accent letters must look exactly as they always did",
        );
    }

    /// **The rule is about the chip, not about the theme being dark.** A light theme whose accent is
    /// also light gives a chip that a background-coloured letter disappears into — the same failure
    /// a badly chosen `hint_color` produces, reached without touching the setting.
    #[test]
    fn a_pale_chip_on_a_pale_theme_switches_the_letter_too() {
        use super::keycap_glyph_color;
        let mut light = heca_theme::Theme::grid_tron();
        // A light palette is the dark one's two tones swapped — enough to state the rule without
        // depending on which bundled theme happens to be light.
        std::mem::swap(&mut light.background, &mut light.foreground);
        let alpha = light.interaction.keycap;

        // A mid-toned accent is far enough from the pale background: the letter is unchanged.
        assert_eq!(
            keycap_glyph_color(&light, light.accent, alpha),
            light.background,
            "a chip that already stands off the background keeps the design's own letter",
        );

        // A pale accent is not, so the letter takes the contrasting tone rather than vanishing.
        let pale = light.background.lerp(light.accent, 0.1);
        assert_eq!(
            keycap_glyph_color(&light, pale, alpha),
            light.foreground,
            "a chip the eye cannot separate from the background gets the other tone",
        );
    }

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
