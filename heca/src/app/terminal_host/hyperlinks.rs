//! **Follow-link letters** — one labelled keycap per visible link, across every drawn terminal. Where
//! the links are is the terminal's own to say (`Terminal::links`, `cell_origin`).

use super::frames::laid_out_pane_ids;
use crate::app_state::AppState;

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
            let terminal_widget = state.terminals.get(&terminal)?;
            terminal_widget.placed()?;
            Some((terminal, terminal_widget.links()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::backend::HyperlinkSpan;
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
