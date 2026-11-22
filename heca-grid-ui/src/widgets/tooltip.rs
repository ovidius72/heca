//! **The tooltip — a property of every widget, and the bubble the framework draws for it.**
//!
//! A tooltip is a *declaration*: `Button::new("Close").tooltip("Close the pane")`. It is
//! [`Base::tooltip`], so every widget takes one on the same terms, and the framework does the rest
//! — it times the reveal from the hover clock the pointer router already keeps, and paints the
//! bubble in [`paint_child`], beside the hint letter and the drag
//! feedback. **No widget opts in, and no host paints on their behalf.**
//!
//! It used to be a wrapper you put *around* a widget, which meant the rule lived in every caller's
//! discipline: the showcase wraps four buttons in four tooltips in a row, and a widget held inside
//! a typed container (a [`ButtonGroup`](super::ButtonGroup) takes `Button` children) could not be
//! wrapped at all without changing what it was. Same move the pick declaration made when it left
//! [`KeyHint`](super::KeyHint) for every widget.
//!
//! [`Tooltip`] survives for exactly what `KeyHint` survives for — **a region that is not a widget
//! you can put a builder on** — and is implemented in terms of the property, so there is one
//! reveal, one placement and one bubble rather than two that can drift.
//!
//! Placement is **viewport-aware on all four sides**: the bubble centers on the chosen
//! [`TooltipSide`] of the target, but flips to the opposite side when there isn't room
//! (`Top`↔`Bottom`, `Left`↔`Right`), and its cross-axis is clamped to the viewport so it never
//! spills off-screen. That rule is not implemented here — it is the shared [`place_beside`]
//! authority, so the tooltip and any future four-sided popover cannot drift apart. The bubble's
//! surface likewise comes from [`paint_panel_chrome`], the one definition of what an overlay panel
//! looks like, so the user's `overlay_border_style` governs the tooltip exactly as it governs a
//! dialog or a dropdown.

use std::cell::RefCell;
use std::rc::Rc;

use crate::builders::{IntoComponent, LayoutExt, Parent, PlaceExt, StyleExt};
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::Glow;
use crate::style::{Length, Space, Spacing};
use crate::widgets::overlay::{
    BesideSide, PanelChrome, PanelElevation, paint_panel_chrome, place_beside,
};
use crate::widgets::{Flex, Label};
use heca_core::layout::{Point, Rectangle, Size};

/// Smallest rect containing both `a` and `b` (widget bounds ∪ bubble rect).
fn union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x0 = a.loc.x.min(b.loc.x);
    let y0 = a.loc.y.min(b.loc.y);
    let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
    let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
    Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

/// Which side of the target the bubble appears on.
///
/// This is [`BesideSide`] — the shared four-sided placement vocabulary — re-exported under the name
/// that reads better at a call site (`.tooltip_side(TooltipSide::Bottom)`). One type, so the
/// tooltip and [`place_beside`] can never disagree about what `Top` means.
pub type TooltipSide = BesideSide;

/// Seconds the pointer must rest before the bubble shows, until the host says otherwise with
/// [`set_default_delay`] — heca does, from the user's `[appearance] tooltip_delay_ms`.
pub const DEFAULT_DELAY: f32 = 0.5;
/// The shorter rest, as it is at [`DEFAULT_DELAY`]: always half of the current default — see
/// [`default_delay`]. For a tip that is the point of the thing it sits on (a value cut short on
/// purpose) that the reader goes to the pointer for.
/// See [`TooltipExt::tooltip_quick`](crate::builders::TooltipExt::tooltip_quick).
pub const QUICK_DELAY: f32 = DEFAULT_DELAY / 2.0;

thread_local! {
    /// The default rest, in seconds — one value for every tooltip that did not ask for its own.
    static DEFAULT: std::cell::Cell<f32> = const { std::cell::Cell::new(DEFAULT_DELAY) };
}

/// **How long every tooltip waits by default**, in seconds — set once by the host (heca reads the
/// user's config, at start and on reload), so no widget and no caller passes a delay. A tooltip
/// that asked for its own ([`TooltipExt::tooltip_delay`](crate::builders::TooltipExt::tooltip_delay))
/// keeps it; a quick one ([`tooltip_quick`](crate::builders::TooltipExt::tooltip_quick)) waits half
/// of this.
pub fn set_default_delay(seconds: f32) {
    DEFAULT.with(|d| d.set(seconds.max(0.0)));
}

