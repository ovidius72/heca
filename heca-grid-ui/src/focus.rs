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
fn for_each_focusable(
    c: &mut dyn Component,
    idx: &mut usize,
    f: &mut dyn FnMut(usize, &mut dyn Component),
) {
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

fn count_focusable(root: &mut dyn Component) -> usize {
    let mut idx = 0;
    for_each_focusable(root, &mut idx, &mut |_, _| {});
    idx
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

    /// Move focus to the next (`forward = true`) or previous focusable component,
    /// wrapping at the ends. Updates each component's `focused` signal.
    pub fn advance(&mut self, root: &mut dyn Component, forward: bool) {
        let n = count_focusable(root);
        if n == 0 {
            self.focused = None;
            return;
        }
        let next = match self.focused {
            None => {
                if forward {
                    0
                } else {
                    n - 1
                }
            }
            Some(i) => {
                if forward {
                    (i + 1) % n
                } else {
                    (i + n - 1) % n
                }
            }
        };
        self.apply(root, Some(next));
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

    /// Clear focus (e.g. on Escape).
    pub fn clear(&mut self, root: &mut dyn Component) {
        self.apply(root, None);
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
        self.apply(root, hit);
    }

    /// Apply a target focus index across the tree. Fires `on_blur`/`on_focus`
    /// only on the components that actually change — those events add/remove the
    /// focus effect.
    fn apply(&mut self, root: &mut dyn Component, target: Option<usize>) {
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            let want = Some(i) == target;
            let has = c.base().focused.get_untracked();
            if want && !has {
                c.on_focus();
            } else if !want && has {
                c.on_blur();
            }
        });
        self.focused = target;
    }
}
