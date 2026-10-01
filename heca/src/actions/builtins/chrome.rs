//! The built-in Chrome actions, in the order they are listed.

use super::ActionDescriptor;
use crate::args::{ArgDescriptor, ArgKind};
use heca_grid_ui::Glyph;

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "cursor_to",
        label: "Move Container Cursor",
        description: "Put a mounted container's cursor on a named row. Moves the cursor and nothing else.",
        icon: None,
        args: &[
            ArgDescriptor::required(
                "mount",
                ArgKind::Text,
                "Id of the mounted container whose cursor moves.",
            ),
            ArgDescriptor::required(
                "key",
                ArgKind::Text,
                "Identity of the row the cursor moves to.",
            ),
        ],
    },
    ActionDescriptor {
        name: "focus_dock",
        label: "Focus Dock",
        description: "Give chrome keyboard focus to a dock — press a letter to pick one, or name it.",
        icon: None,
        // OPTIONAL, deliberately: the bare binding opens the pick, and a caller that already
        // knows which dock it wants (RPC, a menu entry, a script) names it and skips the pick.
        // A *required* argument would make the bare binding illegal (F003/P010/T006).
        args: &[ArgDescriptor::optional(
            "dock",
            ArgKind::Text,
            "Id of the dock to focus; omit to pick one by letter.",
        )],
    },
    ActionDescriptor {
        name: "toggle_dock",
        label: "Toggle Dock Focus",
        description: "Give a dock the keyboard, or hand it back if that dock already has it.",
        icon: None,
        // The toggle belongs to the *gesture*: pressing a key again plainly means "undo that",
        // while a click, an RPC call and a palette entry all mean "focus it" and nothing more.
        // `global_focus` binds this; everything else binds `focus_dock` (F003/P082/T444).
        args: &[ArgDescriptor::optional(
            "dock",
            ArgKind::Text,
            "Id of the dock to toggle; omit to pick one by letter.",
        )],
    },
    ActionDescriptor {
        name: "clear_search_history",
        label: "Clear Search History",
        description: "Forget the past queries the command palette remembers.",
        icon: None,
        // OPTIONAL: bare forgets every search surface, which is what a key or a palette entry
        // means; a caller that knows which surface it wants names it. A *required* argument
        // would keep it out of the palette entirely.
        args: &[ArgDescriptor::optional(
            "scope",
            ArgKind::Text,
            "Which search surface to forget; omit for all of them.",
        )],
    },
    ActionDescriptor {
        name: "clear_search_ranking",
        label: "Clear Search Ranking",
        description: "Forget which commands you use, so the palette stops ordering by habit.",
        icon: None,
        args: &[ArgDescriptor::optional(
            "scope",
            ArgKind::Text,
            "Which search surface to forget; omit for all of them.",
        )],
    },
    ActionDescriptor {
        name: "unfocus_dock",
        label: "Release Dock Focus",
        description: "Give the keyboard back to the focused pane, releasing chrome focus.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "sidebar_left",
        label: "Toggle Left Sidebar",
        description: "Show or hide the left sidebar.",
        icon: Some(Glyph::Sidebar),
        args: &[],
    },
    ActionDescriptor {
        name: "sidebar_right",
        label: "Toggle Right Sidebar",
        description: "Show or hide the right sidebar.",
        icon: Some(Glyph::Sidebar),
        args: &[],
    },
    // Chrome region show/hide (sidebar-fu-6) — the one visible/hidden state every region has. Unbound by default; the user binds it
    // in config with a region, and the palette offers it once per region.
    ActionDescriptor {
        name: "set_region_visible",
        label: "Set Region Visibility",
        description: "Show, hide or toggle a chrome region (sidebar or bar) — hidden means fully unmounted.",
        icon: Some(Glyph::Sidebar),
        args: &[
            ArgDescriptor::required_enum(
                "region",
                <crate::chrome::RegionId as crate::args::EnumArg>::VALUES,
                "The region: sidebar.left, sidebar.right, bar.top or bar.bottom.",
            ),
            ArgDescriptor::optional_enum(
                "visible",
                <crate::input::RegionVisibility as crate::args::EnumArg>::VALUES,
                "show, hide or toggle; omitted, it toggles.",
            ),
        ],
    },
    // ── Chrome container placement (plugin-04/T1) ──
    // The ONLY built-ins with dotted, namespaced names. Deliberate: this is the id scheme
    // plugins use, and container placement is the first host capability a plugin drives by
    // name. The other ~115 built-ins keep their snake_case config names — renaming them is a
    // migration nobody has decided on, so there are no snake_case aliases for these either
    // (one action, one name). All are parameterized, so they are built through `build_action`
    // and carry no default binding.
    ActionDescriptor {
        name: "chrome.container.move_to_region",
        label: "Move Container to Region",
        description: "Move a chrome container to another region (left/right sidebar, top/bottom bar).",
        icon: Some(Glyph::ArrowLineRight),
        args: &[
            ArgDescriptor::required(
                "container_id",
                ArgKind::Text,
                "Id of the container to move.",
            ),
            ArgDescriptor::required_enum(
                "region",
                <crate::chrome::RegionId as crate::args::EnumArg>::VALUES,
                "The region to move it into.",
            ),
        ],
    },
    ActionDescriptor {
        name: "chrome.container.move_left_sidebar",
        label: "Move Container to Left Sidebar",
        description: "Move a chrome container into the left sidebar.",
        icon: Some(Glyph::ArrowLineLeft),
        args: &[ArgDescriptor::required(
            "container_id",
            ArgKind::Text,
            "Id of the container to move.",
        )],
    },
    ActionDescriptor {
        name: "chrome.container.move_right_sidebar",
        label: "Move Container to Right Sidebar",
        description: "Move a chrome container into the right sidebar.",
        icon: Some(Glyph::ArrowLineRight),
        args: &[ArgDescriptor::required(
            "container_id",
            ArgKind::Text,
            "Id of the container to move.",
        )],
    },
    ActionDescriptor {
        name: "chrome.container.reorder_before",
        label: "Reorder Container Before",
        description: "Move a chrome container before another in its region (omit the target to move it to the end).",
        icon: None,
        args: &[
            ArgDescriptor::required(
                "container_id",
                ArgKind::Text,
                "Id of the container to move.",
            ),
            ArgDescriptor::optional(
                "before_id",
                ArgKind::Text,
                "Id of the container to sit before; omit to move it to the end.",
            ),
        ],
    },
    ActionDescriptor {
        name: "chrome.container.reorder_after",
        label: "Reorder Container After",
        description: "Move a chrome container after another in its region.",
        icon: None,
        args: &[
            ArgDescriptor::required(
                "container_id",
                ArgKind::Text,
                "Id of the container to move.",
            ),
            ArgDescriptor::required(
                "after_id",
                ArgKind::Text,
                "Id of the container to sit after.",
            ),
        ],
    },
    ActionDescriptor {
        name: "collapse_current_workspace",
        label: "Collapse Workspace Row",
        description: "Collapse the active workspace row in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "expand_current_workspace",
        label: "Expand Workspace Row",
        description: "Expand the active workspace row in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "toggle_current_workspace_collapsed",
        label: "Toggle Workspace Row",
        description: "Toggle the active workspace row collapsed state in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "collapse_current_column",
        label: "Collapse Column Row",
        description: "Collapse the focused tiled column row in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "expand_current_column",
        label: "Expand Column Row",
        description: "Expand the focused tiled column row in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "toggle_current_column_collapsed",
        label: "Toggle Column Row",
        description: "Toggle the focused tiled column row collapsed state in the sidebar tree UI.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "submit_overlay",
        label: "Choose Overlay Button",
        description: "Choose a button of the front-most dialog or menu, by its id — as clicking it would.",
        icon: None,
        // The overlay is the one in front, as for `close_overlay`: its id is a runtime counter
        // nothing outside the process could name.
        args: &[ArgDescriptor::required(
            "action",
            ArgKind::Text,
            "Id of the button (or action) to choose, e.g. confirm or cancel.",
        )],
    },
    ActionDescriptor {
        name: "close_overlay",
        label: "Close Overlay",
        description: "Dismiss the front-most overlay — the exposé, a dialog, a menu.",
        icon: None,
        // No arguments at all: the overlay it closes is the one in front, because an OverlayId
        // is a runtime counter nothing outside the process could name.
        args: &[],
    },
    ActionDescriptor {
        name: "show_layer",
        label: "Show Layer",
        description: "Bring an addressable layer into the stack — an expose, a plugin panel.",
        icon: None,
        // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
        // name, and a *required* argument would keep this out of the palette entirely.
        args: &[
            ArgDescriptor::optional(
                "name",
                ArgKind::Text,
                "Layer to show, as owner.short (e.g. heca.expose).",
            ),
            ArgDescriptor::optional(
                "dock",
                ArgKind::Text,
                "Which placement, when a component is seated twice; omit to use the focused one.",
            ),
        ],
    },
    ActionDescriptor {
        name: "hide_layer",
        label: "Hide Layer",
        description: "Take an addressable layer back out of the stack.",
        icon: None,
        // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
        // name, and a *required* argument would keep this out of the palette entirely.
        args: &[
            ArgDescriptor::optional(
                "name",
                ArgKind::Text,
                "Layer to hide, as owner.short (e.g. heca.expose).",
            ),
            ArgDescriptor::optional(
                "dock",
                ArgKind::Text,
                "Which placement, when a component is seated twice; omit to use the focused one.",
            ),
        ],
    },
    ActionDescriptor {
        name: "toggle_layer",
        label: "Toggle Layer",
        description: "Show an addressable layer if hidden, hide it if shown.",
        icon: None,
        // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
        // name, and a *required* argument would keep this out of the palette entirely.
        args: &[
            ArgDescriptor::optional(
                "name",
                ArgKind::Text,
                "Layer to toggle, as owner.short (e.g. heca.expose).",
            ),
            ArgDescriptor::optional(
                "dock",
                ArgKind::Text,
                "Which placement, when a component is seated twice; omit to use the focused one.",
            ),
        ],
    },
    // ── Notifications (F009) ──
    ActionDescriptor {
        name: "notification_dismiss_one",
        label: "Dismiss Notification",
        description: "Dismiss a visible notification by id, when its lifecycle permits it.",
        icon: Some(Glyph::XSquare),
        args: &[ArgDescriptor::required(
            "id",
            ArgKind::Int,
            "The notification's runtime id.",
        )],
    },
    ActionDescriptor {
        name: "notification_dismiss_all",
        label: "Dismiss All Notifications",
        description: "Dismiss every currently visible notification.",
        icon: Some(Glyph::XSquare),
        args: &[],
    },
    ActionDescriptor {
        name: "notification_dismiss_last",
        label: "Dismiss Last Notification",
        description: "Dismiss the first eligible visible notification in stable toast order.",
        icon: Some(Glyph::XSquare),
        args: &[],
    },
    ActionDescriptor {
        name: "notification_pick",
        label: "Pick Notification Action",
        description: "Open a scoped picker over the visible toast actions/dismiss affordances, in addition to their global prefix+/ letters.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "notification_action_relay",
        label: "Run Notification Action",
        description: "Internal: the toast's inline action button cannot carry its own Intent (the notification, and therefore the Intent, does not exist yet when the button is built at mount time), so it names this relay by id + key instead — an address, not a smuggled closure — and the relay resolves it against the store and dispatches it. Not meant to be bound directly.",
        icon: None,
        args: &[
            ArgDescriptor::required("id", ArgKind::Int, "The notification's runtime id."),
            ArgDescriptor::required(
                "key",
                ArgKind::Text,
                "The action's key (its own intent name).",
            ),
        ],
    },
    // ── Horizontal scroll (a chrome container's scroll area; a pane has one axis) ──
    ActionDescriptor {
        name: "scroll_page_left",
        label: "Scroll Page Left",
        description: "Scroll the focused chrome container one page left. Does nothing when no dock holds chrome focus — a terminal viewport has no horizontal axis.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_page_right",
        label: "Scroll Page Right",
        description: "Scroll the focused chrome container one page right. Does nothing when no dock holds chrome focus — a terminal viewport has no horizontal axis.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_to_left_edge",
        label: "Scroll to Left Edge",
        description: "Jump the focused chrome container to its left edge. Does nothing when no dock holds chrome focus.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "scroll_to_right_edge",
        label: "Scroll to Right Edge",
        description: "Jump the focused chrome container to its right edge. Does nothing when no dock holds chrome focus.",
        icon: None,
        args: &[],
    },
];
