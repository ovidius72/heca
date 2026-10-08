//! The built-in System actions, in the order they are listed.

use super::ActionDescriptor;
use crate::actions::Side;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    // ── Font zoom ──
    ActionDescriptor {
        name: "app_font_increase",
        side: Side::Client,
        label: "Increase App Font",
        description: "Increase the whole-app font: chrome/UI and every terminal pane.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "app_font_decrease",
        side: Side::Client,
        label: "Decrease App Font",
        description: "Decrease the whole-app font: chrome/UI and every terminal pane.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "app_font_reset",
        side: Side::Client,
        label: "Reset App Font",
        description: "Reset the whole-app font to the configured sizes.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "command_palette",
        side: Side::Client,
        label: "Command Palette",
        description: "Search every action — the app's and every mounted component's — and run one.",
        icon: Some(Glyph::Search),
        // Both OPTIONAL: bare is the actions list with an empty query, which is what a plain
        // binding means. A *required* argument would take this action out of the palette's own
        // listing, which offers only what it can run with nothing supplied.
        args: &[
            ArgDescriptor::optional(
                "mode",
                ArgKind::Text,
                "Which list to open in: pane or workspace; omit for actions.",
            ),
            ArgDescriptor::optional(
                "query",
                ArgKind::Text,
                "Text to start the search with; omit to open empty.",
            ),
        ],
    },
    ActionDescriptor {
        name: "reload_config",
        side: Side::Client,
        label: "Reload Config",
        description: "Reload keymaps, theme, and settings from config.toml without restarting.",
        icon: Some(Glyph::Gear),
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
        name: "app_font_zoom",
        side: Side::Client,
        label: "App Font Zoom",
        description: "Step the whole app's font size up, down, or back to the configured size.",
        icon: None,
        args: &[ArgDescriptor::required_enum(
            "step",
            <crate::input::FontZoomStep as crate::args::EnumArg>::VALUES,
            "Which way to step.",
        )],
    },
    ActionDescriptor {
        name: "spawn_command",
        side: Side::Server,
        label: "Spawn Command",
        description: "Open a new pane running a command.",
        icon: None,
        args: &[
            ArgDescriptor::required("command", ArgKind::Text, "The command line to run."),
            ArgDescriptor::optional_enum(
                "kind",
                <crate::input::SpawnKind as crate::args::EnumArg>::VALUES,
                "What kind of pane to open. Default terminal.",
            ),
            ArgDescriptor::optional(
                "float",
                ArgKind::Bool,
                "Open it as a floating pane. Default false.",
            ),
            ArgDescriptor::optional(
                "cwd",
                ArgKind::Text,
                "The folder to start in. Default: heca's own. A folder that does not exist is an error.",
            ),
            ArgDescriptor::optional(
                "close_pane",
                ArgKind::Bool,
                "Close the pane when the command exits. Default false.",
            ),
            ArgDescriptor::optional(
                "keep_on_error",
                ArgKind::Bool,
                "Keep the pane open when the command fails. Default false.",
            ),
            ArgDescriptor::optional(
                "keep_on_success",
                ArgKind::Bool,
                "Keep the pane open when the command succeeds. Default false.",
            ),
        ],
    },
];
