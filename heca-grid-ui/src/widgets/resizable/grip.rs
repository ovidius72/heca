//! The corner resize mark. Line owns the strokes; Grid overlays their shared box.

use crate::{Base, Component, Grid, LayoutExt, Line, PaintCx, Parent};
use crate::builders::PlaceExt;
use crate::style::{GridTemplate, Length};

pub(super) struct CornerGrip { grid: Grid }

impl CornerGrip {
    pub fn new() -> Self {
        let mut grid = Grid::new().template_column("1fr").template_row("1fr").padding("hairline");
        for start in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
            grid = grid.child(Line::new().start_x(start).start_y(1.0).end_x(1.0).end_y(start)
                .height("100%").row(1).column(1));
        }
        Self { grid }
    }
}

impl Component for CornerGrip {
    fn base(&self) -> &Base { self.grid.base() }
    fn base_mut(&mut self) -> &mut Base { self.grid.base_mut() }
    fn grid_template(&self) -> Option<GridTemplate<'_>> { self.grid.grid_template() }
    fn taffy_style(&self) -> taffy::Style {
        let mut style = self.grid.taffy_style();
        let side = taffy::Dimension::Length(self.base().font);
        if self.base().style.layout.width == Length::Auto { style.size.width = side; }
        if self.base().style.layout.height == Length::Auto { style.size.height = side; }
        style
    }
    fn paint(&self, cx: &mut PaintCx) { self.grid.paint(cx); }
}
impl LayoutExt for CornerGrip {}
