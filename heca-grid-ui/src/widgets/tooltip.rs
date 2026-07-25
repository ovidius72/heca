//! [`Tooltip`] — a hover-revealed floating label for any child.
//!
//! A **transparent wrapper** (like [`KeyHint`](super::KeyHint)): it forwards
//! layout/events to the wrapped child and only *adds paint*. While the pointer
//! rests over the child past a short delay, a small bubble (rounded surface +
//! border + glow + text) is drawn on the scene's **overlay layer** (via
//! [`PaintCx::with_overlay`](crate::component::PaintCx::with_overlay)) so it sits
//! above later siblings and is never clipped. It captures **no** input — a
//! tooltip never eats clicks — so it does not use the `overlay_active` path that
//! input-grabbing popovers (`Select`) do.
//!
//! Placement is **viewport-aware on all four sides**: the bubble centers on the
//! chosen [`TooltipSide`] of the target, but flips to the opposite side when there
//! isn't room (`Top`↔`Bottom`, `Left`↔`Right`), and its cross-axis is clamped to
//! the viewport so it never spills off-screen. That rule is not implemented here —
//! it is the shared [`place_beside`] authority, so the tooltip and any future
//! four-sided popover cannot drift apart. The bubble's surface likewise comes from
//! [`paint_panel_chrome`], the one definition of what an overlay panel looks like,
//! so the user's `overlay_border_style` governs the tooltip exactly as it governs
//! a dialog or a dropdown.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{
    Base, Component, Event, Handled, PaintCx, paint_child, route_event, soonest_redraw,
};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::style::Length;
use crate::widgets::overlay::{
    paint_panel_chrome, place_beside, BesideSide, PanelChrome, PanelElevation,
};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;
use std::time::Instant;

/// Smallest rect containing both `a` and `b` (child bounds ∪ bubble rect).
fn union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x0 = a.loc.x.min(b.loc.x);
    let y0 = a.loc.y.min(b.loc.y);
    let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
    let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
    Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

/// Which side of the target the bubble appears on.
///
/// This is [`BesideSide`] — the shared four-sided placement
/// vocabulary — re-exported under the name that reads better at a tooltip call
/// site (`Tooltip::new(..).side(TooltipSide::Bottom)`). One type, so the tooltip
/// and [`place_beside`] can never disagree about what `Top` means.
pub type TooltipSide = BesideSide;

/// Seconds the pointer must rest before the bubble shows.
const DEFAULT_DELAY: f32 = 0.5;
/// Gap between the target and the bubble (logical px).
const GAP: f64 = 6.0;
/// Bubble inner padding.
const PAD_X: f64 = 8.0;
const PAD_Y: f64 = 5.0;
/// Halo falloff on the bubble — the tooltip's own accent identity, layered onto the
/// shared panel chrome. Deliberately tighter/fainter than a dropdown's: a hover
/// bubble should read as a light surface, not a panel demanding attention.
const BUBBLE_GLOW_RADIUS: f32 = 5.0;
const BUBBLE_GLOW_INTENSITY: f32 = 0.2;

/// A transparent wrapper that reveals a floating label on hover.
pub struct Tooltip {
    base: Base,
    text: Signal<String>,
    seen_text: String,
    side: TooltipSide,
    delay: f32,
    /// When the pointer entered the child (`None` = not hovering). The bubble shows
    /// once this is `delay` seconds in the past. Wall-clock (like the `Input` caret)
    /// so the host can sleep through the delay and wake once, rather than ticking
    /// every frame — see [`next_redraw`](Component::next_redraw).
    hover_since: Option<Instant>,
    /// Viewport cached at paint so `tick`/`damage_bounds` can place the bubble.
    viewport: Cell<Size>,
    /// Last-painted bubble visibility, to detect show/hide transitions in `tick`.
    last_shown: Cell<bool>,
}

#[heca_grid_ui_macros::props]
impl Tooltip {
    /// Wrap `child`, showing `text` on hover.
    pub fn new(child: impl Component + 'static, text: impl Into<String>) -> Self {
        let mut base = Base::new();
        // Hug the child so the wrapper's bounds match it (hover + anchor use them).
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.children.push(Box::new(child));
        let text = signal(text.into());
        Self {
            base,
            seen_text: text.get_untracked(),
            text,
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
            hover_since: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            last_shown: Cell::new(false),
        }
    }

    /// Wrap `child`, showing reactive `text` on hover.
    pub fn new_signal(child: impl Component + 'static, text: Signal<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.children.push(Box::new(child));
        Self {
            base,
            seen_text: text.get_untracked(),
            text,
            side: TooltipSide::default(),
            delay: DEFAULT_DELAY,
            hover_since: None,
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            last_shown: Cell::new(false),
        }
    }

    /// Which side of the target to anchor to (default [`TooltipSide::Top`]).
    #[heca_grid_ui_macros::prop]
    pub fn side(mut self, side: TooltipSide) -> Self {
        self.side = side;
        self
    }

    /// Hover delay before the bubble appears, in seconds (default `0.5`).
    #[heca_grid_ui_macros::prop]
    pub fn delay(mut self, seconds: f32) -> Self {
        self.delay = seconds.max(0.0);
        self
    }

