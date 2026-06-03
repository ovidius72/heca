//! Keyboard focus traversal for a component tree.
//!
//! [`FocusManager`] tracks which focusable component (by depth-first z-order
//! index) currently holds focus, moves focus on Tab/Shift+Tab (wrapping), and
//! delivers key events to the focused component. Each component's focus state
//! lives in `Base.focused`, so widgets can render a focus ring reactively.

use crate::component::{Component, Event, GridKey, Handled};
use crate::reactive::{SignalGet, SignalUpdate};

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
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |i, c| {
            let want = i == next;
            if c.base().focused.get_untracked() != want {
                c.base_mut().focused.set(want);
            }
        });
        self.focused = Some(next);
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
        let mut idx = 0;
        for_each_focusable(root, &mut idx, &mut |_, c| {
            if c.base().focused.get_untracked() {
                c.base_mut().focused.set(false);
            }
        });
        self.focused = None;
    }
}
