//! A segment inside a layout box. Positions are fractions of that box, so resizing keeps it joined.

use crate::{Base, Component, LayoutExt, PaintCx, Point};
use crate::scene::{ArrowHeads, LineCmd};

/// A theme-driven connection. Defaults to a horizontal line across the centre of its box.
///
/// ```
/// use heca_grid_ui::{Line, LayoutExt};
/// use heca_grid_ui::scene::ArrowHeads;
/// let arrow = Line::new().arrows(ArrowHeads::End).width("100%").height(20.0);
/// ```
///
/// Stroke color inherits the enclosing content color, falling back to the theme accent. Width
/// comes from the theme border width; arrow size follows the resolved font. There are no children
/// or events. Endpoints are shares of the layout box, clamped to `0..=1`.
///
/// ```ignore
/// use heca_view::{ViewNode, WidgetId, PropValue};
/// let arrow = ViewNode::new(WidgetId::Line)
///     .prop("arrows", PropValue::Text("end".into()))
///     .prop("start_y", PropValue::Float(0.0))
///     .prop("end_y", PropValue::Float(1.0));
/// ```
pub struct Line {
    base: Base,
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    arrows: ArrowHeads,
}

#[heca_grid_ui_macros::props]
impl Line {
    /// A horizontal segment; the caller supplies its layout box.
    pub fn new() -> Self {
        Self { base: Base::new(), start_x: 0.0, start_y: 0.5,
            end_x: 1.0, end_y: 0.5, arrows: ArrowHeads::None }
    }
    /// Starting horizontal share (default zero).
    #[heca_grid_ui_macros::prop]
    pub fn start_x(mut self, share: f32) -> Self { self.start_x = share.clamp(0.0, 1.0); self }
    /// Starting vertical share (default one half).
    #[heca_grid_ui_macros::prop]
    pub fn start_y(mut self, share: f32) -> Self { self.start_y = share.clamp(0.0, 1.0); self }
    /// Ending horizontal share (default one).
    #[heca_grid_ui_macros::prop]
    pub fn end_x(mut self, share: f32) -> Self { self.end_x = share.clamp(0.0, 1.0); self }
    /// Ending vertical share (default one half).
    #[heca_grid_ui_macros::prop]
    pub fn end_y(mut self, share: f32) -> Self { self.end_y = share.clamp(0.0, 1.0); self }
    /// Arrow placement (default none). Size follows the resolved font.
    #[heca_grid_ui_macros::prop]
    pub fn arrows(mut self, arrows: ArrowHeads) -> Self { self.arrows = arrows; self }
}

impl Default for Line { fn default() -> Self { Self::new() } }
impl Component for Line {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn taffy_style(&self) -> taffy::Style {
        let mut layout = self.base.style.layout;
        if layout.width == crate::Length::Auto { layout.width = crate::Length::Percent(1.0); }
        if layout.height == crate::Length::Auto { layout.height = crate::Length::Px(self.base.font * crate::font::MONO_LINE_RATIO); }
        layout.to_taffy(self.base.font)
    }
    fn paint(&self, cx: &mut PaintCx) {
        let b = self.base.bounds;
        let point = |x: f32, y: f32| Point::new(b.loc.x + b.size.w * x as f64,
            b.loc.y + b.size.h * y as f64);
        cx.line(LineCmd { start: point(self.start_x, self.start_y), end: point(self.end_x, self.end_y),
            color: cx.content_color().unwrap_or_else(|| cx.accent()), width: cx.theme().colors.border_width,
            arrows: self.arrows, arrow_size: self.base.font });
    }
}
impl LayoutExt for Line {}
