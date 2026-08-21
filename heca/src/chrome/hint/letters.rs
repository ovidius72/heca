//! **Who offered which letter, and who takes it back** — the ownership rule.
//!
//! Several things offer letters: the universal picker, a `KeyHintGroup` a surface opened for
//! itself, and the move/swap/take/pick input modes. The only thing that keeps them from fighting is
//! **ownership** — this remembers exactly which targets *it* lettered and withdraws exactly those.
//! It never clears a label it did not set.
//!
//! **This replaces a mode check, and that is the point.** The first fix was
//! `if matches!(state.input_mode, HintPick { .. }) { return }`, which names a *host* mode: a plugin
//! opening its own group is not `HintPick`, so its letters were clobbered identically and the
//! plugin author had no way to add themselves to that match. Ownership has no such list.

use crate::app_state::InputMode;
use super::targets::visible_pane_targets;
use heca_core::layout::PaneId;
use crate::providers::workspaces::{column_key, pane_key, workspace_key};

/// The targets this module has a letter on, by `key`. Withdrawal is exactly this set, which is
/// what makes the rule ownership rather than "clear everything and hope".
#[derive(Default, Debug)]
pub(crate) struct OfferedLetters {
    keys: Vec<String>,
}

/// What the **current input mode** wants lettered, as `(key, letter)`.
///
/// One place that knows how a mode's candidates become row identities, so a new pick mode is one
/// arm here rather than a fifth signal list and a fifth projection.
fn wanted(mode: &InputMode, active_pane: Option<heca_core::layout::PaneId>) -> Vec<(String, char)> {
    let mut out = Vec::new();
    if let Some(cands) = mode.candidates() {
        // **The focused pane is never a target**, even when the mode lists it: every one of these
        // picks means "the other one", so lettering where you already are offers a move to nowhere.
        out.extend(
            cands
                .iter()
                .filter(|(_, id)| Some(*id) != active_pane)
                .map(|(ch, id)| (pane_key(*id), *ch)),
        );
    }
    if let Some(cands) = mode.ws_candidates() {
        out.extend(cands.iter().map(|(ch, ws)| (workspace_key(*ws), *ch)));
    }
    if let Some(cands) = mode.col_candidates() {
        out.extend(
            cands
                .iter()
                .map(|(ch, ws, col)| (column_key(*ws, *col), *ch)),
        );
    }
    // A dock names itself with `scope_key` rather than `key` — a container's identity, not a
    // row's — and `offer_hint_by_key` matches either, so this is the same one line as the rest.
    if let Some(cands) = mode.dock_candidates() {
        out.extend(cands.iter().map(|(ch, id)| (id.clone(), *ch)));
    }
    out
}

/// Bring the offered letters in line with the active mode: withdraw what we lettered and no longer
/// want, offer what we want now.
///
/// Runs every frame and is cheap when nothing is picking — the common case is two empty vectors.
/// Returns whether anything changed, so the caller can decide to repaint.
pub(crate) fn sync_offered_letters(state: &crate::app_state::AppState) -> bool {
    let wanted = wanted(&state.input_mode, state.focused_pane);
    // **Which pane shells could actually show a letter.** Computed ONCE per pass, not per key: it
    // resolves the whole surface stack.
    let visible_panes = if wanted.is_empty() {
        std::collections::HashSet::new()
    } else {
        visible_pane_targets(state)
    };
    let mut changed = false;

    // Withdraw first, so a target that keeps its letter across a mode change is not briefly cleared.
    let stale: Vec<String> = state
        .offered_letters
        .borrow()
        .keys
        .iter()
        .filter(|k| !wanted.iter().any(|(want, _)| want == *k))
        .cloned()
        .collect();
    for key in &stale {
        offer_in_every_tree(state, key, None, &visible_panes);
        changed = true;
    }

    for (key, ch) in &wanted {
        if offer_in_every_tree(state, key, Some(ch.to_string()), &visible_panes) {
            changed = true;
        }
    }

    // **Say what is lettered, once, when it changes** — `hint.changed`. A plugin subscribes to this
    // to render its own prompt or highlight beside heca's keycaps (`app.on("hint.changed", …)`),
    // which is why it is emitted here rather than by whatever happened to set a mode: this is the
    // one place that knows the whole picture.
    //
    // Change-guarded, like every other chrome event: this runs every frame and the common case is
    // that nothing is picking.
    let mut offered = state.offered_letters.borrow_mut();
    let next: Vec<String> = wanted.iter().map(|(k, _)| k.clone()).collect();
    if offered.keys != next {
        state
            .chrome_state
            .events()
            .emit(crate::chrome::ChromeEvent::HintLettersChanged {
                letters: wanted.iter().map(|(k, ch)| (*ch, k.clone())).collect(),
            });
    }
    offered.keys = next;
    changed
}

