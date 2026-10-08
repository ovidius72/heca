//! Actions on a terminal and its viewport: scrollback, font zoom, running and ending a program, links, spawning a command.

use super::{get_enum, get_string, get_u64, get_usize};
use crate::input::{SpawnKind, WmAction};
use heca_core::layout::PaneId;
use heca_core::runtime::PaneClosePolicy;

pub(super) fn build_terminal(
    name: &str,
    args: &std::collections::HashMap<String, String>,
) -> Option<WmAction> {
    match name {
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
        "open_link" => Some(WmAction::OpenLink {
            url: get_string(args, "url")?,
        }),

        // ── Chrome container placement (plugin-04/T1) ──
        // These carry DOTTED, namespaced ids — unlike every other built-in, whose config name is
        // snake_case. That asymmetry is deliberate: the dotted namespace is the id scheme plugins
        // use (`plugin.docker.restart`), and chrome placement is the first host capability a plugin
        // is meant to drive by name. Renaming the ~115 existing snake_case built-ins to a dotted
        // scheme is a MIGRATION nobody has decided on — do not start it here by adding aliases.
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
        _ => None,
    }
}
