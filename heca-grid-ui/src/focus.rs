//! Keyboard focus traversal for a component tree.
//!
//! [`FocusManager`] tracks which focusable component (by depth-first z-order
//! index) currently holds focus, moves focus on Tab/Shift+Tab (wrapping), and
//! delivers key events to the focused component. Each component's focus state
//! lives in `Base.focused`, so widgets can render a focus ring reactively.

use crate::component::{Component, Event, GridKey, Handled};
use crate::reactive::SignalGet;
use heca_core::layout::Point;

/// Visit every focusable component depth-first, calling `f(index, component)`.
///
/// A subtree hidden via `style.hidden` (taffy `display: none`, e.g. a collapsed
/// [`ItemGroup`](crate::widgets::ItemGroup)/[`DockFrame`](crate::widgets::DockFrame))
/// is skipped entirely — matching the web, where `display: none` removes an
/// element and its descendants from the tab order — so collapsed content can't
/// be Tab-focused or receive key events while invisible.
///
/// A subtree under a [`Base::focus_barrier`](crate::component::Base::focus_barrier) is not
/// descended into: the barrier widget itself is visited (if focusable), its children never are.
/// That's what makes a control which *composes* its content — a [`Button`](crate::widgets::Button)
/// holding an `Icon` + `Label`, or any deeper tree — stay exactly **one** Tab stop, matching the
/// single click target its `event` implements.
fn for_each_focusable(
    c: &mut dyn Component,
    idx: &mut usize,
    f: &mut dyn FnMut(usize, &mut dyn Component),
) {
    if c.base().style.layout.hidden {
        return;
    }
    if c.focusable() {
        f(*idx, c);
        *idx += 1;
    }
    if c.base().focus_barrier {
        return;
    }
    let count = c.base().children.len();
    for i in 0..count {
        let child = c.base_mut().children[i].as_mut();
        for_each_focusable(child, idx, f);
    }
}

/// Tracks and moves keyboard focus across a component tree.
#[derive(Default)]
pub struct FocusManager {
    focused: Option<usize>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current focus index, if any.
    pub fn focused(&self) -> Option<usize> {
        self.focused
    }

