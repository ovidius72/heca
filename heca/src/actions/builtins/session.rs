//! One declaration for session actions, shared by GUI, bindings, plugins and RPC.
use super::ActionDescriptor;
use crate::{
    actions::Side,
    args::{ArgDescriptor, ArgKind},
};
use heca_grid_ui::Glyph;

const TARGET: &[ArgDescriptor] = &[ArgDescriptor::optional(
    "session",
    ArgKind::Text,
    "Session name; omit for the requesting window's session, or the last session for unattached RPC.",
)];

pub(super) const ACTIONS: &[ActionDescriptor] = &[
    ActionDescriptor {
        name: "list_sessions",
        side: Side::Server,
        label: "Sessions",
        description: "List sessions as JSON over RPC, or open the session palette in a window.",
        icon: Some(Glyph::Cards),
        args: &[],
    },
    ActionDescriptor {
        name: "kill_server",
        side: Side::Server,
        label: "Kill Server",
        description: "Stop the server, and with it every session, pane and terminal. `heca --kill-server` runs this.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "create_session",
        side: Side::Server,
        label: "Create Session",
        description: "Create a session in an absolute project folder.",
        icon: Some(Glyph::Plus),
        args: &[
            ArgDescriptor::required(
                "folder",
                ArgKind::Text,
                "Absolute project folder; created if missing.",
            ),
            ArgDescriptor::optional(
                "name",
                ArgKind::Text,
                "Unique name; omit for the configured default.",
            ),
            ArgDescriptor::optional("label", ArgKind::Text, "Optional extension-owned label."),
        ],
    },
    ActionDescriptor {
        name: "new_session",
        side: Side::Client,
        label: "New Session",
        description: "Choose a project folder and create a session.",
        icon: Some(Glyph::Plus),
        args: &[],
    },
    ActionDescriptor {
        name: "rename_session",
        side: Side::Client,
        label: "Rename Session",
        description: "Choose a new name for a session (default: this window's), in the rename dialog.",
        icon: None,
        args: &[ArgDescriptor::optional(
            "session",
            ArgKind::Text,
            "Session to rename; omit for this window's.",
        )],
    },
    ActionDescriptor {
        name: "rename_session_to",
        side: Side::Server,
        label: "Rename Session",
        description: "Set a unique session name.",
        icon: None,
        args: &[
            ArgDescriptor::optional(
                "session",
                ArgKind::Text,
                "Session name; omit for the current session.",
            ),
            ArgDescriptor::required("name", ArgKind::Text, "New unique name."),
        ],
    },
    ActionDescriptor {
        name: "terminate_session",
        side: Side::Server,
        label: "Terminate Session",
        description: "Stop its processes and retain the session for explicit restart. Its windows move to the most recent running session.",
        icon: None,
        args: TARGET,
    },
    ActionDescriptor {
        name: "restart_session",
        side: Side::Server,
        label: "Restart Session",
        description: "Explicitly restart a terminated session.",
        icon: None,
        args: TARGET,
    },
    ActionDescriptor {
        name: "delete_session",
        side: Side::Server,
        label: "Delete Session",
        description: "Remove this session; its windows move to the most recent running session. Never deletes project files.",
        icon: None,
        args: TARGET,
    },
    ActionDescriptor {
        name: "switch_session",
        side: Side::Client,
        label: "Switch Session",
        description: "Attach this window to a named session.",
        icon: Some(Glyph::Cards),
        args: &[ArgDescriptor::required(
            "name",
            ArgKind::Text,
            "Existing session name.",
        )],
    },
    ActionDescriptor {
        name: "detach_session",
        side: Side::Client,
        label: "Detach Session",
        description: "Leave this session running and choose another session, create one, or exit.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "new_window",
        side: Side::Client,
        label: "New Window",
        description: "Open another window on the same server, on this window's session; each window has its own view and can switch session on its own.",
        icon: None,
        args: &[ArgDescriptor::optional(
            "session",
            ArgKind::Text,
            "Session to open it on; omit for this window's.",
        )],
    },
    ActionDescriptor {
        name: "close_window",
        side: Side::Client,
        label: "Close Window",
        description: "Close this window; its session keeps running. heca ends with its last window.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "last_session",
        side: Side::Client,
        label: "Last Session",
        description: "Switch this window back to the session it showed before; again, and it comes back.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "reconnect_session",
        side: Side::Client,
        label: "Restart And Reconnect",
        description: "Start a missing server and reconnect this window to its session.",
        icon: None,
        args: &[],
    },
    ActionDescriptor {
        name: "follow",
        side: Side::Client,
        label: "Follow Window",
        description: "Mirror another window on this session; omit the target to stop following.",
        icon: Some(Glyph::Cards),
        args: &[ArgDescriptor::optional(
            "target",
            ArgKind::Text,
            "Id of the window to follow, from the window directory.",
        )],
    },
];
