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
        let node = self.build(root);
        let space = taffy::Size {
            width: AvailableSpace::Definite(available.w as f32),
            height: AvailableSpace::Definite(available.h as f32),
        };
        self.tree
            .compute_layout(node, space)
            .expect("taffy layout should not fail for a well-formed tree");
        self.assign(root, Point::new(0.0, 0.0));
    }

    /// Recursively create taffy nodes for `c` and its children.
    fn build(&mut self, c: &mut dyn Component) -> taffy::NodeId {
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
                self.base_font * s.font_scale * s.size.font_scale()
            }
        };
        c.base_mut().font = resolved;
        c.remeasure();
        let style = c.taffy_style();
        let child_count = c.base().children.len();
        let mut child_nodes = Vec::with_capacity(child_count);
        for i in 0..child_count {
            let child = &mut c.base_mut().children[i];
            child_nodes.push(self.build(child.as_mut()));
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
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}