/// The current default rest, in seconds ([`set_default_delay`]).
pub fn default_delay() -> f32 {
    DEFAULT.with(std::cell::Cell::get)
}

/// How long a tooltip waits: the default, half of it, or its own.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Delay {
    /// [`default_delay`], read when the pointer rests — so a changed setting reaches every tip.
    Default,
    /// Half of [`default_delay`].
    Quick,
    /// Its own, in seconds.
    Seconds(f32),
}
/// The air between the target and the bubble, and inside the bubble around what it holds — steps
/// of the theme's rhythm, so they follow the font like every other space.
const GAP: Spacing = Spacing::Sm;
const PAD_X: Spacing = Spacing::Sm;
const PAD_Y: Spacing = Spacing::Sm;
/// The bubble's halo is the theme's own, one level lighter: a hover bubble should read as a light
/// surface, not a panel demanding attention. The user's `glow_size` still scales it on top, as it
/// does every glow.
const BUBBLE_GLOW: crate::theme::GlowLevel = crate::theme::GlowLevel::Thin;
/// The name the bubble's own panel answers to inside the tree that lays it out.
const BUBBLE: &str = "tooltip.bubble";

/// **What a widget says on hover** — the declaration held in [`Base::tooltip`].
///
/// Built by [`TooltipExt::tooltip`](crate::builders::TooltipExt::tooltip) from words, a live
/// signal or a whole widget ([`IntoTip`]); a widget that declares none has `None` here and costs
/// nothing.
///
/// **The bubble is widgets, laid out by the engine** — a panel holding the content, padded with
/// steps of the theme's rhythm — so anything fits: two lines, a segmented [`Tag`](super::Tag), a
/// row of parts. Words are a wrapping [`Label`] that keeps their line breaks.
#[derive(Clone)]
pub struct Tip {
    /// The words — what the bubble's [`Label`] shows, followed live. Empty for a tip whose content
    /// is a widget, and an empty word tip shows nothing.
    pub text: Signal<String>,
    /// Which side of the widget to anchor to. Flipped automatically when there is no room.
    pub side: TooltipSide,
    /// How long the pointer must rest before the bubble appears ([`delay`](Self::delay)).
    delay: Delay,
    /// Made by a label for its own cut text ([`Tip::cut_text`]): a tooltip written by hand on the
    /// label or on anything holding it wins over this one.
    automatic: bool,
    /// Whether the content is a widget rather than the words.
    widget: bool,
    /// The bubble's tree, kept across reveals: laid out and painted only while it shows.
    bubble: Rc<RefCell<Flex>>,
    /// Where the bubble was last **painted** (`None`: not painted) — what is on screen once that
    /// frame is shown.
    drawn: Rc<std::cell::Cell<Option<Rectangle>>>,
    /// What a repaint was last told about ([`damage`]). Appearing, moving and going are each a
    /// change to repaint — with every place it was or is included — in any host that repaints only
    /// what changed, whether it paints before or after it asks.
    announced: Rc<std::cell::Cell<Option<Rectangle>>>,
}

impl Tip {
    /// A tip with the default side and delay.
    pub fn new(text: impl Into<String>) -> Self {
        Self::from_signal(signal(text.into()))
    }

    /// A tip whose words are a live signal.
    pub fn from_signal(text: Signal<String>) -> Self {
        let words = Label::new("")
            .with_text_signal(text)
            .wrap(true)
            .preserve_whitespace(true);
        Self::holding(text, false, words.into_component())
    }

    /// A tip whose bubble holds `content` — any widget.
    pub fn of(content: impl IntoComponent) -> Self {
        Self::holding(signal(String::new()), true, content.into_component())
    }

    fn holding(text: Signal<String>, widget: bool, content: crate::WidgetModel) -> Self {
        // The bubble hugs what it holds: placed at the start of a row that spans the room it may
        // use, so the panel is its content's size and not the room's.
        let panel = Flex::column()
            .padding_x(PAD_X)
            .padding_y(PAD_Y)
            .area(BUBBLE)
            .child(content);
        let room = Flex::row().align("start").child(panel);
        Self {
            text,
            side: TooltipSide::default(),
            delay: Delay::Default,
            automatic: false,
            widget,
            bubble: Rc::new(RefCell::new(room)),
            drawn: Rc::default(),
            announced: Rc::default(),
        }
    }

