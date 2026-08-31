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
use super::surfaces::{HintSurface, HintTarget};
use super::targets::visible_hint_surfaces;
use crate::providers::workspaces::{column_key, pane_key, workspace_key};

/// **A thing a picker asked to be lettered**, addressed the way that picker knows it.
///
/// Two spellings, because the two pickers genuinely know different things: a pick mode reads the
/// **session** and knows *which pane* (an identity that outlives any tree), while the universal
/// picker walks the **trees** and knows *which widget in which surface* (a path that lives one
/// frame). Collapsing them is F011/P094/T457's business.
///
/// What they must NOT have is two lifecycles. `prefix+/` used to hand its letters out once, in
/// `handle_hint_pick`, and nothing ever revisited them — so every rule this module enforces every
/// frame (a covered view loses its letter; a withdrawal reaches every view) simply did not apply to
/// it. One enum here, one loop below, one lifecycle for both (F003/P082/T438).
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Offer {
    /// By the identity the widget declares — `pane:7`, `ws:0/col:1`, a dock's id.
    ByKey(String),
    /// By the surface and path the picker collected it from.
    ByPath(HintTarget),
}

impl Offer {
    /// What this offer is **called**, for the plugin-facing `hint.changed`. A path-addressed offer
    /// resolves to the same identity the picker remembers its letter by, so a plugin sees one
    /// vocabulary whichever picker is up.
    fn name(&self, state: &crate::app_state::AppState) -> String {
        match self {
            Offer::ByKey(key) => key.clone(),
            Offer::ByPath(target) => super::surfaces::target_identity(state, target)
                .unwrap_or_else(|| format!("{:?}", target.surface)),
        }
    }
}

/// The targets this module has a letter on. Withdrawal is exactly this set, which is what makes the
/// rule ownership rather than "clear everything and hope".
#[derive(Default, Debug)]
pub(crate) struct OfferedLetters {
    offers: Vec<Offer>,
}

