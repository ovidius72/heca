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
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO, mono_cells, wrap_lines};
use crate::style::WidgetSize;
use heca_core::layout::{Point, Rectangle, Size};
use taffy::prelude::*;

/// Default base font (logical px) when the host doesn't set one — matches the
/// default theme's `font_size`.
pub(crate) const DEFAULT_BASE_FONT: f32 = 15.0;

/// Text whose **height depends on the width the engine offers it** — a wrapping label.
///
/// A widget returns one from [`Component::measure_text`] and the engine attaches it to that node as
/// taffy's node context, so taffy can ask for the height *after* it has resolved the width. It
/// carries the text and the font **by value**: the measure runs while taffy owns the tree, so it
/// cannot reach back into the component, and a self-contained context is what makes that a
/// non-issue rather than a lifetime fight.
///
/// Every other widget measures itself in [`Component::remeasure`] and needs none of this — a fixed
/// size is not a function of the width it is given.
#[derive(Debug, Clone)]
pub struct TextMeasure {
    /// The text to wrap.
    pub text: String,
    /// The resolved font size, already inherited and size-variant scaled by the layout pass.
    pub font: f32,
    /// Does the text reflow? A wrapping text answers with as many lines as the width needs; a
    /// cutting one is always **one** line and simply accepts whatever width it is given.
    pub wrap: bool,
}

/// Computes layout for a component tree using `taffy`.
pub struct LayoutEngine {
    tree: TaffyTree<TextMeasure>,
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
            .compute_layout_with_measure(node, space, measure_text_node)
            .expect("taffy layout should not fail for a well-formed tree");
        // Taffy lays the root out inside the space it is given, so the root has no
        // parent box to be offset within and its margin is dropped. Apply it here, or
        // `.margin_left(x)` on a root silently does nothing while working on every
        // child — which is not a difference a caller can see. That trap put a
        // correctly-built overlay a whole pane away from its target: no error, no
        // warning, it simply drew somewhere else.
        let style = &root.base().style.layout;
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
            let s = &c.base().style.layout;
            if s.size_explicit { s.size } else { inherited_size }
        };
        c.base_mut().style.layout.size = size;

        // Resolve the inherited font (own `style.font_size` if set, else the base)
        // and let the widget re-measure from it before we read its taffy style.
        let resolved = {
            let s = &c.base().style;
            if s.visual.font_size > 0.0 {
                s.visual.font_size
            } else {
                // The size variant scales the inherited font too, so text adapts for
                // every widget without per-widget wiring (controls scale their own
                // padding in `remeasure`).
                self.base_font * s.visual.font_scale * size.font_scale()
            }
        };
        c.base_mut().font = resolved;
        // Resolve theme spacing tokens (font-relative) into concrete padding px, so a
        // container takes its padding from the theme instead of a hand-computed value.
        {
            let s = &mut c.base_mut().style.layout;
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
        // A leaf that measures itself from the width it is offered gets taffy's node context; a
        // widget with children is laid out by its children and never measures its own text.
        let node = match c.measure_text() {
            Some(ctx) if child_nodes.is_empty() => self
                .tree
                .new_leaf_with_context(style, ctx)
                .expect("taffy leaf creation should succeed"),
            _ => self
                .tree
                .new_with_children(style, &child_nodes)
                .expect("taffy node creation should succeed"),
        };
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

/// Taffy's measure callback: how tall is this text in the width being offered?
///
/// Called only for nodes carrying a [`TextMeasure`], and only while taffy is resolving them — which
/// is the whole point: the width is known here and nowhere earlier.
///
/// The three width questions taffy asks are answered separately, because a wrapping label has three
/// honest answers. Collapsing them onto the definite case makes a label in an `auto`-sized parent
/// measure one line and then paint three.
fn measure_text_node(
    known: taffy::Size<Option<f32>>,
    available: taffy::Size<AvailableSpace>,
    _node: taffy::NodeId,
    ctx: Option<&mut TextMeasure>,
    _style: &taffy::Style,
) -> taffy::Size<f32> {
    let Some(ctx) = ctx else {
        return taffy::Size::ZERO;
    };
    let cell = (ctx.font * MONO_ADVANCE_RATIO) as f64;
    let line = ctx.font * MONO_LINE_RATIO;
    // One line, uncut — what the label would ask for if nothing constrained it.
    let natural = ctx.text.chars().count() as f64 * cell;
    let width = match (known.width, available.width) {
        // The engine already resolved a width: wrap into exactly that.
        (Some(w), _) => w as f64,
        (None, AvailableSpace::Definite(w)) => w as f64,
        // "How wide would you like to be?" — one line.
        (None, AvailableSpace::MaxContent) => natural,
        // "How narrow can you get without overflowing?" — the longest word, since that is the one
        // thing wrapping cannot break down further (a longer-than-a-line word is hard-broken, so it
        // never sets the floor).
        (None, AvailableSpace::MinContent) => {
            ctx.text
                .split_whitespace()
                .map(|w| w.chars().count())
                .max()
                .unwrap_or(0) as f64
                * cell
        }
    };
    // A cutting label is one line whatever happens to it — it is the *width* it accepts, not the
    // height. Only a wrapping one turns width into height.
    let lines = if ctx.wrap {
        wrap_lines(&ctx.text, mono_cells(width, cell)).len().max(1)
    } else {
        1
    };
    taffy::Size {
        // Never wider than the text actually is: a short label in a wide box keeps its own width,
        // so `align` still has room to place it — the same measure a non-wrapping label reports.
        width: width.min(natural) as f32,
        height: line * lines as f32,
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