    /// **The whole text of a label that had to cut it** — what a [`Label`] declares on itself while
    /// its text does not fit, and drops when it fits again. Shown after the quick rest: the reader
    /// came to the pointer for it. Any tooltip written by hand — on the label, or on a widget
    /// holding it — wins over this one.
    pub(crate) fn cut_text(text: Signal<String>) -> Self {
        let mut tip = Self::from_signal(text);
        tip.delay = Delay::Quick;
        tip.automatic = true;
        tip
    }

    /// **The words of a button showing only its icon** — offered on hover while the words are not
    /// on screen, dropped when they are back. Like [`cut_text`](Self::cut_text), a tooltip written
    /// by hand wins over it.
    pub(crate) fn hidden_words(text: impl Into<String>) -> Self {
        let mut tip = Self::new(text);
        tip.automatic = true;
        tip
    }

    /// Whether a label made this tip for its own cut text ([`cut_text`](Self::cut_text)).
    pub(crate) fn is_automatic(&self) -> bool {
        self.automatic
    }

    /// **Seconds the pointer must rest before the bubble appears** — its own delay, or the current
    /// default ([`default_delay`]), or half of it for a quick tip.
    pub fn delay(&self) -> f32 {
        match self.delay {
            Delay::Default => default_delay(),
            Delay::Quick => default_delay() / 2.0,
            Delay::Seconds(seconds) => seconds,
        }
    }

    /// Wait `seconds` before showing, whatever the default is.
    pub fn set_delay(&mut self, seconds: f32) {
        self.delay = Delay::Seconds(seconds.max(0.0));
    }

    /// Wait half of the default before showing ([`QUICK_DELAY`] at the shipped default).
    pub fn set_quick(&mut self) {
        self.delay = Delay::Quick;
    }

    /// Whether there is anything to show: a widget, or words that are not empty.
    fn says_something(&self) -> bool {
        self.widget || !self.text.get_untracked().is_empty()
    }
}

/// **What `.tooltip(..)` takes** — the same door for words, live words and a whole widget:
///
/// ```ignore
/// Button::new("Close").tooltip("Close the pane");               // words
/// Label::new(path).tooltip(full_path);                          // a Signal<String>: followed live
/// row.tooltip(Tag::new().segment(program).segment(folder));     // a widget
/// ```
///
/// An `Option` of any of these is taken too: `None` gives no tooltip and leaves the widget as it was.
pub trait IntoTip {
    /// The tip this makes.
    fn into_tip(self) -> Tip;

    /// The tip this makes, or `None` when there is none to give — what the builder reads, so an
    /// `Option` can say "nothing". Every other kind always has one.
    fn into_tip_if_any(self) -> Option<Tip>
    where
        Self: Sized,
    {
        Some(self.into_tip())
    }
}

impl<T: IntoTip> IntoTip for Option<T> {
    /// `None` reads as an empty tip, which shows no bubble.
    fn into_tip(self) -> Tip {
        self.map_or_else(|| Tip::new(""), IntoTip::into_tip)
    }

    fn into_tip_if_any(self) -> Option<Tip> {
        self.map(IntoTip::into_tip)
    }
}

impl IntoTip for &str {
    fn into_tip(self) -> Tip {
        Tip::new(self)
    }
}

impl IntoTip for String {
    fn into_tip(self) -> Tip {
        Tip::new(self)
    }
}

impl IntoTip for &String {
    fn into_tip(self) -> Tip {
        Tip::new(self.clone())
    }
}

impl IntoTip for Signal<String> {
    fn into_tip(self) -> Tip {
        Tip::from_signal(self)
    }
}

impl<C: IntoComponent> IntoTip for C {
    fn into_tip(self) -> Tip {
        Tip::of(self)
    }
}

/// The font a bubble is drawn at: **the surface's, not the control's**. A bubble is a small panel
/// beside the widget rather than part of it, so an emphasized button must not get emphasized words
/// floating over it.
fn surface_font(base: &Base) -> f32 {
    if base.root_font > 0.0 {
        base.root_font
    } else {
        base.font
    }
}

