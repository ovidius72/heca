//! App-level pane selection helpers.
//!
//! These helpers support cross-workspace letter selection and safe lookup of
//! panes that participate in swap-style actions.

use heca_core::layout::{PaneId, Session};

/// How many targets one pick can letter — the length of the alphabet, not a second number.
pub(crate) const PANE_CANDIDATE_LIMIT: usize = heca_grid_ui::widgets::DEFAULT_LETTERS.len();

/// The candidate label for the `idx`-th item in a letter pick, or `None` past the cap.
///
/// **One alphabet for every pick in the app** — panes, columns, docks, follow-link — and it is the
/// *library's*, so a widget picker (`KeyHintGroup`) and a host pick hand out the same letters in the
/// same order. The app kept its own 52-entry copy until F003/P082/T433; two copies of one decision
/// is how home-row ordering would have had to be applied twice.
pub(crate) fn candidate_letter(idx: usize) -> Option<char> {
    heca_grid_ui::widgets::DEFAULT_LETTERS.chars().nth(idx)
}

pub(crate) fn has_pane_candidate_overflow(session: &Session) -> bool {
    let mut count = 0usize;
    for ws in &session.workspaces {
        count += ws
            .scrolling
            .columns
            .iter()
            .map(|col| col.panes.len())
            .sum::<usize>();
        count += ws.floating_panes.len();
        if count > PANE_CANDIDATE_LIMIT {
            return true;
        }
    }
    false
}

/// Collect ALL panes across ALL workspaces as letter candidates.
/// Hard-capped at 52 unique labels (a–z, A–Z). Beyond that, use sidebar
/// navigation instead of letter selection.
pub(crate) fn collect_all_pane_candidates(session: &Session) -> Vec<(char, PaneId)> {
    let mut candidates = Vec::new();
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                // Running out of letters *is* the cap — no second number to keep in step.
                let Some(ch) = candidate_letter(candidates.len()) else {
                    return candidates;
                };
                candidates.push((ch, pane.id));
            }
        }
        for float in &ws.floating_panes {
            let Some(ch) = candidate_letter(candidates.len()) else {
                return candidates;
            };
            candidates.push((ch, float.pane.id));
        }
    }
    candidates
}

/// Collect workspaces as letter candidates (letter → `ws_idx`) for the "move
/// column/pane to workspace" pick — **excluding the active workspace** (moving the
/// active column/pane to the workspace it already lives in is a no-op). Letters are
/// assigned sequentially over the remaining workspaces; capped at 52.
pub(crate) fn collect_workspace_candidates(session: &Session) -> Vec<(char, usize)> {
    let active = session.active_workspace_idx;
    session
        .workspaces
        .iter()
        .enumerate()
        .filter(|(ws_idx, _)| *ws_idx != active)
        // Zipping the alphabet caps the list at its length, so the cap cannot drift from it.
        .zip(heca_grid_ui::widgets::DEFAULT_LETTERS.chars())
        .map(|((ws_idx, _), ch)| (ch, ws_idx))
        .collect()
}

/// Collect **all** columns across **all** workspaces as letter candidates
/// (letter → `(ws_idx, col_idx)`) for the "move pane to column" pick — a pane can be
/// stacked into a column in any workspace. Capped at 52.
pub(crate) fn collect_column_candidates(session: &Session) -> Vec<(char, usize, usize)> {
    session
        .workspaces
        .iter()
        .enumerate()
        .flat_map(|(ws_idx, ws)| {
            ws.scrolling
                .columns
                .iter()
                .enumerate()
                .map(move |(col_idx, _)| (ws_idx, col_idx))
        })
        .zip(heca_grid_ui::widgets::DEFAULT_LETTERS.chars())
        .map(|((ws_idx, col_idx), ch)| (ch, ws_idx, col_idx))
        .collect()
}

/// Find the (workspace_index, column_index, pane_index) containing a pane.
pub(crate) fn find_pane_location(
    session: &Session,
    pane_id: PaneId,
) -> Option<(usize, usize, usize)> {
    for (ws_idx, ws) in session.workspaces.iter().enumerate() {
        for (col_idx, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pane_idx) = col.panes.iter().position(|p| p.id == pane_id) {
                return Some((ws_idx, col_idx, pane_idx));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        PANE_CANDIDATE_LIMIT, candidate_letter, collect_all_pane_candidates,
        has_pane_candidate_overflow,
    };
    use heca_core::layout::{
        Pane as LayoutPane, PaneId, Session,
        types::{LayoutOptions, SessionId, Size},
    };

    fn make_session_with_panes(count: usize) -> Session {
        let viewport = Size::new(1280.0, 800.0);
        let mut session = Session::new(SessionId(1), viewport, 2.0, LayoutOptions::default());
        for i in 1..=count as u64 {
            let pane = LayoutPane::new(PaneId(i), format!("Pane{i}"));
            session.add_pane(pane, None, true);
        }
        session
    }

    #[test]
    fn candidate_collection_caps_at_limit() {
        let session = make_session_with_panes(PANE_CANDIDATE_LIMIT + 5);
        let candidates = collect_all_pane_candidates(&session);
        assert_eq!(candidates.len(), PANE_CANDIDATE_LIMIT);
        assert!(has_pane_candidate_overflow(&session));
    }

    #[test]
    fn candidate_letter_spans_az_then_caps() {
        assert_eq!(candidate_letter(0), Some('a'));
        assert_eq!(candidate_letter(25), Some('z'));
        assert_eq!(candidate_letter(26), Some('A'));
        assert_eq!(candidate_letter(PANE_CANDIDATE_LIMIT - 1), Some('Z'));
        // Past the 52-label cap there is no letter.
        assert_eq!(candidate_letter(PANE_CANDIDATE_LIMIT), None);
    }
}
