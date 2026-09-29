//! The pick letters, and every action that asks the user to pick a target by letter.

use crate::app_state::{AppState, InputMode, WorkspacePickTarget};
use crate::collect_all_pane_candidates;
use crate::input::WmAction;
use heca_core::layout::PaneId;

/// **Every pane, each keeping the letter it had last time** — the stable half of a pane pick.
///
/// `collect_all_pane_candidates` orders the panes and applies the 52 cap; the letters it hands out
/// are its *index*, which is the defect: anything appearing earlier in the list shifts every letter
/// after it, so splitting a column renumbers panes you were aiming at. Antonio, driving,
/// 2026-08-17: *"i want to expand a pane, prefix+/ and `k` appears on that icon... Then I want to
/// collapse. prefix+/ and `j` appears on that button, while I was expecting `k`."*
///
/// So the order comes from there and the LETTERS come from [`assign_letters`], keyed by the pane's
/// own identity — the same function and the same remembered map `prefix+/` uses. One assignment
/// rule for both pickers, which is the whole point of F011/P094/T451.
pub(super) fn pane_candidates_with_stable_letters(state: &mut AppState) -> Vec<(char, PaneId)> {
    let panes: Vec<PaneId> = collect_all_pane_candidates(&state.session, state.focused_pane)
        .into_iter()
        .map(|(_, id)| id)
        .collect();
    let identities: Vec<Option<String>> = panes
        .iter()
        .map(|id| Some(crate::chrome::pane_key(*id)))
        .collect();
    let letters = assign_letters(&identities, &state.remembered_letters);
    // Merged, not rebuilt: a pane that is off screen keeps its letter for when it comes back.
    state.remembered_letters.extend(
        identities
            .iter()
            .zip(letters.iter())
            .filter_map(|(id, ch)| Some((id.clone()?, (*ch)?))),
    );
    panes
        .into_iter()
        .zip(letters)
        .filter_map(|(id, ch)| ch.map(|ch| (ch, id)))
        .collect()
}

/// **Start a pick, or say why it cannot start.**
///
/// The one door every letter pick goes through. Each handler used to write `if
/// !candidates.is_empty() { state.input_mode = ..; state.needs_redraw = true; }` for itself — ten
/// copies of one rule — and the empty case simply fell off the end. `prefix+g` with a single
/// workspace open did nothing at all: no prompt, no message, no flash, which is indistinguishable
/// from an unbound key and is exactly how a working feature gets reported as broken.
///
/// There is no branch here for a caller to leave out. A handler hands over the mode it wants and
/// this decides; a pick that has nothing to offer says so, and a plugin's pick gets the same
/// sentence heca's does with nothing written.
///
/// The words come from the [`ActionCatalog`](crate::actions::ActionCatalog), which is the single
/// source of truth for what an action is called, plus the pick's own
/// [`subject`](crate::app_state::PickKind::subject) — the one thing the catalog cannot know,
/// because it describes what an action does rather than what it picks among.
pub(crate) fn begin_pick(state: &mut AppState, mode: InputMode) {
    let empty = mode.pick_candidate_count() == Some(0);
    if !empty {
        state.input_mode = mode;
        return;
    }
    let Some(pending) = mode.pending_pick(&state.action_catalog) else {
        return;
    };
    state.status_note = Some(pick_refusal(&pending));
}

