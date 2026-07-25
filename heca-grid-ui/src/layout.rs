//! Layout engine: drives `taffy` over the component tree.
//!
//! Each layout pass (Phase A: a full rebuild, not yet incremental):
//! 1. Walk the tree, creating a `taffy` node per component from its [`Style`].
//! 2. `taffy.compute_layout` over the available space.
//! 3. Walk again, copying each node's computed rect into `Base.bounds`
//!    (accumulating parent origins, since taffy locations are parent-relative).
//!
//! [`Style`]: crate::style::Style

use crate::component::Component;
use crate::style::WidgetSize;
use heca_core::layout::{Point, Rectangle, Size};
use taffy::prelude::*;

/// Default base font (logical px) when the host doesn't set one — matches the
/// default theme's `font_size`.
pub(crate) const DEFAULT_BASE_FONT: f32 = 15.0;

/// Computes layout for a component tree using `taffy`.
pub struct LayoutEngine {
    tree: TaffyTree<()>,
    /// Base font size widgets inherit unless they set their own `style.font_size`.
    base_font: f32,
}

impl LayoutEngine {
    /// A fresh layout engine.
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            base_font: DEFAULT_BASE_FONT,
        }
    }

    /// Set the base font size widgets inherit (the host passes `theme.font_size`),
    /// so a global font change reflows every widget without per-widget wiring.
    pub fn base_font(mut self, base_font: f32) -> Self {
        self.base_font = base_font;
        self
    }

    /// Lay out `root` within `available` (logical pixels) and write the computed
    /// absolute bounds into every component's `Base.bounds`.
    pub fn compute(&mut self, root: &mut dyn Component, available: Size) {
        self.tree.clear();
        // The root has no parent to inherit a size variant from — start at the default.
        let node = self.build(root, WidgetSize::default());
        let space = taffy::Size {
            width: AvailableSpace::Definite(available.w as f32),
            height: AvailableSpace::Definite(available.h as f32),
        };
        self.tree
            .compute_layout(node, space)
            .expect("taffy layout should not fail for a well-formed tree");
        // Taffy lays the root out inside the space it is given, so the root has no
        // parent box to be offset within and its margin is dropped. Apply it here, or
        // `.margin_left(x)` on a root silently does nothing while working on every
        // child — which is not a difference a caller can see. That trap put a
        // correctly-built overlay a whole pane away from its target: no error, no
        // warning, it simply drew somewhere else.
        let style = &root.base().style;
        let origin = Point::new(
            style.margin_left.unwrap_or(style.margin) as f64,
            style.margin_top.unwrap_or(style.margin) as f64,
        );
        self.assign(root, origin);
    }

    /// Recursively create taffy nodes for `c` and its children.
    ///
    /// `inherited_size` is the size variant flowing down from the parent (see
    /// [`Style::size_explicit`](crate::style::Style::size_explicit)). The root starts at the
    /// default.
    fn build(&mut self, c: &mut dyn Component, inherited_size: WidgetSize) -> taffy::NodeId {
        // Resolve the size variant *before* the font: a widget that didn't choose one adopts its
        // parent's, so a control's composed content (`Icon`/`Label`, at any depth) scales with the
        // control instead of staying at the default. Written back into the style so the widget's
        // own `remeasure` / `Base::size_scale` see the effective variant without extra plumbing.
        let size = {
            let s = &c.base().style;
            if s.size_explicit { s.size } else { inherited_size }
        };
        c.base_mut().style.size = size;

        // Resolve the inherited font (own `style.font_size` if set, else the base)
        // and let the widget re-measure from it before we read its taffy style.
        let resolved = {
            let s = &c.base().style;
            if s.font_size > 0.0 {
                s.font_size
            } else {
                // The size variant scales the inherited font too, so text adapts for
                // every widget without per-widget wiring (controls scale their own
                // padding in `remeasure`).
                self.base_font * s.font_scale * size.font_scale()
            }
        };
        c.base_mut().font = resolved;
        // Resolve theme spacing tokens (font-relative) into concrete padding px, so a
        // container takes its padding from the theme instead of a hand-computed value.
        {
            let s = &mut c.base_mut().style;
            if let Some(sp) = s.pad_spacing_x {
                s.padding_x = Some(resolved * sp.scale());
            }
            if let Some(sp) = s.pad_spacing_y {
                s.padding_y = Some(resolved * sp.scale());
            }
            if let Some(sp) = s.gap_spacing {
                s.gap = resolved * sp.scale();
            }
        }
        c.remeasure();
        let style = c.taffy_style();
        let child_count = c.base().children.len();
        let mut child_nodes = Vec::with_capacity(child_count);
        for i in 0..child_count {
            let child = &mut c.base_mut().children[i];
            // Children inherit this node's effective variant unless they chose their own.
            child_nodes.push(self.build(child.as_mut(), size));
        }
        let node = self
            .tree
            .new_with_children(style, &child_nodes)
            .expect("taffy node creation should succeed");
        c.base_mut().node = Some(node);
        node
    }

    /// Recursively copy computed layout into `Base.bounds`, accumulating origin.
    fn assign(&mut self, c: &mut dyn Component, origin: Point) {
        let node = c.base().node.expect("node assigned during build");
        let layout = self.tree.layout(node).expect("layout computed");
        let abs = Point::new(
            origin.x + layout.location.x as f64,
            origin.y + layout.location.y as f64,
        );
        c.base_mut().bounds = Rectangle::new(
            abs,
            Size::new(layout.size.width as f64, layout.size.height as f64),
        );
        let child_count = c.base().children.len();
        for i in 0..child_count {
            let child = &mut c.base_mut().children[i];
            self.assign(child.as_mut(), abs);
        }
        // Post-order: this node's bounds and all descendants' are now freshly
        // computed, so a widget can reset layout-derived state (e.g. a scroll
        // viewport clears the shift baked into its children's bounds).
        c.on_layout();
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{LayoutExt, Parent};
    use crate::style::Length;
    use crate::widgets::Flex;

    /// **Regression guard.** Taffy leaves the root at the origin of the space it is
    /// given, so a margin on the root used to be silently dropped — it worked on every
    /// child, and nothing distinguished the root. A caller positioning a box by margin
    /// got no error and no warning; the box simply drew at the window's corner instead
    /// of where it was put. That cost a mis-placed overlay a whole pane away from its
    /// target before the engine applied it here.
    #[test]
    fn the_root_is_offset_by_its_own_margin() {
        let mut root = Flex::row()
            .width(Length::Px(100.0))
            .height(Length::Px(50.0))
            .margin_left(600.0)
            .margin_top(100.0);
        LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
        assert_eq!(root.base().bounds.loc, Point::new(600.0, 100.0));
    }

    /// The offset carries into descendants — a child of an offset root must move with
    /// it, not stay behind at the window's corner.
    #[test]
    fn a_root_margin_moves_the_whole_subtree() {
        let mut root = Flex::row()
            .width(Length::Px(100.0))
            .height(Length::Px(50.0))
            .margin_left(600.0)
            .margin_top(100.0)
            .child(Flex::row().width(Length::Px(20.0)).height(Length::Px(20.0)));
        LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
        let child = root.base().children[0].base().bounds.loc;
        assert_eq!(child, Point::new(600.0, 100.0));
    }

    /// A root without a margin still starts at the origin — the common case must not
    /// shift.
    #[test]
    fn a_root_without_a_margin_starts_at_the_origin() {
        let mut root = Flex::row().width(Length::Px(100.0)).height(Length::Px(50.0));
        LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
        assert_eq!(root.base().bounds.loc, Point::new(0.0, 0.0));
    }
}
