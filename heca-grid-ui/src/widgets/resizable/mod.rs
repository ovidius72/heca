//! A content-sized box with a vertical resize grip, composed from Grid, Line and Splitter.

use std::{cell::Cell, rc::Rc};
use crate::{Base, Component, Glyph, Grid, Icon, LayoutExt, PaintCx, Parent, Splitter};
use crate::builders::{IntoComponent, PlaceExt, PresenceExt};
use crate::style::{GridTemplate, Length};

mod grip;

/// A vertically resizable container. It hugs its content until the grip is dragged.
/// Set minimum and maximum height on this container with the shared layout builders.
/// The content fills the chosen height and owns its own overflow and input.
/// The content's pixel minimum is preserved, so an Input retains a complete readable line.
///
/// ```
/// use heca_grid_ui::{Input, Resizable, LayoutExt};
/// let editor = Resizable::new(Input::new().multiline(true))
///     .min_height(60.0).max_height(240.0).width("100%");
/// ```
///
/// The default grip is three diagonal corner strokes, at the inherited font size. Override with
/// [`grip`](Self::grip). There are no new key bindings; Splitter owns pointer capture.
/// The grip overlays the bottom-right corner with the theme's `xs` inset; it adds no footer row.
/// Described children form the content (several children are stacked in a column).
///
/// ```ignore
/// use heca_view::{ViewNode, WidgetKind, PropValue};
/// let editor = ViewNode::new(WidgetKind::Resizable)
///     .prop("min_height", PropValue::Float(60.0))
///     .prop("max_height", PropValue::Float(240.0))
///     .child(ViewNode::new(WidgetKind::Input).prop("multiline", PropValue::Bool(true)));
/// ```
pub struct Resizable {
    grid: Grid,
    requested: Rc<Cell<Option<f32>>>,
    actual: Rc<Cell<f32>>,
    minimum: Rc<Cell<f32>>,
    applied: Option<f32>,
    content_height: Length,
}

#[heca_grid_ui_macros::props]
impl Resizable {
    /// Wrap any content. Starts at natural height and preserves the content's pixel minimum.
    pub fn new(content: impl IntoComponent) -> Self {
        let mut content = content.into_component();
        let content_height = content.base().style.layout.height;
        content.base_mut().style.layout.flex_shrink = Some(1.0);
        let requested = Rc::new(Cell::new(None::<f32>));
        let actual = Rc::new(Cell::new(0.0));
        let minimum = Rc::new(Cell::new(0.0));
        let (pending, size, floor) = (requested.clone(), actual.clone(), minimum.clone());
        let handle = Grid::new().template_row("auto").template_column("auto").padding("xs")
            .align_self("end").justify_self("end")
            .child(grip::CornerGrip::new().row(1).column(1))
            .child(Splitter::horizontal().grab("none")
                .row(1).column(1).on_resize(move |delta| {
                    pending.set(Some((pending.get().unwrap_or_else(|| size.get()) + delta).max(floor.get())));
                    crate::component::request_frame();
                }));
        let grid = Grid::new().template_column("1fr").template_row("auto")
            .template_area(["content"]).keyboard_to("content").align_self("start")
            .child(content.area("content")).child(handle.row(1).column(1));
        Self { grid, requested, actual, minimum, applied: None, content_height }
    }

    /// Replace the default diagonal corner strokes with an icon. Colors and size follow the theme.
    #[heca_grid_ui_macros::prop]
    pub fn grip(mut self, glyph: Glyph) -> Self {
        self.grid.base_mut().children[1].base_mut().children[0] = Box::new(Icon::new(glyph).row(1).column(1));
        self
    }

    fn content_minimum(&self) -> f32 {
        match self.base().children[0].taffy_style().min_size.height {
            taffy::Dimension::Length(height) => height.max(0.0),
            _ => 0.0,
        }
    }
}

impl Component for Resizable {
    fn base(&self) -> &Base { self.grid.base() }
    fn base_mut(&mut self) -> &mut Base { self.grid.base_mut() }
    fn grid_template(&self) -> Option<GridTemplate<'_>> { self.grid.grid_template() }
    fn taffy_style(&self) -> taffy::Style {
        let mut style = self.grid.taffy_style();
        style.min_size.height = match style.min_size.height {
            taffy::Dimension::Auto => taffy::Dimension::Length(self.minimum.get()),
            taffy::Dimension::Length(height) => taffy::Dimension::Length(height.max(self.minimum.get())),
            other => other,
        };
        if let Some(height) = self.requested.get() { style.size.height = taffy::Dimension::Length(height); }
        style
    }
    fn remeasure(&mut self) {
        self.minimum.set(self.content_minimum());
        let fixed = self.requested.get().is_some() || self.base().style.layout.height != Length::Auto;
        let height = if fixed { Length::Percent(1.0) } else { self.content_height };
        self.base_mut().children[0].base_mut().style.layout.height = height;
    }
    fn on_layout(&mut self) {
        // Children now have their resolved theme metrics. Settle a changed floor before painting.
        let minimum = self.content_minimum();
        if self.minimum.replace(minimum) != minimum { self.base_mut().mark_needs_layout(); }
        let height = self.base().bounds.size.h as f32;
        self.actual.set(height);
        if self.requested.get().is_some() { self.requested.set(Some(height)); }
        self.applied = self.requested.get();
    }
    fn tick(&mut self, dt: f32) -> bool {
        if self.requested.get() != self.applied { self.base_mut().mark_needs_layout(); }
        let mut moving = self.base_mut().tick_presence(dt);
        for child in &mut self.base_mut().children { moving |= child.tick(dt); }
        moving
    }
    fn paint(&self, cx: &mut PaintCx) { self.grid.paint(cx); }
}
impl LayoutExt for Resizable {}
