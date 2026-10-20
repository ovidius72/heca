//! Pure action rows, argument choices and focused-first ordering.

use super::*;

/// Build the palette's rows from the catalog — **pure**, so the ordering and labelling rules are
/// testable without a window.
///
/// `owners` maps an action name to the placement that would receive it; an action absent from the
/// map is the app's own. The order is: the focused component's actions first, then everything else
/// in the catalog's stable order. Nothing is hidden — an action whose component is mounted but not
/// focused is listed exactly like the rest, because choosing it focuses that component first.
pub(crate) fn entries(
    catalog: &crate::actions::ActionCatalog,
    owners: &std::collections::HashMap<String, OwnerInfo>,
    shortcuts: &ActionShortcuts,
) -> Vec<PaletteEntry> {
    let mut rows: Vec<(bool, PaletteEntry)> = catalog
        .all()
        .flat_map(|meta| offers(meta).into_iter().map(move |choice| (meta, choice)))
        .map(|(meta, choice)| {
            let owner = owners.get(&meta.name);
            let base = match &choice {
                Some((_, value)) => format!("{} › {value}", meta.label),
                None => meta.label.clone(),
            };
            let label = match owner {
                Some(o) => format!("{} › {base}", o.title),
                None => base,
            };
            let mut action = Intent::new(&meta.name);
            if let Some((arg, value)) = &choice {
                action = action.arg(arg.clone(), PropValue::Text(value.clone()));
            }
            let intent = match owner {
                Some(o) => InteractionIntent::FocusContainerThenAction {
                    container: o.mount.clone(),
                    action,
                },
                None => InteractionIntent::View(action),
            };
            // A name alone, or the name and the one word chosen — unique, and the same tomorrow.
            let id = match &choice {
                Some((arg, value)) => format!("{} {arg}={value}", meta.name),
                None => meta.name.clone(),
            };
            (
                owner.is_some_and(|o| o.focused),
                PaletteEntry {
                    id: id.clone(),
                    rank_id: Some(id),
                    pane: None,
                    current: false,
                    preselect: false,
                    label,
                    description: meta.description.clone(),
                    // Through the catalog, not off the meta: the catalog answers with the action's
                    // own glyph *or its category's*, so no row is ever blank and the palette makes
                    // no decision of its own about iconography.
                    icon: catalog.icon(&meta.name),
                    shortcuts: shortcuts.chords(&meta.name).to_vec(),
                    intent,
                },
            )
        })
        .collect();
    // Stable: within each group the catalog's own order survives.
    rows.sort_by_key(|(focused, _)| !*focused);
    rows.into_iter().map(|(_, entry)| entry).collect()
}

/// The focused component's actions lead while the query is empty.
///
/// The focused-first rule exists so that focusing a component puts *its* verbs at hand, and
/// with nothing typed that focus is the only context there is. Once something is typed the query is
/// better context and the widget drops the blocks entirely — an action whose name the user spelled
/// correctly must not sit below one they did not, merely because its component holds focus.
pub(super) fn group_of(entry: &PaletteEntry, owners: &std::collections::HashMap<String, OwnerInfo>) -> u16 {
    match owners.get(&entry.id) {
        Some(o) if o.focused => 0,
        _ => 1,
    }
}

/// The ways a palette can run this action — **one row each**. A palette offers a name and nothing
/// else, so what it can run is decided by what the action requires:
///
/// - **Nothing required** → one row, run bare (`None`).
/// - **Exactly one required argument, and it is a fixed word list** → one row per word, the word
///   supplied (`Some((arg, word))`) — `set_region_visible` is offered once per region. Aliases of
///   one word (`sidebar.left`, `left-sidebar`, `left`) would be the same command three times, so a
///   word is kept only when it builds an action no earlier word built; the parser is the judge, and
///   there is no second list of "canonical" spellings to keep in step.
/// - **Anything else** → no row. Thirty-odd built-ins take a target (`close_pane_by_id`,
///   `move_column`, `float_at`, …); each is the targeted form of an act whose bare form is already
///   listed, and exists for the surfaces that *do* carry a target — a header button, a context-menu
///   entry, a drag, an RPC line. Listing one would look like a working command and do nothing.
///
/// This is arity, **not policy**: what the current domain permits is judged at dispatch, and what an
/// action takes is fixed at its declaration — so the rule reads the `args` every action is already
/// required to declare, and no hand-written list can drift from it.
pub(super) fn offers(meta: &crate::actions::ActionMeta) -> Vec<Option<(String, String)>> {
    let mut required = meta.args.iter().filter(|a| a.required);
    let (Some(only), None) = (required.next(), required.next()) else {
        return if meta.args.iter().any(|a| a.required) {
            Vec::new()
        } else {
            vec![None]
        };
    };
    if only.kind != crate::args::ArgKind::Enum {
        return Vec::new();
    }
    let mut built: Vec<crate::input::WmAction> = Vec::new();
    only.values
        .iter()
        .filter(|value| {
            let args = std::collections::HashMap::from([(only.name.clone(), (*value).clone())]);
            // A name-keyed (component or plugin) action has no `WmAction` to compare; its words are
            // its own and are all offered.
            match crate::input::resolve_action(&meta.name, &args) {
                Some(action) if built.contains(&action) => false,
                Some(action) => {
                    built.push(action);
                    true
                }
                None => true,
            }
        })
        .map(|value| Some((only.name.clone(), value.clone())))
        .collect()
}
