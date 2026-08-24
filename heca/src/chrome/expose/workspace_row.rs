//! **One workspace's row** — its columns side by side at the widths they really have, with its
//! floating panes drawn over them where they really sit.
//!
//! It owns the **horizontal share** and the **float placement**. Both are answered against one
//! denominator, and choosing that denominator is the whole of what this component knows:
//!
//! - the row's own **extent** — its strip, or a float that sticks out past the end of it — decides
//!   how the columns divide the row;
//! - the **widest extent in the map** decides how much of the map this row takes, so two workspaces
//!   are drawn at the same scale as each other and the picture reads as one picture.
//!
//! Nothing here multiplies anything by a scale: a column is a percentage, a float is a rect of
//! percentages, and taffy resolves both against whatever room the window turns out to give.

use heca_core::layout::PaneId;
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::style::{Align, Justify, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{Flex, GridCell};

use super::column_card::ColumnCard;
use super::model::ExposeWorkspace;
use super::pane_card::{ExposeCallbacks, PaneCard};

/// A workspace's row in the map.
pub(crate) struct WorkspaceRow<'a> {
    pub(crate) workspace: &'a ExposeWorkspace,
    /// **The widest extent in the whole map**, never this row's own — the one shared denominator
    /// that keeps every row to the same scale. A row measured against itself always fills the
    /// width, so a workspace holding one pane would look exactly like one holding six.
    pub(crate) widest: f64,
    /// The air a row reserves around its strip, in the model's own units — see
    /// [`ExposeGrid`](super::expose_grid::ExposeGrid), which computes it once for the map.
    pub(crate) gap: f64,
    /// The pane a back-and-forth binding would return to, if it is in this workspace.
    pub(crate) previous: Option<PaneId>,
    pub(crate) theme: &'a GuiTheme,
    pub(crate) cb: &'a ExposeCallbacks,
}

/// **How far this workspace reaches**: its strip, or a floating pane that sticks out past the end
/// of it. A card off the right edge is exactly what the shares exist to stop, so a float's own
/// extent counts.
pub(crate) fn extent(ws: &ExposeWorkspace) -> f64 {
    ws.floating
        .iter()
        .map(|f| f.x + f.w)
        .fold(ws.strip_width, f64::max)
        .max(1.0)
}

