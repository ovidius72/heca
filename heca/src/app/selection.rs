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

/// Collect every pane that could be picked, across all workspaces, as letter candidates.
/// Hard-capped at 52 unique labels (a–z, A–Z). Beyond that, use sidebar
/// navigation instead of letter selection.
///
/// **`except` is never a candidate** — every pane pick means "the other one" (focus it, swap with
/// it, take it), so offering the pane you are already on is a move to nowhere.
///
/// That rule used to live in the letter layer instead, which meant the candidate list and the
/// letters gave different answers: with a single pane open, `prefix+q` counted one candidate,
/// entered the pick and announced itself in the bottom bar — and then the only letter was filtered
/// away, so the prompt sat there over a screen with nothing to press (Antonio, driving). Whoever
/// counts the candidates has to be the one who decides what a candidate is.
pub(crate) fn collect_all_pane_candidates(
    session: &Session,
    except: Option<PaneId>,
) -> Vec<(char, PaneId)> {
    let mut candidates = Vec::new();
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                if Some(pane.id) == except {
                    continue;
                }
                // Running out of letters *is* the cap — no second number to keep in step.
                let Some(ch) = candidate_letter(candidates.len()) else {
                    return candidates;
                };
                candidates.push((ch, pane.id));
            }
        }
        for float in &ws.floating_panes {
            if Some(float.pane.id) == except {
                continue;
            }
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
pub(crate) fn collect_workspace_candidates(
    session: &Session,
) -> Vec<(char, usize, heca_core::layout::WorkspaceId)> {
    let active = session.active_workspace_idx;
    session
        .workspaces
        .iter()
        .enumerate()
        .filter(|(ws_idx, _)| *ws_idx != active)
        // Zipping the alphabet caps the list at its length, so the cap cannot drift from it.
        .zip(heca_grid_ui::widgets::DEFAULT_LETTERS.chars())
        .map(|((ws_idx, ws), ch)| (ch, ws_idx, ws.id))
        .collect()
}

/// Collect **all** columns across **all** workspaces as letter candidates
/// (letter → `(ws_idx, col_idx)`) for the "move pane to column" pick — a pane can be
/// stacked into a column in any workspace. Capped at 52.
pub(crate) fn collect_column_candidates(
    session: &Session,
) -> Vec<(char, usize, usize, heca_core::layout::ColumnId)> {
    session
        .workspaces
        .iter()
        .enumerate()
        .flat_map(|(ws_idx, ws)| {
            ws.scrolling
                .columns
                .iter()
                .enumerate()
                .map(move |(col_idx, col)| (ws_idx, col_idx, col.id))
        })
        .zip(heca_grid_ui::widgets::DEFAULT_LETTERS.chars())
        .map(|((ws_idx, col_idx, col_id), ch)| (ch, ws_idx, col_idx, col_id))
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
        let candidates = collect_all_pane_candidates(&session, None);
        assert_eq!(candidates.len(), PANE_CANDIDATE_LIMIT);
        assert!(has_pane_candidate_overflow(&session));
    }

    /// **The pane you are on is not a candidate**, so the count and the letters are one answer.
    ///
    /// The rule used to be applied by the letter layer instead, which meant a pick could be entered
    /// with a candidate that was never going to be lettered: with a single pane open, `prefix+q`
    /// counted one, entered the pick, put "pick a letter" in the bottom bar — and drew no letter
    /// anywhere (Antonio, driving). Whoever counts has to decide.
    #[test]
    fn the_pane_you_are_on_is_never_a_candidate() {
        let session = make_session_with_panes(3);
        let all = collect_all_pane_candidates(&session, None);
        assert_eq!(all.len(), 3);

        let others = collect_all_pane_candidates(&session, Some(PaneId(2)));
        assert_eq!(others.len(), 2, "the pane you are on is gone");
        assert!(
            !others.iter().any(|(_, id)| *id == PaneId(2)),
            "and it is that pane, not merely one fewer",
        );
    }

    /// **The only pane there is leaves nothing to pick.** This is the case that showed a prompt
    /// with no letters: the pick must come out empty so it can be refused out loud instead.
    #[test]
    fn a_single_pane_leaves_no_one_to_pick() {
        let session = make_session_with_panes(1);
        assert!(
            collect_all_pane_candidates(&session, Some(PaneId(1))).is_empty(),
            "with one pane open, every pane pick has nothing to offer",
        );
    }

    /// The letters are handed out **after** the exclusion, so the panes that remain get the front of
    /// the alphabet rather than inheriting a gap where the excluded one was — two panes to pick from
    /// wear the same two letters however many were left out.
    #[test]
    fn the_letters_close_up_around_the_pane_that_was_left_out() {
        let letters = |session, except| -> Vec<char> {
            collect_all_pane_candidates(session, except)
                .into_iter()
                .map(|(ch, _)| ch)
                .collect()
        };
        let two = make_session_with_panes(2);
        let three = make_session_with_panes(3);
        assert_eq!(
            letters(&three, Some(PaneId(1))),
            letters(&two, None),
            "the alphabet is the library's; what matters is that it starts at the beginning",
        );
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
