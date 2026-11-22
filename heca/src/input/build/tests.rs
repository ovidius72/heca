//! Tests for [`super`].

use super::*;
use crate::input::{SpawnKind, WmAction};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

#[test]
fn test_build_spawn_command_with_options() {
    let args = std::collections::HashMap::from([
        ("command".to_string(), "lazygit".to_string()),
        ("kind".to_string(), "terminal".to_string()),
        ("float".to_string(), "true".to_string()),
        ("close_pane".to_string(), "true".to_string()),
        ("keep_on_error".to_string(), "true".to_string()),
    ]);
    assert_eq!(
        build_action("spawn_command", &args),
        Some(WmAction::SpawnCommand {
            command: "lazygit".to_string(),
            kind: SpawnKind::Terminal,
            float: true,
            close_policy: PaneClosePolicy {
                close_pane: true,
                keep_on_error: true,
                keep_on_success: false,
            },
            cwd: None,
        })
    );
}

#[test]
fn test_build_scroll_to_offset() {
    let args = std::collections::HashMap::from([("rows".to_string(), "42".to_string())]);
    assert_eq!(
        build_action("scroll_to_offset", &args),
        Some(WmAction::ScrollToOffset {
            rows: 42,
            terminal: None
        })
    );
}

/// **A scroll can name the terminal it scrolls** — the chip or scrollbar of the one clicked; without
/// a name it is the keyboard's terminal, as before.
#[test]
fn a_scroll_names_its_terminal_or_takes_the_keyboards() {
    let named = std::collections::HashMap::from([
        ("rows".to_string(), "3".to_string()),
        ("terminal".to_string(), "9".to_string()),
    ]);
    assert_eq!(
        build_action("scroll_to_offset", &named),
        Some(WmAction::ScrollToOffset {
            rows: 3,
            terminal: Some(9)
        })
    );
    let terminal = std::collections::HashMap::from([("terminal".to_string(), "9".to_string())]);
    assert_eq!(
        build_action("scroll_to_bottom", &terminal),
        Some(WmAction::ScrollToBottom { terminal: Some(9) })
    );
    assert_eq!(
        build_action("scroll_to_bottom", &std::collections::HashMap::new()),
        Some(WmAction::ScrollToBottom { terminal: None })
    );
}

/// **One workspace's row folds by its index**, and a bare name folds nothing rather than row 0.
#[test]
fn a_workspace_row_folds_by_its_index_and_never_by_default() {
    let args = std::collections::HashMap::from([("ws_idx".to_string(), "2".to_string())]);
    assert_eq!(
        build_action("toggle_workspace_collapsed", &args),
        Some(WmAction::ToggleWorkspaceCollapsed { ws_idx: 2 })
    );
    assert_eq!(
        build_action(
            "toggle_workspace_collapsed",
            &std::collections::HashMap::new()
        ),
        None
    );
}

#[test]
fn test_build_rename_column_by_idx() {
    let args = std::collections::HashMap::from([
        ("ws_idx".to_string(), "1".to_string()),
        ("col_idx".to_string(), "2".to_string()),
    ]);
    assert_eq!(
        build_action("rename_column_by_idx", &args),
        Some(WmAction::RenameColumnByIdx {
            ws_idx: 1,
            col_idx: 2
        })
    );
    // Missing args → not built (both indices are required).
    assert_eq!(
        build_action("rename_column_by_idx", &std::collections::HashMap::new()),
        None
    );
}

#[test]
fn test_build_reset_name_by_target() {
    let pane_args = std::collections::HashMap::from([("pane_id".to_string(), "7".to_string())]);
    assert_eq!(
        build_action("reset_pane_name_by_id", &pane_args),
        Some(WmAction::ResetPaneNameById { pane_id: PaneId(7) })
    );
    let ws_args = std::collections::HashMap::from([("ws_idx".to_string(), "2".to_string())]);
    assert_eq!(
        build_action("reset_workspace_name_by_idx", &ws_args),
        Some(WmAction::ResetWorkspaceNameByIdx { ws_idx: 2 })
    );
}

