//! RPC command parser — text-based remote control interface.
//!
//! Commands are simple text strings:
//!   focus-pane <pane_id>
//!   focus-workspace <ws_idx>
//!   split-h | split-v
//!   close-pane | close-pane-id <pane_id>
//!   float | float-at <pane_id> <x> <y> <w> <h>
//!   resize <column|pane> <axis> <amount>
//!   resize-column <col_idx> <delta>
//!   resize-pane-height <col_idx> <pane_idx> <delta>
//!   move-pane <pane_id> <target_col>
//!   move-pane-to-workspace <pane_id> <ws_idx>
//!   move-pane-to-column <pane_id> <ws_idx> <col_idx>
//!   move-pane-left [pane_id]
//!   move-pane-right [pane_id]
//!   swap <a_id> <b_id>
//!   move-column-to-workspace <col_idx> <ws_idx>
//!   move-column <src_ws> <src_col> <dst_ws> <dst_idx>
//!   swap-columns <a_ws> <a_col> <b_ws> <b_col>
//!   focus-left | focus-right | focus-up | focus-down
//!   workspace-next | workspace-prev
//!   sidebar-left | sidebar-right
//!   collapse-current-workspace | expand-current-workspace | toggle-current-workspace-collapsed
//!   collapse-current-column | expand-current-column | toggle-current-column-collapsed
//!   rename-pane | rename-workspace
//!   command-palette
//!   spawn-command [--kind terminal|app|plugin] [--float] [--close-pane]
//!     [--keep-on-error] [--keep-on-success] -- <command...>
//!   enter-selection-mode | clear-selection | copy-selection | paste-clipboard
//!     (selection is a host capability; these commands are reachable from
//!      RPC, keyboard bindings, and future mouse/UI dispatch)

use crate::input::{FontZoomStep, ResizeTarget, SpawnKind, WmAction};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