    /// The visit indices of all focusables in **Tab order**: those with an
    /// explicit `tab_index` first (ascending), then unindexed ones in tree
    /// position order. Returns visit indices (as used by [`apply`](Self::apply)).
    fn tab_order(root: &mut dyn Component) -> Vec<usize> {
        let mut items: Vec<(usize, i32)> = Vec::new();
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            items.push((i, c.base().tab_index.unwrap_or(i32::MAX)));
        });
        // Sort by (tab_index, visit position); both keys are already in `items`.
        items.sort_by_key(|&(visit, ti)| (ti, visit));
        items.into_iter().map(|(visit, _)| visit).collect()
    }

    /// Move focus to the next (`forward = true`) or previous focusable component
    /// in Tab order, wrapping at the ends. Updates each component's `focused`
    /// signal.
    pub fn advance(&mut self, root: &mut dyn Component, forward: bool) {
        let order = Self::tab_order(root);
        let n = order.len();
        if n == 0 {
            self.focused = None;
            return;
        }
        // Current position within the Tab order (by visit index identity).
        let pos = self
            .focused
            .and_then(|f| order.iter().position(|&v| v == f));
        let next_pos = match pos {
            None => {
                if forward {
                    0
                } else {
                    n - 1
                }
            }
            Some(p) => {
                if forward {
                    (p + 1) % n
                } else {
                    (p + n - 1) % n
                }
            }
        };
        self.apply(root, Some(order[next_pos]), true); // keyboard focus → show ring
    }

    /// Deliver a key press to the focused component. Returns whether it consumed it.
    pub fn deliver_key(&mut self, root: &mut dyn Component, key: GridKey) -> Handled {
        self.deliver_event(root, &Event::Key { key, pressed: true })
    }

    /// Deliver an arbitrary event to the focused component only. Returns whether it
    /// consumed it. Used for field-first delivery of semantic events (e.g. a
    /// [`Event::Widget`](crate::component::Event::Widget) intent forwarded to the
    /// focused text field inside a `Dialog`).
    pub fn deliver_event(&mut self, root: &mut dyn Component, ev: &Event) -> Handled {
        let Some(target) = self.focused else {
            return Handled::No;
        };
        let mut handled = Handled::No;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if i == target {
                handled = c.event(ev);
            }
        });
        handled
    }

    /// Index of the first focusable with an open overlay, if any.
    fn overlay_index(root: &mut dyn Component) -> Option<usize> {
        let mut found = None;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if found.is_none() && c.overlay_active() {
                found = Some(i);
            }
        });
        found
    }

    /// Whether any focusable currently has an open overlay (dropdown/popover).
    /// The host checks this to give the overlay first dibs on pointer/key input.
    pub fn overlay_active(&self, root: &mut dyn Component) -> bool {
        Self::overlay_index(root).is_some()
    }

    /// Offer an event to an open overlay first. Returns `Handled::Yes` only if an
    /// overlay is open **and** it actually consumed the event.
    ///
    /// This is the single "overlay first-dibs + consume-when-handled" gate every
    /// event type shares: a grabbing overlay (Modal/dropdown) swallows the input,
    /// while a non-grabbing one (e.g. a [`ToastStack`](crate::widgets::ToastStack))
    /// returns `Handled::No` so the event falls through to the content behind it.
    /// Route input through here before any default/host handling.
    pub fn offer_to_overlay(&self, root: &mut dyn Component, ev: &Event) -> Handled {
        if self.overlay_active(root) {
            self.deliver_to_overlay(root, ev)
        } else {
            Handled::No
        }
    }

    /// Deliver an event to the overlay-active focusable (if any) so it can
    /// capture input outside its layout bounds (e.g. clicks on dropdown rows).
    /// Returns `Handled::Yes` if consumed.
    pub fn deliver_to_overlay(&self, root: &mut dyn Component, ev: &Event) -> Handled {
        let Some(target) = Self::overlay_index(root) else {
            return Handled::No;
        };
        let mut handled = Handled::No;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if i == target {
                handled = c.event(ev);
            }
        });
        handled
    }

    /// Focus the first focusable **without** showing the focus ring (focus-visible = false).
    /// For programmatic initial focus — e.g. a modal's safe-default button — so Enter/activation
    /// works immediately but the ring appears only once the user navigates by keyboard
    /// ([`advance`](Self::advance) shows it). Mirrors the web's focus-visible behaviour.
    pub fn focus_first_quiet(&mut self, root: &mut dyn Component) {
        let target = Self::tab_order(root).first().copied();
        self.apply(root, target, false);
    }

    /// Clear focus (e.g. on Escape).
    pub fn clear(&mut self, root: &mut dyn Component) {
        self.apply(root, None, false);
    }

    /// Index of the top-most focusable component containing `pos` (last match
    /// wins = top-most in z-order), or `None` if the point misses every focusable.
    fn hit_test(root: &mut dyn Component, pos: Point) -> Option<usize> {
        let mut hit = None;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if c.base().bounds.contains(pos) {
                hit = Some(i);
            }
        });
        hit
    }

    /// Focus the top-most focusable component containing `pos` (e.g. on a mouse
    /// click); **clears** focus if the click misses every focusable. This is the
    /// page-level "click empty space to blur" semantics.
    pub fn focus_at(&mut self, root: &mut dyn Component, pos: Point) {
        let hit = Self::hit_test(root, pos);
        self.apply(root, hit, false); // mouse focus → no ring (focus-visible)
    }

    /// Trapped-focus variant of [`focus_at`](Self::focus_at): a click that hits a
    /// focusable moves focus to it, but a click that **misses** every focusable is
    /// a **no-op for focus** — the current focus is kept, not cleared. Use this
    /// where focus is trapped inside an overlay panel (a modal / dropdown): clicking
    /// the panel *body* (not a control) must not blur the focused control.
    pub fn focus_at_trapped(&mut self, root: &mut dyn Component, pos: Point) {
        if let Some(hit) = Self::hit_test(root, pos) {
            self.apply(root, Some(hit), false); // mouse focus → no ring (focus-visible)
        }
        // Miss inside a trapped panel → keep the current focus.
    }

    /// Route a **pointer/scroll** event through the tree with overlay-first dibs
    /// and the manager's standard focus semantics, returning whether it was
    /// consumed so the host can fall back (e.g. page-scroll on an unconsumed
    /// [`Event::Scroll`](crate::component::Event::Scroll)).
    ///
    /// - An open overlay gets first dibs (see [`offer_to_overlay`](Self::offer_to_overlay));
    ///   if it consumes, routing stops and returns `Handled::Yes`.
    /// - [`Event::PointerPressed`](crate::component::Event::PointerPressed) focuses
    ///   the clicked widget (clearing focus on a miss), then delivers the press.
    /// - Any other event (pointer move, scroll, …) is delivered to the tree as-is.
    ///
    /// Key events are intentionally **not** routed here: the host owns key meaning
    /// (in the app, `config.toml` → keymap → action runs first), so it resolves its
    /// own bindings and uses [`offer_to_overlay`](Self::offer_to_overlay) +
    /// [`deliver_key`](Self::deliver_key) for the leftovers.
    pub fn dispatch(&mut self, root: &mut dyn Component, ev: &Event) -> Handled {
        self.dispatch_inner(root, ev, false)
    }

    /// Trapped-focus variant of [`dispatch`](Self::dispatch): identical routing, but
    /// a [`PointerPressed`](crate::component::Event::PointerPressed) that misses every
    /// focusable **keeps** the current focus instead of clearing it (see
    /// [`focus_at_trapped`](Self::focus_at_trapped)). For modal/overlay panels that
    /// trap focus — clicking the panel body must not blur the focused control.
    pub fn dispatch_trapped(&mut self, root: &mut dyn Component, ev: &Event) -> Handled {
        self.dispatch_inner(root, ev, true)
    }

    fn dispatch_inner(&mut self, root: &mut dyn Component, ev: &Event, trapped: bool) -> Handled {
        if self.offer_to_overlay(root, ev) == Handled::Yes {
            return Handled::Yes;
        }
        match ev {
            Event::PointerPressed { pos } => {
                if trapped {
                    self.focus_at_trapped(root, *pos);
                } else {
                    self.focus_at(root, *pos);
                }
                root.event(ev)
            }
            _ => root.event(ev),
        }
    }

    /// Apply a target focus index across the tree. Fires `on_blur`/`on_focus`
    /// only on the components that actually change — those events add/remove the
    /// focus effect. `visible` = keyboard focus (ring shown) vs mouse.
    fn apply(&mut self, root: &mut dyn Component, target: Option<usize>, visible: bool) {
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            let want = Some(i) == target;
            let has = c.base().focused.get_untracked();
            if want && !has {
                c.on_focus(visible);
            } else if !want && has {
                c.on_blur();
            }
        });
        self.focused = target;
    }
}
