//! RPC command parser — text-based remote control interface.
//!
//! Commands are simple text strings:
//!   focus-pane <pane_id>
//!   focus-workspace <ws_idx>
//!   split-h | split-v
//!   close-pane | close-pane-id <pane_id>
//!   float | float-at <pane_id> <x> <y> <w> <h>
//!   resize <column|pane> <axis> <amount>
//!   move-pane <pane_id> <target_col>
//!   move-pane-to-workspace <pane_id> <ws_idx>
//!   move-pane-to-column <pane_id> <ws_idx> <col_idx>
//!   swap <a_id> <b_id>
//!   focus-left | focus-right | focus-up | focus-down
//!   workspace-next | workspace-prev
//!   sidebar-left | sidebar-right
//!   collapse-current-workspace | expand-current-workspace | toggle-current-workspace-collapsed
//!   collapse-current-column | expand-current-column | toggle-current-column-collapsed
//!   rename-pane | rename-workspace
//!   command-palette

use crate::input::{ResizeTarget, WmAction};

/// Errors that can occur when parsing an RPC command.
// Transitional: will be used by the RPC server / socket listener in Phase 5.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcError {
    UnknownCommand(String),
    MissingArgument { cmd: String, arg: String },
    ParseInt { cmd: String, value: String },
    ParseFloat { cmd: String, value: String },
    /// The app is not yet initialized (no state available).
    NotInitialized,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RpcError::UnknownCommand(cmd) => write!(f, "unknown command: {cmd}"),
            RpcError::MissingArgument { cmd, arg } => {
                write!(f, "command '{cmd}' missing argument: {arg}")
            }
            RpcError::ParseInt { cmd, value } => {
                write!(f, "command '{cmd}' expected integer, got: {value}")
            }
            RpcError::ParseFloat { cmd, value } => {
                write!(f, "command '{cmd}' expected float, got: {value}")
            }
            RpcError::NotInitialized => write!(f, "app not initialized"),
        }
    }
}

impl std::error::Error for RpcError {}

