//! The built-in Pane actions, in the order they are listed.

use super::ActionDescriptor;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    // ── Font zoom ──
    ActionDescriptor {
        name: "pane_terminal_font_increase",
        label: "Increase Terminal Font",
        description: "Increase the focused pane's terminal font size.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "pane_terminal_font_decrease",
        label: "Decrease Terminal Font",
        description: "Decrease the focused pane's terminal font size.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "pane_terminal_font_reset",
        label: "Reset Terminal Font",
        description: "Reset the focused pane to follow the app-wide font size.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "close",
        label: "Close Pane",
        description: "Close the active pane.",
        // Remove/close pane; pairs with add-pane's FolderSimplePlus (both act on a
        // pane "slot" in a column).
        icon: Some(Glyph::FolderSimpleMinus),
        args: &[],
    },
    ActionDescriptor {
        name: "float",
        label: "Toggle Float",
        description: "Toggle the active pane between tiling and floating.",
        icon: Some(Glyph::Cards),
        args: &[],
    },
    ActionDescriptor {
        name: "pane_select",
        label: "Quick-Select Pane",
        description: "Press a letter to focus it.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "open_link",
        label: "Open Link",
        description: "Open the hyperlink in the OS default handler.",
        // Constructed with a URL (mouse/HintKey/selection/menu); no global key.
        icon: Some(Glyph::ArrowRight),
        args: &[ArgDescriptor::required(
            "url",
            ArgKind::Text,
            "The link to open.",
        )],
    },
    ActionDescriptor {
        name: "follow_link",
        label: "Follow Link",
        description: "Press a letter to open the link.",
        icon: Some(Glyph::GitBranch),
        args: &[],
    },
    // Pick-mode prompts (`description`) double as the in-progress pick text shown
    // in `InputMode::pending_pick()` — single source of truth, not duplicated. The
    // "+ focus" variants make the focus-follow difference explicit.
    ActionDescriptor {
        name: "swap_pane",
        label: "Quick-Swap Pane",
        description: "Select a pane to swap with — focus stays where it is.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "swap_and_focus_pane",
        label: "Swap and Focus",
        description: "Select a pane to swap with, then follow focus to it.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "pane_take",
        label: "Take Pane",
        description: "Select a pane to pull into the active column — focus stays where it is.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "pane_take_and_focus",
        label: "Take and Focus",
        description: "Select a pane to pull into the active column, then focus it.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "rename_pane",
        label: "Rename Pane",
        description: "Rename the active pane/tab.",
        icon: Some(Glyph::NotePencil),
        args: &[],
    },
    ActionDescriptor {
        name: "reset_pane_name",
        label: "Use Process Name",
        description: "Clear the pane's custom name, reverting to the program name.",
        icon: Some(Glyph::Backspace),
        args: &[],
    },
    // ── Scrollback (host terminal viewport) ──
    ActionDescriptor {
        name: "scrollback_page_up",
        label: "Scrollback Page Up",
        description: "Scroll the terminal viewport up by one page and enter selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scrollback_page_down",
        label: "Scrollback Page Down",
        description: "Scroll the terminal viewport down by one page and enter selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scrollback_line_up",
        label: "Scrollback Line Up",
        description: "Scroll the terminal viewport up by a configurable number of lines (selection mode).",
        icon: None,
        args: &[ArgDescriptor::optional(
            "amount",
            ArgKind::Int,
            "Notches to scroll; each is multiplied by `terminal_wheel_scroll_lines`. Default 1.",
        )],
    },
    ActionDescriptor {
        name: "scrollback_line_down",
        label: "Scrollback Line Down",
        description: "Scroll the terminal viewport down by a configurable number of lines (selection mode).",
        icon: None,
        args: &[ArgDescriptor::optional(
            "amount",
            ArgKind::Int,
            "Notches to scroll; each is multiplied by `terminal_wheel_scroll_lines`. Default 1.",
        )],
    },
    ActionDescriptor {
        name: "scrollback_to_top",
        label: "Scrollback to Top",
        description: "Jump the terminal viewport to the top of scrollback history.",
        icon: Some(Glyph::CaretUp),
        args: &[],
    },
    ActionDescriptor {
        name: "scrollback_to_bottom",
        label: "Scrollback to Bottom",
        description: "Snap the terminal viewport to the live bottom (latest output).",
        icon: Some(Glyph::CaretDown),
        args: &[],
    },
    ActionDescriptor {
        name: "exit_scrollback",
        label: "Exit Scrollback",
        description: "Snap to the live bottom, clear selection, and exit selection mode.",
        icon: Some(Glyph::XCircle),
        args: &[],
    },
    // ── Direct scroll (non-prefix, no selection mode entry) ──
    ActionDescriptor {
        name: "scroll_page_up",
        label: "Scroll Page Up",
        description: "Scroll the terminal viewport up by one page immediately. Stays in Normal mode, repeatable.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_page_down",
        label: "Scroll Page Down",
        description: "Scroll the terminal viewport down by one page immediately. Stays in Normal mode, repeatable.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_line_up",
        label: "Scroll Line Up",
        description: "Scroll the terminal viewport up by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_line_down",
        label: "Scroll Line Down",
        description: "Scroll the terminal viewport down by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_to_top",
        label: "Scroll to Top",
        description: "Jump the terminal viewport to the top of scrollback history immediately. Stays in Normal mode, repeatable.",
        icon: Some(Glyph::CaretUp),
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_to_bottom",
        label: "Scroll to Bottom",
        description: "Snap the terminal viewport to the live bottom immediately. Stays in Normal mode, repeatable.",
        icon: Some(Glyph::CaretDown),
        args: &[],
    },
    // ── Horizontal scroll (a chrome container's scroll area; a pane has one axis) ──
    ActionDescriptor {
        name: "scroll_to_offset",
        label: "Scroll to Offset",
        description: "Jump the terminal viewport to an explicit offset in rows above the live bottom. Used by the GUI scrollbar and RPC; no default keybinding.",
        icon: None,
        args: &[ArgDescriptor::required(
            "rows",
            ArgKind::Int,
            "Rows above the live bottom to jump to.",
        )],
    },
    // ── Selection (host capability) ──
    ActionDescriptor {
        name: "enter_selection_mode",
        label: "Enter Selection Mode",
        description: "Enter the host-owned selection input mode. Selection data is driven by surface adapters (mouse, keyboard, RPC).",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "selection_left",
        label: "Selection Left",
        description: "Move the active selection focus one cell left in selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "selection_right",
        label: "Selection Right",
        description: "Move the active selection focus one cell right in selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "selection_up",
        label: "Selection Up",
        description: "Move the active selection focus one row up in selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "selection_down",
        label: "Selection Down",
        description: "Move the active selection focus one row down in selection mode.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "clear_selection",
        label: "Clear Selection",
        description: "Clear the active selection and exit selection mode if active.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "copy_selection",
        label: "Copy Selection",
        description: "Copy the active selection text to the system clipboard.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "paste_clipboard",
        label: "Paste Clipboard",
        description: "Paste system clipboard content into the focused pane (bracketed-paste aware).",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "begin_selection",
        label: "Begin Selection",
        description: "Start a selection from the caret position in selection mode. No-op if a selection already exists — clear first to restart.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "toggle_selection_endpoint",
        label: "Toggle Selection Endpoint",
        description: "Swap which end of the selection is active so movement grows from the other side.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "open_link_at_caret",
        label: "Open Link at Caret",
        description: "Open the hyperlink under the selection caret.",
        icon: Some(Glyph::GitBranch),
        args: &[],
    },
    ActionDescriptor {
        name: "search_scrollback",
        label: "Search Scrollback",
        description: "Type to search the scrollback; Enter keeps matches, Esc cancels.",
        icon: Some(Glyph::Search),
        args: &[],
    },
    ActionDescriptor {
        name: "search_next_match",
        label: "Next Search Match",
        description: "Jump to the next scrollback-search match.",
        icon: Some(Glyph::Search),
        args: &[],
    },
    ActionDescriptor {
        name: "search_prev_match",
        label: "Previous Search Match",
        description: "Jump to the previous scrollback-search match.",
        icon: Some(Glyph::CaretDown),
        args: &[],
    },
    // ── Actions that name their target (action-task-E) ──
    //
    // These are built by name + arguments through `build_action` — from a `config.toml`
    // binding with an `args` table, a menu entry's `Intent`, a mouse handler, or RPC. They
    // never carry a bare default keybinding, because a key cannot supply a pane id.
    //
    // Until action-task-E they were the only dispatchable actions with **no descriptor at
    // all**: `list-actions` could not see them, they had no label or category, and nothing
    // anywhere said what arguments they took. The `args` below are that missing half; the
    // labels and categories come with it, since metadata lives in one place per action.
    ActionDescriptor {
        name: "float_at",
        label: "Float Pane at Position",
        description: "Float a pane at an explicit position and size.",
        icon: None,
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to float."),
            ArgDescriptor::required("x", ArgKind::Float, "Left edge."),
            ArgDescriptor::required("y", ArgKind::Float, "Top edge."),
            ArgDescriptor::required("width", ArgKind::Float, "Width of the floating pane."),
            ArgDescriptor::required("height", ArgKind::Float, "Height of the floating pane."),
        ],
    },
    ActionDescriptor {
        name: "close_pane_by_id",
        label: "Close Pane by Id",
        description: "Close a specific pane by id, whether or not it is focused.",
        icon: Some(Glyph::XSquare),
        args: &[ArgDescriptor::required(
            "pane_id",
            ArgKind::Int,
            "The pane to close.",
        )],
    },
    ActionDescriptor {
        name: "rename_target",
        label: "Rename Pane To",
        description: "Set a pane's name directly, without opening the rename prompt.",
        icon: Some(Glyph::NotePencil),
        args: &[
            ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to rename."),
            ArgDescriptor::required("name", ArgKind::Text, "The new name."),
        ],
    },
    ActionDescriptor {
        name: "rename_pane_by_id",
        label: "Rename Pane",
        description: "Open the rename prompt for a specific pane.",
        icon: Some(Glyph::NotePencil),
        args: &[ArgDescriptor::required(
            "pane_id",
            ArgKind::Int,
            "The pane to rename.",
        )],
    },
    ActionDescriptor {
        name: "reset_pane_name_by_id",
        label: "Reset Pane Name",
        description: "Drop a pane's custom name so it follows its process again.",
        icon: None,
        args: &[ArgDescriptor::required(
            "pane_id",
            ArgKind::Int,
            "The pane whose name to reset.",
        )],
    },
    ActionDescriptor {
        name: "pane_terminal_font_zoom",
        label: "Pane Font Zoom",
        description: "Step one terminal pane's font size up, down, or back to following the app.",
        icon: None,
        args: &[
            ArgDescriptor::optional(
                "pane_id",
                ArgKind::Int,
                "The pane to zoom; omit for the focused one.",
            ),
            ArgDescriptor::required_enum(
                "step",
                <crate::input::FontZoomStep as crate::args::EnumArg>::VALUES,
                "Which way to step.",
            ),
        ],
    },
];
