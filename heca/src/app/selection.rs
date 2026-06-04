//! App-level pane selection helpers.
//!
//! These helpers support cross-workspace letter selection and safe lookup of
//! panes that participate in swap-style actions.

use heca_core::layout::Session;

/// Extended alphabet for pane candidate labels (52 chars).
const CANDIDATE_ALPHABET: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

/// Collect ALL panes across ALL workspaces as letter candidates.
/// Hard-capped at 52 unique labels (a–z, A–Z). Beyond that, use sidebar
/// navigation instead of letter selection.
pub(crate) fn collect_all_pane_candidates(session: &Session) -> Vec<(char, u64)> {
    let mut candidates = Vec::new();
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                if candidates.len() >= CANDIDATE_ALPHABET.len() {
                    return candidates;
                }
                let ch = CANDIDATE_ALPHABET[candidates.len()];
                candidates.push((ch, pane.id.0));
            }
        }
        for float in &ws.floating_panes {
            if candidates.len() >= CANDIDATE_ALPHABET.len() {
                return candidates;
            }
            let ch = CANDIDATE_ALPHABET[candidates.len()];
            candidates.push((ch, float.pane.id.0));
        }
    }
    candidates
}

/// Find the (workspace_index, column_index, pane_index) containing a pane.
pub(crate) fn find_pane_location(session: &Session, pane_id: u64) -> Option<(usize, usize, usize)> {
    let target = heca_core::layout::PaneId(pane_id);
    for (ws_idx, ws) in session.workspaces.iter().enumerate() {
        for (col_idx, col) in ws.scrolling.columns.iter().enumerate() {
            if let Some(pane_idx) = col.panes.iter().position(|p| p.id == target) {
                return Some((ws_idx, col_idx, pane_idx));
            }
        }
    }
    None
}
