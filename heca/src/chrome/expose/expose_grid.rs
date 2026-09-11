//! **The map itself** — every workspace's row, stacked, each taking the share of the screen its
//! own screen is worth.
//!
//! It owns the **row share** and the **cursor**. The cursor is [`CardGrid`]'s: this component hands
//! it the cells each row produced, in the order the eye reads them, and binds the three things a
//! host must hear — the cursor moved, a card was chosen, the map was dismissed.
//!
//! **Nothing in it computes a pixel.** The map fills the room it is given, whatever that is, and
//! every row is a `grow` weight against every other. That is why there is no fit to get wrong and
//! no scroll region underneath: a picture built out of shares of the window cannot overflow it.

use heca_core::layout::PaneId;
use heca_grid_ui::builders::LayoutExt;
use heca_grid_ui::style::Length;
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::CardGrid;

use super::model::ExposeWorkspace;
use super::pane_card::ExposeCallbacks;
use super::workspace_row::{self, WorkspaceRow};

/// The whole map. Properties in, one navigable grid out.
pub(crate) struct ExposeGrid<'a> {
    pub(crate) rows: &'a [ExposeWorkspace],
    /// The gap between rows as a **fraction of a screen height** — niri's `overview_gap`, and the
    /// reason it is a fraction: a tenth of a screen stays the same picture at every window size,
    /// while a token tuned to text would read as a hairline in a large map and a canyon in a small
    /// one.
    pub(crate) gap_frac: f64,
    /// Where the cursor opens, if that pane is still on the map.
    pub(crate) start: Option<PaneId>,
    /// The pane a back-and-forth binding would return to — marked, not selected.
    pub(crate) previous: Option<PaneId>,
    pub(crate) theme: &'a GuiTheme,
    pub(crate) cb: &'a ExposeCallbacks,
}

