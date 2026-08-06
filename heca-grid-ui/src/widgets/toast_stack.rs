//! [`ToastStack`] — an overlay manager that arranges a host-supplied set of
//! notifications into a corner stack.
//!
//! **Presentation only**, like every grid-ui widget: it does *not* own the
//! notification queue, lifetimes, auto-dismiss timers, or semantic dedup policy —
//! that business logic is the **host application's** job. The host owns a
//! [`Signal<Vec<ToastSpec>>`](crate::reactive::Signal) (its render list); the
//! stack reflects it each frame, **reconciling cached [`Toast`] widgets by id**.
//! Stable payloads retain the whole widget; changed payloads rebuild only that
//! entry while preserving its transition and surviving control state. The stack
//! corner-anchors cards on the scene's **overlay layer**, animates them in/out, routes
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
use crate::component::{shift_subtree, Base, Component, Event, Handled, PaintCx};
use crate::hint::HintTargetId;
use crate::layout::LayoutEngine;
use crate::reactive::{Signal, SignalGet};
use crate::style::Length;
use crate::widgets::{Glyph, Toast, ToastSeverity};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// Toast card width (logical px) owned by the viewport stack. Inline [`Toast`] widgets hug content.
const TOAST_W: f32 = 320.0;
/// Gap between stacked toasts.
const DEFAULT_GAP: f32 = 10.0;
/// Inset from the viewport edges.
const DEFAULT_MARGIN: f32 = 16.0;
/// Seconds for a toast to slide fully into/out of place or reflow to a new slot.
const TRANSITION_DURATION: f32 = 0.20;

/// Smoothstep keeps slot reflow at rest at both ends instead of snapping into place.
fn smoothstep(progress: f32) -> f32 {
    progress * progress * (3.0 - 2.0 * progress)
}

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
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    fn is_right(self) -> bool {
        matches!(self, ToastCorner::TopRight | ToastCorner::BottomRight)
    }
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
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
    /// Opaque host-owned target for the action button. Ignored without `action`.
    pub action_target: Option<HintTargetId>,
    /// Whether the × dismiss affordance is shown.
    pub dismissible: bool,
    /// Opaque host-owned target for dismiss. Ignored when not dismissible.
    pub dismiss_target: Option<HintTargetId>,
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
            action_target: None,
            dismissible: true,
            dismiss_target: None,
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

    /// Attach an opaque runtime target to the action button. The host retains
    /// the id→intent mapping; grid-ui only carries geometry and identity.
    pub fn action_target(mut self, id: HintTargetId) -> Self {
        self.action_target = Some(id);
        self
    }

    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Attach an opaque runtime target to the dismiss button.
    pub fn dismiss_target(mut self, id: HintTargetId) -> Self {
        self.dismiss_target = Some(id);
        self
    }
}

/// A cached toast widget + its transient animation state, keyed by spec id.
struct Entry {
    /// Last host snapshot realized into `toast`.
    spec: ToastSpec,
    toast: Toast,
    /// Visibility progress: 0.0 (outside) → 1.0 (settled).
    visibility: f32,
    /// Current animated Y position. `None` means the entry has not received its
    /// first corner slot yet.
    slot_y: Option<f64>,
    /// Reflow segment. A changed target starts again from the current position,
    /// so another removal during motion never teleports a card.
    reflow_from_y: f64,
    reflow_target_y: f64,
    reflow_progress: f32,
    /// Whether the id is present in the current host snapshot. Removed entries
    /// remain retained only until `visibility` reaches zero.
    present: bool,
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
            // Non-finite means "no painted viewport yet" and selects the
            // deterministic headless fallback in `slots()`.
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
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
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn(u64) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Called with the toast's id when its inline action is clicked.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
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
        if let Some(id) = spec.action_target {
            t = t.action_target(id);
        }
        t = t.dismissible(spec.dismissible);
        if let Some(id) = spec.dismiss_target {
            t = t.dismiss_target(id);
        }
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

