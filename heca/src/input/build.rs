//! An action from its name and its arguments as text — the one constructor a binding, a menu
//! entry, a plugin and RPC all reach.

use super::{SpawnKind, WmAction};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

// ── Parameterized action builders ──

fn get_u64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<u64> {
    args.get(key)?.parse().ok()
}
fn get_usize(args: &std::collections::HashMap<String, String>, key: &str) -> Option<usize> {
    args.get(key)?.parse().ok()
}
fn get_f64(args: &std::collections::HashMap<String, String>, key: &str) -> Option<f64> {
    args.get(key)?.parse().ok()
}
fn get_string(args: &std::collections::HashMap<String, String>, key: &str) -> Option<String> {
    args.get(key).cloned()
}
fn get_enum<T: std::str::FromStr>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    args.get(key)?.parse().ok()
}
/// An **optional** vocabulary argument: absent means the default, and a value that does not parse
/// is a mistake worth failing on rather than silently becoming the default.
fn get_enum_or_default<T: std::str::FromStr + Default>(
    args: &std::collections::HashMap<String, String>,
    key: &str,
) -> Option<T> {
    match args.get(key) {
        Some(raw) => raw.parse().ok(),
        None => Some(T::default()),
    }
}

/// **The one door from a name and its arguments to a [`WmAction`]** — what a `config.toml` binding, a
/// menu entry's `Intent`, the palette, a plugin, RPC and the catalog all call.
///
/// The arguments are read first ([`build_action`]); a name that says everything on its own falls
/// back to [`action_from_name`](super::action_from_name). `None` when neither builds it — an unknown
/// name, or a required argument missing or malformed. The two builders are private to `input` so no
/// caller can combine them in its own order again.
pub fn resolve_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    build_action(name, args).or_else(|| super::action_from_name(name))
}

