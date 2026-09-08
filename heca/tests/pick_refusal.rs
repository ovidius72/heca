//! **A pick that has nothing to offer must say so.**
//!
//! `prefix+g` moves the focused pane to another workspace. With a single workspace open there is
//! nowhere to move it, and the handler returned without entering the mode: no prompt, no message,
//! no flash. That is indistinguishable from an unbound key, which is exactly how a working feature
//! gets reported as broken (Antonio, driving, 2026-08-24).
//!
//! It was never one handler's bug. Ten handlers each wrote `if !candidates.is_empty() { .. }` for
//! themselves, and the empty case fell off the end of every one of them. Ten copies of a rule means
//! the rule is missing, so there is now one door — `begin_pick` — with no branch a caller can leave
//! out.
//!
//! A **lint**, because what it guards cannot be unit-tested: every pick handler takes
//! `&mut AppState`, which needs a window, so there is no headless call to make. Same reason
//! `by_id_actions.rs` is written this way.

use std::path::{Path, PathBuf};

fn handlers_rs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers.rs")
}

/// Every pick enters through the one door, so a refusal cannot be forgotten.
///
/// The tell that the class is back is a handler assigning a pick mode to `input_mode` itself: that
/// is the shape that had no room for the empty case. Assigning a *non-pick* mode (`Normal`,
/// `Prefix`, a custom mode) is a different act and stays a plain assignment.
#[test]
fn a_pick_mode_is_only_ever_entered_through_begin_pick() {
    let src = std::fs::read_to_string(handlers_rs()).expect("read the handlers");
    let picks = [
        "PaneSelect",
        "PaneSwap",
        "PaneTake",
        "WorkspacePick",
        "ColumnPick",
        "DockPick",
        "FollowLink",
        "HintPick",
    ];
    let offenders: Vec<&str> = picks
        .into_iter()
        .filter(|pick| src.contains(&format!("input_mode = InputMode::{pick}")))
        .collect();

    assert!(
        offenders.is_empty(),
        "these picks are entered by assigning `input_mode` directly: {offenders:?}\n\
         That is the shape with nowhere to put the empty case — the handler simply does nothing, \
         and the key looks unbound. Hand the mode to `begin_pick(state, ..)` instead; it enters the \
         pick when there is something to pick and says so when there is not.",
    );
}

/// The door is still there, and still the thing that decides.
#[test]
fn the_one_door_still_reports_a_refused_pick() {
    let src = std::fs::read_to_string(handlers_rs()).expect("read the handlers");
    assert!(
        src.contains("fn begin_pick("),
        "`begin_pick` is gone — if it moved, move this guard with it rather than deleting it",
    );
    assert!(
        src.contains("state.status_note = Some(pick_refusal(&pending))"),
        "`begin_pick` no longer reports a refused pick, which is the whole of what it is for",
    );
}
