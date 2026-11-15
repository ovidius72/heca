//! OpenCode discovers a local CLI plugin. No plugin-list override or temporary plugin is needed.

use super::*;
use super::resume::{Arity::{Flag, One}, Grammar};

/// **Resuming**, from `opencode --help` (opencode v2.1.4): `--session, -s string` is the "Session ID
/// to continue". The id goes first, then the user's own options and the directory
/// (`opencode <subcommand> [flags] [<directory>]`, the one plain word that is kept); `--prompt` and
/// the options that pick another session are dropped. Another subcommand (`run`, `mini`, `serve`,
/// ...) is not the interactive session to continue.
static GRAMMAR: Grammar = Grammar {
    options: &[
        ("--server", One), ("--session", One), ("-s", One), ("--prompt", One), ("--log-level", One),
        ("--completions", One), ("--standalone", Flag), ("--auto", Flag), ("--continue", Flag), ("-c", Flag),
        ("--print-logs", Flag), ("--wizard", Flag), ("-h", Flag), ("--help", Flag), ("-v", Flag), ("--version", Flag),
    ],
    subcommands: &[
        "upgrade", "update", "uninstall", "acp", "api", "debug", "auth", "mcp", "plugin", "models", "stats",
        "mini", "run", "session", "service", "reload", "pair", "serve",
    ],
    resumes_with: None,
    replaced: &["--session", "-s", "--continue", "-c", "--prompt"],
    refused: &["-h", "--help", "-v", "--version", "--wizard", "--completions"],
    resume: "--session",
    keeps_first_word: true,
};

pub(super) fn configured(config: &AgentsConfig) -> Option<Driver> {
    Driver::configured("opencode", config, AgentIcon::svg(include_str!("../assets/icons/opencode.svg")), environment, integration, Some(&GRAMMAR))
}

fn integration(driver: &Driver, context: &IntegrationContext) -> Result<AgentIntegration, AgentError> {
    let directory = if let Some(path) = context.environment.get("OPENCODE_CONFIG_DIR") {
        std::path::PathBuf::from(path)
    } else {
        context.location("XDG_CONFIG_HOME", ".config").join("opencode")
    };
    bundled(driver, vec![IntegrationResource::File {
        path: directory.join("plugins/heca-status/tui.ts"),
        content: include_bytes!("../assets/opencode.ts").to_vec(),
    }])
}