/// **Is this widget's bubble up?** Hovered, declared, with something to say, and rested past the
/// delay.
///
/// One answer, read by the paint, the damage and the wake — so a bubble is never drawn in a frame
/// that was not repainted for it, and never leaves a ghost behind.
fn shown(base: &Base) -> Option<&Tip> {
    let tip = base.tooltip.as_ref()?;
    let rested = base.pointer.hovered_for()?;
    (rested >= tip.delay() && tip.says_something()).then_some(tip)
}

/// Lay the bubble out for `base` and place it beside the widget, inside `viewport`. The panel ends
/// up where it is drawn; its rect is returned.
fn place_bubble(base: &Base, tip: &Tip, viewport: Size) -> Option<Rectangle> {
    let font = surface_font(base);
    let mut room = tip.bubble.borrow_mut();
    crate::LayoutEngine::new()
        .base_font(font)
        .compute(&mut *room, viewport);
    let laid = crate::component::area(&*room, BUBBLE)?.base().bounds;
    let gap = Space::Step(GAP).resolve(font) as f64;
    let rect = place_beside(base.bounds, laid.size, viewport, gap, tip.side);
    crate::component::shift_subtree(&mut *room, rect.loc.x - laid.loc.x, rect.loc.y - laid.loc.y);
    Some(rect)
}

/// **Draw the widget's tooltip, if it has one and the pointer has rested on it.**
///
/// Called from [`paint_child`] for every widget in the tree, beside
/// the hint letter and the drag feedback — the three things the framework draws *over* a widget
/// from state it already keeps, so nothing opts in.
pub(crate) fn paint_tooltip(c: &dyn Component, cx: &mut PaintCx) {
    let base = c.base();
    let Some(declared) = base.tooltip.as_ref() else {
        return;
    };
    let quiet = declared.is_automatic() && cx.hand_tip_above;
    let tip = (base.visible.get_untracked()
        && !quiet
        && !base
            .children
            .iter()
            .any(|child| owns_hover_tip(child.as_ref())))
    .then(|| shown(base))
    .flatten();
    let rect = tip.and_then(|tip| place_bubble(base, tip, cx.viewport()));
    declared.drawn.set(rect);
    let (Some(tip), Some(rect)) = (tip, rect) else {
        return;
    };
    let (accent, glow, tip_border) = {
        let t = cx.theme();
        let glow = Glow {
            color: t.colors.glow,
            radius: t.colors.active_glow_radius * BUBBLE_GLOW.radius_scale(),
            intensity: t.colors.active_glow_strength * BUBBLE_GLOW.strength_scale(),
        };
        (cx.accent(), glow, t.colors.interaction.tooltip_border)
    };
    // Drawn on the overlay layer so it sits above later siblings and is never clipped.
    cx.with_overlay(|cx| {
        // The shared panel painter owns the surface: fill, drop shadow, and the edge the user
        // configured via `overlay_border_style`. The tooltip supplies only its own identity — its
        // accent edge colour and halo.
        let border = cx.border(accent.with_alpha(tip_border));
        paint_panel_chrome(
            cx,
            rect,
            PanelChrome {
                border,
                glow: Some(glow),
                // A hover bubble is not at dialog depth: the panel shadow was tuned for surfaces
                // hundreds of px across and, unscaled, is bigger than this ~30px bubble.
                elevation: PanelElevation::Hover,
            },
        );
        let room = tip.bubble.borrow();
        if let Some(panel) = crate::component::area(&*room, BUBBLE) {
            paint_child(panel, cx);
        }
    });
}

/// A descendant's specific explanation takes priority over its ancestor's general help,
/// including while its reveal delay is pending. Pointer routing already owns the hover path.
/// A label's own cut-text tooltip does not: a tooltip written by hand around it wins.
fn owns_hover_tip(c: &dyn Component) -> bool {
    let base = c.base();
    base.visible.get_untracked()
        && base.hovered()
        && (base
            .tooltip
            .as_ref()
            .is_some_and(|tip| tip.says_something() && !tip.is_automatic())
            || base
                .children
                .iter()
                .any(|child| owns_hover_tip(child.as_ref())))
}

/// **Seconds until this widget's bubble appears**, or `None` when nothing is pending.
///
/// Folded into [`Component::next_redraw`] so the host
/// sleeps through the delay and wakes exactly once, rather than redrawing every frame — and so a
/// bubble appears on time even when the pointer has stopped moving and nothing else would.
pub(crate) fn wake(base: &Base) -> Option<f32> {
    let tip = base.tooltip.as_ref()?;
    if let Some(rested) = base.pointer.hovered_for()
        && rested < tip.delay()
    {
        return Some(tip.delay() - rested);
    }
    // Due, but not painted where it should be yet — it came due after this frame was painted: one
    // more frame, now.
    (bubble_now(base) != tip.drawn.get()).then_some(0.0)
}

