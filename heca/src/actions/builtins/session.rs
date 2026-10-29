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
    ActionDescriptor { name: "list_sessions", side: Side::Server, label: "Sessions",
        description: "List sessions as JSON over RPC, or open the session palette in a window.",
        icon: Some(Glyph::Cards), args: &[] },
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
        description: "Choose a new name for this session.",
        icon: None,
        args: &[],
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
        description: "Stop its processes and retain the session for explicit restart.",
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
        description: "Remove this session and close its windows, without deleting project files.",
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
        description: "Close this window without terminating its session.",
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
];
