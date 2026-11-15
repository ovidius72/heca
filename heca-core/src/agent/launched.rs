//! **Which program a recorded launch runs** — for telling, from what was recorded alone, that a
//! terminal held a harness, even when reporting was off and nothing was learned from it.
//!
//! A terminal records what it was started with: the program itself, or, for a command typed at a
//! prompt, the user's shell and the command line it was handed (`zsh -ic "claude --model x"`). The
//! programs such a launch runs are the executable and, for a shell, the first word of each command in
//! the command line.

use std::ffi::OsString;
use std::sync::Arc;

use super::{AgentDriver, AgentDrivers};
use crate::backend::ProcessLaunch;

/// Shells whose `-c <command>` runs a command line.
const SHELLS: &[&str] = &["sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh", "nu", "elvish", "pwsh", "powershell", "cmd"];

impl AgentDrivers {
    /// **The driver whose program this launch runs**, directly or as a command a shell was given.
    /// By each driver's own [`recognizes`](AgentDriver::recognizes), whether or not the driver is
    /// switched on: this says what the program *is*, not whether it reports.
    pub fn recognize_launch(&self, launch: &ProcessLaunch) -> Option<&Arc<dyn AgentDriver>> {
        let programs = programs_run(launch);
        self.iter().find(|driver| programs.iter().any(|program| driver.recognizes(program)))
    }
}

/// Every program the launch runs, in the order it runs them.
fn programs_run(launch: &ProcessLaunch) -> Vec<String> {
    let executable = launch.executable.to_string_lossy().into_owned();
    // Either separator: a launch recorded on Windows is read the same on every system.
    let name = executable.rsplit(['/', '\\']).next().unwrap_or(&executable);
    let is_shell = SHELLS.iter().any(|shell| name.eq_ignore_ascii_case(shell) || name.eq_ignore_ascii_case(&format!("{shell}.exe")));
    let mut programs = vec![executable];
    if is_shell && let Some(command) = command_line(&launch.arguments) {
        programs.extend(command_words(&command));
    }
    programs
}

/// The command line a shell was handed: the word after `-c` (or a cluster holding it, `-ic`, `-lc`),
/// or after `/c` or `-Command`.
fn command_line(arguments: &[OsString]) -> Option<String> {
    let mut words = arguments.iter().map(|word| word.to_string_lossy());
    while let Some(word) = words.next() {
        let cluster = word.starts_with('-') && !word.starts_with("--") && word[1..].contains('c');
        if cluster || word.eq_ignore_ascii_case("/c") || word.eq_ignore_ascii_case("-command") {
            return words.next().map(|command| command.into_owned());
        }
    }
    None
}

/// The first word of each command in a command line — past `VAR=value` assignments and `exec` —
/// with quotes taken off. Commands end at `;`, `&`, `|` and a newline outside quotes.
fn command_words(line: &str) -> Vec<String> {
    let mut commands: Vec<Vec<String>> = vec![Vec::new()];
    let (mut word, mut quote, mut started) = (String::new(), None::<char>, false);
    let mut chars = line.chars();
    let end_word = |commands: &mut Vec<Vec<String>>, word: &mut String, started: &mut bool| {
        if std::mem::take(started) {
            commands.last_mut().expect("one command is always open").push(std::mem::take(word));
        }
    };
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (Some('"'), '\\') | (None, '\\') => {
                if let Some(next) = chars.next() { word.push(next); started = true; }
            }
            (Some(_), c) => word.push(c),
            (None, '\'' | '"') => { quote = Some(c); started = true; }
            (None, c) if c.is_whitespace() && c != '\n' => end_word(&mut commands, &mut word, &mut started),
            (None, ';' | '&' | '|' | '\n') => {
                end_word(&mut commands, &mut word, &mut started);
                commands.push(Vec::new());
            }
            (None, c) => { word.push(c); started = true; }
        }
    }
    end_word(&mut commands, &mut word, &mut started);
    commands.into_iter()
        .filter_map(|command| command.into_iter().find(|word| word != "exec" && !is_assignment(word)))
        .collect()
}

fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.starts_with(|c: char| c.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::tests::ExternalDriver;

    fn launch(executable: &str, arguments: &[&str]) -> ProcessLaunch {
        ProcessLaunch { executable: executable.into(), arguments: arguments.iter().map(Into::into).collect(), cwd: None }
    }

    fn drivers() -> AgentDrivers {
        let mut drivers = AgentDrivers::new();
        drivers.register(ExternalDriver::new("demo")).unwrap(); // recognizes `demo-agent`
        drivers
    }

    #[test]
    fn a_harness_is_recognised_directly_and_through_the_shell_it_was_typed_in() {
        let drivers = drivers();
        for (case, launch) in [
            ("itself", launch("/opt/bin/demo-agent", &["--model", "x"])),
            ("zsh -ic", launch("/bin/zsh", &["-ic", "demo-agent "])),
            ("bash -lc with arguments and quotes", launch("/bin/bash", &["-lc", "demo-agent --label 'two words' \"a b\""])),
            ("after a cd", launch("/bin/zsh", &["-ic", "cd ~/work && demo-agent"])),
            ("after assignments", launch("/bin/sh", &["-c", "FOO=1 BAR='x y' exec /usr/local/bin/demo-agent"])),
            ("quoted program", launch("/bin/zsh", &["-ic", "'demo-agent' go"])),
            ("in a pipe", launch("/bin/zsh", &["-c", "echo hi | demo-agent"])),
            ("windows", launch("C:\\Windows\\System32\\cmd.exe", &["/C", "demo-agent"])),
        ] {
            assert_eq!(drivers.recognize_launch(&launch).map(|driver| driver.id()), Some("demo"), "{case}");
        }
    }

    #[test]
    fn an_ordinary_command_is_not_a_harness() {
        let drivers = drivers();
        for (case, launch) in [
            ("a server", launch("/bin/zsh", &["-ic", "npm run dev"])),
            ("an idle shell", launch("/bin/zsh", &[])),
            ("a program", launch("/opt/tools/serve", &["--port", "80"])),
            ("a word that is only an argument", launch("/bin/zsh", &["-ic", "echo demo-agent"])),
            ("inside quotes", launch("/bin/zsh", &["-ic", "echo 'x; demo-agent'"])),
            ("a shell not given a command", launch("/bin/zsh", &["-i", "demo-agent"])),
        ] {
            assert!(drivers.recognize_launch(&launch).is_none(), "{case}");
        }
    }
}
