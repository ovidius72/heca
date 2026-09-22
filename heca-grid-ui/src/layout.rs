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
        // **Settle before anyone paints.** A widget may only be able to decide its content once it
        // knows the room it got — a row of actions deciding how many fit, a panel deciding whether
        // it needs its scrollbar — and it says so from `on_layout`, which runs *after* the pass
        // that told it. Left to the host's own `needs_layout` check, that answer lands on the NEXT
        // frame, so the arrangement being replaced is painted once first. That is a visible flash,
        // and no caller can prevent it or is even in a position to know about it (Antonio, driving,
        // 2026-09-05: the pane header's buttons blinked on every command, on a focus change, on a
        // split, and when the working directory was detected — four symptoms, one widget).
        //
        // So the pass repeats here until nothing asks again. The host's check still exists and is
        // still right: it catches a layout asked for by something *other* than a layout — a signal
        // firing, a widget revealed. This only closes the case where the layout is what prompted it.
        //
        // ⚠️ **Bounded, because a widget may genuinely never settle.** `Display::Auto` is the known
        // case: taking the words off makes the row narrower, so it then fits, which is the condition
        // for putting them back. A cap turns that into a fixed cost per frame instead of a hang, and
        // leaves it looking exactly as it does today.
        for _ in 0..Self::SETTLE_PASSES {
            self.compute_once(root, available);
            if !crate::component::needs_layout(root) {
                return;
            }
        }
        // Out of passes: lay out once more so what is painted matches the last decision made,
        // rather than the arrangement that decision was about to replace.
        self.compute_once(root, available);
    }

    /// How many times a single [`compute`](Self::compute) will re-run for widgets that change what
    /// they hold in response to the room they were given.
    ///
    /// Two is enough for the shapes here — decide, then lay the decision out — and the third is
    /// headroom for one widget's decision changing another's. It is a cap, not a target: the common
    /// case settles on the first pass and never runs a second.
    const SETTLE_PASSES: usize = 3;

    fn compute_once(&mut self, root: &mut dyn Component, available: Size) {
        self.tree.clear();
        // The root has no parent to inherit a size variant from — start at the default.
        let node = self.build(root, WidgetSize::default());
        // Publish the size the tree is being laid out against **before** anything is placed: a
        // widget that clamps a floating panel on screen does it in `on_layout`, and reading the
        // viewport one pass later (from `PaintCx`) is what made a context menu appear at the raw
        // anchor and jump on the next frame.
        Self::write_viewport_impl(root, available);
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
        // A **percentage** margin resolves against the parent box, and a root has none — so on the
        // root it can only mean a fraction of the space the root was given.
        let root_margin = |side: Option<crate::style::Length>, uniform: f32, space: f64| match side
        {
            Some(crate::style::Length::Px(px)) => px as f64,
            Some(crate::style::Length::Percent(f)) => f as f64 * space,
            Some(crate::style::Length::Auto) | None => uniform as f64,
        };
        let root_font = root.base().font;
        let uniform_margin = style.margin.resolve(root_font);
        let origin = Point::new(
            root_margin(style.margin_left, uniform_margin, available.w),
            root_margin(style.margin_top, uniform_margin, available.h),
        );
        self.assign(root, origin);
        // The first layout pass that sees a widget is the first moment it is both in a live tree
        // and has a place in it — which is what `mount` means. Anything earlier would fire from a
        // constructor, before the widget is anywhere.
        crate::pointer::fire_mounts(root);
    }

    /// Stamp `viewport` on every node in the tree.
    fn write_viewport_impl(c: &mut dyn Component, viewport: Size) {
        c.base_mut().viewport = viewport;
        let n = c.base().children.len();
        for i in 0..n {
            let child = &mut c.base_mut().children[i];
            Self::write_viewport_impl(child.as_mut(), viewport);
        }
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
            if s.size_explicit {
                s.size
            } else {
                inherited_size
            }
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
        // The tree's own base, unscaled by any size variant — what a widget floats *beside* itself
        // is a small panel belonging to the surface, not a part of the control.
        c.base_mut().root_font = self.base_font;
        // A step of the theme's rhythm becomes pixels in `Space::resolve`, against the font just
        // written above — the one place that conversion happens, reached from `taffy_style` below
        // and from any widget measuring its own padding. It used to be copied back into the px
        // fields here, which destroyed the authored step after the first pass: a font change then
        // had nothing left to re-resolve.
        c.remeasure();
        let style = c.taffy_style();
        // **A viewport's content does not give way — that is what a viewport is for.**
        //
        // Shrinking is the default (flexbox's), so an item too big for its line is squeezed to fit.
        // Inside a **clipping** container that is precisely wrong: a 600px column in a 100px scroll
        // region would be squashed to 100 and there would be nothing left to scroll. `clips_children`
        // is the framework's existing name for "my content may exceed me", and it is the one place
        // that knows it — asked here so any viewport widget, including one nobody has written yet,
        // gets the rule without declaring it (F003/P082/T438).
        let content_may_overflow = c.clips_children();
        // A percentage needs something to be a percentage **of**. Against a parent that hugs its
        // content there is no basis, and capping there is meaningless — worse, it resolves to
        // nothing and takes the child with it, which is what emptied the showcase's command
        // palette: that widget positions its own rows in a pass with no definite width
        // (F003/P082/T438).
        let caps_children = !matches!(c.base().style.layout.width, crate::style::Length::Auto);
        // **A child's grid placement is settled here, against the parent that holds it.**
        //
        // A child says `1 / -1` or `grid-area: title` about itself, as in CSS — and neither means
        // anything until you know the template. Resolving it in the layout pass is what lets a
        // child be built before its parent, and lets a template change re-place children without
        // rebuilding any of them. A child of something that is not a grid is left alone.
        // **A share means "take the room this parent has to give", and the parent decides how.**
        //
        // In a grid the track already sized the cell and the item stretches into it, so a share is
        // nothing. In anything else it is CSS `flex: <n> 1 0` — grow alone distributes only free
        // space, so a column of grown children collapses to its content instead of dividing itself.
        //
        // Resolved here, against the parent, because a caller cannot know which kind of parent will
        // end up holding them — and writing the flex spelling by hand put a *definite zero height*
        // in a grid cell, which drew a whole container as its title row and nothing else.
        let in_a_grid = c.grid_template().is_some();
        let child_count_for_share = c.base().children.len();
        for i in 0..child_count_for_share {
            let s = &mut c.base_mut().children[i].base_mut().style.layout;
            let Some(share) = s.share else { continue };
            if in_a_grid {
                continue;
            }
            s.flex_grow = share;
            if share > 0.0 {
                s.flex_basis = Some(crate::style::Length::Px(0.0));
                s.min_height = s.min_height.or(Some(crate::style::Length::Px(0.0)));
                s.flex_shrink = s.flex_shrink.or(Some(1.0));
            }
        }

        let placement = c.grid_template().map(|t| {
            (
                t.counts(),
                c.base()
                    .children
                    .iter()
                    .map(|k| k.base().grid_area.as_ref().and_then(|n| t.area(n)))
                    .collect::<Vec<_>>(),
            )
        });
        let child_count = c.base().children.len();
        let mut child_nodes = Vec::with_capacity(child_count);
        for i in 0..child_count {
            {
                let named = placement.as_ref().and_then(|(_, areas)| areas[i]);
                let counts = placement.as_ref().map(|(counts, _)| *counts);
                let child = &mut c.base_mut().children[i];
                let s = &mut child.base_mut().style.layout;
                // A named area places the child outright; otherwise whatever it said about its own
                // column and row stands, with `ALL` spans given the real track counts.
                if let Some(cell) = named {
                    s.grid_cell = Some(cell);
                } else if let (Some(cell), Some((cols, rows))) = (s.grid_cell, counts) {
                    s.grid_cell = Some(cell.resolved(cols, rows));
                }
                if content_may_overflow {
                    s.flex_shrink = s.flex_shrink.or(Some(0.0));
                } else {
                    // **Giving way is not optional when the row has run out of room.**
                    //
                    // Shrinking is already the default, but flexbox hands every item a floor it
                    // never asked for — `min-width: auto`, its own content width — so a row whose
                    // children *are* willing to shrink still cannot fit them, and lays the overflow
                    // out past its own edge instead. That is one defect wearing four faces: a
                    // disclosure caret, a drag handle, a severity icon and a leading slot, each
                    // placed at its full size before the text, each leaving the text a start
                    // position outside the box. `DockFrame` at 60px began its title AT the right
                    // edge; the whole title was outside the frame.
                    //
                    // The floor is what has to go, and it goes here rather than in the four
                    // widgets, because none of them is doing anything wrong: they compose a row and
                    // let the engine place it. A widget that must keep a size still says so —
                    // `min_width` set explicitly, or `flex_shrink(0.0)` — and this leaves it alone
                    // (F003/P082/T481).
                    //
                    // Not inside a viewport: there, exceeding the box is the point, and the floor
                    // is what keeps a 600px column 600px wide in a 100px scroll region.
                    s.min_width = s.min_width.or(Some(crate::style::Length::Px(0.0)));
                    if caps_children {
                        // **Nothing is wider than what holds it** — CSS's `max-width: 100%`,
                        // applied where the invariant lives rather than inside each widget that
                        // happens to carry a design width. `Alert` is 360px, `Toast` 320, `Input`
                        // 240: in a narrower panel they painted straight through its border and out
                        // the other side, at *every* window size, because an intrinsic width never
                        // consults the box it was given (F003/P082/T438).
                        s.max_width = s.max_width.or(Some(crate::style::Length::Percent(1.0)));
                    }
                }
            }
            // Children inherit this node's effective variant unless they chose their own.
            let child = &mut c.base_mut().children[i];
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
        // "How narrow can you get without overflowing?"
        //
        // **A cutting label can get down to one character — that is what cutting is, and where it
        // stops.** Answering with its longest word made it its own container's floor: a card whose
        // folder line reads `~/projects/heca` could not be laid out narrower than that path, so the
        // card overflowed its box and every card in a narrow window drew across its neighbours.
        //
        // One character rather than **zero**: `min_width: auto` means "my floor is whatever I
        // answered here", so answering zero is saying *my floor is nothing* — and a box resolved to
        // nothing holds no characters and draws no text, which is how a `Card` lost its title
        // outright. Text with something to say is never silent; at its narrowest it is `…`
        // (F003/P082/T438).
        (None, AvailableSpace::MinContent) if !ctx.wrap => cell,
        // A **wrapping** label is the case the longest word belongs to: wrapping cannot break a word
        // down further, so that word is a real floor (a longer-than-a-line word is hard-broken, so
        // it never sets one).
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
        //
        // **Rounded UP, because half a character is not a character.** Six cells of 8.1px want
        // 48.6px; reporting that gets a box floored to 48, and a cutting label then finds itself
        // one cell short of its own text and draws `edit…` where `editor` fits. `mono_cells` keeps
        // half a pixel of slack for exactly this, and half a pixel is not enough — the loss is up
        // to a whole one. The measure is the place to fix it: a box that cannot hold the text it
        // was measured for is wrong before anyone looks at it (F003/P082/T438).
        width: width.min(natural).ceil() as f32,
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
            .width(100.0)
            .height(50.0)
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
            .width(100.0)
            .height(50.0)
            .margin_left(600.0)
            .margin_top(100.0)
            .child(Flex::row().width(20.0).height(20.0));
        LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
        let child = root.base().children[0].base().bounds.loc;
        assert_eq!(child, Point::new(600.0, 100.0));
    }

    /// A root without a margin still starts at the origin — the common case must not
    /// shift.
    #[test]
    fn a_root_without_a_margin_starts_at_the_origin() {
        let mut root = Flex::row().width(100.0).height(50.0);
        LayoutEngine::new().compute(&mut root, Size::new(1000.0, 800.0));
        assert_eq!(root.base().bounds.loc, Point::new(0.0, 0.0));
    }

    /// ⚠️ **A percentage margin resolves against the parent's WIDTH — on both axes.**
    ///
    /// This is CSS's rule and taffy implements it faithfully, but it is the opposite of
    /// what "a fraction of the parent" reads as on the vertical, and it is silent: a
    /// `margin_top(Percent(0.25))` produces a number, just the wrong one, on any parent that
    /// is not square. So a caller placing a box at a **fractional rect** can express its
    /// `x` this way and **not** its `y` — the vertical fraction has to be a share
    /// (`grow` weights) or a wrapper the engine can measure against the right axis.
    ///
    /// Written down here because the exposé's float placement was designed around
    /// percentage margins on both axes, and nothing in the API says which axis it means.
    #[test]
    fn a_percentage_margin_resolves_against_the_parents_width_on_both_axes() {
        let mut root = Flex::row().width(800.0).height(400.0).child(
            Flex::row()
                .width(10.0)
                .height(10.0)
                .margin_left(Length::Percent(0.25))
                .margin_top(Length::Percent(0.25)),
        );
        LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
        // A quarter of the width on **both** — not (200, 100), which is what a
        // per-axis reading would give.
        assert_eq!(
            root.base().children[0].base().bounds.loc,
            Point::new(200.0, 200.0)
        );
    }

    /// **`at_rect` is the answer the margin could not give**: a fractional rect where each
    /// percentage resolves against its own axis, so a box lands where the caller said on a parent
    /// of any shape. This is what places a floating pane over the workspace it belongs to.
    #[test]
    fn a_fractional_rect_resolves_each_percentage_against_its_own_axis() {
        let mut root = Flex::row()
            .width(800.0)
            .height(400.0)
            .child(Flex::row().at_rect(
                Length::Percent(0.25),
                Length::Percent(0.25),
                Length::Percent(0.5),
                Length::Percent(0.5),
            ));
        LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
        let placed = root.base().children[0].base().bounds;
        assert_eq!(placed.loc, Point::new(200.0, 100.0));
        assert_eq!(placed.size, Size::new(400.0, 200.0));
    }

    /// **Placing something is not resizing it.** An axis left `Auto` in a placement is a question
    /// the caller did not answer, so the widget's own size stands there — which is what lets a
    /// host seat a surface at the window origin without also deciding how big it is.
    ///
    /// Reading `Auto` as "shrink to content" instead is how a menu seated as a surface ended up
    /// stretched down the whole window: the seat handed it the viewport, and a menu is not a layer
    /// — it *is* its panel. Both halves are here, because the trap is that one of them is silent:
    /// a layer that declares its own `Percent(1.0)` must keep filling the window.
    #[test]
    fn a_placement_that_leaves_an_axis_auto_keeps_the_widgets_own_size() {
        let mut root = Flex::row()
            .width(800.0)
            .height(400.0)
            // Sizes itself, like a menu panel: the seat must not touch it.
            .child(Flex::row().width(220.0).height(90.0).at_rect(
                Length::Percent(0.0),
                Length::Percent(0.0),
                Length::Auto,
                Length::Auto,
            ))
            // Declares itself the whole window, like every layer-shaped surface.
            .child(
                Flex::row()
                    .width(Length::FULL)
                    .height(Length::FULL)
                    .at_rect(
                        Length::Percent(0.0),
                        Length::Percent(0.0),
                        Length::Auto,
                        Length::Auto,
                    ),
            );
        LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));

        assert_eq!(
            root.base().children[0].base().bounds.size,
            Size::new(220.0, 90.0),
            "the panel kept the size it set on itself",
        );
        assert_eq!(
            root.base().children[1].base().bounds.size,
            Size::new(800.0, 400.0),
            "and the layer still fills the window",
        );
    }

    /// **A leading icon never pushes the text out of the row** (F003/P082/T481).
    ///
    /// The row is willing to shrink and the label is willing to be cut, and it still overflowed:
    /// flexbox floors every item at its own content width, so the icon kept its 40px, the label
    /// kept its text width, and the sum was laid out past the row's right edge. Four widgets — a
    /// dock header, a group header, a toast, a list row — showed it as text drawn across whatever
    /// sat beside them.
    #[test]
    fn a_leading_slot_never_pushes_the_text_past_the_rows_edge() {
        use crate::widgets::{Ellipsis, Label};

        let mut row = Flex::row()
            .width(60.0)
            .height(30.0)
            .child(Flex::row().width(40.0).height(20.0))
            .child(Label::new("a title far too long for this").truncate(Ellipsis::End));
        LayoutEngine::new().compute(&mut row, Size::new(200.0, 100.0));

        for child in &row.base().children {
            let b = child.base().bounds;
            assert!(
                b.loc.x >= -0.5 && b.loc.x + b.size.w <= 60.5,
                "content laid out at {}..{} in a 60px row",
                b.loc.x,
                b.loc.x + b.size.w,
            );
        }
    }

    /// **A widget that must keep its size still keeps it.** The floor is removed by default, not
    /// forbidden: `flex_shrink(0.0)` is how a control opts out, and it must survive the rule above
    /// — otherwise every icon in a tight row would be squeezed to a smear instead of the text
    /// giving way.
    #[test]
    fn a_widget_that_refuses_to_shrink_is_left_alone() {
        let mut row = Flex::row()
            .width(60.0)
            .height(30.0)
            .child(Flex::row().width(40.0).height(20.0).shrink(0.0))
            .child(Flex::row().width(40.0).height(20.0));
        LayoutEngine::new().compute(&mut row, Size::new(200.0, 100.0));
        assert_eq!(row.base().children[0].base().bounds.size.w, 40.0);
    }

    /// **A placed box is out of the flow** — it takes no space from its siblings and does not move
    /// them, which is what makes it draw *over* the row rather than beside it. Without this a
    /// floating pane would push the columns it floats above along the strip.
    #[test]
    fn a_placed_box_takes_no_space_from_its_siblings() {
        let mut root = Flex::row()
            .width(800.0)
            .height(400.0)
            .child(Flex::row().width(Length::FULL).height(Length::FULL))
            .child(Flex::row().at_rect(
                Length::Percent(0.5),
                Length::Percent(0.5),
                Length::Px(100.0),
                Length::Px(100.0),
            ));
        LayoutEngine::new().compute(&mut root, Size::new(800.0, 400.0));
        // The in-flow sibling still has the whole row…
        assert_eq!(
            root.base().children[0].base().bounds.size,
            Size::new(800.0, 400.0)
        );
        // …and the placed box sits on top of it at its own rect.
        assert_eq!(
            root.base().children[1].base().bounds.loc,
            Point::new(400.0, 200.0)
        );
    }
}