/// Errors that can occur when parsing an RPC command.
// Transitional: will be used by the RPC server / socket listener in Phase 5.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RpcError {
    UnknownCommand(String),
    MissingArgument {
        cmd: String,
        arg: String,
    },
    MissingSeparator {
        cmd: String,
    },
    ParseInt {
        cmd: String,
        value: String,
    },
    ParseFloat {
        cmd: String,
        value: String,
    },
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
            RpcError::MissingSeparator { cmd } => {
                write!(f, "command '{cmd}' missing '--' separator before command")
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
///     Ok(WmAction::FocusPane { pane_id: PaneId(42) }),
/// );
/// ```
// Transitional: will be used by the RPC server / socket listener in Phase 5.
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
            let pane_id = PaneId(parse_u64!(arg, "pane_id"));
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
        "scroll-view-left" | "scroll-left" => Ok(WmAction::ScrollViewLeft),
        "scroll-view-right" | "scroll-right" => Ok(WmAction::ScrollViewRight),
        "split-v" | "split-vertical" => Ok(WmAction::SplitVertical),
        "close-pane" => Ok(WmAction::ClosePane),
        "close-pane-id" => {
            let arg = expect_arg!("pane_id");
            let pane_id = PaneId(parse_u64!(arg, "pane_id"));
            Ok(WmAction::ClosePaneById { pane_id })
        }
        "float" => Ok(WmAction::Float),
        "float-at" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = PaneId(parse_u64!(pane_arg, "pane_id"));
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
        "resize-column" => {
            let col_arg = expect_arg!("col_idx");
            let col_idx = parse_usize!(col_arg, "col_idx");
            let delta_arg = expect_arg!("delta");
            let delta = parse_f64!(delta_arg, "delta");
            Ok(WmAction::ResizeColumnBy { col_idx, delta })
        }
        "resize-pane-height" => {
            let col_arg = expect_arg!("col_idx");
            let col_idx = parse_usize!(col_arg, "col_idx");
            let pane_arg = expect_arg!("pane_idx");
            let pane_idx = parse_usize!(pane_arg, "pane_idx");
            let delta_arg = expect_arg!("delta");
            let delta = parse_f64!(delta_arg, "delta");
            Ok(WmAction::ResizePaneHeightBy {
                col_idx,
                pane_idx,
                delta,
            })
        }
        "move-pane" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = PaneId(parse_u64!(pane_arg, "pane_id"));
            let col_arg = expect_arg!("target_col");
            let target_col = parse_usize!(col_arg, "target_col");
            Ok(WmAction::Move {
                pane_id,
                target_col,
            })
        }
        "move-pane-to-workspace" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = PaneId(parse_u64!(pane_arg, "pane_id"));
            let ws_arg = expect_arg!("ws_idx");
            let ws_idx = parse_usize!(ws_arg, "ws_idx");
            Ok(WmAction::MovePaneToWorkspace { pane_id, ws_idx })
        }
        "move-pane-to-column" => {
            let pane_arg = expect_arg!("pane_id");
            let pane_id = PaneId(parse_u64!(pane_arg, "pane_id"));
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
        "move-pane-left" => {
            // Optional pane_id: omitted ⇒ active pane (mirrors the keybind).
            let pane_id = match parts.next() {
                Some(arg) => Some(PaneId(parse_u64!(arg, "pane_id"))),
                None => None,
            };
            Ok(WmAction::MovePaneLeft { pane_id })
        }
        "move-pane-right" => {
            let pane_id = match parts.next() {
                Some(arg) => Some(PaneId(parse_u64!(arg, "pane_id"))),
                None => None,
            };
            Ok(WmAction::MovePaneRight { pane_id })
        }
        "swap" => {
            let a_arg = expect_arg!("a_id");
            let a_id = PaneId(parse_u64!(a_arg, "a_id"));
            let b_arg = expect_arg!("b_id");
            let b_id = PaneId(parse_u64!(b_arg, "b_id"));
            Ok(WmAction::Swap { a_id, b_id })
        }
        "move-column-to-workspace" => {
            let col_arg = expect_arg!("col_idx");
            let col_idx = parse_usize!(col_arg, "col_idx");
            let ws_arg = expect_arg!("ws_idx");
            let ws_idx = parse_usize!(ws_arg, "ws_idx");
            Ok(WmAction::MoveColumnToWorkspace {
                col_idx,
                ws_idx,
                focus: true,
            })
        }
        "move-column" => {
            let src_ws_arg = expect_arg!("src_ws");
            let src_ws = parse_usize!(src_ws_arg, "src_ws");
            let src_col_arg = expect_arg!("src_col");
            let src_col = parse_usize!(src_col_arg, "src_col");
            let dst_ws_arg = expect_arg!("dst_ws");
            let dst_ws = parse_usize!(dst_ws_arg, "dst_ws");
            let dst_idx_arg = expect_arg!("dst_idx");
            let dst_idx = parse_usize!(dst_idx_arg, "dst_idx");
            Ok(WmAction::MoveColumn {
                src_ws,
                src_col,
                dst_ws,
                dst_idx,
                focus: true,
            })
        }
        "swap-columns" => {
            let a_ws_arg = expect_arg!("a_ws");
            let a_ws = parse_usize!(a_ws_arg, "a_ws");
            let a_col_arg = expect_arg!("a_col");
            let a_col = parse_usize!(a_col_arg, "a_col");
            let b_ws_arg = expect_arg!("b_ws");
            let b_ws = parse_usize!(b_ws_arg, "b_ws");
            let b_col_arg = expect_arg!("b_col");
            let b_col = parse_usize!(b_col_arg, "b_col");
            Ok(WmAction::SwapColumns {
                a_ws,
                a_col,
                b_ws,
                b_col,
            })
        }
        "rename-pane" => Ok(WmAction::RenamePane),
        "rename-column" => Ok(WmAction::RenameColumn),
        "rename-workspace" => Ok(WmAction::RenameWorkspace),
        "sidebar-left" => Ok(WmAction::SidebarLeft),
        "sidebar-right" => Ok(WmAction::SidebarRight),
        "collapse-current-workspace" => Ok(WmAction::CollapseCurrentWorkspace),
        "expand-current-workspace" => Ok(WmAction::ExpandCurrentWorkspace),
        "toggle-current-workspace-collapsed" => Ok(WmAction::ToggleCurrentWorkspaceCollapsed),
        "collapse-current-column" => Ok(WmAction::CollapseCurrentColumn),
        "expand-current-column" => Ok(WmAction::ExpandCurrentColumn),
        "toggle-current-column-collapsed" => Ok(WmAction::ToggleCurrentColumnCollapsed),
        "command-palette" => Ok(WmAction::CommandPalette),
        "spawn-command" => {
            let mut kind = SpawnKind::Terminal;
            let mut float = false;
            let mut close_policy = PaneClosePolicy::default();
            let rest: Vec<&str> = parts.collect();
            let split = rest
                .iter()
                .position(|part| *part == "--")
                .ok_or_else(|| RpcError::MissingSeparator { cmd: cmd.clone() })?;

            let mut idx = 0;
            while idx < split {
                match rest[idx] {
                    "--kind" => {
                        let value = rest.get(idx + 1).ok_or_else(|| RpcError::MissingArgument {
                            cmd: cmd.clone(),
                            arg: "kind".to_string(),
                        })?;
                        kind = value
                            .parse()
                            .map_err(|_| RpcError::UnknownCommand((*value).to_string()))?;
                        idx += 2;
                    }
                    "--float" => {
                        float = true;
                        idx += 1;
                    }
                    "--close-pane" => {
                        close_policy.close_pane = true;
                        idx += 1;
                    }
                    "--keep-on-error" => {
                        close_policy.keep_on_error = true;
                        idx += 1;
                    }
                    "--keep-on-success" => {
                        close_policy.keep_on_success = true;
                        idx += 1;
                    }
                    other => return Err(RpcError::UnknownCommand(other.to_string())),
                }
            }

            let command = rest[split + 1..].join(" ");
            if command.trim().is_empty() {
                return Err(RpcError::MissingArgument {
                    cmd: cmd.clone(),
                    arg: "command after '--'".to_string(),
                });
            }

            Ok(WmAction::SpawnCommand {
                command,
                kind,
                float,
                close_policy,
            })
        }
        // Selection (host capability, Task 02). Reachable from RPC, keyboard,
        // and mouse/UI dispatch. `copy-selection` is implemented via the shared
        // host selection model; `paste-clipboard` remains the deferred Phase 10
        // behavior.
        "enter-selection-mode" => Ok(WmAction::EnterSelectionMode),
        "selection-left" => Ok(WmAction::SelectionLeft),
        "selection-right" => Ok(WmAction::SelectionRight),
        "selection-up" => Ok(WmAction::SelectionUp),
        "selection-down" => Ok(WmAction::SelectionDown),
        "clear-selection" => Ok(WmAction::ClearSelection),
        "copy-selection" => Ok(WmAction::CopySelection),
        "paste-clipboard" => Ok(WmAction::PasteClipboard),
        // Scrollback (host capability, Slice 3). Reachable from RPC, keyboard,
        // and mouse/UI dispatch.
        "scrollback-page-up" | "scroll-page-up" => Ok(WmAction::ScrollbackPageUp),
        "scrollback-page-down" | "scroll-page-down" => Ok(WmAction::ScrollbackPageDown),
        "scrollback-line-up" | "scroll-line-up" => Ok(WmAction::ScrollbackLineUp { amount: 1 }),
        "scrollback-line-down" | "scroll-line-down" => Ok(WmAction::ScrollbackLineDown { amount: 1 }),
        "scrollback-to-top" | "scroll-to-top" => Ok(WmAction::ScrollbackToTop),
        "scrollback-to-bottom" | "scroll-to-bottom" => Ok(WmAction::ScrollbackToBottom),
        "exit-scrollback" => Ok(WmAction::ExitScrollback),
        // Direct scroll (non-prefix, no selection mode entry).
        // These use distinct names from the scrollback variants above.
        "direct-scroll-page-up" => Ok(WmAction::ScrollPageUp),
        "direct-scroll-page-down" => Ok(WmAction::ScrollPageDown),
        "direct-scroll-line-up" => Ok(WmAction::ScrollLineUp),
        "direct-scroll-line-down" => Ok(WmAction::ScrollLineDown),
        "direct-scroll-to-top" => Ok(WmAction::ScrollToTop),
        "direct-scroll-to-bottom" => Ok(WmAction::ScrollToBottom),
        "direct-scroll-to-offset" => {
            let rows_str = expect_arg!("rows");
            let rows = rows_str.parse::<usize>().map_err(|_| RpcError::ParseInt {
                cmd: cmd.clone(),
                value: rows_str.to_string(),
            })?;
            Ok(WmAction::ScrollToOffset { rows })
        }
        // Font zoom: `<in|out|reset>` step; the pane variant optionally takes a
        // trailing pane_id (omitted → focused pane).
        "app-font" => {
            let step_arg = expect_arg!("step");
            let step = step_arg
                .parse::<FontZoomStep>()
                .map_err(|_| RpcError::UnknownCommand(step_arg.to_string()))?;
            Ok(WmAction::AppFontZoom { step })
        }
        "pane-terminal-font" => {
            let step_arg = expect_arg!("step");
            let step = step_arg
                .parse::<FontZoomStep>()
                .map_err(|_| RpcError::UnknownCommand(step_arg.to_string()))?;
            let pane_id = match parts.next() {
                Some(id) => Some(PaneId(parse_u64!(id, "pane_id"))),
                None => None,
            };
            Ok(WmAction::PaneTerminalFontZoom { pane_id, step })
        }
        "open-link" => {
            // The URL is the remainder of the line (URLs are normally one token,
            // but join defensively in case of stray spaces).
            let rest: Vec<&str> = parts.collect();
            if rest.is_empty() {
                return Err(RpcError::MissingArgument {
                    cmd: cmd.clone(),
                    arg: "url".to_string(),
                });
            }
            Ok(WmAction::OpenLink {
                url: rest.join(" "),
            })
        }
        _ => Err(RpcError::UnknownCommand(cmd)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{FontZoomStep, ResizeTarget, WmAction};
    use heca_core::layout::PaneId;

    #[test]
    fn test_focus_pane() {
        assert_eq!(
            parse_rpc_command("focus-pane 42"),
            Ok(WmAction::FocusPane {
                pane_id: PaneId(42)
            }),
        );
    }

    #[test]
    fn test_move_column() {
        assert_eq!(
            parse_rpc_command("move-column 0 2 1 0"),
            Ok(WmAction::MoveColumn {
                src_ws: 0,
                src_col: 2,
                dst_ws: 1,
                dst_idx: 0,
                focus: true
            }),
        );
        assert!(matches!(
            parse_rpc_command("move-column 0 2 1"),
            Err(RpcError::MissingArgument { .. }),
        ));
    }

    #[test]
    fn test_swap_columns() {
        assert_eq!(
            parse_rpc_command("swap-columns 0 1 2 3"),
            Ok(WmAction::SwapColumns {
                a_ws: 0,
                a_col: 1,
                b_ws: 2,
                b_col: 3
            }),
        );
    }

    #[test]
    fn test_move_column_to_workspace() {
        assert_eq!(
            parse_rpc_command("move-column-to-workspace 2 1"),
            Ok(WmAction::MoveColumnToWorkspace {
                col_idx: 2,
                ws_idx: 1,
                focus: true
            }),
        );
        assert!(matches!(
            parse_rpc_command("move-column-to-workspace 2"),
            Err(RpcError::MissingArgument { .. }),
        ));
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
            Ok(WmAction::ClosePaneById { pane_id: PaneId(7) }),
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
                pane_id: PaneId(3),
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
    fn test_resize_column_and_pane_height() {
        assert_eq!(
            parse_rpc_command("resize-column 2 0.05"),
            Ok(WmAction::ResizeColumnBy {
                col_idx: 2,
                delta: 0.05,
            }),
        );
        assert_eq!(
            parse_rpc_command("resize-pane-height 1 0 -40"),
            Ok(WmAction::ResizePaneHeightBy {
                col_idx: 1,
                pane_idx: 0,
                delta: -40.0,
            }),
        );
        // Missing args are a parse error, not a panic.
        assert!(parse_rpc_command("resize-column 2").is_err());
        assert!(parse_rpc_command("resize-pane-height 1 0").is_err());
    }

    #[test]
    fn test_move_pane() {
        assert_eq!(
            parse_rpc_command("move-pane 5 2"),
            Ok(WmAction::Move {
                pane_id: PaneId(5),
                target_col: 2,
            }),
        );
    }

    #[test]
    fn test_move_pane_left_right() {
        // With an explicit pane id (pane-header button / RPC).
        assert_eq!(
            parse_rpc_command("move-pane-left 7"),
            Ok(WmAction::MovePaneLeft {
                pane_id: Some(PaneId(7))
            }),
        );
        assert_eq!(
            parse_rpc_command("move-pane-right 7"),
            Ok(WmAction::MovePaneRight {
                pane_id: Some(PaneId(7))
            }),
        );
        // Without an id ⇒ active pane (mirrors the keybind).
        assert_eq!(
            parse_rpc_command("move-pane-left"),
            Ok(WmAction::MovePaneLeft { pane_id: None }),
        );
        assert_eq!(
            parse_rpc_command("move-pane-right"),
            Ok(WmAction::MovePaneRight { pane_id: None }),
        );
    }

    #[test]
    fn test_swap() {
        assert_eq!(
            parse_rpc_command("swap 1 2"),
            Ok(WmAction::Swap {
                a_id: PaneId(1),
                b_id: PaneId(2)
            }),
        );
    }

    #[test]
    fn test_move_pane_to_workspace() {
        assert_eq!(
            parse_rpc_command("move-pane-to-workspace 5 1"),
            Ok(WmAction::MovePaneToWorkspace {
                pane_id: PaneId(5),
                ws_idx: 1
            }),
        );
    }

    #[test]
    fn test_move_pane_to_column() {
        assert_eq!(
            parse_rpc_command("move-pane-to-column 5 0 2"),
            Ok(WmAction::MovePaneToColumn {
                pane_id: PaneId(5),
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
    fn test_spawn_command_with_options() {
        assert_eq!(
            parse_rpc_command(
                "spawn-command --kind terminal --float --close-pane --keep-on-error -- lazygit"
            ),
            Ok(WmAction::SpawnCommand {
                command: "lazygit".to_string(),
                kind: SpawnKind::Terminal,
                float: true,
                close_policy: PaneClosePolicy {
                    close_pane: true,
                    keep_on_error: true,
                    keep_on_success: false,
                },
            })
        );
    }

    #[test]
    fn test_spawn_command_reports_missing_separator() {
        assert!(matches!(
            parse_rpc_command("spawn-command --kind terminal lazygit"),
            Err(RpcError::MissingSeparator { .. })
        ));
    }

    #[test]
    fn test_spawn_command_reports_missing_command_after_separator() {
        assert!(matches!(
            parse_rpc_command("spawn-command --kind terminal --"),
            Err(RpcError::MissingArgument { arg, .. }) if arg == "command after '--'"
        ));
    }

    #[test]
    fn test_selection_commands() {
        // Selection is a host capability (Task 02) and must be reachable
        // from RPC, not just from keyboard bindings.
        assert_eq!(
            parse_rpc_command("enter-selection-mode"),
            Ok(WmAction::EnterSelectionMode)
        );
        assert_eq!(
            parse_rpc_command("selection-left"),
            Ok(WmAction::SelectionLeft)
        );
        assert_eq!(
            parse_rpc_command("selection-right"),
            Ok(WmAction::SelectionRight)
        );
        assert_eq!(parse_rpc_command("selection-up"), Ok(WmAction::SelectionUp));
        assert_eq!(
            parse_rpc_command("selection-down"),
            Ok(WmAction::SelectionDown)
        );
        assert_eq!(
            parse_rpc_command("clear-selection"),
            Ok(WmAction::ClearSelection)
        );
        assert_eq!(
            parse_rpc_command("copy-selection"),
            Ok(WmAction::CopySelection)
        );
        assert_eq!(
            parse_rpc_command("paste-clipboard"),
            Ok(WmAction::PasteClipboard)
        );
    }

    #[test]
    fn test_scrollback_commands() {
        // Scrollback (Slice 3) must be reachable from RPC, not just keyboard.
        assert_eq!(
            parse_rpc_command("scrollback-page-up"),
            Ok(WmAction::ScrollbackPageUp)
        );
        assert_eq!(
            parse_rpc_command("scroll-page-up"),
            Ok(WmAction::ScrollbackPageUp)
        );
        assert_eq!(
            parse_rpc_command("scrollback-page-down"),
            Ok(WmAction::ScrollbackPageDown)
        );
        assert_eq!(
            parse_rpc_command("scroll-page-down"),
            Ok(WmAction::ScrollbackPageDown)
        );
        assert_eq!(
            parse_rpc_command("scrollback-line-up"),
            Ok(WmAction::ScrollbackLineUp { amount: 1 })
        );
        assert_eq!(
            parse_rpc_command("scroll-line-up"),
            Ok(WmAction::ScrollbackLineUp { amount: 1 })
        );
        assert_eq!(
            parse_rpc_command("scrollback-line-down"),
            Ok(WmAction::ScrollbackLineDown { amount: 1 })
        );
        assert_eq!(
            parse_rpc_command("scroll-line-down"),
            Ok(WmAction::ScrollbackLineDown { amount: 1 })
        );
        assert_eq!(
            parse_rpc_command("scrollback-to-top"),
            Ok(WmAction::ScrollbackToTop)
        );
        assert_eq!(
            parse_rpc_command("scroll-to-top"),
            Ok(WmAction::ScrollbackToTop)
        );
        assert_eq!(
            parse_rpc_command("scrollback-to-bottom"),
            Ok(WmAction::ScrollbackToBottom)
        );
        assert_eq!(
            parse_rpc_command("scroll-to-bottom"),
            Ok(WmAction::ScrollbackToBottom)
        );
        assert_eq!(
            parse_rpc_command("exit-scrollback"),
            Ok(WmAction::ExitScrollback)
        );
        // Direct scroll (non-prefix, no selection mode entry).
        assert_eq!(
            parse_rpc_command("direct-scroll-page-up"),
            Ok(WmAction::ScrollPageUp)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-page-down"),
            Ok(WmAction::ScrollPageDown)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-line-up"),
            Ok(WmAction::ScrollLineUp)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-line-down"),
            Ok(WmAction::ScrollLineDown)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-to-top"),
            Ok(WmAction::ScrollToTop)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-to-bottom"),
            Ok(WmAction::ScrollToBottom)
        );
        assert_eq!(
            parse_rpc_command("direct-scroll-to-offset 42"),
            Ok(WmAction::ScrollToOffset { rows: 42 })
        );
    }

    #[test]
    fn test_font_zoom_commands() {
        assert_eq!(
            parse_rpc_command("app-font in"),
            Ok(WmAction::AppFontZoom {
                step: FontZoomStep::In
            })
        );
        assert_eq!(
            parse_rpc_command("app-font reset"),
            Ok(WmAction::AppFontZoom {
                step: FontZoomStep::Reset
            })
        );
        // Pane variant: optional trailing pane_id (omitted → focused pane).
        assert_eq!(
            parse_rpc_command("pane-terminal-font out"),
            Ok(WmAction::PaneTerminalFontZoom {
                pane_id: None,
                step: FontZoomStep::Out
            })
        );
        assert_eq!(
            parse_rpc_command("pane-terminal-font in 7"),
            Ok(WmAction::PaneTerminalFontZoom {
                pane_id: Some(PaneId(7)),
                step: FontZoomStep::In
            })
        );
        // Unknown step is rejected.
        assert!(matches!(
            parse_rpc_command("app-font sideways"),
            Err(RpcError::UnknownCommand(_))
        ));
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

    #[test]
    fn test_open_link() {
        assert_eq!(
            parse_rpc_command("open-link https://example.com/x"),
            Ok(WmAction::OpenLink {
                url: "https://example.com/x".to_string()
            })
        );
        assert!(
            matches!(
                parse_rpc_command("open-link"),
                Err(RpcError::MissingArgument { .. })
            ),
            "open-link with no URL is a missing-argument error"
        );
    }
}