/// Parse a text RPC command into a `WmAction`.
///
/// Example commands:
/// ```
/// use heca::rpc::parse_rpc_command;
/// use heca::input::WmAction;
///
/// assert_eq!(
///     parse_rpc_command("focus-pane 42"),
///     Ok(WmAction::FocusPane { pane_id: 42 }),
/// );
/// ```
// Transitional: will be used by the RPC server / socket listener in Phase 5.
#[allow(dead_code)]
pub fn parse_rpc_command(input: &str) -> Result<WmAction, RpcError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(RpcError::UnknownCommand("<empty>".to_string()));
    }

    let mut parts = input.split_whitespace();
    let cmd = parts.next().unwrap().to_lowercase();

    macro_rules! expect_arg {
        ($name:literal) => {
            parts.next().ok_or_else(|| RpcError::MissingArgument {
                cmd: cmd.clone(),
                arg: $name.to_string(),
            })?
        };
    }

    macro_rules! parse_u64 {
        ($value:expr, $name:literal) => {
            $value.parse::<u64>().map_err(|_| RpcError::ParseInt {
                cmd: cmd.clone(),
                value: $value.to_string(),
            })?
        };
    }

    macro_rules! parse_usize {
        ($value:expr, $name:literal) => {
            $value.parse::<usize>().map_err(|_| RpcError::ParseInt {
                cmd: cmd.clone(),
                value: $value.to_string(),
            })?
        };
    }

    macro_rules! parse_f64 {
        ($value:expr, $name:literal) => {
            $value.parse::<f64>().map_err(|_| RpcError::ParseFloat {
                cmd: cmd.clone(),
                value: $value.to_string(),
            })?
        };
    }

    match cmd.as_str() {
        "focus-pane" => {
            let arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(arg, "pane_id");
            Ok(WmAction::FocusPane { pane_id })
        }
        "focus-workspace" => {
            let arg = expect_arg!("ws_idx");
            let ws_idx = parse_usize!(arg, "ws_idx");
            Ok(WmAction::FocusWorkspace { ws_idx })
        }
        "focus-left" => Ok(WmAction::FocusLeft),
        "focus-right" => Ok(WmAction::FocusRight),
        "focus-up" => Ok(WmAction::FocusUp),
        "focus-down" => Ok(WmAction::FocusDown),
        "workspace-next" => Ok(WmAction::WorkspaceNext),
        "workspace-prev" => Ok(WmAction::WorkspacePrev),
        "split-h" | "split-horizontal" => Ok(WmAction::SplitHorizontal),
        "zoom-column" | "zoom-col" => Ok(WmAction::ZoomColumn),
        "split-v" | "split-vertical" => Ok(WmAction::SplitVertical),
        "close-pane" => Ok(WmAction::ClosePane),
        "close-pane-id" => {
            let arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(arg, "pane_id");
            Ok(WmAction::ClosePaneById { pane_id })
        }
        "float" => Ok(WmAction::Float),
        "float-at" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(pane_arg, "pane_id");
            let x_arg = expect_arg!("x");
            let x = parse_f64!(x_arg, "x");
            let y_arg = expect_arg!("y");
            let y = parse_f64!(y_arg, "y");
            let w_arg = expect_arg!("width");
            let w = parse_f64!(w_arg, "width");
            let h_arg = expect_arg!("height");
            let h = parse_f64!(h_arg, "height");
            Ok(WmAction::FloatAt {
                pane_id,
                x,
                y,
                width: w,
                height: h,
            })
        }
        "resize" => {
            let target_arg = expect_arg!("target");
            let target = match target_arg.to_lowercase().as_str() {
                "column" | "col" => ResizeTarget::Column,
                "pane" => ResizeTarget::Pane,
                _ => {
                    return Err(RpcError::ParseInt {
                        cmd: cmd.clone(),
                        value: target_arg.to_string(),
                    });
                }
            };
            let axis_arg = expect_arg!("axis");
            let axis = match axis_arg.to_lowercase().as_str() {
                "x" | "horizontal" | "width" => crate::input::ResizeAxis::X,
                "y" | "vertical" | "height" => crate::input::ResizeAxis::Y,
                _ => {
                    return Err(RpcError::ParseInt {
                        cmd: cmd.clone(),
                        value: axis_arg.to_string(),
                    });
                }
            };
            let amount_arg = expect_arg!("amount");
            let amount = amount_arg
                .parse::<f64>()
                .map_err(|_| RpcError::ParseFloat {
                    cmd: cmd.clone(),
                    value: amount_arg.to_string(),
                })?;
            Ok(WmAction::Resize {
                target,
                axis,
                amount,
            })
        }
        "move-pane" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(pane_arg, "pane_id");
            let col_arg = expect_arg!("target_col");
            let target_col = parse_usize!(col_arg, "target_col");
            Ok(WmAction::Move {
                pane_id,
                target_col,
            })
        }
        "move-pane-to-workspace" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(pane_arg, "pane_id");
            let ws_arg = expect_arg!("ws_idx");
            let ws_idx = parse_usize!(ws_arg, "ws_idx");
            Ok(WmAction::MovePaneToWorkspace { pane_id, ws_idx })
        }
        "move-pane-to-column" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = parse_u64!(pane_arg, "pane_id");
            let ws_arg = expect_arg!("ws_idx");
            let ws_idx = parse_usize!(ws_arg, "ws_idx");
            let col_arg = expect_arg!("col_idx");
            let col_idx = parse_usize!(col_arg, "col_idx");
            Ok(WmAction::MovePaneToColumn {
                pane_id,
                ws_idx,
                col_idx,
            })
        }
        "swap" => {
            let a_arg = expect_arg!("a_id");
            let a_id = parse_u64!(a_arg, "a_id");
            let b_arg = expect_arg!("b_id");
            let b_id = parse_u64!(b_arg, "b_id");
            Ok(WmAction::Swap { a_id, b_id })
        }
        "rename-pane" => Ok(WmAction::RenamePane),
        "rename-column" => Ok(WmAction::RenameColumn),
        "rename-workspace" => Ok(WmAction::RenameWorkspace),
        "sidebar-left" => Ok(WmAction::SidebarLeft),
        "sidebar-right" => Ok(WmAction::SidebarRight),
        "collapse-current-workspace" => Ok(WmAction::CollapseCurrentWorkspace),
        "expand-current-workspace" => Ok(WmAction::ExpandCurrentWorkspace),
        "toggle-current-workspace-collapsed" => {
            Ok(WmAction::ToggleCurrentWorkspaceCollapsed)
        }
        "collapse-current-column" => Ok(WmAction::CollapseCurrentColumn),
        "expand-current-column" => Ok(WmAction::ExpandCurrentColumn),
        "toggle-current-column-collapsed" => Ok(WmAction::ToggleCurrentColumnCollapsed),
        "command-palette" => Ok(WmAction::CommandPalette),
        _ => Err(RpcError::UnknownCommand(cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{ResizeTarget, WmAction};

    #[test]
    fn test_focus_pane() {
        assert_eq!(
            parse_rpc_command("focus-pane 42"),
            Ok(WmAction::FocusPane { pane_id: 42 }),
        );
    }

    #[test]
    fn test_focus_workspace() {
        assert_eq!(
            parse_rpc_command("focus-workspace 1"),
            Ok(WmAction::FocusWorkspace { ws_idx: 1 }),
        );
    }

    #[test]
    fn test_focus_directions() {
        assert_eq!(parse_rpc_command("focus-left"), Ok(WmAction::FocusLeft));
        assert_eq!(parse_rpc_command("focus-right"), Ok(WmAction::FocusRight));
        assert_eq!(parse_rpc_command("focus-up"), Ok(WmAction::FocusUp));
        assert_eq!(parse_rpc_command("focus-down"), Ok(WmAction::FocusDown));
    }

    #[test]
    fn test_workspace_nav() {
        assert_eq!(
            parse_rpc_command("workspace-next"),
            Ok(WmAction::WorkspaceNext)
        );
        assert_eq!(
            parse_rpc_command("workspace-prev"),
            Ok(WmAction::WorkspacePrev)
        );
    }

    #[test]
    fn test_split_commands() {
        assert_eq!(parse_rpc_command("split-h"), Ok(WmAction::SplitHorizontal));
        assert_eq!(
            parse_rpc_command("split-horizontal"),
            Ok(WmAction::SplitHorizontal)
        );
        assert_eq!(parse_rpc_command("split-v"), Ok(WmAction::SplitVertical));
        assert_eq!(
            parse_rpc_command("split-vertical"),
            Ok(WmAction::SplitVertical)
        );
    }

    #[test]
    fn test_rename_column_command() {
        assert_eq!(
            parse_rpc_command("rename-column"),
            Ok(WmAction::RenameColumn)
        );
    }

    #[test]
    fn test_zoom_column_commands() {
        assert_eq!(parse_rpc_command("zoom-column"), Ok(WmAction::ZoomColumn));
        assert_eq!(parse_rpc_command("zoom-col"), Ok(WmAction::ZoomColumn));
    }

    #[test]
    fn test_close_pane() {
        assert_eq!(parse_rpc_command("close-pane"), Ok(WmAction::ClosePane));
    }

    #[test]
    fn test_close_pane_by_id() {
        assert_eq!(
            parse_rpc_command("close-pane-id 7"),
            Ok(WmAction::ClosePaneById { pane_id: 7 }),
        );
    }

    #[test]
    fn test_float() {
        assert_eq!(parse_rpc_command("float"), Ok(WmAction::Float));
    }

    #[test]
    fn test_float_at() {
        assert_eq!(
            parse_rpc_command("float-at 3 10.5 20.0 300 200"),
            Ok(WmAction::FloatAt {
                pane_id: 3,
                x: 10.5,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            }),
        );
    }

    #[test]
    fn test_resize() {
        assert_eq!(
            parse_rpc_command("resize column x 50"),
            Ok(WmAction::Resize {
                target: ResizeTarget::Column,
                axis: crate::input::ResizeAxis::X,
                amount: 50.0,
            }),
        );
        assert_eq!(
            parse_rpc_command("resize pane y -25"),
            Ok(WmAction::Resize {
                target: ResizeTarget::Pane,
                axis: crate::input::ResizeAxis::Y,
                amount: -25.0,
            }),
        );
    }

    #[test]
    fn test_move_pane() {
        assert_eq!(
            parse_rpc_command("move-pane 5 2"),
            Ok(WmAction::Move {
                pane_id: 5,
                target_col: 2,
            }),
        );
    }

    #[test]
    fn test_swap() {
        assert_eq!(
            parse_rpc_command("swap 1 2"),
            Ok(WmAction::Swap { a_id: 1, b_id: 2 }),
        );
    }

    #[test]
    fn test_move_pane_to_workspace() {
        assert_eq!(
            parse_rpc_command("move-pane-to-workspace 5 1"),
            Ok(WmAction::MovePaneToWorkspace {
                pane_id: 5,
                ws_idx: 1
            }),
        );
    }

    #[test]
    fn test_move_pane_to_column() {
        assert_eq!(
            parse_rpc_command("move-pane-to-column 5 0 2"),
            Ok(WmAction::MovePaneToColumn {
                pane_id: 5,
                ws_idx: 0,
                col_idx: 2
            }),
        );
    }

    #[test]
    fn test_rename_commands() {
        assert_eq!(parse_rpc_command("rename-pane"), Ok(WmAction::RenamePane));
        assert_eq!(
            parse_rpc_command("rename-workspace"),
            Ok(WmAction::RenameWorkspace)
        );
    }

    #[test]
    fn test_sidebar_commands() {
        assert_eq!(parse_rpc_command("sidebar-left"), Ok(WmAction::SidebarLeft));
        assert_eq!(
            parse_rpc_command("sidebar-right"),
            Ok(WmAction::SidebarRight)
        );
        assert_eq!(
            parse_rpc_command("collapse-current-workspace"),
            Ok(WmAction::CollapseCurrentWorkspace)
        );
        assert_eq!(
            parse_rpc_command("expand-current-workspace"),
            Ok(WmAction::ExpandCurrentWorkspace)
        );
        assert_eq!(
            parse_rpc_command("toggle-current-workspace-collapsed"),
            Ok(WmAction::ToggleCurrentWorkspaceCollapsed)
        );
        assert_eq!(
            parse_rpc_command("collapse-current-column"),
            Ok(WmAction::CollapseCurrentColumn)
        );
        assert_eq!(
            parse_rpc_command("expand-current-column"),
            Ok(WmAction::ExpandCurrentColumn)
        );
        assert_eq!(
            parse_rpc_command("toggle-current-column-collapsed"),
            Ok(WmAction::ToggleCurrentColumnCollapsed)
        );
    }

    #[test]
    fn test_command_palette() {
        assert_eq!(
            parse_rpc_command("command-palette"),
            Ok(WmAction::CommandPalette)
        );
    }

    #[test]
    fn test_unknown_command() {
        assert!(
            matches!(
                parse_rpc_command("foobar"),
                Err(RpcError::UnknownCommand(_)),
            ),
            "expected UnknownCommand"
        );
    }

    #[test]
    fn test_missing_argument() {
        assert!(
            matches!(
                parse_rpc_command("focus-pane"),
                Err(RpcError::MissingArgument { .. }),
            ),
            "expected MissingArgument"
        );
    }

    #[test]
    fn test_parse_int_error() {
        assert!(
            matches!(
                parse_rpc_command("focus-pane abc"),
                Err(RpcError::ParseInt { .. }),
            ),
            "expected ParseInt"
        );
    }

    #[test]
    fn test_parse_float_error() {
        assert!(
            matches!(
                parse_rpc_command("float-at 1 x 0 100 100"),
                Err(RpcError::ParseFloat { .. }),
            ),
            "expected ParseFloat"
        );
    }

    #[test]
    fn test_empty_input() {
        assert!(
            matches!(parse_rpc_command(""), Err(RpcError::UnknownCommand(_)),),
            "expected UnknownCommand for empty input"
        );
    }
}