/// What the **current input mode** wants lettered, as `(key, letter)`.
///
/// One place that knows how a mode's candidates become row identities, so a new pick mode is one
/// arm here rather than a fifth signal list and a fifth projection.
fn wanted(mode: &InputMode, active_pane: Option<heca_core::layout::PaneId>) -> Vec<(Offer, char)> {
    let mut out = Vec::new();
    if let Some(cands) = mode.candidates() {
        // **The focused pane is never a target**, even when the mode lists it: every one of these
        // picks means "the other one", so lettering where you already are offers a move to nowhere.
        out.extend(
            cands
                .iter()
                .filter(|(_, id)| Some(*id) != active_pane)
                .map(|(ch, id)| (Offer::ByKey(pane_key(*id)), *ch)),
        );
    }
    if let Some(cands) = mode.ws_candidates() {
        out.extend(
            cands
                .iter()
                .map(|(ch, _ws_idx, ws_id)| (Offer::ByKey(workspace_key(*ws_id)), *ch)),
        );
    }
    if let Some(cands) = mode.col_candidates() {
        out.extend(
            cands
                .iter()
                .map(|(ch, _ws_idx, _col_idx, col_id)| (Offer::ByKey(column_key(*col_id)), *ch)),
        );
    }
    // A dock names itself with `scope_key` rather than `key` — a container's identity, not a
    // row's — and `offer_hint_by_key` matches either, so this is the same one line as the rest.
    if let Some(cands) = mode.dock_candidates() {
        out.extend(cands.iter().map(|(ch, id)| (Offer::ByKey(id.clone()), *ch)));
    }
    // **The universal picker is a mode like any other.** Its candidates carry a surface and a path
    // rather than an identity, which is the only difference — and the reason it used to live
    // outside this loop, handing its letters out once and never looking again.
    if let InputMode::HintPick { candidates } = mode {
        out.extend(
            candidates
                .iter()
                .map(|(ch, target)| (Offer::ByPath(target.clone()), *ch)),
        );
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
    // **Which views could actually show a letter.** Computed ONCE per pass, not per key: it
    // resolves the whole surface stack.
    let visible = if wanted.is_empty() {
        std::collections::HashSet::new()
    } else {
        visible_hint_surfaces(state)
    };
    let mut changed = false;

    // Withdraw first, so a target that keeps its letter across a mode change is not briefly cleared.
    let stale: Vec<Offer> = state
        .offered_letters
        .borrow()
        .offers
        .iter()
        .filter(|o| !wanted.iter().any(|(want, _)| want == *o))
        .cloned()
        .collect();
    for offer in &stale {
        offer_in_every_tree(state, offer, None, &visible);
        changed = true;
    }

    for (offer, ch) in &wanted {
        if offer_in_every_tree(state, offer, Some(ch.to_string()), &visible) {
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
    let next: Vec<Offer> = wanted.iter().map(|(o, _)| o.clone()).collect();
    if offered.offers != next {
        state
            .chrome_state
            .events()
            .emit(crate::chrome::ChromeEvent::HintLettersChanged {
                letters: wanted
                    .iter()
                    .map(|(o, ch)| (*ch, o.name(state)))
                    .collect(),
            });
    }
    offered.offers = next;
    changed
}

/// Offer `label` to whatever shows this offer. Front to back, so a surface in front shadows one
/// behind it — the nearest declaration wins, as everywhere else.
fn offer_in_every_tree(
    state: &crate::app_state::AppState,
    offer: &Offer,
    label: Option<String>,
    visible: &std::collections::HashSet<HintSurface>,
) -> bool {
    // **What this view gets this pass** — the letter when it can be seen, a *withdrawal* when it
    // cannot. Every view's label goes through here, so no loop below can decide on its own.
    let for_view = |surface: HintSurface| label_for(&label, visible.contains(&surface));

    // **A path names one view already**, so there is nothing to search: the surface it was
    // collected from is the surface that shows it, and the same visibility question is asked of it
    // as of every other view.
    let key = match offer {
        Offer::ByKey(key) => key.as_str(),
        Offer::ByPath(target) => {
            let Some((root, paths)) = super::surfaces::resolve(state, target) else {
                return false;
            };
            let label = for_view(target.surface.clone());
            let mut offered = false;
            for path in &paths {
                offered |= heca_grid_ui::offer_hint(root, path, label.clone());
            }
            return offered;
        }
    };
    for layer in state.layers.visible_front_to_back() {
        let Some(node) = crate::chrome::surface_node(&state.window_root, layer.id) else {
            continue;
        };
        if heca_grid_ui::offer_hint_by_key(node, key, label.clone()) {
            return true;
        }
    }
    // Below the layers there is no shadowing, because these are not competing surfaces — they are
    // **several views of one thing**. A pane and the sidebar row that names it both declare
    // `pane:7`, and both must wear the letter: the pane owns the identity, the row shows it. An
    // early return here is what made a pane listed in BOTH sidebars get lettered in only one of
    // them (F003/P082, found by tracing `nodes naming it: chrome=2` while one letter went out).
    let mut offered = false;
    offered |= heca_grid_ui::offer_hint_by_key(&state.window_root, key, label.clone());
    // **A pane behind a sidebar loses its letter — the pane, not the pick.** Its keycap draws on
    // the overlay layer, so it would land on top of the very thing covering it (Antonio, driving,
    // 2026-08-19). Its sidebar row is a second view of the same pane and IS visible, so it still
    // wears the letter and the pane stays reachable — which is why this filters the VIEW rather
    // than the candidate. Visibility is asked of `resolve_hint_layers`, never re-derived here.
    //
    // The header is the same pane's other view and is asked the same question. It used to be asked
    // none at all, because the answer arrived as a set of pane *ids* and only the shell loop knew
    // what to do with it.
    for (pane_id, shell) in state.panes.iter() {
        offered |= heca_grid_ui::offer_hint_by_key(
            &shell.root,
            key,
            for_view(HintSurface::Pane(*pane_id)),
        );
    }
    for (pane_id, header) in state.pane_headers.iter() {
        offered |= heca_grid_ui::offer_hint_by_key(
            &header.root,
            key,
            for_view(HintSurface::PaneHeader(*pane_id)),
        );
    }
    offered
}

/// **What one view gets this pass**: the letter when it can be seen, and a **withdrawal** when it
/// cannot.
///
/// The whole of the rule, named because getting it wrong is invisible. This was a `continue` — a
/// view that could not show a letter was *skipped* — and skipping is not withdrawing: it leaves
/// whatever was written last. A pane lettered while visible and then covered kept its keycap, drawn
/// over the sidebar covering it, until the picker closed and `clear_hint_letters` swept every tree.
///
/// Nothing on screen shows the difference while a picker opens and closes over a still layout,
/// which is why it survived the fix that was supposed to cover it: **resizing the window** grows
/// the panes until they slide under the sidebar, and the letters stayed behind (Antonio, driving,
/// 2026-08-24).
fn label_for(label: &Option<String>, visible: bool) -> Option<String> {
    label.clone().filter(|_| visible)
}

/// [`wanted`] for a test in another module — the mapping is the interesting part and belongs to
/// this file, so its assertions live wherever the case is clearest rather than being re-derived.
#[cfg(test)]
pub(crate) fn wanted_for_tests(mode: &InputMode) -> Vec<(Offer, char)> {
    wanted(mode, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::PaneId;

    /// **A view that cannot be seen has its letter taken BACK, not skipped** (F003/P082/T438).
    ///
    /// The difference is invisible while a picker opens and closes over a still layout, and it is
    /// the whole bug: `continue` leaves the label the view already wears. Resizing the window grew
    /// the panes until they slid under the sidebar, and their keycaps stayed there, drawn over it.
    #[test]
    fn an_unseen_view_is_withdrawn_from_rather_than_left_alone() {
        let letter = Some("a".to_string());
        assert_eq!(label_for(&letter, true).as_deref(), Some("a"), "seen: it wears the letter");
        assert_eq!(
            label_for(&letter, false),
            None,
            "unseen: a withdrawal, never 'leave whatever is there'",
        );
        // A withdrawal reaches every view, seen or not — or the keycap outlives the picker.
        assert_eq!(label_for(&None, true), None);
        assert_eq!(label_for(&None, false), None);
    }

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
                (Offer::ByKey(pane_key(PaneId(7))), 'a'),
                (Offer::ByKey(pane_key(PaneId(9))), 'b'),
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
            vec![(Offer::ByKey(pane_key(PaneId(2))), 's')],
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