/// **What a pick with nothing to offer says.**
///
/// Split out because the app needs a window, so `begin_pick` cannot be called in a test and this is
/// the half worth asserting. It names the act in the action's own words — the ones the command
/// palette and the tooltip already use — and then what there was none of.
///
/// It goes in the bottom bar, which is where that pick's *prompt* would have appeared, so the
/// answer lands where the question would have. A toast was tried first and is too much for it: a
/// key that cannot do its thing is not an event worth covering the work.
/// **What a key that could not act says** — the act in the action's own words, then why.
///
/// The sibling of [`pick_refusal`], for an action that is not a pick. Both exist for one reason: a
/// key that cannot do its thing is a reply to what you pressed, and saying nothing is
/// indistinguishable from a keypress that never registered (Antonio, 2026-09-10, on
/// `move_pane_to_new_column` doing nothing for a pane already alone in its column).
///
/// The label comes from the catalog, so the bar names the act exactly as the command palette and
/// its tooltip do — one vocabulary, never a sentence written at the call site.
pub(crate) fn act_refusal(
    catalog: &crate::actions::ActionCatalog,
    action_name: &str,
    because: &str,
) -> String {
    match catalog.describe(action_name) {
        Some(d) => format!("{} — {because}", d.label),
        None => format!("{action_name} — {because}"),
    }
}

pub(crate) fn pick_refusal(pending: &crate::app_state::PendingPick) -> String {
    format!(
        "{} — there is no other {} to pick",
        pending.label,
        pending.kind.subject()
    )
}

/// Enter the universal picker (`prefix+/`): assign a letter to every region on screen that said
/// what a pick does to it (document order) and show a keycap over each; the next keypress runs that
/// region's declaration. Says so when nothing on screen declares one.
/// **Hand out the letters, giving each target back the one it had last time** (F003/P082/T445).
///
/// `identities` is one entry per target, in document order — `None` for a target with no identity to
/// remember it by. `remembered` is what each identity wore when the picker last opened.
///
/// Two passes, and the order is the whole point:
///
/// 1. every target that had a letter and is still here **keeps it**;
/// 2. the rest fill the gaps from the shared alphabet, in order.
///
/// Without the first pass the letter is simply the target's index, so anything appearing earlier in
/// the tree shifts every letter after it — expand a pane and the button you were aiming at moves
/// from `k` to `j`.
///
/// A pure function of plain data so the rule is testable without a window (the shape
/// `surface_action` and `dock_focus_outcome` use). Returns one letter per target, `None` past the
/// end of the alphabet — 52 targets, one keystroke each, never a longer one.
pub(crate) fn assign_letters(
    identities: &[Option<String>],
    remembered: &std::collections::HashMap<String, char>,
) -> Vec<Option<char>> {
    use std::collections::{HashMap, HashSet};
    let mut taken: HashSet<char> = HashSet::new();
    // **One letter per NAME, not per target.** A pane in the scrolling area and its sidebar row are
    // two views of one pane, so they answer to one name and must wear one letter — two would ask
    // you to pick which picture of the same thing you meant, and would burn the alphabet twice as
    // fast, which is what forces uppercase.
    let mut chosen: HashMap<&str, char> = HashMap::new();

    // 1. Every name that had a letter and is still here keeps it.
    for id in identities.iter().flatten() {
        if chosen.contains_key(id.as_str()) {
            continue;
        }
        if let Some(&ch) = remembered.get(id)
            && taken.insert(ch)
        {
            chosen.insert(id, ch);
        }
    }

    // 2. The gaps, in order. A name new since last time takes the first letter nobody kept, so
    //    adding one costs one letter rather than renaming everything after it.
    let mut free = heca_grid_ui::widgets::DEFAULT_LETTERS
        .chars()
        .filter(|c| !taken.contains(c));
    let mut out: Vec<Option<char>> = vec![None; identities.len()];
    for (i, id) in identities.iter().enumerate() {
        out[i] = match id {
            Some(id) => match chosen.get(id.as_str()) {
                Some(&ch) => Some(ch),
                None => {
                    let ch = free.next();
                    if let Some(ch) = ch {
                        chosen.insert(id, ch);
                    }
                    ch
                }
            },
            // A target with no name at all cannot be the same thing as any other, so it takes a
            // letter of its own.
            None => free.next(),
        };
    }
    out
}

