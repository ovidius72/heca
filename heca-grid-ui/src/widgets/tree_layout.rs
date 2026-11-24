//! A parent above equally shared branches. Grid and Flex own all node sizing.

use crate::builders::{IntoComponent, PlaceExt};
use crate::scene::{ArrowHeads, LineCmd};
use crate::{Base, Component, Flex, Grid, LayoutExt, PaintCx, Parent, Point};

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
pub struct TreeLayout {
    base: Base,
}

/// The tree's own parts, found by name: the grid, the parent above, and the row of branches.
const GRID_AREA: &str = "tree";
const PARENT_AREA: &str = "parent";
const BRANCHES_AREA: &str = "branches";

#[heca_grid_ui_macros::props]
impl TreeLayout {
    /// Place `parent` above an initially empty row of branches. Content supplies its own controls.
    pub fn new(parent: impl IntoComponent) -> Self {
        let grid = Grid::new()
            .template_row("auto auto")
            .template_column("1fr")
            .child(
                Flex::row()
                    .justify("center")
                    .child(parent.into_component())
                    .area(PARENT_AREA),
            )
            .child(Flex::row().gap("md").align("start").area(BRANCHES_AREA))
            .area(GRID_AREA);
        let mut base = Base::container();
        base.style.layout.gap = "lg".into();
        base.children.push(crate::WidgetModel::new(grid));
        Self { base }
    }

    /// Append an equally weighted branch. No application-specific node or identifier is required.
    #[heca_grid_ui_macros::host_only("composed content crosses as children")]
    pub fn branch(mut self, child: impl IntoComponent) -> Self {
        let branch = Flex::column()
            .align("center")
            .flex(1.0)
            .min_width(0.0)
            .child(child.into_component());
        if let Some(branches) = self.branches_mut() {
            branches.children.push(crate::WidgetModel::new(branch));
        }
        self
    }

    fn grid(&self) -> Option<&Base> {
        self.base.child_area(GRID_AREA).map(|grid| grid.base())
    }

    fn grid_mut(&mut self) -> Option<&mut Base> {
        self.base
            .child_in_area(GRID_AREA)
            .map(|grid| grid.base_mut())
    }

    fn branches_mut(&mut self) -> Option<&mut Base> {
        self.grid_mut()?
            .child_in_area(BRANCHES_AREA)
            .map(|branches| branches.base_mut())
    }
}

impl Component for TreeLayout {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn remeasure(&mut self) {
        let gap = self.base.style.layout.gap;
        let Some(grid) = self.grid_mut() else { return };
        let no_branches = grid
            .child_area(BRANCHES_AREA)
            .is_none_or(|branches| branches.base().children.is_empty());
        grid.style.layout.gap = if no_branches { "none".into() } else { gap };
    }
    fn paint(&self, cx: &mut PaintCx) {
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
        let Some(grid) = self.grid() else { return };
        let (Some(parent), Some(branches)) =
            (grid.child_area(PARENT_AREA), grid.child_area(BRANCHES_AREA))
        else {
            return;
        };
        let parent = parent.base().bounds;
        let start = Point::new(
            parent.loc.x + parent.size.w / 2.0,
            parent.loc.y + parent.size.h,
        );
        for branch in &branches.base().children {
            let b = branch.base().bounds;
            let end = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y);
            let middle = (start.y + end.y) / 2.0;
            let points = [
                start,
                Point::new(start.x, middle),
                Point::new(end.x, middle),
                end,
            ];
            for pair in points.windows(2) {
                cx.line(LineCmd {
                    start: pair[0],
                    end: pair[1],
                    color: cx.content_color().unwrap_or_else(|| cx.accent()),
                    width: cx.theme().colors.border_width,
                    arrows: ArrowHeads::None,
                    arrow_size: self.base.font,
                });
            }
        }
    }
}
impl LayoutExt for TreeLayout {}
