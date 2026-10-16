//! The entries the built-in menus carry, and that every one of them runs a real action.

use crate::chrome::surfaces::context_menu::pane_menu::*;
use crate::chrome::surfaces::context_menu::*;
use crate::chrome::{DropdownItem, PropValue};
use crate::providers::workspaces::{column_row_items, pane_row_items, workspace_row_items};

/// **A destructive entry uses the same verb as the action it names**.
///
/// A `DropdownItem`'s `id` is its catalog identity — the icon and the shortcut resolve from it —
/// while its label is written by hand at the call site. Nothing compared the two, so the sidebar
/// pane row said "Delete pane" while its own action, its palette entry, the content-pane menu
/// and its confirm dialog all said close. The user found it by reading the menu.
///
/// **Only the destructive three**, deliberately. A menu label is free to differ from the catalog
/// in general — "Zoom / unzoom" is a better menu entry than "Toggle Column Zoom" — and holding
/// every entry to the catalog's wording would flag those as drift. What may not differ is the
/// verb on something irreversible: close and delete promise different consequences.
///
/// The verb alone is compared, since a menu is sentence case ("Close pane") and the catalog is
/// title case ("Close Pane") — two conventions, each applied consistently.
#[test]
fn a_destructive_entry_uses_the_same_verb_as_the_action_it_names() {
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let mut every_item = Vec::new();
    every_item.extend(pane_action_items(true));
    every_item.extend(pane_row_items(PaneId(1), Some((0, 0)), true));
    every_item.extend(column_row_items(0, 0));
    every_item.extend(workspace_row_items(0, true));

    let verb = |s: &str| s.split_whitespace().next().unwrap_or("").to_lowercase();
    let mut checked = 0;
    for item in &every_item {
        if !["close", "delete_column", "delete_workspace"].contains(&item.id.as_str()) {
            continue;
        }
        let meta = catalog
            .find(&item.id)
            .unwrap_or_else(|| panic!("`{}` is a built-in action", item.id));
        assert_eq!(
            verb(&item.label),
            verb(&meta.label),
            "menu entry '{}' (id `{}`) disagrees with its action's label '{}'",
            item.label,
            item.id,
            meta.label,
        );
        checked += 1;
    }
    assert_eq!(checked, 4, "two close entries + one column + one workspace");
}

#[test]
fn pane_action_items_include_rename_and_end_in_close() {
    let items = pane_action_items(true);
    assert_eq!(items[0].id, "split_horizontal");
    assert!(items.iter().any(|i| i.id == "rename_pane"), "has Rename");
    assert!(
        items.iter().any(|i| i.id == "rename_workspace"),
        "has Rename workspace"
    );
    let last = items.last().unwrap();
    assert_eq!(last.id, "close");
    assert!(last.danger, "close is danger-styled");
    // Pane menu carries no workspace-reset action (that lives in the workspace menu).
    assert!(
        !items.iter().any(|i| i.id == "reset_workspace_name"),
        "pane menu must not reset the workspace"
    );
}

#[test]
fn reset_name_entries_are_conditional_on_a_custom_name() {
    // "Use process name" only when the pane has a custom name.
    assert!(
        pane_action_items(true)
            .iter()
            .any(|i| i.id == "reset_pane_name"),
        "reset shows with a custom name"
    );
    assert!(
        !pane_action_items(false)
            .iter()
            .any(|i| i.id == "reset_pane_name"),
        "reset hidden without a custom name"
    );
    // "Use default name" only when the workspace has a custom name.
    assert!(
        workspace_row_items(0, true)
            .iter()
            .any(|i| i.id == "reset_workspace_name")
    );
    assert!(
        !workspace_row_items(0, false)
            .iter()
            .any(|i| i.id == "reset_workspace_name")
    );
}