/// `place_pane` is reachable by name and args — the one path a key binding, a plugin and the
/// RPC's `action place_pane pane_id=.. ws_idx=.. col_idx=.. [pane_idx=..]` all take. Without a
/// row it means a new column of its own.
#[test]
fn place_pane_builds_from_its_name_and_args() {
    let args = |pairs: &[(&str, &str)]| -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    assert_eq!(
        build_action(
            "place_pane",
            &args(&[
                ("pane_id", "5"),
                ("ws_idx", "1"),
                ("col_idx", "2"),
                ("pane_idx", "0")
            ])
        ),
        Some(WmAction::PlacePane {
            pane_id: PaneId(5),
            ws_idx: 1,
            col_idx: 2,
            pane_idx: Some(0),
        }),
    );
    assert_eq!(
        build_action(
            "place_pane",
            &args(&[("pane_id", "5"), ("ws_idx", "1"), ("col_idx", "2")])
        ),
        Some(WmAction::PlacePane {
            pane_id: PaneId(5),
            ws_idx: 1,
            col_idx: 2,
            pane_idx: None,
        }),
    );
    assert_eq!(build_action("place_pane", &args(&[("pane_id", "5")])), None);
}

/// `focus_dock` is **one** action with two doors: a bare name for the keybinding (which opens the
/// pick) and a `dock` argument for a caller that already knows the answer.
///
/// The bare form is only legal because the argument is **optional** — an action with a required
/// argument must not resolve from its name alone (`every_action_that_needs_a_target_refuses_to_
/// default_it`), and that is exactly the rule this action is shaped around.
#[test]
fn focus_dock_resolves_bare_and_with_a_dock() {
    let none = std::collections::HashMap::new();
    assert_eq!(
        resolve_action("focus_dock", &none),
        Some(WmAction::FocusDock { dock: None }),
        "bare: nothing named ⇒ pick one by letter",
    );
    let mut args = std::collections::HashMap::new();
    args.insert("dock".to_string(), "workspaces".to_string());
    assert_eq!(
        resolve_action("focus_dock", &args),
        Some(WmAction::FocusDock {
            dock: Some("workspaces".to_string())
        }),
        "named: focus it directly, no pick — the arguments are read before the bare name",
    );
}

/// One action shows, hides or toggles any region; `visible` left out means toggle.
#[test]
fn set_region_visible_builds_from_region_and_visibility() {
    use crate::chrome::RegionId;
    use crate::input::RegionVisibility;
    let args = |pairs: &[(&str, &str)]| -> std::collections::HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    };
    assert_eq!(
        build_action(
            "set_region_visible",
            &args(&[("region", "sidebar.left"), ("visible", "hide")])
        ),
        Some(WmAction::SetRegionVisible {
            region: RegionId::LeftSidebar,
            visible: RegionVisibility::Hide,
        })
    );
    assert_eq!(
        build_action("set_region_visible", &args(&[("region", "bottom")])),
        Some(WmAction::SetRegionVisible {
            region: RegionId::BottomBar,
            visible: RegionVisibility::Toggle,
        })
    );
    assert_eq!(
        build_action(
            "set_region_visible",
            &args(&[("region", "bar.top"), ("visible", "maybe")])
        ),
        None,
        "a visibility nobody declared is refused, not defaulted"
    );
    assert_eq!(
        build_action("set_region_visible", &args(&[])),
        None,
        "region is required"
    );
}

