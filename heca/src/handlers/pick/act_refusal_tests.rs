use crate::actions::ActionCatalog;

/// **A key that could not act names the act and why**.
///
/// The words come from the action's own catalog entry, so the bar names it exactly as the
/// command palette and its tooltip do — one vocabulary, not a sentence written at the call site.
#[test]
fn a_refused_act_names_itself_in_the_catalogs_words() {
    let catalog = ActionCatalog::with_builtins();
    let note = super::act_refusal(
        &catalog,
        "move_pane_to_new_column",
        "it is already the only pane in its column",
    );
    assert_eq!(
        note,
        "Move Pane to New Column — it is already the only pane in its column",
    );
}

/// An action the catalog does not know still says something, rather than an empty bar.
#[test]
fn an_unknown_action_still_answers() {
    let catalog = ActionCatalog::with_builtins();
    let note = super::act_refusal(&catalog, "plugin.something", "there is nowhere to put it");
    assert_eq!(note, "plugin.something — there is nowhere to put it");
}
