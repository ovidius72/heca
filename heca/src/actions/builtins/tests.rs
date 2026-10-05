use super::*;
use crate::actions::Side;

/// The actions that run on the server, by name. A change here is a decision about which side of
/// the split an action is on, so it is written out — not derived from the descriptors it checks.
const SERVER_SIDE: &[&str] = &[
        "add_column_to_workspace",
        "add_pane_to_column",
        "close",
        "close_pane_by_id",
        "create_workspace",
        "delete_column",
        "delete_workspace",
        "float",
        "float_at",
        "move",
        "move_column",
        "move_column_down",
        "move_column_to_workspace",
        "move_column_up",
        "move_pane_left",
        "move_pane_right",
        "move_pane_to_column",
        "move_pane_to_new_column",
        "move_pane_to_workspace",
        "pane_height_decrease",
        "pane_height_increase",
        "place_pane",
        "reset_pane_name",
        "reset_pane_name_by_id",
        "reset_workspace_name",
        "reset_workspace_name_by_idx",
        "resize",
        "resize_column_by",
        "resize_decrease",
        "resize_increase",
        "resize_pane_height_by",
        "resize_to",
        "rename_column_to",
        "rename_target",
        "rename_workspace_to",
        "spawn_command",
        "split_horizontal",
        "split_vertical",
        "swap",
        "swap_columns",
        "swap_down",
        "swap_left",
        "swap_right",
        "swap_up",
        "take_pane",
        "terminal_kill",
        "terminal_run",
        "zoom_column",
        "zoom_column_at_index",
];

/// **Every action says which side it is on.** The field has no default, so the compiler already
/// refuses a descriptor without one; this reads the source as well, so a future `..` filler or a
/// macro that supplied a default would be caught here instead of putting an action on a side
/// nobody chose.
#[test]
fn every_builtin_descriptor_declares_its_side() {
    for (file, src) in [
        ("navigation", include_str!("navigation.rs")),
        ("layout", include_str!("layout.rs")),
        ("pane", include_str!("pane.rs")),
        ("workspace", include_str!("workspace.rs")),
        ("chrome", include_str!("chrome.rs")),
        ("system", include_str!("system.rs")),
    ] {
        let named = src.matches("name: \"").count();
        let sided = src.matches("side: Side::").count();
        assert!(named > 0, "{file}: no descriptors found — the reader no longer understands it");
        assert_eq!(named, sided, "{file}: {named} actions but {sided} sides");
    }
}

/// **The server side is exactly what changes what every window shares** — panes, columns,
/// workspaces, their names, the terminals behind them. Everything else is the window's own.
#[test]
fn the_server_side_is_exactly_the_listed_actions() {
    let mut server: Vec<&str> = builtins()
        .filter(|d| d.side == Side::Server)
        .map(|d| d.name)
        .collect();
    server.sort_unstable();
    let mut expected = SERVER_SIDE.to_vec();
    expected.sort_unstable();
    assert_eq!(server, expected);
}

/// What a window only shows or is doing stays in the window, however much it sounds like layout:
/// scrolling the view, picking a target, opening a prompt, moving focus.
#[test]
fn what_only_a_window_shows_or_does_is_client_side() {
    let side = |name: &str| builtins().find(|d| d.name == name).map(|d| d.side);
    for name in [
        "scroll_view_left",
        "move_pane_to_workspace_pick",
        "rename_pane",
        "focus_left",
        "command_palette",
        "enter_selection_mode",
        "sidebar_left",
    ] {
        assert_eq!(side(name), Some(Side::Client), "{name}");
    }
}

/// A caller discovering a running heca (RPC `describe-action`) is told the side.
#[test]
fn the_side_is_reported_to_a_caller_that_asks() {
    let catalog = crate::actions::ActionCatalog::with_builtins();
    let info = catalog.describe("split_horizontal").expect("a built-in");
    assert_eq!(info.side, Some(Side::Server));
    let json = serde_json::to_string(&info).expect("serializes");
    assert!(json.contains("\"side\":\"server\""), "{json}");
}

/// **A name-keyed action has no side.** A dock's or a plugin's action dispatches built-ins, and each
/// of those goes to its own side; giving the wrapper one too would be a second answer to a question
/// the built-in already settled.
#[test]
fn a_name_keyed_action_has_no_side_of_its_own() {
    let mut catalog = crate::actions::ActionCatalog::with_builtins();
    let _ = catalog.insert_dynamic(crate::actions::ActionMeta::new("pro.start"));
    let info = catalog.describe("pro.start").expect("it was added");
    assert_eq!(info.side, None);
    assert_eq!(crate::actions::catalog::builtin_side("pro.start"), None);
    assert_eq!(crate::actions::catalog::builtin_side("close"), Some(Side::Server));
}
