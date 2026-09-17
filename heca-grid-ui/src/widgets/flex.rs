//! [`Flex`] — the **layout-only** flexible box.
//!
//! `Flex` arranges children (direction, justify, align, gap, padding, grow) but
//! carries **no visual styling** — there is no `.background()`/`.border()`/
//! `.glow()` on it. Decoration lives on surface components
//! ([`Surface`](super::Surface), [`Card`](super::Card), [`Button`](super::Button)).
//!
//! Use `Flex` to *group and position*; reach for a surface when you want
//! something actually drawn. Builders come from [`LayoutExt`] and [`Parent`].

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component};
use crate::style::Direction;

/// A layout-only flexible container of child components.
pub struct Flex {
    base: Base,
}

impl Flex {
    /// A row (horizontal main axis).
    pub fn row() -> Self {
        Self::with_direction(Direction::Row)
    }

    /// A column (vertical main axis).
    pub fn column() -> Self {
        Self::with_direction(Direction::Column)
    }

    /// A zero-size placeholder — an empty slot that takes no layout space. Used
    /// to fill an optional slot (e.g. an [`Item`](super::Item) leading/trailing
    /// slot, or a [`DockFrame`](super::DockFrame) header-controls slot) until a
    /// real component replaces it.
    pub fn empty() -> Self {
        Self::row().width(0.0).height(0.0)
    }

    fn with_direction(direction: Direction) -> Self {
        let mut base = Base::new();
        base.style.layout.direction = direction;
        Self { base }
    }
}

impl Component for Flex {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// **Text runs on one line share a baseline.**
    ///
    /// A row of text at two different sizes cannot be lined up by any box alignment: the renderer
    /// centres each run's line box in the rect it is given and drops the baseline an ascent below
    /// that box's top, and *both* of those scale with the font size. Centre the boxes and the
    /// smaller run's baseline lands above the larger one's — it reads as a superscript. `Start`
    /// aligns their tops, `End` their bottoms; none of the four is what a reader means by "on the
    /// same line".
    ///
    /// So the row does it: measure where each text child's baseline falls inside its own box, and
    /// drop the shallower ones until they all meet the deepest. Nothing is added to the row's
    /// height — a child is only ever moved down into space its taller sibling already claimed.
    ///
    /// It lives here rather than at a call site because every row of text wants it and none of them
    /// should have to know it: a name beside a dimmed `(program)` suffix, a heading beside a count,
    /// a label beside a unit. The alternative is each caller forcing one font size on both runs,
    /// which is a design decision made to work around a layout defect.
    fn on_layout(&mut self) {
        if self.base.style.layout.direction != Direction::Row {
            return;
        }
        // **Look through wrappers.** A child is rarely the text itself: it is a `Visibility`, a
        // `KeyHint`, a `Tooltip` — transparent things whose whole point is that they behave as the
        // widget inside them. Asking only the direct child "are you text?" makes every wrapped run
        // invisible to this pass, which is exactly how the sidebar's `(program)` suffix went on
        // being misaligned while a two-bare-label test said it was fixed.
        fn text_leaf(c: &dyn Component) -> Option<&dyn Component> {
            if c.measure_text().is_some() {
                return Some(c);
            }
            // **Only through a transparent wrapper** — a node that exists to stand for the one
            // inside it. `Base::transparent` is the same answer the picker uses to the same
            // question, so there is one notion of "this node is not a thing in its own right".
            //
            // ⚠️ Recursing into ANY container is what made this wrong: a row whose children are
            // layouts rather than text runs — the exposé's strip, whose children are columns of
            // pane cards — found the first label buried somewhere inside each column and treated
            // them as words on one line. A column of two cards has its first label halfway down
            // its first card, a column of one has it halfway down the column, so the shallower
            // baseline was "corrected" downwards and a whole column of panes was drawn below the
            // box it belongs to.
            if !c.base().transparent {
                return None;
            }
            c.base().children.iter().find_map(|k| text_leaf(k.as_ref()))
        }
        // Where that text's baseline falls, in the row's own coordinates.
        //
        // **Only text that is actually inside the child counts.** A widget that positions its own
        // content puts some of it *outside* its box — an open `Select`'s option rows hang below the
        // trigger in a panel — and that text is not on this row's line. Without this check the hunt
        // found the last row of an open dropdown, called it the line's deepest baseline, and
        // dropped every label beside the select 40px to meet it: neighbours laid out below the row
        // they belong to, drawing over whatever was under them (F003/P096/T483).
        fn baseline_of(c: &dyn Component) -> Option<f64> {
            let leaf = text_leaf(c)?.base();
            let (own, box_) = (leaf.bounds, c.base().bounds);
            let inside = own.loc.y >= box_.loc.y - 0.5
                && own.loc.y + own.size.h <= box_.loc.y + box_.size.h + 0.5;
            if !inside {
                return None;
            }
            let line = (leaf.font * crate::font::MONO_LINE_RATIO) as f64;
            let ascent =
                (leaf.font * crate::font::MONO_LINE_RATIO * crate::font::BASELINE_RATIO) as f64;
            Some(leaf.bounds.loc.y + (leaf.bounds.size.h - line) / 2.0 + ascent)
        }
        let deepest = self
            .base
            .children
            .iter()
            .filter_map(|c| baseline_of(c.as_ref()))
            .fold(f64::NEG_INFINITY, f64::max);
        if !deepest.is_finite() {
            return;
        }
        for child in self.base.children.iter_mut() {
            let Some(own) = baseline_of(child.as_ref()) else {
                continue;
            };
            let dy = deepest - own;
            if dy.abs() > 0.01 {
                crate::component::shift_subtree(child.as_mut(), 0.0, dy);
            }
        }
    }
}

