//! Where a hint letter goes over a target, and the universal painter that stamps
//! it. The placement vocabulary ([`HintPlacement`]/[`HintStyle`]), the compact/fit
//! rules, and [`paint_hint_label`] — called by `paint_child` for every widget
//! carrying a letter, which is what lets any widget be pickable without a wrapper.

use super::keycap::{keycap_size, paint_keycap, KeycapVariant};
use crate::Tone;
use crate::color::Color;
use crate::component::{Component, PaintCx};
use crate::reactive::SignalGet;
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
    /// [`offset_y`](super::KeyHint::offset_y) to drop it onto the header line.
    TopRight,
    /// Pinned just inside the **top-left**, centred within a shallow band from the target's top
    /// edge — for a large target (a card in the exposé, a content pane) where the letter should
    /// stay out of the way of what the target shows and never sit on its border.
    ///
    /// This is what the universal picker drew for every large target before the letters became the
    /// widget's own to place, and it is the reason it exists as a variant: the rule was real, it
    /// just lived in a host paint pass where no call site could ask for it.
    TopLeft,
}

/// **How a widget's hint keycap is drawn.**
///
/// These four knobs were private fields on [`KeyHint`](super::KeyHint), which is why a letter could only be drawn
/// by wrapping a widget in one. They live on [`Base`](crate::component::Base) now, so the framework draws the cap for *any*
/// widget carrying a letter and a widget places its own.
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
    pub tone: Option<Tone>,
    /// Extra vertical nudge applied after placement — positive moves it down.
    pub offset_y: f64,
}

/// **What colour a keycap is: an explicit choice, then a declared meaning, then the picker's own.**
///
/// A widget composing itself has no theme at build time, so a literal is not something it can
/// write — a tone is. It has to be resolved *before* the picker's fallback: with the fallback
/// first, `color` is always `Some` by the time the tone is consulted and the tone is silently
/// dead, which looks from the outside like a placement that moved and a colour that did not.
pub(crate) fn cap_tint(style: &HintStyle, theme: &crate::theme::Theme) -> Color {
    style
        .color
        .or_else(|| style.tone.map(|t| t.resolve(theme)))
        .unwrap_or(theme.hint_color)
}

/// Keycap font size as a fraction of the wrapped component's resolved font.
const HINT_FONT_MUL: f32 = 1.05;
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
/// paints.
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
/// fold that lands on the dock's frame, outside the sidebar entirely. The renderer cannot stop it:
/// the cap is drawn into the overlay band, which starts unclipped so a dropdown can escape a scroll
/// region.
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
/// `paint_child` for *every* widget, right after the widget has
/// painted itself — which is what lets any widget be pickable without being wrapped in a
/// [`KeyHint`](super::KeyHint).
///
/// The rules that were learned the hard way, and are the reason this is one function rather than
/// something each widget does:
///
/// - **The viewport is the PAINT context's**, not `Base::viewport`. The latter is written per
///   *tree* by the layout pass, so inside a pane header it is the header's own box — a few dozen
///   pixels tall. Reading it there made every compact cap "not fit below", flip above, and get
///   clipped by the pane frame.
/// - **The cap is nudged, never cut.** A row half past a sidebar's fold is still a target — you can
///   see it, so you can aim at it — and its letter is drawn **whole** so it stays readable.
///   Clipping it would make it unreadable, so instead it is moved inside the visible part of its
///   own row — [`fit_into_view`] — and dropped when that part is too small to hold it. What stops a
///   cap for a row nobody can see at all is **candidacy**, one level up:
///   [`hint::collect`](crate::hint) drops a target outside its clipping ancestors, so the letter is
///   never handed out.
/// - **The placement says where the cap may live, not the target box.** [`keycap_rect`] returns the
///   permitted rect beside the cap, because a compact target's cap is placed *outside* it on
///   purpose. Re-deriving that from the bounds is what dragged every icon's caption back on top of
///   the icon.
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
    // in a different colour, in the same picker.
    let picker_font = cx.theme().hint_font_size;
    // **Explicit colour, then the declared MEANING, then the picker's own token.** A widget
    // composing itself has no theme at build time, so a tone is the only thing it can say — and it
    // has to be resolved here, before the fallback, or the fallback always wins and the tone is
    // silently dead.
    style.color = Some(cap_tint(&style, cx.theme()));
    let (cap, permitted) = keycap_rect(base.bounds, picker_font, &style, &text, cx.viewport());
    let Some(cap) = fit_into_view(cap, permitted, cx.clip()) else {
        return;
    };
    let font = hint_font(picker_font, &style, base.bounds);
    cx.with_overlay(|cx| {
        paint_keycap(cx, cap, &text, font, style.color, KeycapVariant::Filled);
    });
}

