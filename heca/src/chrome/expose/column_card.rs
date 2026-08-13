//! **One column of a workspace** — its panes stacked, sharing the column's height by the heights
//! they really have.
//!
//! It owns the **vertical share** and nothing else: how wide the column is belongs to the
//! [`WorkspaceRow`](super::workspace_row::WorkspaceRow), which is the only thing that can see all
//! the columns there are to compare it against.

use heca_core::layout::PaneId;
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::style::Spacing;
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Flex, GridCell};

use super::model::ExposeColumn;
use super::pane_card::{ExposeCallbacks, PaneCard};

/// A column in the map. Invisible grouping — no box, no border, no fill; only the panes show.
pub(crate) struct ColumnCard<'a> {
    pub(crate) column: &'a ExposeColumn,
    /// The workspace this column sits in, for the letter that deletes it.
    pub(crate) ws_idx: usize,
    /// The pane a back-and-forth binding would return to, if any.
    pub(crate) previous: Option<PaneId>,
    /// Where the cursor goes when one of these cards is deleted — asked per pane, while the row it
    /// belongs to is still whole.
    pub(crate) next_of: &'a dyn Fn(PaneId) -> Option<PaneId>,
    pub(crate) theme: &'a GuiTheme,
    pub(crate) cb: &'a ExposeCallbacks,
}

impl ColumnCard<'_> {
    /// Build the stack and the cells the grid navigates it by, top to bottom.
    ///
    /// The concrete [`Flex`] comes back rather than a box because the row still has to give it its
    /// width — see [`PaneCard::build`](super::pane_card::PaneCard::build) for the same reason.
    pub(crate) fn build(self) -> (Flex, Vec<GridCell>) {
        // **The air between columns is this column's padding, not the row's gap.**
        //
        // A gap would be added *outside* the width shares, so the widest row would come out at
        // 100% plus its gaps and overflow by exactly that much — the same "one term forgotten"
        // failure this whole task exists to remove. Padding sits **inside** the border box, so the
        // share stays exact and the air appears between the cards all the same. Vertically a gap is
        // safe (`grow` divides what is left *after* gaps), so the panes below use one.
        let mut stack = Flex::column().pad_x(Spacing::Xs).gap_spacing(Spacing::Xs);
        let mut cells = Vec::new();
        for pane in &self.column.panes {
            let (card, cell) = PaneCard {
                pane_id: pane.pane_id,
                name: &pane.name,
                active: pane.active,
                previous: self.previous == Some(pane.pane_id),
                ws_idx: self.ws_idx,
                col_idx: self.column.col_idx,
                next: (self.next_of)(pane.pane_id),
                theme: self.theme,
                cb: self.cb,
            }
            .build();
            cells.push(cell);
            // **A pane's share of its column is its real height**, so a stack the user has dragged
            // out of balance is drawn out of balance. The weights are the resolved heights the
            // layout already decided, and the column is the screen at this row's scale, so the
            // ratios come out true without this component scaling anything itself.
            stack = stack.child(super::share_v(card, pane.height));
        }
        (stack, cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::expose::model::ExposePane;
    use crate::chrome::expose::testing::{callbacks, card_of, lay_out, theme};
    use crate::chrome::expose::pane_card::pane_nav_key;
    use heca_grid_ui::style::Length;

    fn column(heights: &[f64]) -> Box<dyn heca_grid_ui::Component> {
        let (cb, _sink) = callbacks();
        let theme = theme();
        let col = ExposeColumn {
            col_idx: 0,
            width: 400.0,
            panes: heights
                .iter()
                .enumerate()
                .map(|(i, h)| ExposePane {
                    pane_id: PaneId(i as u64 + 1),
                    name: format!("p{i}"),
                    active: i == 0,
                    height: *h,
                })
                .collect(),
        };
        let (stack, _cells) = ColumnCard {
            column: &col,
            ws_idx: 0,
            previous: None,
            next_of: &|_| None,
            theme: &theme,
            cb: &cb,
        }
        .build();
        // The width a row would give it; the height is the box it must divide.
        lay_out(stack.width(Length::Pct(1.0)).height(Length::Pct(1.0)), 400.0, 600.0)
    }

    /// **A pane takes the share of its column its real height is worth.** A stack the user dragged
    /// out of balance is drawn out of balance — the map divided every column evenly once, and a
    /// pane at twice its neighbour's height was drawn as its twin.
    #[test]
    fn panes_take_the_share_of_the_column_their_real_heights_are_worth() {
        let root = column(&[400.0, 200.0]);
        let top = card_of(root.as_ref(), &pane_nav_key(PaneId(1))).expect("the first card");
        let bottom = card_of(root.as_ref(), &pane_nav_key(PaneId(2))).expect("the second");
        let drawn = top.size.h / bottom.size.h;
        assert!(
            (drawn - 2.0).abs() < 0.1,
            "twice the height is drawn twice as tall, got {drawn:.2}: {top:?} over {bottom:?}",
        );
    }

    /// **The stack never exceeds the column it is given** — at any number of panes, at any heights.
    /// A share cannot overflow by construction, and this is the test that says so out loud: it is
    /// the arithmetic that used to be done by hand, one forgotten term at a time.
    #[test]
    fn a_stack_never_exceeds_the_column_it_is_given() {
        for heights in [
            vec![100.0],
            vec![100.0, 100.0],
            vec![900.0, 40.0, 300.0],
            vec![10.0; 12],
        ] {
            let root = column(&heights);
            let box_h = root.base().bounds.size.h;
            let mut low = 0.0_f64;
            for i in 0..heights.len() {
                let c = card_of(root.as_ref(), &pane_nav_key(PaneId(i as u64 + 1)))
                    .unwrap_or_else(|| panic!("card {i} of {heights:?}"));
                low = low.max(c.loc.y + c.size.h);
            }
            assert!(
                low <= box_h + 1.0,
                "{} panes at {heights:?} reach {low} in a column of {box_h}",
                heights.len(),
            );
        }
    }
}
