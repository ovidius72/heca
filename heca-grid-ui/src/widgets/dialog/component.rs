//! The dialog's runtime: the `Component` impl, and the panel it composes.
//!
//! The keyboard and the pointer are the composed [`Overlay`](crate::widgets::Overlay)'s: Tab and the
//! arrows across the buttons (`item_keys`), Esc (`on_dismiss`), a press outside
//! (`on_outside_click`), the panel never wider or taller than the window. The panel adds the one
//! thing only a dialog knows: Enter, when nothing focused took it, presses the primary action.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::component::{Component, Event, Handled, WidgetIntent};
use crate::reactive::SignalGet;
use crate::style::Length;
use heca_core::layout::Point;

impl Dialog {
    fn is_open(&self) -> bool {
        self.base.open.get_untracked()
    }
}

impl Component for Dialog {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn authored_style(&self) -> &crate::style::Style {
        &self.authored_style
    }
    fn authored_style_mut(&mut self) -> &mut crate::style::Style {
        &mut self.authored_style
    }

    /// The panel is put in step first (built, the first time), so a dialog opened and ticked before
    /// any layout already has the panel its opening acts on.
    fn tick(&mut self, dt: f32) -> bool {
        crate::builders::apply_bound(&mut self.base);
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// Hand the caller's panel style to the panel before it lays itself out.
    fn remeasure(&mut self) {
        self.share_panel_style();
    }

    /// Focusable only while open, so the host's overlay scan routes input here.
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open dialog is **modal**: its scrim covers the whole viewport, so every point is occluded
    /// — a host must not synthesize a page-level action anywhere while it is open.
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }
}

/// **The dialog's panel** — a padded box holding the title, body and action areas, sized as the
/// caller asked (`panel_size`, `panel_max_width`) or, by default, from a `22em` baseline up to the
/// theme's readable width (`[dialog] max_width_em`). The overlay around it keeps it inside the
/// window. Enter, when nothing focused took it, presses the primary (first) action.
pub(super) struct DialogPanel {
    base: Base,
    /// The caller's style, shared with the dialog, and the copy last laid out.
    authored: Rc<RefCell<crate::style::Style>>,
    applied: Option<crate::style::Style>,
}

impl DialogPanel {
    pub(super) fn new(
        layout: impl crate::builders::IntoComponent,
        authored: Rc<RefCell<crate::style::Style>>,
    ) -> Self {
        let mut base = Base::container();
        base.style.layout.direction = crate::style::Direction::Column;
        base.children.push(layout.into_component());
        Self {
            base,
            authored,
            applied: None,
        }
    }

    /// The panel's style: the caller's, with a dialog's defaults where the caller said nothing —
    /// a `22em` baseline for a content-sized panel, and the theme's readable width as its ceiling.
    fn styled(&self) -> crate::style::Style {
        let mut style = *self.authored.borrow();
        // The size variant reaches the panel by inheritance, from the dialog (its `LayoutExt::size`).
        style.layout.size = self.base.style.layout.size;
        style.layout.size_explicit = self.base.style.layout.size_explicit;
        if style.layout.width == Length::Auto && style.layout.min_width.is_none() {
            style.layout.min_width = Some(Dialog::BASELINE_WIDTH);
        }
        style
            .layout
            .max_width
            .get_or_insert(Length::Em(self.base.dialog_style.max_width_em));
        style.layout.max_height.get_or_insert(Length::Percent(1.0));
        style
    }

    /// **Press the primary action** — the first button of the action row, found by the row's name.
    fn activate_primary(&mut self) -> bool {
        let row = self
            .base
            .child_in_area(LAYOUT_AREA)
            .and_then(|layout| layout.base_mut().child_in_area(ACTIONS_AREA));
        match row {
            // The first declared action is the primary one — the dialog's rule, written once here.
            Some(row) => crate::component::click(row.as_mut(), &[0]) == Handled::Yes,
            None => false,
        }
    }
}

impl Component for DialogPanel {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Take the caller's style when it changed — only then, so the overlay's narrowing to the
    /// window stays in place between layouts.
    fn remeasure(&mut self) {
        let styled = self.styled();
        if self.applied != Some(styled) {
            self.applied = Some(styled);
            self.base.style = crate::style::Style {
                visual: self.base.style.visual,
                ..styled
            };
        }
    }

    /// **Enter that nothing focused took presses the primary action**, as clicking it would.
    fn on_event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::Widget(WidgetIntent::Activate) if self.activate_primary() => Handled::Yes,
            _ => Handled::No,
        }
    }
}

#[cfg(test)]
use crate::component::GridKey;
#[cfg(test)]
use heca_core::layout::{Rectangle, Size};

#[cfg(test)]
impl Dialog {
    /// The panel's laid-out bounds, for the tests (a zero rect before the first layout).
    pub(super) fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .and_then(|overlay| overlay.base().children.first())
            .map(|panel| panel.base().bounds)
            .unwrap_or_else(|| Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)))
    }

    /// The panel's part in `name`'s area, for the tests — through the overlay and the panel.
    pub(super) fn part_mut(&mut self, name: &str) -> Option<&mut crate::WidgetModel> {
        crate::builders::apply_bound_tree(self);
        self.base
            .children
            .first_mut()?
            .base_mut()
            .children
            .first_mut()?
            .base_mut()
            .child_in_area(LAYOUT_AREA)?
            .base_mut()
            .child_in_area(name)
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "tab_repro.rs"]
mod tab_repro;

#[cfg(test)]
#[path = "sizing_tests.rs"]
mod sizing_tests;
