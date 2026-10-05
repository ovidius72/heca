//! Layout engine: drives `taffy` over the component tree.
//!
//! Each layout pass:
//! 1. Walk the tree, giving each component a `taffy` node from its [`Style`]. **The nodes are kept
//!    with the tree** (on its root), so a component that already has one keeps it and its style is
//!    set only when it differs from what the node holds: taffy then dirties exactly the path from
//!    the widget that changed up to the root, and its own cache answers for everything else.
//! 2. `taffy.compute_layout` over the available space — a cache hit for an unchanged tree.
//! 3. Walk again, copying each node's computed rect into `Base.bounds`
//!    (accumulating parent origins, since taffy locations are parent-relative). This one runs every
//!    time: bounds are rewritten from the layout, so a host that shifts a subtree afterwards starts
//!    from the origin again and a scroll region can re-apply its offset.
//!
//! Nothing here is the host's to remember: `LayoutEngine::new().compute(root, size)` on a tree that
//! was laid out before reuses what that tree kept, and on a new tree starts fresh.
//!
//! [`Style`]: crate::style::Style

use crate::component::Component;
use crate::style::WidgetSize;
use heca_core::layout::{Point, Rectangle, Size};
use measure::measure_text_node;
use std::collections::HashSet;
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
#[derive(Debug, Clone, PartialEq)]
pub struct TextMeasure {
    /// The text to wrap.
    pub text: String,
    /// The resolved font size, already inherited and size-variant scaled by the layout pass.
    pub font: f32,
    /// Does the text reflow? A wrapping text answers with as many lines as the width needs; a
    /// cutting one is always **one** line and simply accepts whatever width it is given.
    pub wrap: bool,
    /// The most lines a wrapping text may take (`None` = as many as it needs). The same cap the
    /// paint applies, so the box is as tall as the lines that are drawn.
    pub max_lines: Option<usize>,
}

/// **What a tree remembers of its last layout**, kept on its root ([`Base::layout_cache`]): the taffy
/// tree, which tree it is (so a component moved in from another one is not mistaken for a node
/// here), and which nodes are live (so one that left the tree can be dropped).
pub(crate) struct RetainedLayout {
    tree: TaffyTree<TextMeasure>,
    id: u64,
    live: HashSet<taffy::NodeId>,
}

thread_local! {
    /// Hands out the identity each retained tree is known by.
    static NEXT_TREE: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

impl RetainedLayout {
    fn fresh() -> Self {
        let id = NEXT_TREE.with(|n| {
            let id = n.get();
            n.set(id + 1);
            id
        });
        Self {
            tree: TaffyTree::new(),
            id,
            live: HashSet::new(),
        }
    }
}

/// Computes layout for a component tree using `taffy`.
pub struct LayoutEngine {
    /// The tree being laid out: the root's retained one, moved in for the length of a
    /// [`compute`](Self::compute) and back out after.
    tree: TaffyTree<TextMeasure>,
    /// Which retained tree `tree` is.
    tree_id: u64,
    /// The nodes that were live before this pass.
    live: HashSet<taffy::NodeId>,
    /// The nodes this pass found a component for.
    seen: HashSet<taffy::NodeId>,
    /// How many nodes this pass had to change (create, restyle, re-measure, re-parent). Zero means
    /// nothing in the tree changed since the last one.
    updated: usize,
    /// Base font size widgets inherit unless they set their own `style.font_size`.
    base_font: f32,
}

impl LayoutEngine {
    /// A fresh layout engine.
    pub fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            tree_id: 0,
            live: HashSet::new(),
            seen: HashSet::new(),
            updated: 0,
            base_font: DEFAULT_BASE_FONT,
        }
    }

    /// **How many nodes the last pass changed** — zero when the tree was the same as the one laid
    /// out before, which is when taffy answers from its own cache.
    pub fn updated_nodes(&self) -> usize {
        self.updated
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
        let retained = root
            .base()
            .layout_cache
            .borrow_mut()
            .take()
            .unwrap_or_else(|| Box::new(RetainedLayout::fresh()));
        let RetainedLayout { tree, id, live } = *retained;
        self.tree = tree;
        self.tree_id = id;
        self.live = live;
        let laid_out = self.settle(root, available);
        let kept = std::mem::replace(&mut self.tree, TaffyTree::new());
        let live = std::mem::take(&mut self.live);
        // A pass that failed leaves nothing worth keeping: the next one starts the tree afresh.
        *root.base().layout_cache.borrow_mut() = laid_out.then(|| {
            Box::new(RetainedLayout {
                tree: kept,
                id,
                live,
            })
        });
    }

    /// The passes of one [`compute`](Self::compute), run on the tree it moved in. `false` when a
    /// pass failed: taffy refused a node it had just been given. That cannot happen for nodes the
    /// engine itself made, so it is reported once and the components keep the bounds they had —
    /// the frame is drawn from the last good layout instead of panicking the window.
    fn settle(&mut self, root: &mut dyn Component, available: Size) -> bool {
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
            if let Err(err) = self.compute_once(root, available) {
                return Self::failed(err);
            }
            if !crate::component::needs_layout(root) {
                return true;
            }
        }
        // Out of passes: lay out once more so what is painted matches the last decision made,
        // rather than the arrangement that decision was about to replace.
        match self.compute_once(root, available) {
            Ok(()) => true,
            Err(err) => Self::failed(err),
        }
    }

    /// Report a failed pass, once per process. Layout runs every frame, so a persistent failure
    /// must not become a line per frame.
    fn failed(err: taffy::TaffyError) -> bool {
        static REPORTED: std::sync::Once = std::sync::Once::new();
        REPORTED.call_once(|| eprintln!("[layout] a pass failed and was skipped: {err}"));
        false
    }

    /// How many times a single [`compute`](Self::compute) will re-run for widgets that change what
    /// they hold in response to the room they were given.
    ///
    /// Two is enough for the shapes here — decide, then lay the decision out — and the third is
    /// headroom for one widget's decision changing another's. It is a cap, not a target: the common
    /// case settles on the first pass and never runs a second.
    const SETTLE_PASSES: usize = 3;

    fn compute_once(
        &mut self,
        root: &mut dyn Component,
        available: Size,
    ) -> Result<(), taffy::TaffyError> {
        self.seen.clear();
        self.updated = 0;
        // The root has no parent to inherit a size variant from — start at the default.
        let node = self.build(root, WidgetSize::default())?;
        // What was in the tree last time and is not now goes: nothing refers to it any more.
        for gone in self
            .live
            .difference(&self.seen)
            .copied()
            .collect::<Vec<_>>()
        {
            let _ = self.tree.remove(gone);
        }
        self.live = std::mem::take(&mut self.seen);
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
            .compute_layout_with_measure(node, space, measure_text_node)?;
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
        Ok(())
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

    /// Recursively copy computed layout into `Base.bounds`, accumulating origin.
    fn assign(&mut self, c: &mut dyn Component, origin: Point) {
        // A component the build did not reach has no node: it keeps the bounds it had.
        let Some(node) = c.base().node else { return };
        let Ok(layout) = self.tree.layout(node) else {
            return;
        };
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

mod build;
mod measure;
#[cfg(test)]
mod tests;
