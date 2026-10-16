//! The cursor and the verbs that act on the row under it.

use super::*;

#[test]
fn it_declares_only_what_acts_on_its_own_cursor() {
    let declared: Vec<String> = WorkspacesContainerProvider::new("workspaces")
        .actions()
        .into_iter()
        .map(|m| m.name)
        .collect();

    for id in [
        CURSOR_UP,
        CURSOR_DOWN,
        COLLAPSE_ROW,
        ACTIVATE_SELECTED,
        PEEK_SELECTED,
        TOGGLE_SELECTED,
    ] {
        assert!(declared.contains(&id.to_string()), "{id} is declared");
    }
    assert!(
        declared.iter().all(|n| n.starts_with("workspaces.")),
        "every id is namespaced by kind(), so two components can both have a cursor: {declared:?}",
    );
    // The point of the contract: it redeclares nothing that already exists.
    for existing in ["zoom_column", "close", "next_pane", "create_workspace"] {
        assert!(
            !declared.iter().any(|n| n == existing),
            "{existing} already exists — bind it, do not redeclare it",
        );
    }
}

// ── What `perform` does with it ──

#[test]
fn the_cursor_moves_over_the_components_own_rows() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());

    let first = store.workspaces.tree().cursor;
    assert_eq!(
        p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx),
        Handled::Yes,
    );
    assert_ne!(store.workspaces.tree().cursor, first, "the cursor moved");

    p.perform(CURSOR_UP, &Intent::new(CURSOR_UP), &mut cx);
    assert_eq!(store.workspaces.tree().cursor, first, "and moved back");
}

/// Moving the cursor publishes it, so the row outlines follow with no rebuild — and it lands on
/// **this mount's** cursor, not a global one.
#[test]
fn moving_the_cursor_publishes_it_for_this_mount() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());

    p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);

    assert!(cx.selected().is_some(), "the mount's cursor names a row");
    assert_eq!(
        ProviderCx::new("workspaces.right", store).selected(),
        None,
        "the other placement of the same component has its own cursor",
    );
}

/// Activating a leaf asks the host to focus the pane **and** to hand the keyboard back; hinting
/// asks only for the focus. Both go out as queued intents — a component never mutates app state.
#[test]
fn activating_a_row_asks_the_host_rather_than_acting() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");

    let mut cx = ProviderCx::new("workspaces", store.clone());
    p.perform(ACTIVATE_SELECTED, &Intent::new(ACTIVATE_SELECTED), &mut cx);
    let asked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
    assert!(
        asked.contains(&"focus_pane".to_string()) || asked.contains(&"focus_workspace".to_string())
    );

    let mut cx = ProviderCx::new("workspaces", store);
    p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
    let asked: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
    assert!(
        !asked.contains(&"unfocus_dock".to_string()),
        "hint keeps the keyboard on the dock: {asked:?}",
    );
}

/// **The cursor and the click are the same thing**: a click writes the placement's cursor key —
/// the one record — so the next `k` continues from the clicked row rather than from wherever the
/// list stood before.
#[test]
fn a_click_moves_the_cursor_so_stepping_continues_from_it() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());

    // The fixture's rows are the workspace and its one pane; the click lands on the pane.
    cx.set_selected(Some(pane_key(PaneId(1))));
    p.perform(CURSOR_UP, &Intent::new(CURSOR_UP), &mut cx);

    assert_eq!(
        cx.selected().as_deref(),
        Some(super::selection_key(store.workspaces.tree().selection().expect("a row")).as_str()),
    );
    assert!(
        matches!(
            store.workspaces.tree().current_item(),
            Some(WorkspaceRow::Workspace { .. }),
        ),
        "`k` went to the row above the one clicked",
    );
}

/// **Each placement acts on its own cursor.** Another placement of the same component leaving the
/// list on a different row does not change what this one acts on: its key is loaded first.
#[test]
fn a_placement_acts_on_its_own_row_not_where_another_left_the_list() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");

    // The right-hand placement was last on the pane, so that is where the shared list stands.
    let mut right = ProviderCx::new("workspaces.right", store.clone());
    right.set_selected(Some(pane_key(PaneId(1))));
    p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut right);

    // The left one is on the workspace row; activating it must act on that row.
    let workspace_key = super::selection_key(store.workspaces.tree().flat_items[0].selection());
    let mut left = ProviderCx::new("workspaces", store.clone());
    left.set_selected(Some(workspace_key.clone()));
    p.perform(
        ACTIVATE_SELECTED,
        &Intent::new(ACTIVATE_SELECTED),
        &mut left,
    );

    let asked: Vec<String> = left.drain().into_iter().map(|i| i.action).collect();
    assert_eq!(
        asked,
        ["focus_workspace"],
        "the left placement's own row, not the pane"
    );
    assert_eq!(
        left.selected(),
        Some(workspace_key),
        "and its cursor stays where it was"
    );
    assert_eq!(
        right.selected(),
        Some(pane_key(PaneId(1))),
        "the right one is untouched"
    );
}