pub fn handle_hint_pick(state: &mut AppState, _action: &WmAction) {
    // Which targets are reachable is decided by the layered surface compositor — one rule
    // (active context + geometric occlusion, no hardcoded z) over the whole surface stack.
    // See `chrome::active_hint_targets` and `docs/surface-compositor.md`.
    let targets: Vec<crate::chrome::HintTarget> = crate::chrome::active_hint_targets(state)
        .into_iter()
        .map(|(target, _)| target)
        .collect();
    // **What each target is called**, so it can be given the letter it had last time (T445). The
    // path a target is addressed by lives one frame; the identity outlives the tree.
    let identities: Vec<Option<String>> = targets
        .iter()
        .map(|t| crate::chrome::target_identity(state, t))
        .collect();
    let letters = assign_letters(&identities, &state.remembered_letters);

    let candidates: Vec<(char, crate::chrome::HintTarget)> = targets
        .into_iter()
        .zip(letters.iter())
        .filter_map(|(target, ch)| ch.map(|ch| (ch, target)))
        .collect();

    // **Merged, not rebuilt.** A target that is off screen keeps its letter for when it comes back:
    // zoom in far enough that only one pane is visible and the rest stop being targets
    // (`chrome::hint` counts a target only if it lies in the viewport), so rebuilding from what is
    // on screen made every pane take a fresh letter on the way back (Antonio, driving, 2026-08-18).
    //
    // A remembered entry for an absent target blocks nothing: the "already taken" set holds only the
    // letters handed out *this* round, so a present target always wins the letter it asks for.
    state.remembered_letters.extend(
        identities
            .iter()
            .zip(letters.iter())
            .filter_map(|(id, ch)| Some((id.clone()?, (*ch)?))),
    );

    // **Entering the mode is the whole of it.** The letters are handed out by
    // `sync_offered_letters`, every frame, exactly as every other pick mode's are — so the rules
    // that live there apply here too: a view that becomes covered loses its letter, and a
    // withdrawal reaches every view. Handing them out once from here is what left `prefix+/`
    // outside all of it.
    begin_pick(state, InputMode::HintPick { candidates });
}

/// Enter the "move active column → workspace" letter pick: assign a letter to each
/// workspace (shown as a `KeyHint` over its dock); the next keypress moves the active
/// column into that workspace. Says so when there is no other workspace.
pub fn handle_move_column_to_workspace_pick(state: &mut AppState, _action: &WmAction) {
    let ws_idx = state.session.active_workspace_idx;
    let col_idx = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.active_column_idx)
        .unwrap_or(0);
    let candidates = crate::app::selection::collect_workspace_candidates(&state.session);
    begin_pick(
        state,
        InputMode::WorkspacePick {
            candidates,
            target: WorkspacePickTarget::Column { ws_idx, col_idx },
        },
    );
}

/// Enter the "move active pane → workspace" letter pick (see
/// [`handle_move_column_to_workspace_pick`]). Does nothing without a focused pane.
pub fn handle_move_pane_to_workspace_pick(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    let candidates = crate::app::selection::collect_workspace_candidates(&state.session);
    begin_pick(
        state,
        InputMode::WorkspacePick {
            candidates,
            target: WorkspacePickTarget::Pane(pane_id),
        },
    );
}

pub fn handle_move_pane_to_column_pick(state: &mut AppState, _action: &WmAction) {
    let Some(pane_id) = state.focused_pane else {
        return;
    };
    // The column this pane is already in is not a destination — moving it there does nothing.
    let here = crate::app::selection::column_of_pane(&state.session, pane_id);
    // **A new column is offered only when it would change something.** A pane alone in its column
    // moved into a fresh one leaves the strip exactly as it was, so it is not offered rather than
    // refused after the letter is pressed — the rule Part B already settled.
    let offer_new = crate::app::selection::column_of_pane_has_siblings(&state.session, pane_id);
    let candidates =
        crate::app::selection::collect_column_candidates(&state.session, here, offer_new);
    begin_pick(
        state,
        InputMode::ColumnPick {
            candidates,
            pane_id,
        },
    );
}

#[cfg(test)]
mod act_refusal_tests;

#[cfg(test)]
mod pick_refusal_tests;

#[cfg(test)]
mod letter_memory_tests;
