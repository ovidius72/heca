//! **The tooltip — a property of every widget, and the bubble the framework draws for it.**
//!
//! A tooltip is a *declaration*: `Button::new("Close").tooltip("Close the pane")`. It is
//! [`Base::tooltip`], so every widget takes one on the same terms, and the framework does the rest
//! — it times the reveal from the hover clock the pointer router already keeps, and paints the
//! bubble in [`paint_child`](crate::component::paint_child), beside the hint letter and the drag
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

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::Length;
use crate::widgets::overlay::{
    BesideSide, PanelChrome, PanelElevation, paint_panel_chrome, place_beside,
};
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

/// Seconds the pointer must rest before the bubble shows.
pub const DEFAULT_DELAY: f32 = 0.5;
/// Gap between the target and the bubble (logical px).
const GAP: f64 = 6.0;
/// Bubble inner padding.
const PAD_X: f64 = 8.0;
const PAD_Y: f64 = 5.0;
/// Halo falloff on the bubble — the tooltip's own accent identity, layered onto the shared panel
/// chrome. Deliberately tighter/fainter than a dropdown's: a hover bubble should read as a light
/// surface, not a panel demanding attention.
const BUBBLE_GLOW_RADIUS: f32 = 5.0;
const BUBBLE_GLOW_INTENSITY: f32 = 0.2;

/// **What a widget says on hover** — the declaration held in [`Base::tooltip`].
///
/// Built by [`ComponentExt::tooltip`](crate::builders::ComponentExt::tooltip); a widget that
/// declares none has `None` here and costs nothing.
#[derive(Clone)]
pub struct Tip {
    /// The words. A signal, so a tooltip that tracks live state (an action's current keybinding)
    /// updates without rebuilding the widget.
    pub text: Signal<String>,
    /// Which side of the widget to anchor to. Flipped automatically when there is no room.
    pub side: TooltipSide,
    /// Seconds the pointer must rest before the bubble appears.
    pub delay: f32,
}

impl Tip {
    /// A tip with the default side and delay.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: signal(text.into()),
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
        }
    }

    /// A tip whose words are a live signal.
    pub fn from_signal(text: Signal<String>) -> Self {
        Self {
            text,
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
        }
    }
}

/// The bubble's size for `text` at `font` (logical px).
fn bubble_size(text: &str, font: f32) -> (f64, f64) {
    let chars = text.chars().count() as f64;
    let w = chars * (font * MONO_ADVANCE_RATIO) as f64 + 2.0 * PAD_X;
    let h = (font * MONO_LINE_RATIO) as f64 + 2.0 * PAD_Y;
    (w, h)
}

/// **Is this widget's bubble up?** Hovered, declared, non-empty, and rested past the delay.
///
/// One answer, read by the paint, the damage and the wake — so a bubble is never drawn in a frame
/// that was not repainted for it, and never leaves a ghost behind.
fn shown(base: &Base) -> Option<(String, f32)> {
    let tip = base.tooltip.as_ref()?;
    let rested = base.pointer.hovered_for()?;
    if rested < tip.delay {
        return None;
    }
    let text = tip.text.get_untracked();
    // **The surface's font, not the control's.** A bubble is a small panel beside the widget rather
    // than part of it, so an emphasized button must not get emphasized words floating over it.
    let font = if base.root_font > 0.0 {
        base.root_font
    } else {
        base.font
    };
    (!text.is_empty()).then_some((text, font))
}

/// Where this widget's bubble sits, if it is up.
fn rect_of(base: &Base, viewport: Size) -> Option<(String, Rectangle)> {
    let (text, font) = shown(base)?;
    let (w, h) = bubble_size(&text, font);
    let tip = base.tooltip.as_ref()?;
    Some((
        text,
        place_beside(base.bounds, Size::new(w, h), viewport, GAP, tip.side),
    ))
}

/// **Draw the widget's tooltip, if it has one and the pointer has rested on it.**
///
/// Called from [`paint_child`](crate::component::paint_child) for every widget in the tree, beside
/// the hint letter and the drag feedback — the three things the framework draws *over* a widget
/// from state it already keeps, so nothing opts in.
pub(crate) fn paint_tooltip(c: &dyn Component, cx: &mut PaintCx) {
    let base = c.base();
    if !base.visible.get_untracked() {
        return;
    }
    let Some((text, rect)) = rect_of(base, cx.viewport()) else {
        return;
    };
    let (accent, glow_c, foreground, tip_border) = {
        let t = cx.theme();
        (
            cx.accent(),
            t.colors.glow,
            t.colors.foreground,
            t.colors.interaction.tooltip_border,
        )
    };
    let font = if base.root_font > 0.0 {
        base.root_font
    } else {
        base.font
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
                glow: Some(Glow {
                    color: glow_c,
                    radius: BUBBLE_GLOW_RADIUS,
                    intensity: BUBBLE_GLOW_INTENSITY,
                }),
                // A hover bubble is not at dialog depth: the panel shadow was tuned for surfaces
                // hundreds of px across and, unscaled, is bigger than this ~30px bubble.
                elevation: PanelElevation::Hover,
            },
        );
        cx.text(rect, &text, foreground, font, TextAlign::Center, TextStyle::REGULAR);
    });
}

/// **Seconds until this widget's bubble appears**, or `None` when nothing is pending.
///
/// Folded into [`Component::next_redraw`](crate::component::Component::next_redraw) so the host
/// sleeps through the delay and wakes exactly once, rather than redrawing every frame — and so a
/// bubble appears on time even when the pointer has stopped moving and nothing else would.
pub(crate) fn wake(base: &Base) -> Option<f32> {
    let tip = base.tooltip.as_ref()?;
    let rested = base.pointer.hovered_for()?;
    (rested < tip.delay).then_some(tip.delay - rested)
}

/// **The rect a repaint must cover** for this widget — its own bounds, plus its bubble when one is
/// up, because the bubble draws on the overlay layer well outside them.
pub(crate) fn damage(base: &Base) -> Rectangle {
    // The viewport the tree was last laid out against, published onto every widget by the layout
    // engine — the same number the paint pass will use, so the damaged rect and the drawn bubble
    // cannot disagree.
    match rect_of(base, base.viewport) {
        Some((_, bubble)) => union(base.bounds, bubble),
        None => base.bounds,
    }
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
    pub fn new(child: impl Component + 'static, text: impl Into<String>) -> Self {
        Self::wrap(Box::new(child), Tip::new(text))
    }

    /// Wrap `child`, showing reactive `text` on hover.
    pub fn new_signal(child: impl Component + 'static, text: Signal<String>) -> Self {
        Self::wrap(Box::new(child), Tip::from_signal(text))
    }

    fn wrap(child: Box<dyn Component>, tip: Tip) -> Self {
        let mut base = Base::new();
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
            tip.delay = seconds.max(0.0);
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