impl ExposeGrid<'_> {
    pub(crate) fn build(self) -> CardGrid {
        // **One denominator for the whole map.** Measured across every row, so a workspace holding
        // one narrow column is drawn narrow — the comparison between workspaces is the information
        // a bird's-eye exists to give.
        let widest = self
            .rows
            .iter()
            .map(workspace_row::extent)
            .fold(1.0_f64, f64::max);
        // The air between rows, in the model's own units: a fraction of the **tallest** screen, so
        // every row reserves the same amount and the gaps up the map are even.
        let gap = self
            .rows
            .iter()
            .map(|ws| ws.viewport_h)
            .fold(0.0_f64, f64::max)
            * self.gap_frac;

        let mut grid = CardGrid::new().width(Length::Percent(1.0)).height(Length::Percent(1.0));
        for ws in self.rows {
            let (row, cells) = WorkspaceRow {
                workspace: ws,
                widest,
                gap,
                previous: self.previous,
                theme: self.theme,
                cb: self.cb,
            }
            .build();
            // **A row's share of the map is its own screen height, plus the air it reserves.**
            // `viewport_h` is per workspace and they are not all the same, which is why summing
            // has to be the engine's job: one row's height times N was wrong by exactly the
            // difference, and the last row fell off the bottom however the rest was corrected.
            grid = grid.row(cells, super::share_v(row, ws.viewport_h + gap));
        }
        if let Some(id) = self.start {
            grid = grid.selected(id.0.to_string());
        }
        grid
            // **Every move is reported to the host.** The map is rebuilt from scratch each time it
            // opens, so it has no memory of its own — where the highlight was is something only
            // `AppState` can still know a moment later.
            .on_move({
                let cursor_to = self.cb.cursor_to.clone();
                move |key| {
                    if let Ok(id) = key.parse::<u64>() {
                        cursor_to(PaneId(id));
                    }
                }
            })
            .on_activate({
                let choose = self.cb.choose.clone();
                move |key| {
                    if let Ok(id) = key.parse::<u64>() {
                        choose(PaneId(id));
                    }
                }
            })
            .on_dismiss({
                let dismiss = self.cb.dismiss.clone();
                move || dismiss()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::expose::model::{ExposeColumn, ExposePane, ExposeWorkspace};
    use crate::chrome::expose::pane_card::pane_key;
    use crate::chrome::expose::testing::{callbacks, card_of, cards_bounds, lay_out, theme};

    /// A workspace of `cols` columns on a screen of `screen`, each column one pane.
    fn ws(idx: usize, cols: usize, screen: (f64, f64)) -> ExposeWorkspace {
        let columns: Vec<ExposeColumn> = (0..cols)
            .map(|c| ExposeColumn {
                col_idx: c,
                width: screen.0 / 2.0,
                panes: vec![ExposePane {
                    pane_id: PaneId((idx * 100 + c + 1) as u64),
                    folder: None,
                    name: format!("w{idx}c{c}"),
                    active: c == 0,
                    height: screen.1,
                }],
            })
            .collect();
        let strip: f64 = columns.iter().map(|c| c.width).sum();
        ExposeWorkspace {
            ws_idx: idx,
            name: format!("ws{idx}"),
            active: idx == 0,
            columns,
            floating: Vec::new(),
            viewport: (0.0, screen.0),
            viewport_h: screen.1,
            strip_width: strip,
        }
    }

    fn grid(rows: &[ExposeWorkspace], w: f64, h: f64) -> Box<dyn heca_grid_ui::Component> {
        let (cb, _sink) = callbacks();
        let theme = theme();
        let g = ExposeGrid { rows, gap_frac: 0.1, start: None, previous: None, theme: &theme, cb: &cb }.build();
        lay_out(g, w, h)
    }

    /// **The map's cards stay keyed** (F003/P082/T444) — the exposé's half of the identity-rule
    /// test.
    ///
    /// The map is a collection of collections: workspaces of columns of panes, every one of them a
    /// card the cursor stops on and a letter can land on. Built here so nothing can be told apart by
    /// its content — every pane in every workspace is called `zsh` — because that is the case a
    /// derived identity cannot serve, and the case a real session produces the moment you open two
    /// shells.
    #[test]
    fn every_card_of_the_map_is_keyed_even_when_every_pane_shares_a_name() {
        let twins = |idx: usize| ExposeWorkspace {
            ws_idx: idx,
            name: format!("ws{idx}"),
            active: idx == 0,
            columns: (0..2)
                .map(|c| ExposeColumn {
                    col_idx: c,
                    width: 400.0,
                    panes: vec![
                        ExposePane {
                            pane_id: PaneId((idx * 100 + c * 10 + 1) as u64),
                            folder: None,
                            name: "zsh".into(),
                            active: false,
                            height: 300.0,
                        },
                        ExposePane {
                            pane_id: PaneId((idx * 100 + c * 10 + 2) as u64),
                            folder: None,
                            name: "zsh".into(),
                            active: false,
                            height: 300.0,
                        },
                    ],
                })
                .collect(),
            floating: Vec::new(),
            viewport: (0.0, 800.0),
            viewport_h: 600.0,
            strip_width: 800.0,
        };
        let rows = [twins(0), twins(1)];
        let map = grid(&rows, 900.0, 700.0);

        let ambiguous = heca_grid_ui::nav::ambiguous_identities(map.as_ref());
        assert!(
            ambiguous.is_empty(),
            "a collection in the exposé lost its keys: {ambiguous:#?}",
        );
    }

    /// ⚠️ **THE test.** The whole map stays inside the box it is given — at several window sizes,
    /// several row counts, and rows of unequal height.
    ///
    /// This is the one assertion that would have caught **every** one of the seven attempts that
    /// preceded this design (F003/P082/T420), each of which forgot a different term: the overlay's
    /// viewport margin, the panel's padding, the gaps between columns, one row's height times N
    /// instead of the sum, a boundary landing exactly on the edge. None of those terms exists any
    /// more — a picture built out of shares of the window cannot overflow it — and this is what
    /// says so, rather than a comment claiming it.
    #[test]
    fn the_whole_map_never_exceeds_the_box_it_is_given() {
        let sizes = [(1280.0, 800.0), (1900.0, 1200.0), (640.0, 480.0), (2560.0, 700.0)];
        let sessions: Vec<Vec<ExposeWorkspace>> = vec![
            vec![ws(0, 1, (800.0, 600.0))],
            vec![ws(0, 2, (800.0, 600.0)), ws(1, 6, (800.0, 600.0))],
            // Rows of **unequal** height — the case that made "one row's height times N" wrong.
            vec![
                ws(0, 3, (800.0, 600.0)),
                ws(1, 1, (1600.0, 1000.0)),
                ws(2, 8, (1200.0, 400.0)),
            ],
            (0..6).map(|i| ws(i, i + 1, (800.0, 600.0))).collect(),
        ];
        for rows in &sessions {
            for (w, h) in sizes {
                let root = grid(rows, w, h);
                let drawn = cards_bounds(root.as_ref()).expect("the map has cards");
                assert!(
                    drawn.loc.x >= -1.0
                        && drawn.loc.y >= -1.0
                        && drawn.loc.x + drawn.size.w <= w + 1.0
                        && drawn.loc.y + drawn.size.h <= h + 1.0,
                    "{} workspaces in {w}x{h}: the cards reach {drawn:?}",
                    rows.len(),
                );
                // …and they are cards, not slivers: the map fills what it was given.
                assert!(
                    drawn.size.h > h * 0.5 && drawn.size.w > 1.0,
                    "{} workspaces in {w}x{h}: the map collapsed to {drawn:?}",
                    rows.len(),
                );
            }
        }
    }

    /// **A row takes the share of the map its own screen is worth.** `viewport_h` is per workspace
    /// and they are not all the same — summing is the engine's job, and one row's height times N is
    /// wrong by exactly the difference, which is how the last row kept falling off the bottom.
    #[test]
    fn rows_take_the_share_of_the_map_their_screens_are_worth() {
        let rows = vec![ws(0, 1, (800.0, 600.0)), ws(1, 1, (800.0, 1200.0))];
        let root = grid(&rows, 1200.0, 900.0);
        let first = card_of(root.as_ref(), &pane_key(PaneId(1))).expect("the first row's card");
        let second = card_of(root.as_ref(), &pane_key(PaneId(101))).expect("the second's");
        let ratio = second.size.h / first.size.h;
        assert!(
            (ratio - 2.0).abs() < 0.15,
            "a screen twice as tall gets twice the row, got {ratio:.2}: {first:?} then {second:?}",
        );
    }

    /// **The rows are separated, and by the fraction of a screen the config asks for.** The gap is
    /// a share like everything else, so it stays the same picture at every window size.
    #[test]
    fn the_rows_are_separated_by_a_fraction_of_a_screen() {
        let rows = vec![ws(0, 1, (800.0, 600.0)), ws(1, 1, (800.0, 600.0))];
        let root = grid(&rows, 1200.0, 900.0);
        let first = card_of(root.as_ref(), &pane_key(PaneId(1))).expect("the first");
        let second = card_of(root.as_ref(), &pane_key(PaneId(101))).expect("the second");
        let air = second.loc.y - (first.loc.y + first.size.h);
        // A tenth of a row, give or take the rounding of a share.
        assert!(
            air > first.size.h * 0.05 && air < first.size.h * 0.2,
            "a tenth of a screen between the rows, got {air} against a row of {}",
            first.size.h,
        );
    }

    /// **Exactly one card holds the keyboard as the cursor moves.** `focus_path` takes the deepest
    /// widget carrying the flag, so a second card left holding it silently keeps every key: the
    /// cursor moves, the grid reports the move handled, and the card you left answers anyway
    /// (Antonio, driving, 2026-08-11 — the cursor stuck while `ctrl+l` kept answering `Yes`).
    #[test]
    fn exactly_one_card_holds_the_keyboard_as_the_cursor_moves() {
        use heca_grid_ui::event::{Event, WidgetIntent};
        use heca_grid_ui::reactive::SignalGet;
        let rows = vec![ws(0, 3, (800.0, 600.0))];
        let (cb, _sink) = callbacks();
        let theme = theme();
        // Opened **on a card**: the cursor lights a cell when it is placed, and a map opened on
        // nothing has nothing focused — which is correct, and would make this prove nothing.
        let g = ExposeGrid {
            rows: &rows,
            gap_frac: 0.1,
            start: Some(PaneId(1)),
            previous: None,
            theme: &theme,
            cb: &cb,
        }
        .build();
        let mut root = lay_out(g, 1200.0, 900.0);

        fn focused_keys(n: &dyn heca_grid_ui::Component, out: &mut Vec<String>) {
            if n.base().focused.get_untracked()
                && let Some(k) = n.base().key.as_deref()
            {
                out.push(k.to_string());
            }
            for c in n.base().children.iter() {
                focused_keys(c.as_ref(), out);
            }
        }

        let mut seen = Vec::new();
        for step in 0..3 {
            let mut focused = Vec::new();
            focused_keys(root.as_ref(), &mut focused);
            assert!(
                focused.len() <= 1,
                "step {step}: {} cards hold the keyboard at once: {focused:?}",
                focused.len(),
            );
            seen.push(focused.first().cloned());
            heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
            heca_grid_ui::LayoutEngine::new()
                .compute(root.as_mut(), heca_grid_ui::Size::new(1200.0, 900.0));
        }
        assert!(
            seen.iter().filter(|s| s.is_some()).collect::<std::collections::HashSet<_>>().len() > 1,
            "the focused card must CHANGE as the cursor moves, got {seen:?}",
        );
    }

    /// **The cursor walks from the last tiled pane onto a float.** A float belongs to no column, so
    /// it is in no cell unless the row puts it in one — and moving between panes worked in a row
    /// without floats and stopped in a row with one (Antonio, driving, 2026-08-11).
    #[test]
    fn the_cursor_walks_from_the_last_tiled_pane_onto_a_float() {
        use crate::chrome::expose::model::ExposeFloating;
        use heca_grid_ui::event::{Event, WidgetIntent};
        let mut rows = vec![ws(0, 1, (800.0, 600.0))];
        rows[0].floating.push(ExposeFloating {
            pane_id: PaneId(9),
            folder: None,
            name: "float".into(),
            active: false,
            x: 40.0,
            y: 30.0,
            w: 200.0,
            h: 150.0,
        });
        let (cb, sink) = callbacks();
        let theme = theme();
        let g = ExposeGrid {
            rows: &rows,
            gap_frac: 0.1,
            // Start on the last tiled pane, so one step right is the float.
            start: Some(PaneId(1)),
            previous: None,
            theme: &theme,
            cb: &cb,
        }
        .build();
        let mut root = lay_out(g, 1200.0, 900.0);
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::ItemNext));
        heca_grid_ui::dispatch(root.as_mut(), &Event::Widget(WidgetIntent::Activate));

        let focused: Vec<PaneId> = sink
            .borrow()
            .iter()
            .filter_map(|i| match i {
                crate::app::interaction::InteractionIntent::FocusPaneThenAction { pane_id, .. } => {
                    Some(*pane_id)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            focused,
            vec![PaneId(9)],
            "one step past the last tiled pane lands on the float, and choosing it focuses it",
        );
    }
}
