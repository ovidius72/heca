//! [`ToastStack`] — an overlay manager that arranges a host-supplied set of
//! notifications into a corner stack.
//!
//! **Presentation only**, like every grid-ui widget: it does *not* own the
//! notification queue, lifetimes, auto-dismiss timers, or dedup — that business
//! logic is the **host application's** job. The host owns a
//! [`Signal<Vec<ToastSpec>>`](crate::reactive::Signal) (its render list); the
//! stack reflects it each frame, **reconciling cached [`Toast`] widgets by id**
//! (so each toast keeps its hover/flash state and isn't rebuilt), corner-anchors
//! them on the scene's **overlay layer**, animates new ones sliding in, routes
//! pointer events to the toast under the cursor, and reports interactions back via
//! [`on_dismiss(id)`](ToastStack::on_dismiss) / [`on_action(id)`](ToastStack::on_action).
//! The host then removes the id from its list (which reflows the rest).
//!
//! Input contract: it reports [`overlay_active`](Component::overlay_active) while
//! it has toasts, so the host routes pointer/keys to it first (see
//! [`FocusManager`](crate::focus::FocusManager)); it **consumes** only clicks that
//! land on a toast and passes everything else through (`Handled::No`), so toasts
//! never block the UI behind them.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::reactive::{Signal, SignalGet};
use crate::style::Length;
use crate::widgets::{Glyph, Toast, ToastSeverity};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Toast card width (logical px) — matches [`Toast`]'s default.
const TOAST_W: f32 = 320.0;
/// Gap between stacked toasts.
const DEFAULT_GAP: f32 = 10.0;
/// Inset from the viewport edges.
const DEFAULT_MARGIN: f32 = 16.0;
/// Seconds for a new toast to slide fully into place.
const ENTER_DURATION: f32 = 0.16;

/// Smallest rect containing both `a` and `b` (for the stack's damage region).
fn union(a: Rectangle, b: Rectangle) -> Rectangle {
    let x0 = a.loc.x.min(b.loc.x);
    let y0 = a.loc.y.min(b.loc.y);
    let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
    let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
    Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
}

/// Which viewport corner the stack anchors to (and the direction it grows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastCorner {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[heca_grid_ui_macros::props]
impl ToastCorner {
    fn is_right(self) -> bool {
        matches!(self, ToastCorner::TopRight | ToastCorner::BottomRight)
    }
    fn is_top(self) -> bool {
        matches!(self, ToastCorner::TopRight | ToastCorner::TopLeft)
    }
}

/// A single notification the host wants shown — plain data (no callbacks). The
/// host owns these in a `Signal<Vec<ToastSpec>>`; the stack renders them.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastSpec {
    /// Stable identity — used to reconcile widgets across frames and reported
    /// back by `on_dismiss`/`on_action`.
    pub id: u64,
    pub severity: ToastSeverity,
    pub icon: Option<Glyph>,
    pub title: String,
    pub body: Option<String>,
    /// Inline action button label, if any.
    pub action: Option<String>,
    /// Whether the × dismiss affordance is shown.
    pub dismissible: bool,
}

impl ToastSpec {
    /// A new info spec with `id` + `title`. Chain the setters for the rest.
    pub fn new(id: u64, title: impl Into<String>) -> Self {
        Self {
            id,
            severity: ToastSeverity::Info,
            icon: None,
            title: title.into(),
            body: None,
            action: None,
            dismissible: true,
        }
    }
    #[heca_grid_ui_macros::prop]
    pub fn severity(mut self, s: ToastSeverity) -> Self {
        self.severity = s;
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, g: Glyph) -> Self {
        self.icon = Some(g);
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn body(mut self, b: impl Into<String>) -> Self {
        self.body = Some(b.into());
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn action(mut self, label: impl Into<String>) -> Self {
        self.action = Some(label.into());
        self
    }
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }
}

/// A cached toast widget + its transient animation state, keyed by spec id.
struct Entry {
    id: u64,
    toast: Toast,
    /// Slide-in progress, 0.0 (just arrived) → 1.0 (settled).
    enter: f32,
}

/// An overlay that stacks host-supplied toasts in a corner. Presentation only.
pub struct ToastStack {
    base: Base,
    items: Signal<Vec<ToastSpec>>,
    corner: ToastCorner,
    gap: f32,
    margin: f32,
    on_dismiss: Option<Rc<dyn Fn(u64)>>,
    on_action: Option<Rc<dyn Fn(u64)>>,
    /// Cached toasts (reconciled against `items` by id), in display order.
    entries: RefCell<Vec<Entry>>,
    /// Viewport cached at paint so `event` lays out identically.
    viewport: Cell<Size>,
}

impl ToastStack {
    /// A new stack bound to the host's `items` signal (the render list it owns).
    pub fn new(items: Signal<Vec<ToastSpec>>) -> Self {
        Self {
            base: Base::new(),
            items,
            corner: ToastCorner::default(),
            gap: DEFAULT_GAP,
            margin: DEFAULT_MARGIN,
            on_dismiss: None,
            on_action: None,
            entries: RefCell::new(Vec::new()),
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
        }
    }