/// Offer `label` to whichever retained tree declares `key`. Front to back, so a surface in
/// front shadows one behind it — the nearest declaration wins, as everywhere else.
fn offer_in_every_tree(
    state: &crate::app_state::AppState,
    key: &str,
    label: Option<String>,
    visible_panes: &std::collections::HashSet<PaneId>,
) -> bool {
    for layer in state.layers.visible_front_to_back() {
        if heca_grid_ui::offer_hint_by_key(layer.root(), key, label.clone()) {
            return true;
        }
    }
    // Below the layers there is no shadowing, because these are not competing surfaces — they are
    // **several views of one thing**. A pane and the sidebar row that names it both declare
    // `pane:7`, and both must wear the letter: the pane owns the identity, the row shows it. An
    // early return here is what made a pane listed in BOTH sidebars get lettered in only one of
    // them (F003/P082, found by tracing `nodes naming it: chrome=2` while one letter went out).
    let mut offered = false;
    if let Some(tree) = state.chrome_tree.as_ref() {
        offered |= heca_grid_ui::offer_hint_by_key(&tree.root, key, label.clone());
    }
    // **A pane scrolled behind a sidebar is skipped — the pane, not the pick.** Its keycap draws on
    // the overlay layer, so it would land on top of the very thing covering it (Antonio, driving,
    // 2026-08-19). Its sidebar row is a second view of the same pane and IS visible, so it still
    // wears the letter and the pane stays reachable — which is why this filters the VIEW rather
    // than the candidate. Visibility is asked of `resolve_hint_layers`, never re-derived here.
    for (pane_id, shell) in state.panes.iter() {
        if !visible_panes.contains(pane_id) {
            continue;
        }
        offered |= heca_grid_ui::offer_hint_by_key(&shell.root, key, label.clone());
    }
    for header in state.pane_headers.values() {
        offered |= heca_grid_ui::offer_hint_by_key(&header.root, key, label.clone());
    }
    offered
}

/// [`wanted`] for a test in another module — the mapping is the interesting part and belongs to
/// this file, so its assertions live wherever the case is clearest rather than being re-derived.
#[cfg(test)]
pub(crate) fn wanted_for_tests(mode: &InputMode) -> Vec<(String, char)> {
    wanted(mode, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::PaneId;

    /// **A mode's candidates become row identities, in one place.** A new pick mode is an arm in
    /// `wanted`, not a fifth signal list, a fifth projection and a fifth thing to remember.
    #[test]
    fn a_modes_candidates_become_the_rows_own_identities() {
        let mode = InputMode::PaneSelect {
            candidates: vec![('a', PaneId(7)), ('b', PaneId(9))],
        };
        assert_eq!(
            wanted(&mode, None),
            vec![
                (pane_key(PaneId(7)), 'a'),
                (pane_key(PaneId(9)), 'b'),
            ],
        );
    }

    /// **Nothing picking, nothing wanted** — so nothing is withdrawn from anyone else. This is the
    /// assertion the old per-frame projection failed: it wrote `None` to every target it knew about
    /// whenever its own candidates were empty, which is how it erased the universal picker's
    /// letters and would have erased a plugin's.
    #[test]
    fn a_mode_that_is_not_picking_wants_nothing() {
        assert!(wanted(&InputMode::Normal, None).is_empty());
    }

    /// **The focused pane is never a target**, even when the mode lists it as a candidate: every
    /// one of these picks means "the other one", so a letter where you already are offers a move to
    /// nowhere. The rule came from the host projection this replaced and had to travel with it.
    #[test]
    fn the_pane_you_are_on_gets_no_letter() {
        let mode = InputMode::PaneSelect {
            candidates: vec![('a', PaneId(1)), ('s', PaneId(2))],
        };
        assert_eq!(
            wanted(&mode, Some(PaneId(1))),
            vec![(pane_key(PaneId(2)), 's')],
        );
    }
    /// **`hint.changed` fires when the lettering changes, and not otherwise** — the event a plugin
    /// subscribes to (`app.on("hint.changed", …)`) to render its own prompt beside heca's keycaps.
    ///
    /// It replaced `pane.pick.changed` and `dock.pick.changed`, which were emitted by four per-frame
    /// projections that no longer exist. One mechanism, one event: a pane, a workspace, a column and
    /// a dock are all lettered through the same door now.
    #[test]
    fn the_lettering_is_announced_once_per_real_change() {
        use crate::chrome::SharedChromeState;
        use std::cell::RefCell;
        use std::rc::Rc;

        let store = SharedChromeState::new(300.0, true, 300.0, false);
        let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let _sub = store
            .events()
            .subscribe("hint.changed", move |e| log.borrow_mut().push(e.name().to_string()));

        store
            .events()
            .emit(crate::chrome::ChromeEvent::HintLettersChanged {
                letters: vec![('a', pane_key(PaneId(1)))],
            });

        assert_eq!(seen.borrow().len(), 1, "the name a plugin filters on is `hint.changed`");
    }

}

// ── Which targets exist, and which of them are reachable ─────────────────────