/// Build a `WmAction` from a name and its arguments as text — the constructor a `config.toml`
/// binding, a menu entry's `Intent`, a plugin and RPC all reach.
///
/// `None` when the name is unknown, or when a required argument is missing or does not parse.
///
/// **Each arm's arguments are declared** in that action's
/// [`ActionDescriptor::args`](crate::actions::ActionDescriptor::args), which is what lets a caller
/// discover them (`describe-action`) and what lets
/// [`check_args`](crate::args::check_args) say *which* argument was wrong instead of the whole
/// call quietly evaporating. The list that used to sit here in a doc comment named eleven of the
/// thirty-five and had not been updated in a long time; the declarations replaced it, and
/// `every_declared_argument_is_read_by_the_action` keeps them and these arms in step.
pub(super) fn build_action(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
        "cursor_to" => Some(WmAction::CursorTo {
            mount: args.get("mount")?.clone(),
            key: args.get("key")?.clone(),
        }),
        "focus_pane" => Some(WmAction::FocusPane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "focus_workspace" => Some(WmAction::FocusWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "swap" => Some(WmAction::Swap {
            a_id: PaneId(get_u64(args, "a_id")?),
            b_id: PaneId(get_u64(args, "b_id")?),
        }),
        "move" => Some(WmAction::Move {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            target_col: get_usize(args, "target_col")?,
        }),
        "move_pane_to_workspace" => Some(WmAction::MovePaneToWorkspace {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "move_pane_to_column" => Some(WmAction::MovePaneToColumn {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "place_pane" => Some(WmAction::PlacePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
            pane_idx: get_usize(args, "pane_idx"),
        }),
        "move_column" => Some(WmAction::MoveColumn {
            src_ws: get_usize(args, "src_ws")?,
            src_col: get_usize(args, "src_col")?,
            dst_ws: get_usize(args, "dst_ws")?,
            dst_idx: get_usize(args, "dst_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        "swap_columns" => Some(WmAction::SwapColumns {
            a_ws: get_usize(args, "a_ws")?,
            a_col: get_usize(args, "a_col")?,
            b_ws: get_usize(args, "b_ws")?,
            b_col: get_usize(args, "b_col")?,
        }),
        "move_column_to_workspace" => Some(WmAction::MoveColumnToWorkspace {
            col_idx: get_usize(args, "col_idx")?,
            ws_idx: get_usize(args, "ws_idx")?,
            focus: args
                .get("focus")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        // `pane_id` is OPTIONAL: omitting it moves the FOCUSED pane, which is what a keybinding
        // means. Declaring it keeps the named path level with the RPC command, which has always
        // accepted an explicit pane.
        "move_pane_left" => Some(WmAction::MovePaneLeft {
            pane_id: get_u64(args, "pane_id").map(PaneId),
        }),
        "move_pane_right" => Some(WmAction::MovePaneRight {
            pane_id: get_u64(args, "pane_id").map(PaneId),
        }),
        "resize_column_by" => Some(WmAction::ResizeColumnBy {
            col_idx: get_usize(args, "col_idx")?,
            delta: get_f64(args, "delta")?,
        }),
        "resize_pane_height_by" => Some(WmAction::ResizePaneHeightBy {
            col_idx: get_usize(args, "col_idx")?,
            pane_idx: get_usize(args, "pane_idx")?,
            delta: get_f64(args, "delta")?,
        }),
        "resize" => Some(WmAction::Resize {
            target: get_enum(args, "target")?,
            amount: get_f64(args, "amount")?,
            edge: get_enum_or_default(args, "edge")?,
        }),
        "resize_to" => Some(WmAction::ResizeTo {
            target: get_enum(args, "target")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "float_at" => Some(WmAction::FloatAt {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            x: get_f64(args, "x")?,
            y: get_f64(args, "y")?,
            width: get_f64(args, "width")?,
            height: get_f64(args, "height")?,
        }),
        "close_pane_by_id" => Some(WmAction::ClosePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "notification_dismiss_one" => Some(WmAction::NotificationDismissOne {
            notification_id: get_u64(args, "id")?,
        }),
        "notification_action_relay" => Some(WmAction::NotificationActionRelay {
            notification_id: get_u64(args, "id")?,
            key: get_string(args, "key")?,
        }),
        "rename_target" => Some(WmAction::RenameTarget {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            name: get_string(args, "name")?,
        }),
        "rename_workspace_to" => Some(WmAction::RenameWorkspaceTo {
            ws_idx: get_usize(args, "ws_idx")?,
            name: get_string(args, "name")?,
        }),
        "rename_column_to" => Some(WmAction::RenameColumnTo {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
            name: get_string(args, "name")?,
        }),
        "rename_pane_by_id" => Some(WmAction::RenamePaneById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "rename_workspace_by_idx" => Some(WmAction::RenameWorkspaceByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "rename_column_by_idx" => Some(WmAction::RenameColumnByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "reset_pane_name_by_id" => Some(WmAction::ResetPaneNameById {
            pane_id: PaneId(get_u64(args, "pane_id")?),
        }),
        "reset_workspace_name_by_idx" => Some(WmAction::ResetWorkspaceNameByIdx {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "take_pane" => Some(WmAction::TakePane {
            pane_id: PaneId(get_u64(args, "pane_id")?),
            focus_after: args
                .get("focus_after")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
        }),
        "add_pane_to_column" => Some(WmAction::AddPaneToColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_column" => Some(WmAction::DeleteColumn {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "zoom_column_at_index" => Some(WmAction::ZoomColumnAtIndex {
            ws_idx: get_usize(args, "ws_idx")?,
            col_idx: get_usize(args, "col_idx")?,
        }),
        "delete_workspace" => Some(WmAction::DeleteWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        // Scrollback (parameterized)
        "scrollback_line_up" => Some(WmAction::ScrollbackLineUp {
            amount: get_usize(args, "amount").unwrap_or(1),
        }),
        "scrollback_line_down" => Some(WmAction::ScrollbackLineDown {
            amount: get_usize(args, "amount").unwrap_or(1),
        }),
        "scroll_to_offset" => Some(WmAction::ScrollToOffset {
            rows: get_usize(args, "rows")?,
        }),
        // Font zoom (RPC): `step` = in|out|reset; pane variant optionally targets
        // a specific `pane_id` (omitted → focused pane).
        "app_font_zoom" => Some(WmAction::AppFontZoom {
            step: get_enum(args, "step")?,
        }),
        "pane_terminal_font_zoom" => Some(WmAction::PaneTerminalFontZoom {
            pane_id: get_u64(args, "pane_id").map(PaneId),
            step: get_enum(args, "step")?,
        }),
        // Terminal (RPC): the target is a `terminal` id, a `pane_id`, or — with neither — the
        // focused pane's terminal.
        "terminal_run" => Some(WmAction::TerminalRun {
            pane_id: get_u64(args, "pane_id").map(PaneId),
            terminal: get_u64(args, "terminal"),
            text: get_string(args, "text")?,
            enter: args
                .get("enter")
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }),
        "terminal_kill" => Some(WmAction::TerminalKill {
            pane_id: get_u64(args, "pane_id").map(PaneId),
            terminal: get_u64(args, "terminal"),
        }),

        // ── Context-menu / sidebar targets (context-menu-5) ──
        // Reachable by NAME so a menu entry — built-in or plugin-contributed — carries an `Intent`
        // rather than a `WmAction` (the enum is closed to plugins). Same constructor the config
        // binding and the RPC command use.
        "add_column_to_workspace" => Some(WmAction::AddColumnToWorkspace {
            ws_idx: get_usize(args, "ws_idx")?,
        }),
        "open_link" => Some(WmAction::OpenLink {
            url: get_string(args, "url")?,
        }),

        // ── Chrome container placement (plugin-04/T1) ──
        // These carry DOTTED, namespaced ids — unlike every other built-in, whose config name is
        // snake_case. That asymmetry is deliberate: the dotted namespace is the id scheme plugins
        // use (`plugin.docker.restart`), and chrome placement is the first host capability a plugin
        // is meant to drive by name. Renaming the ~115 existing snake_case built-ins to a dotted
        // scheme is a MIGRATION nobody has decided on — do not start it here by adding aliases.
        "chrome.container.move_to_region" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: get_enum(args, "region")?,
        }),
        // Conveniences over move_to_region with the region fixed: what a menu item or a keybinding
        // ("send this container to the right sidebar") actually wants to say.
        "chrome.container.move_left_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::LeftSidebar,
        }),
        "chrome.container.move_right_sidebar" => Some(WmAction::MoveContainerToRegion {
            container_id: get_string(args, "container_id")?,
            region: crate::chrome::RegionId::RightSidebar,
        }),
        // `before_id` is OPTIONAL: omitting it moves the container to the END of its region.
        "chrome.container.reorder_before" => Some(WmAction::ReorderContainerBefore {
            container_id: get_string(args, "container_id")?,
            before_id: get_string(args, "before_id"),
        }),
        "chrome.container.reorder_after" => Some(WmAction::ReorderContainerAfter {
            container_id: get_string(args, "container_id")?,
            after_id: get_string(args, "after_id")?,
        }),
        // `dock` is OPTIONAL, which is what makes one action serve both doors: a keybinding cannot
        // name a container, so a bare binding picks one by letter; a caller that knows which dock it
        // wants says so and skips the pick. An action with a *required* argument could not be bound
        // bare at all (F003/P010/T006).
        "focus_dock" => Some(WmAction::FocusDock {
            dock: get_string(args, "dock"),
        }),
        "toggle_dock" => Some(WmAction::ToggleDock {
            dock: get_string(args, "dock"),
        }),

        "show_layer" => Some(WmAction::ShowLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),
        "hide_layer" => Some(WmAction::HideLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),
        "toggle_layer" => Some(WmAction::ToggleLayer {
            name: get_string(args, "name"),
            dock: get_string(args, "dock"),
        }),

        // Both OPTIONAL, and they compose into one prefilled query — see the variant.
        "command_palette" => Some(WmAction::CommandPalette {
            mode: get_string(args, "mode"),
            query: get_string(args, "query"),
        }),

        "clear_search_history" => Some(WmAction::ClearSearchHistory {
            scope: get_string(args, "scope"),
        }),
        "clear_search_ranking" => Some(WmAction::ClearSearchRanking {
            scope: get_string(args, "scope"),
        }),

        "spawn_command" => Some(WmAction::SpawnCommand {
            command: get_string(args, "command")?,
            kind: get_enum(args, "kind").unwrap_or(SpawnKind::Terminal),
            float: args
                .get("float")
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            close_policy: PaneClosePolicy {
                close_pane: args
                    .get("close_pane")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
                keep_on_error: args
                    .get("keep_on_error")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
                keep_on_success: args
                    .get("keep_on_success")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(false),
            },
            cwd: get_string(args, "cwd"),
        }),
        "submit_overlay" => Some(WmAction::SubmitOverlay {
            overlay: None,
            action: get_string(args, "action")?,
        }),
        "set_region_visible" => Some(WmAction::SetRegionVisible {
            region: get_enum(args, "region")?,
            visible: get_enum_or_default(args, "visible")?,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Some(WmAction::ScrollToOffset { rows: 42 })
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
    /// pick) and a `dock` argument for a caller that already knows the answer (F003/P011/T020).
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
}