    /// Which viewport corner to anchor to (default [`ToastCorner::TopRight`]).
    #[heca_grid_ui_macros::prop]
    pub fn corner(mut self, corner: ToastCorner) -> Self {
        self.corner = corner;
        self
    }

    /// Gap between stacked toasts (logical px).
    #[heca_grid_ui_macros::prop]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Inset from the viewport edges (logical px).
    #[heca_grid_ui_macros::prop]
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    /// Called with the toast's id when its × is clicked. The host removes the id
    /// from its list (the stack reflows the rest).
    pub fn on_dismiss(mut self, f: impl Fn(u64) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Called with the toast's id when its inline action is clicked.
    pub fn on_action(mut self, f: impl Fn(u64) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// Build a [`Toast`] from a spec, wiring its action/dismiss to the stack's
    /// id-tagged callbacks.
    fn build(&self, spec: &ToastSpec) -> Toast {
        let mut t = Toast::new(spec.title.clone()).severity(spec.severity);
        if let Some(g) = spec.icon {
            t = t.icon(g);
        }
        if let Some(b) = &spec.body {
            t = t.body(b.clone());
        }
        if let Some(label) = &spec.action {
            let cb = self.on_action.clone();
            let id = spec.id;
            t = t.action(label.clone(), move || {
                if let Some(f) = &cb {
                    f(id);
                }
            });
        }
        t = t.dismissible(spec.dismissible);
        if spec.dismissible {
            let cb = self.on_dismiss.clone();
            let id = spec.id;
            t = t.on_dismiss(move || {
                if let Some(f) = &cb {
                    f(id);
                }
            });
        }
        t
    }

    /// Reconcile the cached entries against the host `items` list: keep existing
    /// toasts (by id, preserving their state), build new ones, drop removed ones,
    /// and reorder to match. Returns whether anything changed.
    fn reconcile(&self) {
        let specs = self.items.get_untracked();
        let mut entries = self.entries.borrow_mut();
        // Drop entries whose id is gone from the list.
        entries.retain(|e| specs.iter().any(|s| s.id == e.id));
        // Add + reorder to match the spec order.
        let mut next: Vec<Entry> = Vec::with_capacity(specs.len());
        for spec in &specs {
            if let Some(pos) = entries.iter().position(|e| e.id == spec.id) {
                next.push(entries.remove(pos));
            } else {
                next.push(Entry {
                    id: spec.id,
                    toast: self.build(spec),
                    enter: 0.0,
                });
            }
        }
        *entries = next;
    }

    /// Lay the entries out into corner-anchored slots, returning each slot rect
    /// (in viewport space). `heights[i]` is entry `i`'s measured height.
    fn slots(&self, heights: &[f32]) -> Vec<Rectangle> {
        let vp = self.viewport.get();
        let (vw, vh) = if vp.w.is_finite() {
            (vp.w as f32, vp.h as f32)
        } else {
            (TOAST_W + 2.0 * self.margin, 1000.0)
        };
        let x = if self.corner.is_right() {
            vw - self.margin - TOAST_W
        } else {
            self.margin
        };
        let mut out = Vec::with_capacity(heights.len());
        if self.corner.is_top() {
            let mut y = self.margin;
            for &h in heights {
                out.push(Rectangle::new(
                    Point::new(x as f64, y as f64),
                    Size::new(TOAST_W as f64, h as f64),
                ));
                y += h + self.gap;
            }
        } else {
            let mut y = vh - self.margin;
            for &h in heights {
                y -= h;
                out.push(Rectangle::new(
                    Point::new(x as f64, y as f64),
                    Size::new(TOAST_W as f64, h as f64),
                ));
                y -= self.gap;
            }
        }
        out
    }

    /// Measure + position every cached toast into its slot (with the slide-in
    /// offset applied). Shared by `paint` and `event` so hit-testing matches.
    fn layout(&self) {
        let mut entries = self.entries.borrow_mut();
        let font = self.base.font;
        // Measure each toast's height from its resolved font.
        let heights: Vec<f32> = entries
            .iter_mut()
            .map(|e| {
                e.toast.base_mut().font = font;
                e.toast.base_mut().style.layout.width = Length::Px(TOAST_W);
                e.toast.remeasure();
                match e.toast.base().style.layout.height {
                    Length::Px(h) => h,
                    _ => font,
                }
            })
            .collect();
        let slots = self.slots(&heights);
        // Slide new toasts in from the anchored horizontal edge.
        let right = self.corner.is_right();
        for (e, slot) in entries.iter_mut().zip(slots) {
            let off = (1.0 - e.enter) * (TOAST_W + self.margin);
            let dx = if right { off as f64 } else { -(off as f64) };
            e.toast.base_mut().bounds =
                Rectangle::new(Point::new(slot.loc.x + dx, slot.loc.y), slot.size);
        }
    }
}

impl Component for ToastStack {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable + overlay-active while it has toasts, so it gets first dibs on
    /// input (and passes through clicks that miss every toast).
    fn focusable(&self) -> bool {
        !self.entries.borrow().is_empty()
    }
    fn overlay_active(&self) -> bool {
        !self.entries.borrow().is_empty()
    }

    /// A toast **card** occludes the points it covers (the corner stack draws
    /// above the page), even though the stack as a whole doesn't grab input —
    /// a host must not synthesize a page-level action (e.g. open a context menu)
    /// under a card. Points between/outside the cards are not occluded. Uses the
    /// same `layout()` as `event`, so the answer matches the hit-testing.
    fn overlay_occludes(&self, pos: Point) -> bool {
        if self.entries.borrow().is_empty() {
            return false;
        }
        self.layout();
        self.entries
            .borrow()
            .iter()
            .any(|e| e.toast.base().bounds.contains(pos))
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        self.viewport.set(cx.viewport());
        self.reconcile();
        if self.entries.borrow().is_empty() {
            return;
        }
        self.layout();
        cx.with_overlay(|cx| {
            for e in self.entries.borrow().iter() {
                e.toast.paint(cx);
            }
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.entries.borrow().is_empty() {
            return Handled::No;
        }
        self.layout();
        // Route to the toast under the pointer (press), or all (move, for hover).
        match ev {
            Event::PointerPressed { pos } | Event::PointerReleased { pos } => {
                let mut entries = self.entries.borrow_mut();
                for e in entries.iter_mut() {
                    if e.toast.base().bounds.contains(*pos) {
                        return e.toast.event(ev);
                    }
                }
                // Missed every toast — let it fall through to the UI behind.
                Handled::No
            }
            Event::PointerMoved { .. } => {
                let mut entries = self.entries.borrow_mut();
                for e in entries.iter_mut() {
                    e.toast.event(ev);
                }
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.reconcile();
        let mut animating = false;
        let step = dt / ENTER_DURATION;
        for e in self.entries.borrow_mut().iter_mut() {
            if e.enter < 1.0 {
                e.enter = (e.enter + step).min(1.0);
                animating = true;
            }
            animating |= e.toast.tick(dt);
        }

        // Report a tight damage region so the slide repaints only the toast corner —
        // not the whole frame. Without this the host's safety net treats an animating-
        // but-undamaged stack as a full-frame repaint, re-emitting the entire scene
        // every frame (the slide lag). Lay the toasts out for *this* frame and union
        // their bounds; `collect_damage`'s padding absorbs the per-frame slide delta
        // (and the strip the toast vacates as it moves into place).
        if animating {
            self.layout();
            let mut region: Option<Rectangle> = None;
            for e in self.entries.borrow().iter() {
                let b = e.toast.base().bounds;
                region = Some(region.map_or(b, |r| union(r, b)));
            }
            if let Some(r) = region {
                self.base.bounds = r;
                self.base.mark_needs_paint();
            }
        }
        animating
    }
}

impl LayoutExt for ToastStack {}
