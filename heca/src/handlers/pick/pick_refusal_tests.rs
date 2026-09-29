use crate::actions::ActionCatalog;
use crate::app_state::InputMode;

/// **A refused pick says which act it was and what there was none of.**
///
/// The words come from the action's own catalog entry, so the bar names the act exactly as the
/// command palette and its tooltip do — one vocabulary, not a sentence written here.
#[test]
fn a_refused_pick_names_the_act_and_what_was_missing() {
    let catalog = ActionCatalog::with_builtins();
    let mode = InputMode::WorkspacePick {
        candidates: Vec::new(),
        target: crate::app_state::WorkspacePickTarget::Pane(heca_core::layout::PaneId(1)),
    };
    let pending = mode
        .pending_pick(&catalog)
        .expect("a workspace pick is a pick");
    let note = super::pick_refusal(&pending);

    assert!(
        note.contains("workspace"),
        "it must say what there was none of, not just that something failed: {note}",
    );
    assert!(
        note.contains(&pending.label),
        "and name the act in the catalog's words: {note}",
    );
}

/// The pane picks say "pane", so the sentence is about what you were choosing among rather than
/// a generic "nothing found".
#[test]
fn a_pane_pick_says_pane() {
    let catalog = ActionCatalog::with_builtins();
    let mode = InputMode::PaneSelect {
        candidates: Vec::new(),
    };
    let pending = mode.pending_pick(&catalog).expect("a pane pick is a pick");
    assert!(super::pick_refusal(&pending).contains("pane"));
}
