//! Pi discovers the installed extension; a launch supplies only its temporary reporting connection.

use super::resume::{
    Arity::{Flag, One},
    Grammar,
};
use super::*;

/// **Resuming**, from `pi --help` (pi 1.5.2): `--session <path|id>` opens "a session by file path or
/// full/partial ID". The id goes first, then the user's own options (including `--session-dir`, which
/// is where the session is looked up); messages and `@files` (`pi [options] [--] [@files...]
/// [messages...]`) and every option that picks another session are dropped. `-p/--print`, `--mode`
/// (json and rpc are not interactive), `--export` and `--list-models` are not a conversation to
/// continue. Flags an extension adds are not listed: they are kept, with the word after one.
static GRAMMAR: Grammar = Grammar {
    options: &[
        ("--provider", One),
        ("--model", One),
        ("--api-key", One),
        ("--system-prompt", One),
        ("--append-system-prompt", One),
        ("--mode", One),
        ("--session", One),
        ("--session-id", One),
        ("--fork", One),
        ("--session-dir", One),
        ("-n", One),
        ("--name", One),
        ("--models", One),
        ("-t", One),
        ("--tools", One),
        ("-xt", One),
        ("--exclude-tools", One),
        ("--thinking", One),
        ("-e", One),
        ("--extension", One),
        ("--skill", One),
        ("--prompt-template", One),
        ("--theme", One),
        ("--use-theme", One),
        ("--tui-mode", One),
        ("--export", One),
        ("--print", Flag),
        ("-p", Flag),
        ("--continue", Flag),
        ("-c", Flag),
        ("--resume", Flag),
        ("-r", Flag),
        ("--no-session", Flag),
        ("--no-tools", Flag),
        ("-nt", Flag),
        ("--no-builtin-tools", Flag),
        ("-nbt", Flag),
        ("--no-extensions", Flag),
        ("-ne", Flag),
        ("--no-mcp", Flag),
        ("--no-skills", Flag),
        ("-ns", Flag),
        ("--no-prompt-templates", Flag),
        ("-np", Flag),
        ("--no-themes", Flag),
        ("--no-context-files", Flag),
        ("-nc", Flag),
        ("--verbose", Flag),
        ("--approve", Flag),
        ("-a", Flag),
        ("--no-approve", Flag),
        ("-na", Flag),
        ("--offline", Flag),
        ("--list-models", Flag),
        ("-h", Flag),
        ("--help", Flag),
        ("-v", Flag),
        ("--version", Flag),
    ],
    subcommands: &[
        "install",
        "remove",
        "uninstall",
        "update",
        "list",
        "config",
        "auth",
        "mcp",
    ],
    resumes_with: None,
    replaced: &[
        "--session",
        "--session-id",
        "--fork",
        "-c",
        "--continue",
        "-r",
        "--resume",
        "--no-session",
    ],
    refused: &[
        "-p",
        "--print",
        "--mode",
        "--export",
        "--list-models",
        "-h",
        "--help",
        "-v",
        "--version",
    ],
    resume: "--session",
    keeps_first_word: false,
};

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured(
        "pi",
        config,
        AgentIcon::svg(include_str!("assets/icons/pi.svg")),
        environment,
        integration,
        Some(&GRAMMAR),
    )
}

fn integration(
    driver: &Driver,
    context: &IntegrationContext,
) -> Result<AgentIntegration, AgentError> {
    bundled(
        driver,
        vec![IntegrationResource::File {
            path: context
                .location("PI_CODING_AGENT_DIR", ".pi/agent")
                .join("extensions/heca-status.js"),
            content: include_bytes!("assets/pi.mjs").to_vec(),
        }],
    )
}
