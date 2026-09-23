//! **The exposé's data** — what is on screen, where, and how big, as plain values.
//!
//! One row per workspace, each carrying **all** of that workspace's columns — including the ones
//! scrolled off-screen, which is the whole point of the view — with its panes, its floats and the
//! shape of the screen they live on.
//!
//! **This module is the model, not the widgets.** Reducing the session here, once, is what lets
//! every component above it take plain data and be built in a test without a window
//! ([`super`] § "never `&AppState`"). Nothing in it knows about pixels, cards or the layout
//! engine: it reports the session's own numbers, in the session's own units, and the components
//! turn them into **shares**.
//!
//! Panes are drawn as **boxes** for now: a border, the pane's name and its icon. A real snapshot
//! needs a scene primitive `heca-grid-ui` does not have (its vocabulary is `Rect`, `Text`, `Icon`,
//! clips — there is no image command) plus per-pane offscreen capture in `heca-renderer`. Nothing
//! in the layout, the navigation or the layer changes when that lands: only what fills the box.

use heca_core::layout::{PaneId, Session};

/// One pane in the overview — a box, positioned by its column.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposePane {
    pub(crate) pane_id: PaneId,
    /// The pane's display name. Resolved by the caller through `pane_info_view`, the one name
    /// composer, so a pane reads the same here as in the sidebar and its header.
    pub(crate) name: String,
    /// Is this the focused pane of its workspace?
    pub(crate) active: bool,
    /// **Where the pane is** — its working directory, already home-relative, or `None` when the
    /// backend never reported one. Plain text by the time it reaches a card: `register` resolves it
    /// from `AppState`, so the components below stay testable without a window.
    pub(crate) folder: Option<String>,
    /// The pane's **resolved** height in layout pixels, read from `Column::pane_sizes` for the same
    /// reason [`ExposeColumn::width`] is read from `column_widths`: the layout already decided it,
    /// and a second calculation here would be a second answer that can disagree.
    ///
    /// This is what makes a stack of unequal panes look unequal. The map divided every column
    /// evenly before, so a pane the user had dragged to twice its neighbour's height — its
    /// `preferred_height` — was drawn as its twin, and the picture disagreed with the screen it is
    /// a picture of.
    pub(crate) height: f64,
}

/// One column of a workspace, with the width it really has.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeColumn {
    pub(crate) col_idx: usize,
    /// The column's **resolved** width in layout pixels, read from
    /// `ScrollingSpace::column_widths` rather than recomputed — the layout already decided it, and
    /// a second calculation here would be a second answer that can disagree.
    pub(crate) width: f64,
    pub(crate) panes: Vec<ExposePane>,
}

/// A floating pane, which carries its own geometry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeFloating {
    pub(crate) pane_id: PaneId,
    pub(crate) name: String,
    /// Where the pane is — see [`ExposePane::folder`]. A float is a pane like any other here.
    pub(crate) folder: Option<String>,
    /// Is this the workspace's active pane? A float carries the flag itself
    /// (`FloatingPane::is_active`) because it is not in a column and so has no
    /// `active_pane_idx` to be compared against.
    pub(crate) active: bool,
    /// Position **in strip coordinates**, i.e. already offset by the workspace's scroll.
    ///
    /// A float is positioned relative to the *viewport*, not the strip (`app/render.rs:340`:
    /// `fx = float.position.x + pane_area.loc.x + ws_offset.x`), so it does not scroll with the
    /// columns. A bird's-eye shows the whole strip, so the viewport's own strip position has to be
    /// added back or a float lands in the wrong place the moment the workspace is scrolled. That
    /// position is `view_pos()` — `column_x(active) + view_offset` — **not** `view_offset`, which
    /// is measured from the active column.
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) w: f64,
    pub(crate) h: f64,
}

/// One workspace's row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposeWorkspace {
    pub(crate) ws_idx: usize,
    pub(crate) name: String,
    /// Is this the workspace the user is currently in?
    pub(crate) active: bool,
    pub(crate) columns: Vec<ExposeColumn>,
    pub(crate) floating: Vec<ExposeFloating>,
    /// Where the visible viewport sits within the strip: `(x, width)` in the same units as
    /// [`ExposeColumn::width`]. This is what lets the row show which part you are actually looking
    /// at, and it is why the rows in a bird's-eye are offset from one another.
    pub(crate) viewport: (f64, f64),
    /// The workspace's viewport height — the other half of its shape, so a row can be drawn as the
    /// screen scaled rather than as a bar of arbitrary height.
    pub(crate) viewport_h: f64,
    /// The strip's total width — the sum of the columns, or the viewport when there are none. The
    /// scale factor for the row is this against the room the row is given.
    pub(crate) strip_width: f64,
}

