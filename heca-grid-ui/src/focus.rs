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
fn for_each_focusable(
    c: &mut dyn Component,
    idx: &mut usize,
    f: &mut dyn FnMut(usize, &mut dyn Component),
) {
    if c.base().style.hidden {
        return;
    }
    if c.focusable() {
        f(*idx, c);
        *idx += 1;
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
        let Some(target) = self.focused else {
            return Handled::No;
        };
        let mut handled = Handled::No;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if i == target {
                handled = c.event(&Event::Key { key, pressed: true });
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

    /// Clear focus (e.g. on Escape).
    pub fn clear(&mut self, root: &mut dyn Component) {
        self.apply(root, None, false);
    }

    /// Focus the top-most focusable component containing `pos` (e.g. on a mouse
    /// click); clears focus if the click misses every focusable.
    pub fn focus_at(&mut self, root: &mut dyn Component, pos: Point) {
        let mut hit = None;
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            if c.base().bounds.contains(pos) {
                hit = Some(i); // last match wins = top-most in z-order
            }
        });
        self.apply(root, hit, false); // mouse focus → no ring (focus-visible)
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