#[test]
fn test_build_terminal_run_and_kill() {
    // Bare: types into the focused pane's terminal and presses Enter.
    let args = std::collections::HashMap::from([("text".to_string(), "ls".to_string())]);
    assert_eq!(
        build_action("terminal_run", &args),
        Some(WmAction::TerminalRun {
            pane_id: None,
            terminal: None,
            text: "ls".to_string(),
            enter: true,
        })
    );
    // By terminal id, without Enter.
    let args = std::collections::HashMap::from([
        ("text".to_string(), "vim".to_string()),
        ("terminal".to_string(), "12".to_string()),
        ("enter".to_string(), "false".to_string()),
    ]);
    assert_eq!(
        build_action("terminal_run", &args),
        Some(WmAction::TerminalRun {
            pane_id: None,
            terminal: Some(12),
            text: "vim".to_string(),
            enter: false,
        })
    );
    assert_eq!(
        build_action("terminal_run", &std::collections::HashMap::new()),
        None,
        "text is required"
    );
    let args = std::collections::HashMap::from([("pane_id".to_string(), "3".to_string())]);
    assert_eq!(
        build_action("terminal_kill", &args),
        Some(WmAction::TerminalKill {
            pane_id: Some(PaneId(3)),
            terminal: None,
        })
    );
}

#[test]
fn terminal_rename_requires_a_target_and_keeps_the_name_structured() {
    let args = std::collections::HashMap::from([
        ("terminal".into(), "7".into()),
        ("name".into(), "Build logs".into()),
    ]);
    assert_eq!(
        build_action("terminal_rename", &args),
        Some(WmAction::TerminalRename {
            terminal: "7".into()
        })
    );
    assert_eq!(
        build_action("terminal_rename_to", &args),
        Some(WmAction::TerminalRenameTo {
            terminal: "7".into(),
            name: "Build logs".into()
        })
    );
    assert_eq!(
        build_action("terminal_rename_to", &std::collections::HashMap::new()),
        None
    );
}

#[test]
fn qualified_terminal_names_reach_both_rename_actions() {
    let args = std::collections::HashMap::from([
        ("terminal".into(), "demo.shell".into()),
        ("name".into(), "Build logs".into()),
    ]);
    assert!(build_action("terminal_rename", &args).is_some());
    assert!(build_action("terminal_rename_to", &args).is_some());
}

#[test]
fn test_spawn_command_takes_a_folder() {
    let args = std::collections::HashMap::from([
        ("command".to_string(), "htop".to_string()),
        ("cwd".to_string(), "/tmp/work".to_string()),
    ]);
    let Some(WmAction::SpawnCommand { cwd, .. }) = build_action("spawn_command", &args) else {
        panic!("spawn_command builds");
    };
    assert_eq!(cwd.as_deref(), Some("/tmp/work"));
}

/// **Moving a dock is an action first** (T333): the line a keybinding's `args`, a menu entry, a
/// plugin and an RPC call all send builds the same action the drag will post. A drag that could do
/// something no key can would break input parity.
#[test]
fn a_dock_move_is_built_from_the_line_every_door_sends() {
    let line = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<std::collections::HashMap<_, _>>()
    };
    assert_eq!(
        build_action(
            "chrome.container.move_to_region",
            &line(&[("container_id", "workspaces"), ("region", "sidebar.right")])
        ),
        Some(WmAction::MoveContainerToRegion {
            container_id: "workspaces".into(),
            region: crate::chrome::RegionId::RightSidebar,
        })
    );
    assert_eq!(
        build_action(
            "chrome.container.reorder_before",
            &line(&[("container_id", "notes"), ("before_id", "workspaces")])
        ),
        Some(WmAction::ReorderContainerBefore {
            container_id: "notes".into(),
            before_id: Some("workspaces".into()),
        })
    );
    assert_eq!(
        build_action(
            "chrome.container.reorder_before",
            &line(&[("container_id", "notes")])
        ),
        Some(WmAction::ReorderContainerBefore {
            container_id: "notes".into(),
            before_id: None,
        }),
        "no target: to the end",
    );
    assert_eq!(
        build_action(
            "chrome.container.reorder_after",
            &line(&[("container_id", "notes"), ("after_id", "workspaces")])
        ),
        Some(WmAction::ReorderContainerAfter {
            container_id: "notes".into(),
            after_id: "workspaces".into(),
        })
    );
    assert_eq!(
        build_action(
            "chrome.container.reorder_after",
            &line(&[("container_id", "notes")])
        ),
        None,
        "after nothing means nothing",
    );
}