/// The `Space` regression, pinned at the only level a unit test can reach.
///
/// The two verbs differ by **one queued intent** and nothing else: activate asks for
/// `unfocus_dock`, hint does not. Whether the keyboard leaves is decided here, by what the user
/// asked for, and never by the host as a side effect of focusing a pane.
///
/// The end of the story — the keyboard actually staying on the dock — needs an `AppState`, which
/// cannot be built without a window. The user drives that half in the app.
#[test]
fn hint_and_activate_differ_only_by_the_release_they_ask_for() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");

    // Park the cursor on the pane: the workspace row takes the `focus_workspace` arm, which
    // never releases either way, so it cannot tell the two verbs apart.
    let mut cx = ProviderCx::new("workspaces", store.clone());
    p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);
    cx.drain();
    assert!(
        matches!(
            store.workspaces.tree().current_item(),
            Some(WorkspaceRow::Pane { .. }),
        ),
        "the fixture's second navigable row is the pane",
    );

    p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
    let hinted: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
    assert!(
        hinted.contains(&"focus_pane".to_string()),
        "hint still brings the pane to the front: {hinted:?}",
    );
    assert!(
        !hinted.contains(&"unfocus_dock".to_string()),
        "…and asks for no release, so j/k keep working: {hinted:?}",
    );
    assert!(
        hinted.contains(&"focus_dock".to_string()),
        "…and keeps the keyboard in the dock, by asking for it (a no-op when it is there): {hinted:?}",
    );

    p.perform(ACTIVATE_SELECTED, &Intent::new(ACTIVATE_SELECTED), &mut cx);
    let activated: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
    assert!(
        activated.contains(&"focus_pane".to_string())
            && activated.contains(&"unfocus_dock".to_string()),
        "activate-and-leave asks for the release itself: {activated:?}",
    );
}

/// **An aimed hint lands on the row that was picked, not on the cursor**.
///
/// This is the half `prefix+/` needs: the picker knows which letter was chosen, so it names
/// that row by its nav key and the hint acts there. Without the argument the verb could only
/// ever mean "the row I am already on", which is why the picker used to be pointed at the
/// row's *click* instead — and that click leaves the sidebar.
#[test]
fn a_hint_can_be_aimed_at_a_row_by_key_and_moves_the_cursor_there() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());

    // The cursor starts on the workspace row; aim at the pane row instead.
    let pane_id = match store
        .workspaces
        .tree()
        .flat_items
        .iter()
        .find_map(|row| match row {
            WorkspaceRow::Pane { pane_id } => Some(*pane_id),
            _ => None,
        }) {
        Some(id) => id,
        None => panic!("the fixture has a pane row"),
    };
    let aimed = Intent::new(PEEK_SELECTED).arg("key", PropValue::Text(pane_key(pane_id)));
    p.perform(PEEK_SELECTED, &aimed, &mut cx);

    assert!(
        matches!(
            store.workspaces.tree().current_item(),
            Some(WorkspaceRow::Pane { pane_id: on }) if *on == pane_id,
        ),
        "the cursor moved onto the row that was named",
    );
    let fired: Vec<String> = cx.drain().into_iter().map(|i| i.action).collect();
    assert!(
        fired.contains(&"focus_pane".to_string()) && !fired.contains(&"unfocus_dock".to_string()),
        "…and it hints it: the pane comes forward, the dock keeps the keyboard: {fired:?}",
    );
}

/// **A hint takes the keyboard for the dock it is in.** Landing the cursor on a row is only
/// worth anything if `j`/`k` then move it, so the verb asks for the dock — every time, for its
/// own mount. Whether the dock already has the keyboard is the host's to know (it reads the
/// tree), and `focus_dock` aimed at a dock that has it changes nothing.
#[test]
fn a_hint_asks_for_the_keyboard_for_its_own_dock() {
    let p = WorkspacesContainerProvider::new("workspaces");
    let store = store_with_tree();
    let mut cx = ProviderCx::new("workspaces", store.clone());
    p.perform(PEEK_SELECTED, &Intent::new(PEEK_SELECTED), &mut cx);
    let asked: Vec<crate::chrome::Intent> = cx.drain();
    let focus_dock = asked.iter().find(|i| i.action == "focus_dock");
    assert_eq!(
        focus_dock.map(|i| i.args.get("dock").cloned()),
        Some(Some(PropValue::Text("workspaces".to_string()))),
        "it asks for its own mount: {asked:?}",
    );
}

