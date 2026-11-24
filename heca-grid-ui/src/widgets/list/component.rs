//! The [`Component`] side of [`List`]: building on the first layout, choosing the window after each
//! one, and the keys.

use super::*;
use crate::component::{Component, Event, Handled, WidgetIntent};

impl<T: Clone + PartialEq + 'static> List<T> {
    /// Move the selection by one row, the keyboard's way: it brings the row into view.
    fn step(&mut self, down: bool) {
        let len = self.items.get_untracked().len();
        if len == 0 {
            return;
        }
        let next = match self.selected.get_untracked() {
            None => 0,
            Some(at) if down => (at + 1).min(len - 1),
            Some(at) => at.saturating_sub(1),
        };
        self.selected.set(Some(next));
    }

    /// The viewport's height: the list's own box.
    fn viewport_h(&self) -> f64 {
        self.base.bounds.size.h
    }

    /// After a layout: follow the measured height, then build the items the viewport now shows.
    fn place_window(&mut self) {
        let (sum, count) = self.measured.replace((0.0, 0));
        if count > 0 {
            self.item_h = (sum / count as f64).round();
        }
        let len = self.items.get_untracked().len();
        let offset = self.scroll.map_or(0.0, |s| f64::from(s.get_untracked()));
        let visible = self.visible.as_ref().map(|n| n.now_untracked());
        let (start, end, above, below) = if self.item_h > 0.0 {
            let item_h = self.item_h;
            // At most `visible` items tall, from the items measured — the list's own ceiling.
            if let Some(n) = visible {
                self.base.style.layout.max_height = Some(Length::Px((n as f64 * item_h) as f32));
            }
            let (start, end) = window::window(len, item_h, offset, self.viewport_h());
            (
                start,
                end,
                start as f64 * item_h,
                (len - end) as f64 * item_h,
            )
        } else {
            // Nothing measured yet: build the items that will show — as many as it shows, or the
            // first few — and measure them; no room is guessed for the rest until then, so the
            // first frame is already as tall as the list will be.
            let first = visible.unwrap_or(window::OVERSCAN + 1).min(len);
            (0, first, 0.0, 0.0)
        };
        let mut moved = false;
        if self.window.get_untracked() != (start, end) {
            self.window.set((start, end));
            moved = true;
        }
        if self.above.get_untracked() != above || self.below.get_untracked() != below {
            self.above.set(above);
            self.below.set(below);
            moved = true;
        }
        if moved {
            self.base.mark_needs_layout();
            crate::component::request_frame();
        }
    }

    /// A selection moved by anything but the pointer brings its row into view; one the pointer moved
    /// does not. A row not built yet is scrolled to by the estimate, and brings itself exactly into
    /// view once built.
    fn follow_selection(&mut self) {
        let now = self.selected.get_untracked();
        if now == self.seen {
            return;
        }
        self.seen = now;
        let by_keyboard = !self.pointed.replace(false);
        self.by_keyboard.set(by_keyboard);
        let (Some(at), true, Some(scroll)) = (now, by_keyboard, self.scroll) else {
            return;
        };
        let (start, end) = self.window.get_untracked();
        if (start..end).contains(&at) || self.item_h <= 0.0 {
            return;
        }
        let offset = f64::from(scroll.get_untracked());
        if let Some(to) = window::reveal(at, self.item_h, offset, self.viewport_h()) {
            scroll.set(to as f32);
            self.base.mark_needs_layout();
            crate::component::request_frame();
        }
    }
}

impl<T: Clone + PartialEq + 'static> Component for List<T> {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn remeasure(&mut self) {
        if !self.built {
            self.build();
        }
    }

    fn on_layout(&mut self) {
        self.place_window();
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.follow_selection();
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// The arrows and `Ctrl+j`/`Ctrl+k` move the selection, Enter chooses it — when the keyboard is
    /// in the list, or on anything inside it that leaves them.
    fn on_event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::Widget(WidgetIntent::MenuDown) => self.step(true),
            Event::Widget(WidgetIntent::MenuUp) => self.step(false),
            Event::Widget(WidgetIntent::Activate) => {
                match (self.selected.get_untracked(), &self.on_choose) {
                    (Some(at), Some(choose)) => choose(at),
                    _ => return Handled::No,
                }
            }
            _ => return Handled::No,
        }
        Handled::Yes
    }
}