    fn shown(&self) -> bool {
        self.hover_since
            .is_some_and(|since| since.elapsed().as_secs_f32() >= self.delay)
    }

    /// The bubble's text size (logical px) from the resolved font + label length.
    fn bubble_size(&self) -> (f64, f64) {
        let font = self.base.font;
        let chars = self.text.get_untracked().chars().count() as f64;
        let w = chars * (font * MONO_ADVANCE_RATIO) as f64 + 2.0 * PAD_X;
        let h = (font * MONO_LINE_RATIO) as f64 + 2.0 * PAD_Y;
        (w, h)
    }

    /// Where the bubble would draw for the cached viewport (`None` if no text).
    /// Used to damage the right region on show/hide — the bubble sits off our own
    /// bounds, on the overlay layer.
    fn current_bubble_rect(&self) -> Option<Rectangle> {
        if self.text.get_untracked().is_empty() {
            return None;
        }
        let (w, h) = self.bubble_size();
        Some(self.bubble_rect(self.base.bounds, w, h, self.viewport.get()))
    }

    /// The bubble rect for a `w×h` label anchored to target `b` within viewport `vp`.
    ///
    /// Delegates to the shared [`place_beside`] authority: centered on the chosen
    /// side, flipped when there is no room, cross-axis clamped. The bubble is
    /// **drawn**, never a parent to laid-out children, which is what makes a
    /// viewport-dependent rect safe here (see `place_beside`'s purity note).
    fn bubble_rect(&self, b: Rectangle, w: f64, h: f64, vp: Size) -> Rectangle {
        place_beside(b, Size::new(w, h), vp, GAP, self.side)
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
        // Cache the viewport so `tick`/`damage_bounds` place the bubble identically.
        self.viewport.set(cx.viewport());
        // The wrapped child first (still fully interactive).
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
        let text = self.text.get_untracked();
        if !self.shown() || text.is_empty() {
            return;
        }

        let (accent, glow_c, foreground, tip_border) = {
            let t = cx.theme();
            (
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.interaction.tooltip_border,
            )
        };
        let font = self.base.font;
        let (w, h) = self.bubble_size();
        let rect = self.bubble_rect(self.base.bounds, w, h, cx.viewport());

        // Drawn on the overlay layer so it sits above later siblings.
        cx.with_overlay(|cx| {
            // The shared panel painter owns the surface: fill, drop shadow, and the
            // edge the user configured via `overlay_border_style`. The tooltip
            // supplies only its own identity — its accent edge colour and halo.
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
                    // A hover bubble is not at dialog depth: the panel shadow was
                    // tuned for surfaces hundreds of px across and, unscaled, is
                    // bigger than this ~30px bubble.
                    elevation: PanelElevation::Hover,
                },
            );
            cx.text(rect, &text, foreground, font, TextAlign::Center, TextStyle::REGULAR);
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Track hover, then route to the child (transparent — never consumes).
        if let Event::PointerMoved { pos } = ev {
            let inside = self.base.bounds.contains(*pos);
            if inside != self.hover_since.is_some() {
                // Enter starts the reveal clock; leave clears it (the next `tick`
                // detects the show/hide transition and damages the bubble).
                self.hover_since = inside.then(Instant::now);
            }
        }
        route_event(&mut self.base.children, ev)
    }

    fn tick(&mut self, dt: f32) -> bool {
        let next_text = self.text.get_untracked();
        if next_text != self.seen_text {
            self.seen_text = next_text;
            self.base.mark_needs_paint();
        }
        // The bubble's reveal is timed (wall-clock), not a continuous animation:
        // repaint only when it crosses the show/hide boundary, and damage just the
        // bubble (via `damage_bounds`) instead of forcing a full frame.
        let now_shown = self.shown();
        if now_shown != self.last_shown.get() {
            self.last_shown.set(now_shown);
            self.base.mark_needs_paint();
        }
        // Children still animate normally (they mark their own rects).
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        // Keep the host ticking through the reveal-delay window so the show boundary
        // is reached on time. Otherwise, in a `WaitUntil`-driven host (where redraws
        // only continue while something animates), the bubble wouldn't appear until
        // the next unrelated event — e.g. the user nudging the mouse a second time.
        // (`next_redraw` offers the precise wake; this keeps it correct even when the
        // host schedules frames purely off `tick`'s animating flag.)
        let pending_reveal = self.hover_since.is_some() && !now_shown;
        animating || pending_reveal
    }

    fn next_redraw(&self) -> Option<f32> {
        // While hovering pre-reveal, wake the host exactly when the bubble appears.
        let mut soonest = self.hover_since.and_then(|since| {
            let e = since.elapsed().as_secs_f32();
            (e < self.delay).then_some(self.delay - e)
        });
        for child in self.base.children.iter() {
            soonest = soonest_redraw(soonest, child.next_redraw());
        }
        soonest
    }

    fn damage_bounds(&self) -> Rectangle {
        // The bubble draws on the overlay layer, offset from our own bounds, so a
        // show/hide repaint must cover it (plus our bounds, harmlessly).
        match self.current_bubble_rect() {
            Some(bubble) => union(self.base.bounds, bubble),
            None => self.base.bounds,
        }
    }
}

impl LayoutExt for Tooltip {}
impl StyleExt for Tooltip {}
impl Parent for Tooltip {}
