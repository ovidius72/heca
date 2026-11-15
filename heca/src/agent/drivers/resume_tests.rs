//! **The command each built-in harness is resumed with**, from realistic launches. Each argument
//! list was checked against the program's own `--help`; the sources are beside each grammar.

use super::*;

fn drivers() -> AgentDrivers {
    available(&AgentsConfig::default())
}

fn launch(executable: &str, arguments: &[&str]) -> ProcessLaunch {
    ProcessLaunch {
        executable: executable.into(),
        arguments: arguments.iter().map(Into::into).collect(),
        cwd: Some("/work/project".into()),
    }
}

/// What `driver` resumes `original` as: the arguments, with the executable and folder checked.
fn resumed(driver: &str, original: &ProcessLaunch, id: &str) -> Option<Vec<String>> {
    let launch = drivers().driver(driver).unwrap().resume(original, id)?;
    assert_eq!(launch.executable, original.executable, "{driver}: the same program");
    assert_eq!(launch.cwd, original.cwd, "{driver}: the same folder");
    Some(launch.arguments.iter().map(|argument| argument.to_str().unwrap().to_owned()).collect())
}

const ID: &str = "0a1b2c3d-4e5f-6071-8293-a4b5c6d7e8f9";

#[test]
fn claude_continues_the_session_by_id_with_the_users_options_and_without_the_prompt() {
    let original = launch("/usr/local/bin/claude", &["--model", "opus", "--dangerously-skip-permissions", "fix the failing tests"]);
    assert_eq!(resumed("claude", &original, ID).unwrap(), ["--resume", ID, "--model", "opus", "--dangerously-skip-permissions"]);
    // An earlier resume, a continue and a session of its own are replaced, not stacked.
    let earlier = launch("claude", &["--resume", "old-id", "--permission-mode", "plan", "--name", "refactor", "go on"]);
    assert_eq!(resumed("claude", &earlier, ID).unwrap(), ["--resume", ID, "--permission-mode", "plan", "--name", "refactor"]);
    let chosen = launch("claude", &["-c", "--session-id", "1111", "--effort", "high"]);
    assert_eq!(resumed("claude", &chosen, ID).unwrap(), ["--resume", ID, "--effort", "high"]);
    // A variadic option keeps every value it was given.
    let many = launch("claude", &["--add-dir", "../lib", "../docs", "--model", "sonnet", "do it"]);
    assert_eq!(resumed("claude", &many, ID).unwrap(), ["--resume", ID, "--add-dir", "../lib", "../docs", "--model", "sonnet"]);
    // A non-interactive run, another command and an id that reads as an option are not resumed.
    assert_eq!(resumed("claude", &launch("claude", &["-p", "summarise"]), ID), None);
    assert_eq!(resumed("claude", &launch("claude", &["mcp", "list"]), ID), None);
    assert_eq!(resumed("claude", &original, "--resume"), None);
    assert_eq!(resumed("claude", &original, ""), None);
}

#[test]
fn codex_continues_the_session_with_its_resume_command_and_the_users_options() {
    let original = launch("/opt/homebrew/bin/codex", &["-m", "gpt-5", "-c", "sandbox_mode=\"read-only\"", "--search", "add a test for the parser"]);
    assert_eq!(resumed("codex", &original, ID).unwrap(), ["resume", ID, "-m", "gpt-5", "-c", "sandbox_mode=\"read-only\"", "--search"]);
    // Resumed before: its subcommand, old id and prompt go; images attach to a prompt and go too.
    let earlier = launch("codex", &["resume", "old-id", "--ask-for-approval", "never", "-i", "shot.png", "diagram.png", "carry on"]);
    assert_eq!(resumed("codex", &earlier, ID).unwrap(), ["resume", ID, "--ask-for-approval", "never"]);
    let last = launch("codex", &["resume", "--last", "--cd", "/work/project", "--worktree"]);
    assert_eq!(resumed("codex", &last, ID).unwrap(), ["resume", ID, "--cd", "/work/project"]);
    for other in [&["exec", "write docs"][..], &["fork", "--last"], &["review"], &["-h"]] {
        assert_eq!(resumed("codex", &launch("codex", other), ID), None, "{other:?}");
    }
}

#[test]
fn opencode_continues_the_session_by_id_and_keeps_the_directory() {
    let original = launch("/Users/me/.opencode/bin/opencode", &["--auto", "--prompt", "do the migration", "~/projects/demo"]);
    assert_eq!(resumed("opencode", &original, "ses_01abc").unwrap(), ["--session", "ses_01abc", "--auto", "~/projects/demo"]);
    let chosen = launch("opencode", &["-c", "--log-level", "debug", "--server", "http://localhost:4096"]);
    assert_eq!(resumed("opencode", &chosen, "ses_01abc").unwrap(), ["--session", "ses_01abc", "--log-level", "debug", "--server", "http://localhost:4096"]);
    let earlier = launch("opencode", &["-s", "ses_old", "--standalone"]);
    assert_eq!(resumed("opencode", &earlier, "ses_01abc").unwrap(), ["--session", "ses_01abc", "--standalone"]);
    for other in [&["run", "say hi"][..], &["mini"], &["serve"], &["--version"]] {
        assert_eq!(resumed("opencode", &launch("opencode", other), "ses_01abc"), None, "{other:?}");
    }
}

#[test]
fn pi_continues_the_session_by_id_and_drops_messages_and_files() {
    let original = launch("/Users/me/.local/bin/pi", &["--provider", "openai", "--model", "gpt-4o-mini", "--thinking", "high", "@prompt.md", "Refactor the auth module"]);
    assert_eq!(resumed("pi", &original, ID).unwrap(), ["--session", ID, "--provider", "openai", "--model", "gpt-4o-mini", "--thinking", "high"]);
    let chosen = launch("pi", &["--continue", "--session-dir", "/tmp/sessions", "-n", "auth", "What did we discuss?"]);
    assert_eq!(resumed("pi", &chosen, ID).unwrap(), ["--session", ID, "--session-dir", "/tmp/sessions", "-n", "auth"]);
    let earlier = launch("pi", &["--session", "old", "--no-session", "--", "- a message that starts with a dash"]);
    assert_eq!(resumed("pi", &earlier, ID).unwrap(), ["--session", ID]);
    // An option an extension added is not known: it is kept, with the word after it.
    let extension = launch("pi", &["--fff-mode", "tools-only", "--offline"]);
    assert_eq!(resumed("pi", &extension, ID).unwrap(), ["--session", ID, "--fff-mode", "tools-only", "--offline"]);
    for other in [&["-p", "list files"][..], &["install", "npm:x"], &["--mode", "rpc"], &["--list-models"]] {
        assert_eq!(resumed("pi", &launch("pi", other), ID), None, "{other:?}");
    }
}