/// A key naming a row that is not there leaves the cursor alone rather than failing — a tree
/// rebuilt under the letters is not an error.
#[test]
fn an_aimed_hint_with_a_stale_key_leaves_the_cursor_where_it_was() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());
    let before = store.workspaces.tree().cursor;

    let stale = Intent::new(PEEK_SELECTED).arg("key", PropValue::Text("pane:9999".to_string()));
    p.perform(PEEK_SELECTED, &stale, &mut cx);

    assert_eq!(store.workspaces.tree().cursor, before, "no guess, no move");
}

// ── This component's row menus ──

/// **A row's menu is built from this component's own model**, at the row: the pane's column
/// (absent for a floating pane, which is in none) and whether the workspace has a name to
/// clear. These are the facts the host used to resolve and hand over; now the row reads them
/// where it is drawn, which is why there is no key to map and no path to declare.
#[test]
fn a_row_menu_reads_the_components_own_model() {
    let store = store();
    *store.workspaces.tree_mut() = tree_with_a_floating_pane();
    let tree = store.workspaces.tree();

    let tiled = pane_row_items(PaneId(1), tree.locate_pane(PaneId(1)), false);
    let new_pane = tiled
        .iter()
        .find(|i| i.id == "add_pane_to_column")
        .expect("a tiled pane is in a column");
    assert_eq!(new_pane.intent.args.get("ws_idx"), Some(&PropValue::Int(0)));
    assert_eq!(
        new_pane.intent.args.get("col_idx"),
        Some(&PropValue::Int(0))
    );

    // A floating pane is in no column, so `pane_card` passes `None`.
    let floating = pane_row_items(PaneId(9), None, false);
    assert!(
        !floating.iter().any(|i| i.id == "add_pane_to_column"),
        "a floating pane is in no column, so the entry that needs one is absent rather than \
         aimed at a guess: {:?}",
        floating.iter().map(|i| &i.id).collect::<Vec<_>>(),
    );
    assert!(
        floating.iter().any(|i| i.id == "close"),
        "it is still closable"
    );

    let renamed = tree
        .workspaces
        .iter()
        .find(|w| w.ws_idx == 0)
        .expect("workspace 0");
    let ws = workspace_row_items(0, renamed.custom_name.is_some());
    assert!(
        ws.iter().any(|i| i.id == "reset_workspace_name"),
        "the workspace was renamed, so there is a name to clear",
    );
}

/// Renaming the row the cursor is on is **this component's** verb:
/// it resolves the cursor and dispatches the by-id action for whichever kind of row it is. The
/// app's `rename_pane` / `rename_workspace` keep meaning the focused pane / active workspace.
#[test]
fn rename_selected_targets_the_cursor_row_by_id() {
    let store = store_with_tree();
    let p = WorkspacesContainerProvider::new("workspaces");
    let mut cx = ProviderCx::new("workspaces", store.clone());
    let intent = Intent::new(RENAME_SELECTED);

    // The fixture's first row is the workspace header.
    assert_eq!(p.perform(RENAME_SELECTED, &intent, &mut cx), Handled::Yes);
    let asked = cx.drain();
    assert_eq!(asked[0].action, "rename_workspace_by_idx");
    assert_eq!(asked[0].args.get("ws_idx"), Some(&PropValue::Int(0)));

    // Step onto the pane row and the same key means that pane.
    p.perform(CURSOR_DOWN, &Intent::new(CURSOR_DOWN), &mut cx);
    cx.drain();
    assert_eq!(p.perform(RENAME_SELECTED, &intent, &mut cx), Handled::Yes);
    let asked = cx.drain();
    assert_eq!(asked[0].action, "rename_pane_by_id");
    assert_eq!(asked[0].args.get("pane_id"), Some(&PropValue::Int(1)));
}

#[test]
fn an_action_this_component_does_not_own_declines() {
    let store = store_with_tree();
    let mut cx = ProviderCx::new("workspaces", store);
    assert_eq!(
        WorkspacesContainerProvider::new("workspaces").perform(
            "docker.restart_selected",
            &Intent::new("docker.restart_selected"),
            &mut cx,
        ),
        Handled::No,
    );
}

/// **A plugin reads the selection worked out from the cursor key** of the placement holding the
/// keyboard — nothing stores it twice — and gets `None` when no placement holds it.
#[test]
fn the_sidebar_selection_a_plugin_reads_comes_from_the_focused_placements_key() {
    let store = store_with_tree();
    let view = || crate::host::StateView::over(&store).sidebar_selection();
    store.set_container_cursor("workspaces", Some(pane_key(PaneId(1))));
    assert_eq!(view(), None, "no placement holds the keyboard");

    store.announce_focused_container(Some("workspaces".to_string()));
    assert_eq!(
        view(),
        Some(crate::chrome::SidebarSelection::Pane { pane_id: PaneId(1) }),
    );
}