    /// Reconcile cached entries against the host snapshot.
    ///
    /// IDs are keys, not immutable payloads: a matching entry keeps its transition
    /// progress and retained control state while changed spec data is realized into
    /// that one entry. Duplicate IDs collapse deterministically — the first key
    /// position is retained and its last payload wins. Removed entries become inert
    /// and remain only long enough to finish their slide-out; re-adding the same ID
    /// reverses that transition instead of creating a second card.
    fn reconcile(&self) {
        let raw_specs = self.items.get_untracked();
        let mut specs: Vec<&ToastSpec> = Vec::with_capacity(raw_specs.len());
        for spec in &raw_specs {
            if let Some(existing) = specs.iter_mut().find(|existing| existing.id == spec.id) {
                *existing = spec;
            } else {
                specs.push(spec);
            }
        }

        let mut entries = self.entries.borrow_mut();
        let before = entries
            .iter()
            .map(|entry| (entry.spec.id, entry.present))
            .collect::<Vec<_>>();
        let old = std::mem::take(&mut *entries);
        let mut active_slots: Vec<Option<Entry>> = Vec::with_capacity(old.len());
        let mut active = Vec::new();
        let mut seen = Vec::new();
        let mut changed = false;

        for mut entry in old {
            if seen.contains(&entry.spec.id) {
                // Heal caches produced by the old non-deduplicating reconciler.
                changed = true;
                continue;
            }
            seen.push(entry.spec.id);

            if specs.iter().any(|spec| spec.id == entry.spec.id) {
                if !entry.present {
                    entry.present = true;
                    changed = true;
                }
                active.push(entry);
                active_slots.push(None);
            } else {
                if entry.present {
                    entry.present = false;
                    changed = true;
                }
                active_slots.push(Some(entry));
            }
        }

        let mut ordered = Vec::with_capacity(specs.len());
        for spec in specs {
            if let Some(pos) = active.iter().position(|entry| entry.spec.id == spec.id) {
                let mut entry = active.remove(pos);
                if entry.spec != *spec {
                    let mut toast = self.build(spec);
                    toast.preserve_runtime_from(entry.toast);
                    entry.toast = toast;
                    entry.spec = spec.clone();
                    changed = true;
                }
                ordered.push(entry);
            } else {
                ordered.push(Entry {
                    spec: spec.clone(),
                    toast: self.build(spec),
                    visibility: 0.0,
                    slot_y: None,
                    reflow_from_y: 0.0,
                    reflow_target_y: 0.0,
                    reflow_progress: 1.0,
                    present: true,
                });
                changed = true;
            }
        }

        // Active entries follow host order, but exiting entries keep their old
        // ordinal slot until gone so neighbours do not jump before the animation ends.
        let mut ordered = ordered.into_iter();
        let mut next = Vec::with_capacity(active_slots.len() + raw_specs.len());
        for slot in active_slots {
            match slot {
                Some(exiting) => next.push(exiting),
                None => {
                    if let Some(active) = ordered.next() {
                        next.push(active);
                    }
                }
            }
        }
        next.extend(ordered);

        let after = next
            .iter()
            .map(|entry| (entry.spec.id, entry.present))
            .collect::<Vec<_>>();
        if before != after {
            changed = true;
        }
        *entries = next;
        if changed {
            self.base.mark_needs_paint();
        }
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
    ///
    /// A Toast's content is a real retained subtree. Running the layout engine
    /// here gives every Label/Button/Icon child honest bounds before the whole
    /// subtree is shifted into its viewport-anchored slot.
    fn layout(&self) {
        let mut entries = self.entries.borrow_mut();
        let font = self.base.font;
        let viewport = self.viewport.get();
        let available_h = if viewport.h.is_finite() { viewport.h } else { 1000.0 };

        let heights: Vec<f32> = entries
            .iter_mut()
            .map(|e| {
                e.toast.base_mut().style.layout.width = Length::Px(TOAST_W);
                LayoutEngine::new()
                    .base_font(font)
                    .compute(&mut e.toast, Size::new(TOAST_W as f64, available_h));
                e.toast.base().bounds.size.h as f32
            })
            .collect();
        let slots = self.slots(&heights);

        // Slide new toasts in from the anchored horizontal edge and ease existing
        // cards toward changed vertical slots. Shift the entire retained tree:
        // bounds remain the single source for paint, damage, hints, and input.
        let right = self.corner.is_right();
        for (entry, slot) in entries.iter_mut().zip(slots) {
            match entry.slot_y {
                None => {
                    entry.slot_y = Some(slot.loc.y);
                    entry.reflow_from_y = slot.loc.y;
                    entry.reflow_target_y = slot.loc.y;
                    entry.reflow_progress = 1.0;
                }
                Some(current_y) if (slot.loc.y - entry.reflow_target_y).abs() > f64::EPSILON => {
                    entry.reflow_from_y = current_y;
                    entry.reflow_target_y = slot.loc.y;
                    entry.reflow_progress = 0.0;
                }
                Some(_) => {}
            }

            let eased_visibility = smoothstep(entry.visibility);
            let off = (1.0 - eased_visibility) * (TOAST_W + self.margin);
            let slide = if right { off as f64 } else { -(off as f64) };
            let target = Point::new(
                slot.loc.x + slide,
                entry.slot_y.expect("toast slot initialized above"),
            );
            let current = entry.toast.base().bounds.loc;
            shift_subtree(&mut entry.toast, target.x - current.x, target.y - current.y);
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
        self.reconcile();
        self.entries.borrow().iter().any(|entry| entry.present)
    }
    fn overlay_active(&self) -> bool {
        self.reconcile();
        !self.entries.borrow().is_empty()
    }

    /// A toast **card** occludes the points it covers (the corner stack draws
    /// above the page), even though the stack as a whole doesn't grab input —
    /// a host must not synthesize a page-level action (e.g. open a context menu)
    /// under a card. Points between/outside the cards are not occluded. Uses the
    /// same `layout()` as `event`, so the answer matches the hit-testing.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.reconcile();
        if self.entries.borrow().is_empty() {
            return false;
        }
        self.layout();
        self.entries
            .borrow()
            .iter()
            .any(|e| e.toast.base().bounds.contains(pos))
    }

    /// Toasts are retained by id in `entries` rather than `Base.children`.
    /// Expose their real descendants to the framework collector so the global
    /// picker sees the same action/dismiss bounds used for paint and input.
    fn visit_hint_targets(&self, visitor: &mut dyn FnMut(HintTargetId, Rectangle)) {
        if !self.base.visible.get_untracked()
            || self.base.disabled.get_untracked()
            || self.base.style.layout.hidden
        {
            return;
        }
        if let Some(id) = self.as_hint_target() {
            visitor(id, self.base.bounds);
        }
        self.reconcile();
        self.layout();
        for entry in self.entries.borrow().iter().filter(|entry| entry.present) {
            entry.toast.visit_hint_targets(visitor);
        }
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

    /// Capture: the toasts are not `base.children` — they live in `entries`, reconciled by id — so
    /// there is no framework walk that could reach them. This is the walk.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        self.reconcile();
        if self.entries.borrow().is_empty() {
            return Handled::No;
        }
        self.layout();
        // Route to the toast under the pointer (press), or all (move, for hover).
        match ev {
            Event::PointerPressed { pos } | Event::PointerReleased { pos } => {
                let mut entries = self.entries.borrow_mut();
                for entry in entries.iter_mut() {
                    if entry.toast.base().bounds.contains(*pos) {
                        return if entry.present {
                            crate::component::dispatch(&mut entry.toast, ev)
                        } else {
                            // Still painted and occluding while it exits, but its
                            // host callbacks and targets are already unavailable.
                            Handled::Yes
                        };
                    }
                }
                // Missed every toast — let it fall through to the UI behind.
                Handled::No
            }
            Event::PointerMoved { .. } => {
                let mut entries = self.entries.borrow_mut();
                for e in entries.iter_mut().filter(|entry| entry.present) {
                    crate::component::dispatch(&mut e.toast, ev);
                }
                Handled::No
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.reconcile();
        // Establish current bounds and detect any slot targets changed by this
        // snapshot before capturing the old damage footprint.
        self.layout();
        let old_region = self
            .entries
            .borrow()
            .iter()
            .map(|entry| entry.toast.base().bounds)
            .reduce(union);

        let mut animating = false;
        let mut departed: Option<Rectangle> = None;
        let step = dt / TRANSITION_DURATION;
        {
            let mut entries = self.entries.borrow_mut();
            for entry in entries.iter_mut() {
                let transitioning = if entry.present {
                    entry.visibility < 1.0
                } else {
                    entry.visibility > 0.0
                };
                animating |= transitioning;
                if entry.present {
                    entry.visibility = (entry.visibility + step).min(1.0);
                } else {
                    entry.visibility = (entry.visibility - step).max(0.0);
                }

                if entry.reflow_progress < 1.0 {
                    animating = true;
                    entry.reflow_progress = (entry.reflow_progress + step).min(1.0);
                    let eased = smoothstep(entry.reflow_progress) as f64;
                    entry.slot_y = Some(
                        entry.reflow_from_y
                            + (entry.reflow_target_y - entry.reflow_from_y) * eased,
                    );
                }
                animating |= entry.toast.tick(dt);
            }

            entries.retain(|entry| {
                let keep = entry.present || entry.visibility > 0.0;
                if !keep {
                    let bounds = entry.toast.base().bounds;
                    departed = Some(departed.map_or(bounds, |region| union(region, bounds)));
                }
                keep
            });
        }

        // Re-layout after visibility movement/removal. A departed ordinal changes
        // the neighbours' target slots here; their current Y remains continuous and
        // the next ticks ease them toward those targets.
        self.layout();
        animating |= self
            .entries
            .borrow()
            .iter()
            .any(|entry| entry.reflow_progress < 1.0);

        // Damage must contain BOTH sides of every move. Marking only the new bounds
        // leaves stale fragments where a sliding/reflowing card was last frame.
        if animating {
            let mut region = old_region.or(departed);
            for entry in self.entries.borrow().iter() {
                let bounds = entry.toast.base().bounds;
                region = Some(region.map_or(bounds, |current| union(current, bounds)));
            }
            if let Some(region) = region {
                self.base.bounds = region;
                self.base.mark_needs_paint();
            }
        }
        animating
    }
}

impl LayoutExt for ToastStack {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, PaintCx};
    use crate::hint::collect_hint_targets;
    use crate::reactive::{signal, SignalUpdate};
    use crate::scene::{DrawCommand, Scene};
    use crate::theme::Theme;

    #[test]
    fn same_id_update_refreshes_every_field_without_resetting_enter_or_focus() {
        let old_action = HintTargetId::new(10);
        let new_action = HintTargetId::new(11);
        let old_dismiss = HintTargetId::new(12);
        let items = signal(vec![
            ToastSpec::new(7, "Connecting")
                .action("Cancel")
                .action_target(old_action)
                .dismiss_target(old_dismiss),
        ]);
        let mut stack = ToastStack::new(items).corner(ToastCorner::TopLeft);
        stack.tick(TRANSITION_DURATION / 2.0);
        {
            let mut entries = stack.entries.borrow_mut();
            let content = entries[0].toast.base_mut().children[1].base_mut();
            let action = content.children.last_mut().expect("the old action control");
            action.base_mut().focused.set(true);
            action.base_mut().focus_visible.set(true);
        }

        let updated = ToastSpec::new(7, "Build failed")
            .severity(ToastSeverity::Danger)
            .icon(Glyph::WarningCircle)
            .body("3 errors")
            .action("Open log")
            .action_target(new_action)
            .dismissible(false);
        items.set(vec![updated.clone()]);
        stack.tick(0.0);

        let entries = stack.entries.borrow();
        assert_eq!(entries.len(), 1, "an update never duplicates its keyed entry");
        assert_eq!(entries[0].spec, updated);
        assert!((entries[0].visibility - 0.5).abs() < f32::EPSILON);
        let action = entries[0].toast.base().children[1]
            .base()
            .children
            .last()
            .expect("the refreshed action control");
        assert!(action.base().focused.get_untracked(), "focus follows the logical action");
        assert!(action.base().focus_visible.get_untracked());
        drop(entries);

        assert_eq!(
            collect_hint_targets(&stack)
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec![new_action],
            "changed targets replace stale ones and hidden dismiss contributes none",
        );

        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme)
                .with_viewport(Size::new(800.0, 600.0));
            stack.paint(&mut cx);
        }
        let texts = scene
            .iter()
            .filter_map(|command| match command {
                DrawCommand::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.contains(&"Build failed"), "painted text: {texts:?}");
        assert!(
            texts.iter().any(|text| text.starts_with("3 erro")),
            "painted text: {texts:?}",
        );
        assert!(texts.contains(&"Open log"), "painted text: {texts:?}");
        assert!(!texts.contains(&"Connecting"), "painted text: {texts:?}");
        assert!(!texts.contains(&"Cancel"), "painted text: {texts:?}");
        let expected = theme.colors.surface.lerp(
            theme.colors.danger,
            theme.colors.interaction.toast_tint as f32 / 255.0,
        );
        assert!(
            scene
                .iter()
                .any(|command| matches!(command, DrawCommand::Rect(rect) if rect.fill == expected)),
            "the refreshed severity drives the card tone",
        );

        stack.tick(TRANSITION_DURATION);
        let steady = ToastSpec::new(7, "Build fixed")
            .severity(ToastSeverity::Success)
            .dismiss_target(HintTargetId::new(13));
        items.set(vec![steady.clone()]);
        stack.tick(0.0);
        let entries = stack.entries.borrow();
        assert_eq!(entries[0].spec, steady, "steady entries refresh too");
        assert_eq!(entries[0].visibility, 1.0, "steady updates do not replay enter");
    }

    #[test]
    fn exit_damage_covers_previous_and_current_bounds() {
        let items = signal(vec![ToastSpec::new(1, "Moving out")]);
        let mut stack = ToastStack::new(items);
        let theme = Theme::default();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme)
                .with_viewport(Size::new(800.0, 600.0));
            stack.paint(&mut cx);
        }
        stack.tick(TRANSITION_DURATION);
        stack.layout();
        let old = stack.entries.borrow()[0].toast.base().bounds;