/// A menu entry's **identity** (`id` → icon/label) and its **behaviour** (`intent` → what runs)
/// are separate. The sidebar "Close pane" entry shows the `close` icon but runs
/// `close_pane_by_id` against *that row's* pane — the whole reason a plugin entry can exist at
/// all, since the intent is a NAME, not a closed-enum variant.
#[test]
fn an_entry_identity_and_the_action_it_runs_are_separate() {
    let items = pane_row_items(PaneId(7), Some((2, 3)), false);
    let delete = items.iter().find(|i| i.id == "close").unwrap();
    assert_eq!(delete.label, "Close pane");
    assert!(delete.danger);
    // Identity is `close` (the icon), behaviour is the by-id action with the row's pane.
    assert_eq!(delete.intent.action, "close_pane_by_id");
    assert_eq!(delete.intent.args.get("pane_id"), Some(&PropValue::Int(7)));

    // "New pane" targets THIS pane's column — which the component looked up in its own model.
    let new_pane = items.iter().find(|i| i.id == "add_pane_to_column").unwrap();
    assert_eq!(new_pane.intent.action, "add_pane_to_column");
    assert_eq!(new_pane.intent.args.get("ws_idx"), Some(&PropValue::Int(2)));
    assert_eq!(
        new_pane.intent.args.get("col_idx"),
        Some(&PropValue::Int(3))
    );
}

/// Every built-in entry's intent must actually RESOLVE — a name + args that `build_action` (or
/// `action_from_name`) can turn into a real `WmAction`. Without this, converting the menus from
/// `WmAction` to `Intent` could silently produce entries that do nothing when clicked.
#[test]
fn every_builtin_menu_entry_resolves_to_a_real_action() {
    let mut all = pane_action_items(true);
    all.extend(pane_row_items(PaneId(1), Some((0, 0)), true));
    all.extend(column_row_items(0, 0));
    all.extend(workspace_row_items(0, true));
    all.push(item_running(
        "open_link",
        "Open link",
        "open_link",
        &[("url", PropValue::Text("https://x".into()))],
    ));

    for i in &all {
        let args: std::collections::HashMap<String, String> = i
            .intent
            .args
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    PropValue::Int(n) => n.to_string(),
                    PropValue::Text(t) => t.clone(),
                    other => format!("{other:?}"),
                };
                (k.clone(), s)
            })
            .collect();
        let resolved = crate::input::resolve_action(&i.intent.action, &args).is_some();
        assert!(
            resolved,
            "menu entry {:?} dispatches {:?}, which resolves to no action",
            i.id, i.intent.action
        );
    }
}

#[test]
fn context_menus_differ_by_where_opened() {
    // The content each context produces is distinct — proving the menu adapts to where
    // it is opened (pane vs sidebar column vs sidebar workspace).
    let pane = pane_action_items(true);
    let col = column_row_items(0, 0);
    let ws = workspace_row_items(0, true);
    let labels = |v: &[DropdownItem]| v.iter().map(|i| i.label.clone()).collect::<Vec<_>>();
    let has = |v: &[DropdownItem], s: &str| v.iter().any(|i| i.label == s);

    // Distinct label sets — no two contexts produce the same menu.
    assert_ne!(labels(&pane), labels(&col));
    assert_ne!(labels(&col), labels(&ws));
    assert_ne!(labels(&pane), labels(&ws));

    // Signature entries unique to each context.
    assert!(has(&pane, "Zoom / unzoom") && has(&pane, "Float / unfloat"));
    assert!(has(&col, "Delete column"));
    assert!(has(&ws, "New workspace") && has(&ws, "Delete workspace"));
    // Each sidebar context ends in a danger delete action, keyed by its own action
    // name so it shows that action's icon (Trash / StackMinus), not the pane `close` one.
    assert_eq!(col.last().map(|i| i.id.as_str()), Some("delete_column"));
    assert!(col.last().map(|i| i.danger).unwrap_or(false));
    assert_eq!(ws.last().map(|i| i.id.as_str()), Some("delete_workspace"));
    assert!(ws.last().map(|i| i.danger).unwrap_or(false));
}
