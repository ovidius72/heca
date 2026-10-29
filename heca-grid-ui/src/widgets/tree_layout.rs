//! A parent above equally shared branches. Grid and Flex own all node sizing.

use crate::{Base, Component, Flex, Grid, LayoutExt, PaintCx, Parent, Point};
use crate::builders::IntoComponent;
use crate::scene::{ArrowHeads, LineCmd};

/// A top-down tree of arbitrary components. Nest another tree in a branch to add a level.
///
/// ```
/// use heca_grid_ui::{TreeLayout, Label};
/// let tree = TreeLayout::new(Label::new("root"))
///     .branch(Label::new("one"))
///     .branch(TreeLayout::new(Label::new("two")).branch(Label::new("leaf")));
/// ```
///
/// The level gap defaults to `lg` (override with [`LayoutExt::gap`]); branches use the shared `md`
/// spacing step. Connections use the same inherited color or theme accent and border width as
/// [`crate::Line`]. Controls inside each branch retain their own events.
///
/// A described tree's first child is the parent; remaining children are equally weighted branches.
/// Without children the tree is empty. No tree-specific events or identifiers are required.
///
/// ```ignore
/// use heca_view::{ViewNode, WidgetId};
/// let tree = ViewNode::new(WidgetId::TreeLayout)
///     .child(ViewNode::new(WidgetId::Label).text("root"))
///     .child(ViewNode::new(WidgetId::Label).text("one"))
///     .child(ViewNode::new(WidgetId::Label).text("two"));
/// ```
pub struct TreeLayout { base: Base }

#[heca_grid_ui_macros::props]
impl TreeLayout {
    /// Place `parent` above an initially empty row of branches. Content supplies its own controls.
    pub fn new(parent: impl IntoComponent) -> Self {
        let grid = Grid::new().template_row("auto auto").template_column("1fr")
            .child(Flex::row().justify("center").child(parent.into_component()))
            .child(Flex::row().gap("md").align("start"));
        let mut base = Base::container();
        base.style.layout.gap = "lg".into();
        base.children.push(crate::WidgetModel::new(grid));
        Self { base }
    }

    /// Append an equally weighted branch. No application-specific node or identifier is required.
    #[heca_grid_ui_macros::host_only("composed content crosses as children")]
    pub fn branch(mut self, child: impl IntoComponent) -> Self {
        let branch = Flex::column().align("center").flex(1.0).min_width(0.0).child(child.into_component());
        self.base.children[0].base_mut().children[1].base_mut().children.push(crate::WidgetModel::new(branch));
        self
    }
}

impl Component for TreeLayout {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn remeasure(&mut self) {
        let gap = self.base.style.layout.gap;
        let grid = self.base.children[0].base_mut();
        grid.style.layout.gap = if grid.children[1].base().children.is_empty() { "none".into() } else { gap };
    }
    fn paint(&self, cx: &mut PaintCx) {
        let grid = self.base.children[0].base();
        let parent = grid.children[0].base().bounds;
        let start = Point::new(parent.loc.x + parent.size.w / 2.0, parent.loc.y + parent.size.h);
        for child in &self.base.children { crate::component::paint_child(child.as_ref(), cx); }
        for branch in &grid.children[1].base().children {
            let b = branch.base().bounds;
            let end = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y);
            let middle = (start.y + end.y) / 2.0;
            let points = [start, Point::new(start.x, middle), Point::new(end.x, middle), end];
            for pair in points.windows(2) {
                cx.line(LineCmd { start: pair[0], end: pair[1], color: cx.content_color().unwrap_or_else(|| cx.accent()),
                    width: cx.theme().colors.border_width, arrows: ArrowHeads::None,
                    arrow_size: self.base.font });
            }
        }
    }
}
impl LayoutExt for TreeLayout {}