/// Build the overview model from the session — **pure**, so the geometry is testable without a
/// window (an `AppState` needs one).
///
/// `name_of` resolves a pane's display name; the caller passes it because the one name composer
/// (`pane_info_view`) needs the programs config and the live runtime, neither of which belongs in
/// a geometry function.
/// `name_of` composes a pane's display name (through `pane_info_view`, the one name composer, so a
/// pane reads the same here as in the sidebar and its header). `folders` is `[settings]
/// pane_show_cwd` — read once by `register` and applied here, so a card is handed a folder or it is
/// not, and no component below carries a flag it only passes on.
pub(crate) fn model(
    session: &Session,
    mut name_of: impl FnMut(&heca_core::layout::Pane) -> String,
    folders: bool,
) -> Vec<ExposeWorkspace> {
    session
        .workspaces
        .iter()
        .enumerate()
        .map(|(ws_idx, ws)| {
            let scrolling = &ws.scrolling;
            let columns: Vec<ExposeColumn> = scrolling
                .columns
                .iter()
                .enumerate()
                .map(|(col_idx, col)| ExposeColumn {
                    col_idx,
                    // The resolved width when the layout has one for this index; otherwise the
                    // column's own minimum. A missing entry means the layout has not run yet.
                    width: scrolling
                        .column_widths
                        .get(col_idx)
                        .copied()
                        .unwrap_or(heca_core::layout::scrolling::MIN_COLUMN_WIDTH),
                    panes: col
                        .panes
                        .iter()
                        .enumerate()
                        .map(|(pane_idx, pane)| ExposePane {
                            pane_id: pane.id,
                            name: name_of(pane),
                            // **"Turned off" and "there is none" are the same absence.** The
                            // setting is read once, here, rather than carried down four components
                            // as a flag every one of them would have to pass on untouched.
                            folder: folders
                                .then_some(pane.runtime.cwd.as_deref())
                                .flatten()
                                .map(crate::chrome::home_relative_path),
                            active: pane_idx == col.active_pane_idx
                                && col_idx == scrolling.active_column_idx,
                            // The layout's resolved height, which already accounts for
                            // `preferred_height`. A missing entry means the layout has not run
                            // yet, and equal weights are exactly the even split it falls back to.
                            height: col
                                .pane_sizes
                                .get(pane_idx)
                                .map(|s| s.h)
                                .filter(|h| *h > 0.0)
                                .unwrap_or(1.0),
                        })
                        .collect(),
                })
                .collect();

            let strip: f64 = columns.iter().map(|c| c.width).sum();
            // **`view_offset` is relative to the ACTIVE COLUMN, not to the strip.** The strip
            // position of the visible area is `column_x(active) + view_offset`, which the layout
            // already spells as `view_pos()`. Read raw, the offset put the workspace rectangle at
            // the wrong place in every scrolled workspace — the map centred on a window that was
            // never where it said, and the pane you were on sat off the right edge.
            let view_x = scrolling.view_pos();
            let view_w = scrolling.working_area.size.w;

            ExposeWorkspace {
                ws_idx,
                name: ws
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Workspace {}", ws_idx + 1)),
                active: ws_idx == session.active_workspace_idx,
                floating: ws
                    .floating_panes
                    .iter()
                    .map(|f| ExposeFloating {
                        pane_id: f.pane.id,
                        name: name_of(&f.pane),
                        folder: folders
                            .then_some(f.pane.runtime.cwd.as_deref())
                            .flatten()
                            .map(crate::chrome::home_relative_path),
                        active: f.is_active,
                        // Viewport-relative → strip-relative. See the field's own note.
                        x: f.position.x + view_x,
                        // (`view_x` is `view_pos()`, the viewport's own place in the strip.)
                        y: f.position.y,
                        w: f.size.w,
                        h: f.size.h,
                    })
                    .collect(),
                columns,
                viewport: (view_x, view_w),
                viewport_h: scrolling.working_area.size.h,
                // A workspace with no columns is still a row, as wide as its viewport, so an empty
                // workspace does not collapse to nothing and become unselectable.
                strip_width: if strip > 0.0 { strip } else { view_w },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::expose::testing::session;
    use heca_core::layout::{Pane, Size};

    /// Column widths are the layout's **resolved** ones, and the row knows how wide the whole strip
    /// is — that is what makes the bird's-eye proportional to the real thing rather than a diagram.
    #[test]
    fn a_row_carries_the_real_column_widths_and_the_strip_it_scrolls() {
        let s = session();
        let rows = model(&s, |p| p.title.clone(), false);

        assert_eq!(rows.len(), s.workspaces.len(), "one row per workspace");
        assert_eq!(rows[0].name, "Editing");
        assert!(rows[0].active, "the first workspace is the active one");
        assert_eq!(rows[1].name, "Workspace 2");
        assert!(!rows[1].active);

        let strip: f64 = rows[0].columns.iter().map(|c| c.width).sum();
        assert_eq!(
            rows[0].strip_width, strip,
            "the strip is the sum of its columns"
        );
        assert!(
            rows[0].columns.iter().all(|c| c.width > 0.0),
            "every column has a real width: {:?}",
            rows[0].columns,
        );
        assert_eq!(
            rows[0].viewport.1, 800.0,
            "the viewport is the working area's width"
        );

        // An empty workspace is still a row, as wide as its viewport — it must stay selectable.
        assert!(rows[1].columns.is_empty());
        assert_eq!(rows[1].strip_width, rows[1].viewport.1);
    }

    /// **A stack the user dragged out of balance is carried out of balance.** The map divided every
    /// column evenly once, so a pane resized to twice its neighbour's height was reported as its
    /// twin — the picture disagreeing with the screen it is a picture of. That the *drawn* cards
    /// keep the ratio is `ColumnCard`'s test.
    #[test]
    fn the_model_carries_the_resolved_pane_heights_not_an_even_split() {
        let mut s = session();
        let ws = &mut s.workspaces[0];
        ws.scrolling
            .add_pane_to_column(0, None, Pane::new(PaneId(3), "c"), false);
        let (h, gaps) = (ws.scrolling.working_area.size.h, ws.scrolling.options.gaps);
        ws.scrolling.columns[0].move_pane_boundary(0, 120.0, h, gaps);

        let rows = model(&s, |p| p.title.clone(), false);
        let stacked = &rows[0].columns[0].panes;
        assert_eq!(stacked.len(), 2, "two panes share the first column");
        assert!(
            stacked[0].height > stacked[1].height + 100.0,
            "the model carries the resolved heights: {stacked:?}",
        );
    }

    /// **A floating pane is reported in strip coordinates.** It is positioned against the viewport
    /// and does not scroll with the columns, so the scroll offset has to be added back or it lands
    /// in the wrong place the moment the workspace is scrolled.
    #[test]
    fn a_floating_pane_is_offset_by_the_scroll_so_it_lands_where_it_looks() {
        use heca_core::layout::Point;
        use heca_core::layout::workspace::FloatingPane;
        let mut s = session();
        s.workspaces[0].floating_panes.push(FloatingPane {
            pane: Pane::new(PaneId(9), "float"),
            position: Point::new(40.0, 30.0),
            size: Size::new(200.0, 150.0),
            is_active: false,
            original_column_idx: None,
            original_pane_idx: None,
        });
        s.workspaces[0].scrolling.view_offset =
            heca_core::layout::view_offset::ViewOffset::Static(120.0);

        // **Stand on a later column**, so `column_x(active)` is not zero and reading `view_offset`
        // raw gives a different answer from `view_pos()`. With the active column at 0 the two
        // coincide and the bug hides — which is exactly how it survived until it was on screen.
        s.workspaces[0].scrolling.active_column_idx = 1;
        let anchor = s.workspaces[0].scrolling.column_x(1);
        assert!(
            anchor > 0.0,
            "the second column starts somewhere other than 0: {anchor}"
        );

        let rows = model(&s, |p| p.title.clone(), false);
        let f = &rows[0].floating[0];
        assert_eq!(f.pane_id, PaneId(9));
        assert_eq!(
            f.x,
            40.0 + anchor + 120.0,
            "40 viewport-relative + the viewport's own strip position (column_x(active) + 120)",
        );
        assert_eq!(
            rows[0].viewport.0,
            anchor + 120.0,
            "and the row's visible area is at view_pos(), not at the bare view_offset",
        );
        assert_eq!(f.y, 30.0, "vertical does not scroll");
        assert_eq!((f.w, f.h), (200.0, 150.0), "a float carries its own size");
    }
}