#[cfg(test)]
mod tests {
    use super::{fit_into_view, keycap_rect, HintStyle};
    use heca_core::layout::{Point, Rectangle, Size};

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rectangle {
        Rectangle::new(Point::new(x, y), Size::new(w, h))
    }

    /// **Nothing clipping it, nothing to do** — the cap stays exactly where its placement put it.
    #[test]
    fn an_unclipped_cap_is_left_where_it_was_placed() {
        let cap = rect(180.0, 10.0, 20.0, 20.0);
        assert_eq!(
            fit_into_view(cap, rect(0.0, 0.0, 200.0, 40.0), None),
            Some(cap)
        );
        // …and so is one whose row is fully inside the clip.
        assert_eq!(
            fit_into_view(
                cap,
                rect(0.0, 0.0, 200.0, 40.0),
                Some(rect(0.0, 0.0, 200.0, 400.0))
            ),
            Some(cap),
        );
    }

    /// **A row nine tenths past the fold keeps its letter, inside the dock**.
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
        assert!(
            fitted.loc.y >= row.loc.y,
            "still within its own row, not the one above"
        );
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

    /// **A small icon's caption stays beside it, never on it**.
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
        assert_eq!(
            fitted, cap,
            "and the clamp leaves it beside the icon, not on it"
        );
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
}

#[cfg(test)]
mod keycap_tint {
    use super::*;
    use crate::Tone;
    use crate::builders::HintExt as _;
    use crate::widgets::Surface;

    fn style_of(w: impl Component) -> HintStyle {
        w.base().hint_style
    }

    /// **A declared MEANING colours the cap** — the one thing a widget composing itself can say,
    /// since it has no theme at build time to take a literal from.
    ///
    /// It was dead on arrival once: the picker's own fallback ran first, so `color` was always
    /// `Some` by the time the tone was consulted. From the outside that looked like a placement
    /// that moved and a colour that did not.
    #[test]
    fn a_tone_decides_the_cap_when_no_colour_was_named() {
        let theme = crate::theme::Theme::default();
        let muted = style_of(Surface::new().hint_tone(Tone::Muted));
        assert_eq!(cap_tint(&muted, &theme), theme.colors.muted);
        assert_ne!(
            cap_tint(&muted, &theme),
            cap_tint(&style_of(Surface::new()), &theme),
            "a tone has to change something",
        );
    }

    /// **An explicit colour still wins**, so the app's per-kind tints are untouched.
    #[test]
    fn a_named_colour_outranks_a_tone() {
        let theme = crate::theme::Theme::default();
        let red = crate::Color::new(255, 0, 0, 255);
        let both = style_of(Surface::new().hint_tone(Tone::Muted).hint_color(red));
        assert_eq!(cap_tint(&both, &theme), red);
    }

    /// …and with neither, the picker's own token, exactly as before any of this.
    #[test]
    fn nothing_named_is_the_pickers_own_colour() {
        let theme = crate::theme::Theme::default();
        assert_eq!(
            cap_tint(&style_of(Surface::new()), &theme),
            theme.hint_color
        );
    }
}
