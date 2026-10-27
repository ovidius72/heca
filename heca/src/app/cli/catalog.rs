//! Every command is declared once for dispatch, argument validation and help.

use crate::agent::integration::IntegrationOperation;
use crate::args::{ArgDescriptor, ArgKind};
use super::{Command, LaunchMode, help, integrations, queries};

const PROGRAM: &[ArgDescriptor] = &[ArgDescriptor::required(
    "program", ArgKind::Text, "Registered program id; use `heca list integrations` to find it.",
)];

pub(super) const COMMANDS: &[Command] = &[
    Command {
        name: "--standalone", short: None,
        about: "Run the server and window in one process; end the session with the window.",
        args: &[], json: false, launch: Some(LaunchMode::Standalone),
        run: |_| Ok(String::new()),
    },
    #[cfg(unix)]
    Command {
        name: "--server", short: None,
        about: "Run the session server without a window.",
        args: &[ArgDescriptor::optional("socket", ArgKind::Text, "Override the local socket path.")],
        json: false, launch: None,
        run: |inv| crate::server::runtime::serve(inv.get("socket").map(std::path::Path::new))
            .map(|()| String::new()).map_err(|error| format!("server: {error}\n")),
    },
    #[cfg(unix)]
    Command {
        name: "--kill-server", short: None,
        about: "Stop the running session server, and with it every pane and terminal.",
        args: &[], json: false, launch: None,
        run: |_| Ok(crate::server::socket::kill_server()),
    },
    Command {
        name: "--help",
        short: Some("-h"),
        about: "List these commands and exit.",
        args: &[],
        json: false, launch: None,
        run: |_| Ok(help()),
    },
    Command {
        name: crate::app::keys_show::FLAG,
        short: Some("-k"),
        about: "Every binding name and the key it resolves to, across all layers.",
        args: &[],
        json: true, launch: None,
        run: |inv| Ok(queries::run_keys_show(inv)),
    },
    Command {
        name: "--trust",
        short: None,
        about: "Trust a project's .heca/config.toml as it is now, so it applies (default: this folder's project).",
        args: &[ArgDescriptor::optional(
            "folder",
            ArgKind::Text,
            "A folder inside the project; default the current one.",
        )],
        json: false, launch: None,
        run: |inv| Ok(queries::run_trust(inv, true)),
    },
    Command {
        name: "--untrust",
        short: None,
        about: "Stop trusting a project's .heca/config.toml (default: this folder's project).",
        args: &[ArgDescriptor::optional(
            "folder",
            ArgKind::Text,
            "A folder inside the project; default the current one.",
        )],
        json: false, launch: None,
        run: |inv| Ok(queries::run_trust(inv, false)),
    },
    Command {
        name: "--show-config",
        short: None,
        about: "Every setting your files and the project's .heca/config.toml set, and the file each came from.",
        args: &[],
        json: false, launch: None,
        run: |_| Ok(queries::run_show_config()),
    },
    Command {
        name: "--list-actions",
        short: Some("-a"),
        about: "Every action heca knows, with its category and whether it takes arguments.",
        args: &[],
        json: true, launch: None,
        run: |inv| Ok(queries::run_list_actions(inv)),
    },
    Command {
        name: "--describe-action",
        short: Some("-d"),
        about: "One action in full: what it does, and every argument with its accepted values.",
        args: &[ArgDescriptor::required(
            "name",
            ArgKind::Text,
            "The action to describe. `--list-actions` prints them all.",
        )],
        json: true, launch: None,
        run: |inv| Ok(queries::run_describe_action(inv)),
    },
    Command {
        name: "install integration", short: None,
        about: "Install a program's reporting integration and enable it.",
        args: PROGRAM, json: true, launch: None,
        run: |inv| integrations::apply(inv, IntegrationOperation::Install),
    },
    Command {
        name: "update integration", short: None,
        about: "Update an installed integration, preserving your reporting choices.",
        args: PROGRAM, json: true, launch: None,
        run: |inv| integrations::apply(inv, IntegrationOperation::Update),
    },
    Command {
        name: "uninstall integration", short: None,
        about: "Remove only this integration's owned files and entries.",
        args: PROGRAM, json: true, launch: None,
        run: |inv| integrations::apply(inv, IntegrationOperation::Uninstall),
    },
    Command {
        name: "inspect integration", short: None,
        about: "Check a program's integration without changing it.",
        args: PROGRAM, json: true, launch: None, run: integrations::inspect,
    },
    Command {
        name: "list integrations", short: None,
        about: "List registered integrations and their installation status.",
        args: &[], json: true, launch: None, run: integrations::list,
    },
];