impl WorkspaceRow<'_> {
    /// Build the row and the cells the grid navigates it by, in the order the eye reads them:
    /// down each column, then the next column, then the floats.
    pub(crate) fn build(self) -> (Flex, Vec<Vec<GridCell>>) {
        let ws = self.workspace;
        let extent = extent(ws);
        let screen_h = ws.viewport_h.max(1.0);

        // **Where the cursor goes when a card is deleted: the next one in this row.**
        //
        // Computed here, before anything is deleted, because afterwards the pane is gone and its
        // neighbour can no longer be found from it. The order is the one the eye reads, so "next"
        // is the card to the right or below, and the last card falls back to the one before it.
        // Nothing left in the row means nothing to say, and the map's ordinary fallback takes over.
        let order: Vec<PaneId> = ws
            .columns
            .iter()
            .flat_map(|c| c.panes.iter().map(|p| p.pane_id))
            .chain(ws.floating.iter().map(|f| f.pane_id))
            .collect();
        let next_of = move |id: PaneId| -> Option<PaneId> {
            let at = order.iter().position(|p| *p == id)?;
            order
                .get(at + 1)
                .or_else(|| at.checked_sub(1).and_then(|p| order.get(p)))
                .copied()
        };

        let mut columns_of_cells: Vec<Vec<GridCell>> = Vec::new();
        // **The strip is the workspace's whole scrollable width**, drawn at this row's scale. It is
        // a real box rather than a wrapper for its own sake: the floats are positioned against it,
        // and centring it is what puts a short workspace in the middle of the map instead of
        // against the left edge.
        let mut strip = Flex::row()
            .width(Length::Pct((extent / self.widest) as f32))
            // **The row reserves its own air.** Its allotment includes the gap (see `ExposeGrid`),
            // and the strip takes the screen's share of that, centred — so half a gap sits above
            // and half below, and between two rows they meet as one. Expressed this way the gap is
            // a share like everything else and stays a tenth of a screen at every window size,
            // rather than a token tuned to text.
            .height(Length::Pct((screen_h / (screen_h + self.gap)) as f32));

        for col in &ws.columns {
            let (column, cells) = ColumnCard {
                column: col,
                ws_idx: ws.ws_idx,
                previous: self.previous,
                next_of: &next_of,
                theme: self.theme,
                cb: self.cb,
            }
            .build();
            columns_of_cells.push(cells);
            // **A column's width is its share of the row's extent** — a percentage, not a `grow`
            // weight. `flex-grow` distributes *free space*, so grown columns always fill their
            // container and two of them would spread across the whole strip and look like six.
            // That is what flex-grow means and is not a bug to fix: a share of a fixed denominator
            // is a percentage.
            strip = strip.child(column.width(Length::Pct((col.width / extent) as f32)));
        }

        // **A floating pane is drawn over the strip, where it actually sits.** It belongs to no
        // column, so it cannot be a child of one: it is placed at its own rect, in the strip
        // coordinates the model already resolved, as a fraction of the strip on one axis and of the
        // screen on the other. `at_rect` takes it out of the flow, so it neither takes width from
        // the columns nor is pushed along by them.
        let mut float_cells = Vec::new();
        for float in &ws.floating {
            let (card, cell) = PaneCard {
                pane_id: float.pane_id,
                name: &float.name,
                active: float.active,
                folder: float.folder.as_deref(),
                previous: self.previous == Some(float.pane_id),
                ws_idx: ws.ws_idx,
                // A float has no column, so the letter that deletes a column names the last one —
                // it is still the column the user is looking at. `x` and `d` mean what they mean
                // everywhere else.
                col_idx: ws.columns.len().saturating_sub(1),
                next: next_of(float.pane_id),
                theme: self.theme,
                cb: self.cb,
            }
            .build();
            float_cells.push(cell);
            strip = strip.child(card.at_rect(
                Length::Pct((float.x / extent) as f32),
                Length::Pct((float.y / screen_h) as f32),
                Length::Pct((float.w / extent) as f32),
                Length::Pct((float.h / screen_h) as f32),
            ));
        }
        // **A float is a card like any other, so the cursor must reach it.** The floats become one
        // more column of cells at the end of the row: stepping right off the last tiled column
        // lands on them. Leaving them out is what made pane focus work in a row without floats and
        // stop in a row with one (Antonio, driving, 2026-08-11).
        if !float_cells.is_empty() {
            columns_of_cells.push(float_cells);
        }

        // The row's whole allotment. The strip is centred in it on both axes: horizontally so a
        // workspace narrower than the widest sits in the middle rather than against the edge,
        // vertically so the air it reserved splits evenly above and below.
        let row = Flex::row().justify(Justify::Center).align(Align::Center).child(strip);
        (row, columns_of_cells)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::expose::model::{ExposeColumn, ExposeFloating, ExposePane};
    use crate::chrome::expose::pane_card::pane_key;
    use crate::chrome::expose::testing::{callbacks, card_of, lay_out, theme};
    use heca_core::layout::PaneId;

    fn pane(id: u64, h: f64) -> ExposePane {
        ExposePane { pane_id: PaneId(id), name: format!("p{id}"), folder: None, active: false, height: h }
    }

    fn workspace(widths: &[f64], floats: Vec<ExposeFloating>) -> ExposeWorkspace {
        let columns: Vec<ExposeColumn> = widths
            .iter()
            .enumerate()
            .map(|(i, w)| ExposeColumn {
                col_idx: i,
                width: *w,
                panes: vec![pane(i as u64 + 1, 600.0)],
            })
            .collect();
        let strip: f64 = columns.iter().map(|c| c.width).sum();
        ExposeWorkspace {
            ws_idx: 0,
            name: "ws".into(),
            active: true,
            columns,
            floating: floats,
            viewport: (0.0, 800.0),
            viewport_h: 600.0,
            strip_width: strip,
        }
    }

    fn row(ws: &ExposeWorkspace, widest: f64, w: f64, h: f64) -> Box<dyn heca_grid_ui::Component> {
        let (cb, _sink) = callbacks();
        let theme = theme();
        let (row, _cells) = WorkspaceRow {
            workspace: ws,
            previous: None,
            widest,
            // No air, so an assertion reads against the numbers it was given rather than against
            // the gap as well. The gap has its own test in `ExposeGrid`.
            gap: 0.0,
            theme: &theme,
            cb: &cb,
        }
        .build();
        lay_out(row.width(Length::Pct(1.0)).height(Length::Pct(1.0)), w, h)
    }

    /// **A column takes the share of the row its real width is worth** — measured against the
    /// widest row in the map, so two workspaces are drawn at the same scale as each other. A share
    /// of its *own* row would always fill, and one column would look exactly like six.
    #[test]
    fn columns_take_the_share_of_the_map_their_real_widths_are_worth() {
        let ws = workspace(&[400.0, 200.0], vec![]);
        // Measured against a map twice as wide as this workspace: the row must then take half.
        let root = row(&ws, 1200.0, 1200.0, 600.0);
        let wide = card_of(root.as_ref(), &pane_key(PaneId(1))).expect("the wide column's card");
        let narrow = card_of(root.as_ref(), &pane_key(PaneId(2))).expect("the narrow one's");
        assert!(
            (wide.size.w / narrow.size.w - 2.0).abs() < 0.15,
            "twice the width is drawn twice as wide: {wide:?} vs {narrow:?}",
        );
        // …and the pair of them take half the map, because the map is twice this row's extent.
        let spanned = (narrow.loc.x + narrow.size.w) - wide.loc.x;
        assert!(
            (spanned - 600.0).abs() < 8.0,
            "a 600-wide strip in a 1200-wide map takes half of it, got {spanned}",
        );
    }

    /// **A short row sits in the middle**, rather than against the left edge — the map is a
    /// picture, and a picture is centred.
    #[test]
    fn a_row_narrower_than_the_map_is_centred_in_it() {
        let ws = workspace(&[400.0], vec![]);
        let root = row(&ws, 1600.0, 1600.0, 600.0);
        let card = card_of(root.as_ref(), &pane_key(PaneId(1))).expect("the only card");
        let mid = card.loc.x + card.size.w / 2.0;
        assert!((mid - 800.0).abs() < 4.0, "centred across a 1600 map, got {mid}: {card:?}");
    }

    /// **A float lands at its own fraction of the row** — across the strip, and down the screen.
    ///
    /// Both axes, because they resolve against different things and only one of them can be a
    /// percentage margin: CSS resolves a percentage margin against the parent's **width** on both
    /// axes, so a fractional `y` written that way is silently wrong on any row that is not square.
    /// `at_rect` is what makes the vertical mean what it says.
    #[test]
    fn a_float_lands_at_its_own_fraction_of_the_row() {
        let ws = workspace(
            &[800.0],
            vec![ExposeFloating {
                pane_id: PaneId(9),
                folder: None,
                name: "float".into(),
                active: false,
                x: 200.0,
                y: 150.0,
                w: 400.0,
                h: 300.0,
            }],
        );
        // Extent 800 wide by 600 tall, drawn into exactly that box, so the fractions come out as
        // the model's own numbers and a wrong axis cannot hide behind a coincidence.
        let root = row(&ws, 800.0, 800.0, 600.0);
        let f = card_of(root.as_ref(), &pane_key(PaneId(9))).expect("the float's card");
        // The strip fills the box here (extent == widest == the width given), so its own origin is
        // the box's and the float's fractions come out as the model's own numbers.
        assert!(
            (f.loc.x - 200.0).abs() < 2.0,
            "a quarter across the strip: {f:?}",
        );
        assert!(
            (f.loc.y - 150.0).abs() < 2.0,
            "and a quarter down the screen — the axis a margin could not express: {f:?}",
        );
        assert!(
            (f.size.w - 400.0).abs() < 2.0 && (f.size.h - 300.0).abs() < 2.0,
            "at its own size: {f:?}",
        );
    }

    /// **A float is drawn OVER the strip, not beside it.** It is out of the flow, so the tiled
    /// cards are exactly where they would be in a workspace with no float at all — the failure this
    /// guards is a float pushing the columns along, or the stacking wrapper collapsing their
    /// height to a sliver (Antonio, driving, 2026-08-11).
    #[test]
    fn a_float_changes_nothing_about_the_tiled_cards() {
        let plain = workspace(&[800.0], vec![]);
        let with_float = workspace(
            &[800.0],
            vec![ExposeFloating {
                pane_id: PaneId(9),
                folder: None,
                name: "float".into(),
                active: false,
                x: 100.0,
                y: 100.0,
                w: 300.0,
                h: 200.0,
            }],
        );
        let a = row(&plain, 800.0, 800.0, 600.0);
        let b = row(&with_float, 800.0, 800.0, 600.0);
        let before = card_of(a.as_ref(), &pane_key(PaneId(1))).expect("without");
        let after = card_of(b.as_ref(), &pane_key(PaneId(1))).expect("with");
        assert!(
            (before.loc.x - after.loc.x).abs() < 1.0
                && (before.loc.y - after.loc.y).abs() < 1.0
                && (before.size.w - after.size.w).abs() < 1.0
                && (before.size.h - after.size.h).abs() < 1.0,
            "a float displaces nothing: {before:?} vs {after:?}",
        );
        assert!(after.size.h > 100.0, "and the tiled card keeps its height: {after:?}");
    }

    /// A float reaching past the end of the strip still has to fit: it widens the row's extent, so
    /// the columns divide what is left. A card off the right edge is what the shares exist to stop.
    #[test]
    fn a_float_past_the_end_of_the_strip_widens_the_rows_extent() {
        let ws = workspace(
            &[400.0],
            vec![ExposeFloating {
                pane_id: PaneId(9),
                folder: None,
                name: "float".into(),
                active: false,
                x: 600.0,
                y: 0.0,
                w: 200.0,
                h: 300.0,
            }],
        );
        assert_eq!(extent(&ws), 800.0, "the float reaches to 800, past the 400-wide strip");
        let root = row(&ws, 800.0, 800.0, 600.0);
        let f = card_of(root.as_ref(), &pane_key(PaneId(9))).expect("the float");
        assert!(
            f.loc.x + f.size.w <= 801.0,
            "and it still lands inside the row: {f:?}",
        );
    }

    /// **The air between the cards is the same everywhere** (Antonio, driving, 2026-08-13: *"the
    /// only thing i see here is the different gap between the first 3/4 card at the top and the
    /// last"*).
    ///
    /// A column's width is a percentage of the map's extent, so its boundaries land on fractional
    /// pixels; the air between two columns is made of **two paddings**, one from each. When a
    /// spacing token resolves to half a pixel the two sides round in opposite directions and the
    /// gap comes out 6, 7 or 8 where every one should be 7 — a 14% variation on a 7px gap, which is
    /// exactly the size of thing an eye reads as an uneven rhythm without being able to name it.
    ///
    /// Fixed in `heca-grid-ui`'s layout pass, where a spacing token now resolves to a whole pixel.
    /// This asserts the property rather than the value, so it holds if the token or the font moves.
    #[test]
    fn the_air_between_the_columns_is_the_same_everywhere() {
        let widths: Vec<f64> = vec![300.0; 14];
        let ws = workspace(&widths, vec![]);
        let extent: f64 = widths.iter().sum();
        let root = row(&ws, extent, 1900.0, 600.0);

        let mut edges = Vec::new();
        for i in 0..widths.len() {
            let c = card_of(root.as_ref(), &pane_key(PaneId(i as u64 + 1)))
                .unwrap_or_else(|| panic!("card {i} of the row"));
            edges.push((c.loc.x, c.loc.x + c.size.w));
        }
        let gaps: Vec<f64> = edges.windows(2).map(|w| w[1].0 - w[0].1).collect();
        let first = gaps[0];
        assert!(
            gaps.iter().all(|g| (g - first).abs() < 0.01),
            "the cards do not sit on an even rhythm: {gaps:?}",
        );
    }
}
