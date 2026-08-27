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
        let child: Box<dyn Component> = Box::new(child);
        // Transparent to layout as well as to paint: see `component::wrap_transparently`.
        crate::component::wrap_transparently(&mut base, child.as_ref());
        base.children.push(child);
        let visible = signal(visible);
        let mut this = Self {
            base,
            visible,
            seen: visible.get_untracked(),
        };
        if let Some(child) = this.base.children.first_mut() {
            child.base_mut().set_hidden(!this.seen);
        }
        this
    }

    /// Return the visibility signal so hosts can toggle the wrapped child live.
    pub fn visible_signal(&self) -> Signal<bool> {
        self.visible
    }

    /// **Hidden means gone from the layout — the wrapper's box as well as the child's.**
    ///
    /// A wrapper takes its size from what it wraps (`wrap_transparently`), so a wrapper holding a
    /// hidden child still occupied that child's width: a sidebar row carries four status dots and
    /// shows one, and the three that were hidden went on taking a dot's width each, overflowing a
    /// slot sized for one and drawing the visible dot over the icon beside it. They collapsed
    /// before only because the wrapper was squeezed by a row that had run out of room — the wrong
    /// mechanism producing the right picture (F003/P096/T483).
    fn apply(&mut self, visible: bool) {
        self.base.set_hidden(!visible);
        if let Some(child) = self.base.children.first_mut() {
            child.base_mut().set_hidden(!visible);
        }
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
        self.apply(visible);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        if let Some(child) = self.base.children.first()
            && !child.base().style.layout.hidden
        {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let visible = self.visible.get_untracked();
        if visible != self.seen {
            self.seen = visible;
            self.apply(visible);
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}
