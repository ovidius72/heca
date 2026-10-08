use super::*;

#[test]
fn test_input_mode_candidates_none() {
    assert_eq!(InputMode::Normal.candidates(), None);
    assert_eq!(InputMode::Prefix.candidates(), None);
}

#[test]
fn test_input_mode_candidates_some() {
    let cands = vec![('a', PaneId(1)), ('b', PaneId(2))];
    assert_eq!(
        InputMode::PaneSelect {
            candidates: cands.clone()
        }
        .candidates(),
        Some(cands.as_slice())
    );
    assert_eq!(
        InputMode::PaneSwap {
            candidates: cands.clone(),
            focus_after: false
        }
        .candidates(),
        Some(cands.as_slice())
    );
}

/// **An empty pick is a pick, and must be recognised as one.** This is what decides whether a
/// refusal is reported or the key looks unbound: a pick answering `None` here would be treated
/// as "not a pick" and go back to failing silently.
#[test]
fn a_pick_with_nothing_to_offer_still_counts_as_a_pick() {
    assert_eq!(
        InputMode::PaneSelect {
            candidates: Vec::new()
        }
        .pick_candidate_count(),
        Some(0),
    );
    assert_eq!(
        InputMode::WorkspacePick {
            candidates: Vec::new(),
            target: WorkspacePickTarget::Pane(PaneId(1)),
        }
        .pick_candidate_count(),
        Some(0),
    );
    assert_eq!(
        InputMode::PaneSelect {
            candidates: vec![('a', PaneId(1))]
        }
        .pick_candidate_count(),
        Some(1),
    );
}

/// A mode that is not a pick has no count, so nothing tries to refuse it.
#[test]
fn a_mode_that_is_not_a_pick_has_no_count() {
    assert_eq!(InputMode::Normal.pick_candidate_count(), None);
    assert_eq!(
        InputMode::Search {
            terminal: crate::chrome::terminal::TerminalId(1)
        }
        .pick_candidate_count(),
        None
    );
}

/// **Every pick names what it picks among**, so a refusal reads as a sentence rather than a
/// generic "nothing found". The words are the user's, not the code's.
#[test]
fn every_pick_says_what_it_offers() {
    assert_eq!(PickKind::MovePaneToWorkspace.subject(), "workspace");
    assert_eq!(PickKind::MovePaneToColumn.subject(), "column");
    assert_eq!(PickKind::SelectPane.subject(), "pane");
    assert_eq!(PickKind::FocusDock.subject(), "dock");
}

#[test]
fn test_sidebar_item_state_eq() {
    assert_eq!(SidebarItemState::Active, SidebarItemState::Active);
    assert_ne!(SidebarItemState::Active, SidebarItemState::Visited);
}

#[test]
fn test_rename_target_copy() {
    let t = RenameTarget::Workspace(3);
    let copied = t; // RenameTarget is Copy — `t` stays usable below.
    assert_eq!(t, copied);
}

/// **The rename dialog submits the catalogued action**, so a rename from the dialog, a key, RPC or
/// a plugin is one route. Every target names an action the registry knows.
#[test]
fn the_rename_dialog_submits_the_action_that_names_the_target() {
    use crate::input::WmAction;
    assert_eq!(
        RenameTarget::Pane(PaneId(4)).rename_to("x".into()),
        WmAction::RenameTarget { pane_id: PaneId(4), name: "x".into() }
    );
    assert_eq!(
        RenameTarget::Workspace(2).rename_to("x".into()),
        WmAction::RenameWorkspaceTo { ws_idx: 2, name: "x".into() }
    );
    assert_eq!(
        RenameTarget::Column { ws_idx: 1, col_idx: 3 }.rename_to("x".into()),
        WmAction::RenameColumnTo { ws_idx: 1, col_idx: 3, name: "x".into() }
    );
}
