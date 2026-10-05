//! Turning a component tree into taffy nodes: the style each widget asks for, and the rules the
//! engine applies on its parents' behalf.

use super::{LayoutEngine, TextMeasure};
use crate::component::Component;
use crate::style::WidgetSize;

impl LayoutEngine {
    /// Recursively create taffy nodes for `c` and its children.
    ///
    /// `inherited_size` is the size variant flowing down from the parent (see
    /// [`Style::size_explicit`](crate::style::Style::size_explicit)). The root starts at the
    /// default.
    pub(super) fn build(
        &mut self,
        c: &mut dyn Component,
        inherited_size: WidgetSize,
    ) -> Result<taffy::NodeId, taffy::TaffyError> {
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
        // **`flex: <n>` means "take n parts of the room this parent has to give", and the parent
        // decides how.**
        //
        // In a grid the track already sized the cell and the item stretches into it, so `flex` is
        // nothing — as in CSS. In anything else it is CSS `flex: <n> 1 0` — grow alone distributes
        // only free space, so a column of grown children collapses to its content instead of
        // dividing itself.
        //
        // Only the minimum HEIGHT is zeroed here. The minimum width needs nothing: every child of a
        // non-viewport container already gets `min_width: 0` (the two rules in `style.rs`), which is
        // what lets a row of parts split evenly whatever each holds —
        // `flex_parts_split_a_row_whatever_each_child_holds` holds it.
        //
        // Resolved here, against the parent, because a caller cannot know which kind of parent will
        // end up holding them — and writing the flex spelling by hand put a *definite zero height*
        // in a grid cell, which drew a whole container as its title row and nothing else.
        let in_a_grid = c.grid_template().is_some();
        let child_count = c.base().children.len();
        for i in 0..child_count {
            let s = &mut c.base_mut().children[i].base_mut().style.layout;
            let Some(parts) = s.flex else { continue };
            if in_a_grid {
                continue;
            }
            s.flex_grow = parts;
            if parts > 0.0 {
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
            child_nodes.push(self.build(child.as_mut(), size)?);
        }
        // **`order` moves where a child is laid out, not where it is in the tree** (CSS). Only the
        // list handed to the engine is sorted — stable, so ties and children that said nothing keep
        // the order they were added in. Paint, Tab and the picker keep walking the tree as it is.
        if c.base()
            .children
            .iter()
            .any(|k| k.base().style.layout.order.is_some())
        {
            let keys: Vec<crate::order::Order> = c
                .base()
                .children
                .iter()
                .map(|k| k.base().style.layout.order.unwrap_or_default())
                .collect();
            let mut by_order: Vec<(crate::order::Order, taffy::NodeId)> =
                keys.into_iter().zip(child_nodes.iter().copied()).collect();
            by_order.sort_by_key(|(order, _)| *order);
            child_nodes = by_order.into_iter().map(|(_, n)| n).collect();
        }
        // A leaf that measures itself from the width it is offered gets taffy's node context; a
        // widget with children is laid out by its children and never measures its own text.
        let context = if child_nodes.is_empty() {
            c.measure_text()
        } else {
            None
        };
        let node = match self.reusable(c) {
            Some(node) => {
                self.refresh(node, style, context, &child_nodes);
                node
            }
            None => {
                self.updated += 1;
                match context {
                    Some(ctx) => self.tree.new_leaf_with_context(style, ctx)?,
                    None => self.tree.new_with_children(style, &child_nodes)?,
                }
            }
        };
        self.seen.insert(node);
        c.base().layout_tree.set(self.tree_id);
        c.base_mut().node = Some(node);
        Ok(node)
    }

    /// The node this component already has **in this tree**, if it still exists — a component that
    /// was laid out here before, and not one that came from another tree or was dropped from this.
    fn reusable(&self, c: &dyn Component) -> Option<taffy::NodeId> {
        let node = c.base().node?;
        (c.base().layout_tree.get() == self.tree_id && self.live.contains(&node)).then_some(node)
    }

    /// **Tell a node what changed, and only that.** Each setter dirties the path from this node to
    /// the root; a node whose style, measure and children are what it already holds is left clean,
    /// which is what lets taffy's own cache answer for it.
    fn refresh(
        &mut self,
        node: taffy::NodeId,
        style: taffy::Style,
        context: Option<TextMeasure>,
        children: &[taffy::NodeId],
    ) {
        if self.tree.style(node).ok() != Some(&style) {
            let _ = self.tree.set_style(node, style);
            self.updated += 1;
        }
        if self.tree.get_node_context(node) != context.as_ref() {
            let _ = self.tree.set_node_context(node, context);
            self.updated += 1;
        }
        if self.tree.children(node).ok().as_deref() != Some(children) {
            let _ = self.tree.set_children(node, children);
            self.updated += 1;
        }
    }
}
