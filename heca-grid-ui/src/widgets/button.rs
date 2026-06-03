//! [`Button`] — an interactive surface with a centered label. Tracks hover via a
//! [`Signal`] and invokes a click callback. Demonstrates the event path; it is a
//! surface, so it accepts [`StyleExt`] decoration.

use crate::builders::{LayoutExt, StyleExt};
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::TextAlign;
use crate::style::Length;
use heca_core::layout::Point;

/// A clickable button with a text label.
pub struct Button {
    base: Base,
    label: Signal<String>,
    hovered: Signal<bool>,
    on_click: Option<Box<dyn Fn()>>,
}

impl Button {
    /// A button showing `label`.
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.padding = 10.0;
        let mut button = Self {
            base,
            label: signal(label.into()),
            hovered: signal(false),
            on_click: None,
        };
        button.remeasure();
        button
    }

    /// Set the click callback.
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self
    }

    /// The hover-state signal (true while the pointer is over the button).
    pub fn hovered(&self) -> Signal<bool> {
        self.hovered
    }

    /// Naive monospace sizing (Phase B parity); replaced by real shaping later.
    fn remeasure(&mut self) {
        let chars = self.label.get_untracked().chars().count() as f32;
        let fs = self.base.style.font_size;
        let pad = self.base.style.padding * 2.0;
        self.base.style.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO + pad);
        self.base.style.height = Length::Px(fs * MONO_LINE_RATIO + pad);
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }
}

impl Component for Button {
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
        cx.paint_base(&self.base);
        // Brighten the label on hover.
        let fg = if self.hovered.get_untracked() {
            cx.theme().foreground
        } else {
            cx.theme().muted
        };
        cx.text(
            self.base.bounds,
            &self.label.get_untracked(),
            fg,
            self.base.style.font_size,
            TextAlign::Center,
        );
    }

    fn event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::PointerMoved { pos } => {
                let inside = self.contains(*pos);
                if self.hovered.get_untracked() != inside {
                    self.hovered.set(inside);
                }
                Handled::No
            }
            Event::PointerPressed { pos } if self.contains(*pos) => {
                if let Some(f) = &self.on_click {
                    f();
                }
                Handled::Yes
            }
            _ => Handled::No,
        }
    }
}

impl LayoutExt for Button {}
impl StyleExt for Button {}