/// Where this widget's bubble is now, if it is up — against the viewport the tree was last laid out
/// against, published onto every widget by the layout engine, the same number the paint pass uses.
fn bubble_now(base: &Base) -> Option<Rectangle> {
    shown(base).and_then(|tip| place_bubble(base, tip, base.viewport))
}

/// **Whether the bubble appeared, moved or went since a repaint last asked** — so the widget counts
/// as needing paint at that moment, with nothing marking it: the reveal comes on a timed wake, when
/// no event touched the widget, and a host that repaints only what changed would otherwise draw the
/// bubble cut to whatever else changed, or not at all.
pub(crate) fn bubble_changed(base: &Base) -> bool {
    let Some(tip) = base.tooltip.as_ref() else {
        return false;
    };
    let drawn = tip.drawn.get();
    drawn != tip.announced.get() || bubble_now(base) != drawn
}

/// **The rect a repaint must cover** for this widget — its own bounds, plus its bubble where it is
/// now and where it was last announced, because the bubble draws on the overlay layer well outside
/// them and a bubble that moved or went must be painted over in its old place.
pub(crate) fn damage(base: &Base) -> Rectangle {
    let Some(tip) = base.tooltip.as_ref() else {
        return base.bounds;
    };
    let (now, drawn) = (bubble_now(base), tip.drawn.get());
    let before = tip.announced.replace(drawn);
    [now, drawn, before]
        .into_iter()
        .flatten()
        .fold(base.bounds, union)
}

/// **A tooltip on a region that is not a widget you can put a builder on.**
///
/// The declaration belongs on the widget — `Button::new("Close").tooltip("Close the pane")` — and
/// every widget takes one, so reach for this only when there is no widget to declare it on. It is
/// the same role [`KeyHint`](super::KeyHint) kept when the pick declaration moved onto every
/// widget, and for the same reason.
///
/// It is a **transparent wrapper**: it forwards layout and events to its child untouched, and
/// carries the tip on its own base, so the reveal, the placement and the bubble are the framework's
/// one implementation rather than a second copy.
pub struct Tooltip {
    base: Base,
}

#[heca_grid_ui_macros::props]
impl Tooltip {
    /// Wrap `child`, showing `text` on hover.
    pub fn new(child: impl crate::builders::IntoComponent, text: impl Into<String>) -> Self {
        Self::wrap(child.into_component(), Tip::new(text))
    }

    /// Wrap `child`, showing reactive `text` on hover.
    pub fn new_signal(child: impl crate::builders::IntoComponent, text: Signal<String>) -> Self {
        Self::wrap(child.into_component(), Tip::from_signal(text))
    }

    fn wrap(child: crate::WidgetModel, tip: Tip) -> Self {
        let mut base = Base::container();
        // Hug the child so the wrapper's bounds match it — the anchor is the wrapper's box, and
        // hover is already true here whenever it is true on the child (the CSS rule: a control is
        // hovered when what is inside it is).
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        crate::component::wrap_transparently(&mut base, child.as_ref());
        base.children.push(child);
        base.tooltip = Some(tip);
        Self { base }
    }

    /// Which side of the target to anchor to (default [`TooltipSide::Top`]).
    #[heca_grid_ui_macros::prop]
    pub fn side(mut self, side: TooltipSide) -> Self {
        if let Some(tip) = self.base.tooltip.as_mut() {
            tip.side = side;
        }
        self
    }

    /// Hover delay before the bubble appears, in seconds (default `0.5`).
    #[heca_grid_ui_macros::prop]
    pub fn delay(mut self, seconds: f32) -> Self {
        if let Some(tip) = self.base.tooltip.as_mut() {
            tip.set_delay(seconds);
        }
        self
    }
}

impl Component for Tooltip {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        // The child only. The bubble is drawn by `paint_child` from the declaration on our base,
        // exactly as it is for a widget that declared one directly.
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for Tooltip {}
impl StyleExt for Tooltip {}
impl Parent for Tooltip {}
