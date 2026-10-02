//! **Links and cells** — which cell is under a point, which link is in it, and where a cell is on
//! screen. The geometry is the terminal's own (`Terminal::cell_at`, `cell_origin`); this adds what
//! the backend's snapshot knows about its hyperlinks.

use super::frames::laid_out_pane_ids;
use crate::app_state::AppState;
use heca_core::layout::PaneId;

/// The cell under `pos` in a pane's terminal, from the terminal itself.
fn cell_coords_at_position(
    state: &AppState,
    pane_id: PaneId,
    pos: (f32, f32),
) -> Option<(usize, usize)> {
    crate::chrome::terminal::of_pane(state, pane_id)?
        .cell_at(pos)
        .map(|c| (c.row, c.col))
}

/// If the pointer at `pos` lands on a captured hyperlink cell within `pane_id`,
/// return its target URI.
///
/// Resolves the cell under the pointer (`cell_coords_at_position`) and looks it
/// up against the pane's `snapshot.hyperlinks` (OSC 8 + auto-detected, same
/// pipeline). `start_col` is inclusive and `end_col` is exclusive, matching the
/// capture/renderer contract. Used by the Cmd+click open-link surface
/// (`terminal-task-18`).
pub(crate) fn hyperlink_uri_at_position(
    state: &AppState,
    pane_id: PaneId,
    pos: (f32, f32),
) -> Option<String> {
    let (row, col) = cell_coords_at_position(state, pane_id, pos)?;
    let snapshot = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())?;
    hyperlink_at_cell(&snapshot.hyperlinks, row, col).map(str::to_owned)
}

/// Build the follow-link candidates across **every terminal that was drawn last frame**, whoever
/// owns it: one labelled keycap per visible hyperlink span (OSC 8 + auto-detected, same pipeline),
/// assigned letters sequentially (a–z A–Z, shared 52-letter cap) — the panes in the order they are
/// laid out first, then the terminals no pane owns. Each candidate carries its own terminal. A
/// terminal that says it was not drawn — scrolled off, hidden, or in a tree that is not up — is
/// skipped: ask the terminal, not the layout. terminal-task-18.
pub(crate) fn collect_link_hints(state: &AppState) -> Vec<crate::app_state::LinkHint> {
    let drawn = terminals_in_order(state)
        .into_iter()
        .filter_map(|terminal| {
            state.terminals.get(&terminal)?.placed()?;
            let snapshot = state.backends.get_by_id(terminal)?.terminal_snapshot()?;
            Some((terminal, snapshot.hyperlinks))
        });
    label_links(drawn)
}

/// Give each link, in order, the next letter — and stop at the cap. Pure, so what a letter points at
/// is tested without a window.
fn label_links(
    terminals: impl IntoIterator<
        Item = (
            crate::chrome::terminal::TerminalId,
            Vec<heca_core::backend::HyperlinkSpan>,
        ),
    >,
) -> Vec<crate::app_state::LinkHint> {
    let mut hints = Vec::new();
    for (terminal, spans) in terminals {
        for span in spans {
            let Some(label) = crate::app::selection::candidate_letter(hints.len()) else {
                return hints; // 52-label cap reached.
            };
            hints.push(crate::app_state::LinkHint {
                label,
                terminal,
                row: span.row,
                start_col: span.start_col,
                url: span.uri,
            });
        }
    }
    hints
}

/// Every terminal the app has a widget for: the panes' in layout order, then the rest by id — so a
/// letter stays with the same terminal from one frame to the next.
fn terminals_in_order(state: &AppState) -> Vec<crate::chrome::terminal::TerminalId> {
    let mut ordered: Vec<_> = laid_out_pane_ids(state)
        .into_iter()
        .filter_map(|pane| state.backends.identity_of(pane))
        .collect();
    let mut rest: Vec<_> = state
        .terminals
        .keys()
        .copied()
        .filter(|id| !ordered.contains(id))
        .collect();
    rest.sort_by_key(|id| id.0);
    ordered.extend(rest);
    ordered
}

/// Screen position (logical px, top-left) of cell `(row, col)` in a terminal's content, or `None`
/// if it was not drawn. The inverse of `cell_coords_at_position`; used to stamp follow-link keycaps
/// over a link's first cell.
pub(crate) fn cell_screen_pos(
    state: &AppState,
    terminal: crate::chrome::terminal::TerminalId,
    row: usize,
    col: usize,
) -> Option<(f32, f32)> {
    state.terminals.get(&terminal)?.cell_origin(row, col)
}

