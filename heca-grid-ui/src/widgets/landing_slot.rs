//! [`LandingSlot`] — **a place something can land**, outlined, with a letter in its middle when a
//! key picks it.
//!
//! One widget serves both ways of choosing a place: the places a carried pane could land (empty
//! placeholders that exist only while a drag is in flight), and the places a keyboard pick offers
//! (existing things outlined, plus empty placeholders between them), each with its letter. It fills
//! the box it is given and works out no geometry of its own.

use crate::builders::{ComponentExt, LayoutExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{SignalGet, SignalUpdate};
use crate::scene::{Border, TextAlign, TextStyle};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// How much bigger than the base font the letter is drawn.
const LETTER_SCALE: f32 = 1.4;
/// Alpha of the wash over an empty placeholder.
const WASH: u8 = 28;

/// A place something can land.
pub struct LandingSlot {
    base: Base,
    label: Option<String>,
    filled: bool,
}

#[heca_grid_ui_macros::props]
impl LandingSlot {
    /// An empty placeholder: a faint wash with an accent outline.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Percent(1.0);
        Self {
            base,
            label: None,
            filled: false,
        }
    }

    /// **Outline something that is already there** instead of marking an empty place: no wash, the
    /// thing underneath stays readable.
    #[heca_grid_ui_macros::prop]
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// The letter drawn in the middle — the key that picks this place.
    #[heca_grid_ui_macros::prop]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// **Exist only while a drag of `kind` is in flight.** It accepts that kind, is hidden until the
    /// drag begins and hidden again when it ends — told by the framework, so nothing hands it a
    /// flag.
    #[heca_grid_ui_macros::prop]
    pub fn while_dragging(mut self, kind: impl Into<String>) -> Self {
        self.base.drop_target = true;
        self.base.accepts = vec![kind.into()];
        self.base.visible.set(false);
        let visible = self.base.visible;
        self.on_drag_in_flight(move |_| visible.set(true))
            .on_drag_settled(move |_| visible.set(false))
    }
}

impl Default for LandingSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LandingSlot {
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
        let accent = cx.accent();
        let bounds = self.base.bounds;
        let wash = if self.filled {
            Color::TRANSPARENT
        } else {
            accent.with_alpha(WASH)
        };
        cx.rect(
            bounds,
            wash,
            Some(Border {
                color: accent,
                width: cx.theme().colors.border_width.max(1.5),
            }),
            cx.theme().colors.border_radius,
            None,
        );
        if let Some(letter) = &self.label {
            let size = self.base.font * LETTER_SCALE;
            let line = f64::from(size) * 1.4;
            let rect = Rectangle::new(
                Point::new(bounds.loc.x, bounds.loc.y + (bounds.size.h - line) / 2.0),
                Size::new(bounds.size.w, line),
            );
            cx.text(
                rect,
                letter,
                accent,
                size,
                TextAlign::Center,
                TextStyle::BOLD,
            );
        }
    }
}
impl LayoutExt for LandingSlot {}
