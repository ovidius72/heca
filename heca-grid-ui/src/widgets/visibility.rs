//! [`Visibility`] — a signal-driven wrapper that hides or shows one child
//! without rebuilding the tree.

use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet, signal};

/// A wrapper that toggles one child's layout and paint presence from a boolean signal.
pub struct Visibility {
    base: Base,
    visible: Signal<bool>,
    seen: bool,
}

impl Visibility {
    /// Wrap `child` and show it initially when `visible` is `true`.
    pub fn new(child: impl Component + 'static, visible: bool) -> Self {
        let mut base = Base::new();
        base.children.push(Box::new(child));
        let visible = signal(visible);
        let mut this = Self {
            base,
            visible,
            seen: visible.get_untracked(),
        };
        if let Some(child) = this.base.children.first_mut() {
            child.base_mut().style.layout.hidden = !this.seen;
        }
        this
    }

    /// Return the visibility signal so hosts can toggle the wrapped child live.
    pub fn visible_signal(&self) -> Signal<bool> {
        self.visible
    }
}

impl Component for Visibility {
    fn base(&self) -> &Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn remeasure(&mut self) {
        let visible = self.visible.get_untracked();
        self.seen = visible;
        if let Some(child) = self.base.children.first_mut() {
            child.base_mut().style.layout.hidden = !visible;
        }
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        if let Some(child) = self.base.children.first()
            && !child.base().style.layout.hidden
        {
            child.paint(cx);
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let visible = self.visible.get_untracked();
        if visible != self.seen {
            self.seen = visible;
            if let Some(child) = self.base.children.first_mut() {
                child.base_mut().style.layout.hidden = !visible;
            }
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}