/// Target URI of the hyperlink at a **stable-row** cell in `pane_id`, if any.
///
/// Selection state is keyed by stable rows (history-stable), while hyperlink
/// spans are indexed by visible viewport row; this converts via
/// `stable - viewport_top_stable_row` and returns `None` when the cell is
/// scrolled out of the visible range. Used by follow-link-at-caret (`O` in
/// selection mode). terminal-task-18.
pub(crate) fn hyperlink_uri_at_stable_cell(
    state: &AppState,
    pane_id: PaneId,
    stable_row: isize,
    col: usize,
) -> Option<String> {
    let snapshot = state
        .backends
        .get(pane_id)
        .and_then(|backend| backend.terminal_snapshot())?;
    let visible_row = stable_row - snapshot.viewport_top_stable_row;
    if visible_row < 0 || visible_row >= snapshot.rows as isize {
        return None;
    }
    hyperlink_at_cell(&snapshot.hyperlinks, visible_row as usize, col).map(str::to_owned)
}

/// Find the hyperlink span covering cell `(row, col)`, if any.
///
/// `start_col` is inclusive, `end_col` exclusive (the capture/renderer
/// contract). The first matching span wins; OSC 8 capture never overlaps spans
/// and auto-detection skips cells already linked, so at most one matches.
fn hyperlink_at_cell(
    hyperlinks: &[heca_core::backend::HyperlinkSpan],
    row: usize,
    col: usize,
) -> Option<&str> {
    hyperlinks
        .iter()
        .find(|span| span.row == row && col >= span.start_col && col < span.end_col)
        .map(|span| span.uri.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::backend::HyperlinkSpan;
    fn span(row: usize, start_col: usize, end_col: usize, uri: &str) -> HyperlinkSpan {
        HyperlinkSpan {
            row,
            start_col,
            end_col,
            uri: uri.to_owned(),
        }
    }

    #[test]
    fn hit_test_matches_inclusive_start_and_exclusive_end() {
        let links = [span(2, 4, 9, "https://example.com")];
        // Inclusive start.
        assert_eq!(hyperlink_at_cell(&links, 2, 4), Some("https://example.com"));
        // Interior cell.
        assert_eq!(hyperlink_at_cell(&links, 2, 8), Some("https://example.com"));
        // end_col is exclusive: the cell at end_col is not part of the link.
        assert_eq!(hyperlink_at_cell(&links, 2, 9), None);
        // Just before the start.
        assert_eq!(hyperlink_at_cell(&links, 2, 3), None);
    }

    #[test]
    fn hit_test_is_row_specific() {
        let links = [span(2, 4, 9, "https://example.com")];
        // Right columns, wrong row.
        assert_eq!(hyperlink_at_cell(&links, 1, 5), None);
        assert_eq!(hyperlink_at_cell(&links, 3, 5), None);
    }

    #[test]
    fn hit_test_picks_the_covering_span_among_several() {
        let links = [
            span(0, 0, 3, "http://a"),
            span(0, 10, 14, "http://b"),
            span(5, 2, 6, "mailto:x@y.z"),
        ];
        assert_eq!(hyperlink_at_cell(&links, 0, 1), Some("http://a"));
        assert_eq!(hyperlink_at_cell(&links, 0, 12), Some("http://b"));
        assert_eq!(hyperlink_at_cell(&links, 5, 5), Some("mailto:x@y.z"));
        // Gap between spans on row 0.
        assert_eq!(hyperlink_at_cell(&links, 0, 7), None);
    }

    #[test]
    fn hit_test_empty_list_is_none() {
        assert_eq!(hyperlink_at_cell(&[], 0, 0), None);
    }

    use crate::chrome::terminal::TerminalId;

    fn link(row: usize, uri: &str) -> HyperlinkSpan {
        HyperlinkSpan {
            row,
            start_col: 2,
            end_col: 9,
            uri: uri.to_string(),
        }
    }

    /// **A terminal no pane owns gets link letters like a pane's**: one run of letters across every
    /// drawn terminal, each hint carrying the terminal it points into.
    #[test]
    fn letters_run_across_terminals_and_each_hint_names_its_terminal() {
        let hints = label_links([
            (
                TerminalId(1),
                vec![link(0, "https://a"), link(3, "https://b")],
            ),
            (TerminalId(7), vec![link(1, "https://c")]),
        ]);

        let lettered: Vec<_> = hints
            .iter()
            .map(|h| (h.label, h.terminal, h.url.as_str()))
            .collect();
        let letter = |i| crate::app::selection::candidate_letter(i).expect("a letter");
        assert_eq!(
            lettered,
            vec![
                (letter(0), TerminalId(1), "https://a"),
                (letter(1), TerminalId(1), "https://b"),
                (letter(2), TerminalId(7), "https://c"),
            ]
        );
    }

    #[test]
    fn the_letters_stop_at_the_cap_instead_of_wrapping() {
        let many: Vec<_> = (0..60).map(|i| link(i, "https://x")).collect();
        assert_eq!(label_links([(TerminalId(1), many)]).len(), 52);
    }
}