        items.set(Vec::new());
        stack.tick(TRANSITION_DURATION / 2.0);
        let current = stack.entries.borrow()[0].toast.base().bounds;
        let damage = stack.base.bounds;

        assert!(current.loc.x > old.loc.x, "a top-right exit slides right");
        assert!(damage.loc.x <= old.loc.x, "damage starts at the vacated pixels");
        assert!(
            damage.loc.x + damage.size.w >= current.loc.x + current.size.w,
            "damage also reaches the card's new edge",
        );
    }

    #[test]
    fn departed_slot_reflows_neighbours_smoothly() {
        let items = signal(vec![ToastSpec::new(1, "First"), ToastSpec::new(2, "Second")]);
        let mut stack = ToastStack::new(items);
        stack.tick(TRANSITION_DURATION);
        stack.layout();
        let old_y = stack.entries.borrow()[1]
            .slot_y
            .expect("the second toast has an initial slot");

        items.set(vec![ToastSpec::new(2, "Second")]);
        stack.tick(TRANSITION_DURATION);
        let after_exit = stack.entries.borrow()[0]
            .slot_y
            .expect("the retained toast keeps its current position");
        assert_eq!(after_exit, old_y, "reflow starts without teleporting");

        stack.tick(TRANSITION_DURATION / 2.0);
        let halfway = stack.entries.borrow()[0]
            .slot_y
            .expect("the retained toast is moving to its new slot");
        let target = stack.margin as f64;
        assert!(halfway < old_y && halfway > target, "halfway={halfway}, target={target}");

        stack.tick(TRANSITION_DURATION / 2.0);
        let settled = stack.entries.borrow()[0]
            .slot_y
            .expect("the retained toast reaches its new slot");
        assert!((settled - target).abs() < f64::EPSILON);
    }

    #[test]
    fn duplicate_ids_collapse_and_an_exiting_entry_can_be_revived() {
        let stale = ToastSpec::new(4, "Old").action("Old action");
        let latest = ToastSpec::new(4, "Latest").action("Latest action");
        let items = signal(vec![stale, latest.clone()]);
        let mut stack = ToastStack::new(items);
        stack.tick(TRANSITION_DURATION);
        {
            let entries = stack.entries.borrow();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].spec, latest, "last payload wins at the first key position");
            assert_eq!(entries[0].visibility, 1.0);
        }

        items.set(Vec::new());
        stack.tick(TRANSITION_DURATION / 2.0);
        {
            let entries = stack.entries.borrow();
            assert_eq!(entries.len(), 1, "removal retains one inert exit animation");
            assert!(!entries[0].present);
            assert!((entries[0].visibility - 0.5).abs() < f32::EPSILON);
        }
        assert!(collect_hint_targets(&stack).is_empty(), "exiting cards are not actionable");
        stack.layout();
        let exit_bounds = stack.entries.borrow()[0].toast.base().bounds;
        let exit_hit = crate::component::dispatch(
            &mut stack,
            &Event::PointerPressed {
                pos: Point::new(
                    exit_bounds.loc.x + exit_bounds.size.w / 2.0,
                    exit_bounds.loc.y + exit_bounds.size.h / 2.0,
                ),
            },
        );
        assert_eq!(
            exit_hit,
            Handled::Yes,
            "a visible exiting card occludes the page without firing its stale actions",
        );

        let revived = ToastSpec::new(4, "Back online")
            .action("Details")
            .action_target(HintTargetId::new(20));
        items.set(vec![revived.clone(), revived.clone()]);
        stack.tick(0.0);
        {
            let entries = stack.entries.borrow();
            assert_eq!(entries.len(), 1, "revival reuses the exiting keyed entry");
            assert!(entries[0].present);
            assert_eq!(entries[0].spec, revived);
            assert!((entries[0].visibility - 0.5).abs() < f32::EPSILON);
        }

        items.set(Vec::new());
        stack.tick(TRANSITION_DURATION);
        assert!(stack.entries.borrow().is_empty(), "a completed exit releases the entry");
        assert!(!stack.overlay_active());
    }
}