impl LayoutExt for Flex {}
impl Parent for Flex {}

/// A vertical container — a [`Flex`] column. Convenience alias for readability.
pub type Container = Flex;

/// Construct a vertical container.
pub fn container() -> Flex {
    Flex::column()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A row aligns text runs on its line — not whole layouts that happen to contain text.**
    ///
    /// The baseline pass exists so a name and a dimmed `(program)` suffix sit on one line. It hunts
    /// for the text inside each child, because a run is often wrapped. Hunting through *any*
    /// container made every child of every row a candidate, so a row of columns — the exposé's
    /// strip — had its columns treated as words: the first label of a column of two panes sits
    /// halfway down its first card, the label of a column of one sits halfway down the column, and
    /// the shallower of the two was pushed down to "meet" the deeper. A whole column of panes was
    /// drawn below the box it belongs to (Antonio, driving, 2026-09-16).
    ///
    /// ⚠️ Ran red first: without the wrapper check the second column drops by half a card.
    #[test]
    fn a_row_does_not_baseline_align_children_that_are_layouts() {
        use crate::builders::{LayoutExt, Parent};
        use crate::style::{Justify, Length};
        use crate::widgets::{Flex, Label};
        use crate::{Component, LayoutEngine};
        use heca_core::layout::Size;

        // A card: a share of its column with its label centred inside it.
        let card = || {
            Flex::column()
                .grow(1.0)
                .height(0.0)
                .shrink(1.0)
                .justify(Justify::Center)
                .child(Label::new("zsh"))
        };
        let column = |n: usize| {
            let mut f = Flex::column();
            for _ in 0..n {
                f = f.child(card());
            }
            f.width(Length::Percent(0.5))
        };

        let mut root: Box<dyn Component> = Box::new(
            Flex::row()
                .width(Length::Percent(1.0))
                .height(Length::Percent(1.0))
                .child(column(2))
                .child(column(1)),
        );
        LayoutEngine::new().compute(root.as_mut(), Size::new(1000.0, 1000.0));

        for (i, col) in root.base().children.iter().enumerate() {
            let b = col.base().bounds;
            assert_eq!(
                b.loc.y, 0.0,
                "column {i} starts at the top of the row, not pushed down to meet a baseline",
            );
            assert!(
                b.loc.y + b.size.h <= 1000.5,
                "column {i} stays inside the row: {b:?}",
            );
        }
    }

    use crate::builders::Parent;
    use crate::widgets::{Label, Visibility};
    use crate::{Component, LayoutEngine};
    use heca_core::layout::Size;

    /// Where a run's baseline falls, read the way [`Flex::on_layout`] reads it.
    fn baseline(c: &dyn Component) -> Option<f64> {
        fn leaf(c: &dyn Component) -> Option<&dyn Component> {
            if c.measure_text().is_some() {
                return Some(c);
            }
            c.base().children.iter().find_map(|k| leaf(k.as_ref()))
        }
        let b = leaf(c)?.base();
        let line = (b.font * crate::font::MONO_LINE_RATIO) as f64;
        let ascent = (b.font * crate::font::MONO_LINE_RATIO * crate::font::BASELINE_RATIO) as f64;
        Some(b.bounds.loc.y + (b.bounds.size.h - line) / 2.0 + ascent)
    }

    fn laid_out(row: Flex) -> Flex {
        let mut row = row;
        LayoutEngine::new()
            .base_font(14.0)
            .compute(&mut row, Size::new(400.0, 100.0));
        row
    }

    /// **Two runs at different sizes sit on one baseline** — the thing no box alignment can do.
    #[test]
    fn text_runs_of_different_sizes_share_a_baseline() {
        let row = laid_out(
            Flex::row()
                .child(Label::new("NAME").bold(true))
                .child(Label::new("(zsh)").font_scale(0.8))
                .child(Label::new("tiny").font_scale(0.6)),
        );
        let mut seen = row.base().children.iter().filter_map(|c| baseline(c.as_ref()));
        let first = seen.next().expect("the row has text in it");
        for other in seen {
            assert!(
                (other - first).abs() < 0.01,
                "baselines drifted: {first} vs {other}",
            );
        }
    }

    /// **…including a run inside a transparent wrapper**, which is the case that actually occurs:
    /// a widget is rarely bare, it is wrapped in a `Visibility` / `KeyHint` / `Tooltip` whose whole
    /// point is to behave as the thing inside it. Asking only the direct child "are you text?" made
    /// every wrapped run invisible to the pass, so the row it was written for stayed misaligned
    /// while a two-bare-label test reported success.
    #[test]
    fn a_wrapped_text_run_shares_the_baseline_too() {
        let row = laid_out(
            Flex::row()
                .child(Label::new("fafdsa").bold(true))
                .child(Visibility::new(Label::new("(zsh)").font_scale(0.8), true)),
        );
        let name = baseline(row.base().children[0].as_ref()).expect("the name is text");
        let wrapped = baseline(row.base().children[1].as_ref()).expect("the wrapper holds text");
        assert!(
            (name - wrapped).abs() < 0.01,
            "a wrapped run did not join the baseline: {name} vs {wrapped}",
        );
    }

    /// Aligning baselines must never make the row taller — a run only moves down into space a
    /// taller sibling already claimed.
    #[test]
    fn sharing_a_baseline_does_not_grow_the_row() {
        let tall = laid_out(Flex::row().child(Label::new("NAME").bold(true))).base().bounds.size.h;
        let mixed = laid_out(
            Flex::row()
                .child(Label::new("NAME").bold(true))
                .child(Label::new("(zsh)").font_scale(0.8)),
        )
        .base()
        .bounds
        .size
        .h;
        assert_eq!(tall, mixed, "the row grew to fit a baseline shift");
    }
}
