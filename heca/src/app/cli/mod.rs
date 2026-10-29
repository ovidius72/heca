//! **The command line — one table that dispatches, documents and validates.**
//!
//! Every question heca answers before opening a window lives in [`COMMANDS`], and each entry
//! carries four things: the command, what it does, the arguments it takes, and the function that
//! runs it. Everything else is derived from that one declaration:
//!
//! - `--help` is rendered from it, so it cannot list a command that does nothing, nor miss one that
//!   works;
//! - arguments are **checked** against it before a command runs, so a command cannot read one it
//!   never declared, and a missing or misspelled one is a sentence rather than silence;
//! - an unrecognised flag is answered, with the nearest real one offered.
//!
//! ⚠️ **The argument declaration is [`ArgDescriptor`] — the very type an action uses**, checked by
//! the very same [`check_args`]. A CLI flag and a WM action are different things, but "what
//! arguments does this take" is one idea, and heca had already solved it once for actions. A second
//! shape here would be a second thing to keep in step.
//!
//! These all run **before the window**: no GPU, no event loop, so a script can ask heca a question
//! without a display.

use crate::args::{ArgDescriptor, ArgSpec, check_args};
use std::collections::HashMap;

mod catalog;
mod integrations;
mod queries;
mod sessions;
#[cfg(test)]
mod tests;

use catalog::COMMANDS;
type Answer = Result<String, String>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum LaunchMode {
    #[default]
    Attached,
    Standalone,
    Session(String),
    New { path: Option<String>, name: Option<String> },
}

#[derive(Clone, Copy)]
enum LaunchKind { Standalone, Session, New }

/// One thing heca can be asked from the command line.
struct Command {
    /// Command path or flag, as shown in help.
    name: &'static str,
    /// **A one-letter form of the same command.** One letter, not two: `-la` reads as the combined
    /// `-l -a` everywhere else on the machine, and a flag that looks like a convention it does not
    /// follow is worse than no short form at all. `None` where no letter is free or obvious.
    short: Option<&'static str>,
    /// One line, shown by `--help`.
    about: &'static str,
    /// **What it takes, as data.** Positional: the words after the flag are matched to these in
    /// order. Declared with the same type an action's arguments use, and checked the same way.
    args: &'static [ArgDescriptor],
    /// Named value options, checked by the same argument declarations as positionals.
    options: &'static [ArgDescriptor],
    /// Whether `--json` means anything to it.
    json: bool,
    /// A window launch mode rather than a command that prints and exits.
    launch: Option<LaunchKind>,
    /// Runs it, given the checked arguments.
    run: fn(&Invocation) -> Answer,
}

/// A command about to run: its arguments, already checked against what it declared.
pub(crate) struct Invocation {
    args: HashMap<String, String>,
    json: bool,
}

impl Invocation {
    /// A declared argument's value, if it was supplied.
    fn get(&self, name: &str) -> Option<&str> {
        self.args.get(name).map(String::as_str)
    }
}

/// Switches the output to JSON, for scripting. Declared once rather than repeated in every
/// command's description.
const JSON_FLAG: &str = "--json";
/// Its one-letter form.
const JSON_SHORT: &str = "-j";

impl Command {
    fn prefix_len(&self, words: &[&str]) -> Option<usize> {
        if words.first().is_some_and(|word| self.short == Some(word)) { return Some(1) }
        let path: Vec<_> = self.name.split_whitespace().collect();
        words.starts_with(&path).then_some(path.len())
    }

    /// How it is written, both forms: `-a, --list-actions`.
    fn spelling(&self) -> String {
        match self.short {
            Some(short) => format!("{short}, {}", self.name),
            None => format!("    {}", self.name),
        }
    }
}

/// **Does this argument name a flag rather than a value?** A leading `-` — except on a negative
/// number, which is a value however it starts.
fn is_flag(arg: &str) -> bool {
    arg.starts_with('-') && !arg[1..].starts_with(|c: char| c.is_ascii_digit())
}

/// Answer a command-line question and exit, if the arguments asked one.
///
/// Returns `true` when it handled the invocation and the caller should not open a window.
pub(crate) fn run_if_requested(args: &[String]) -> bool {
    #[cfg(unix)]
    if let Some(output) = crate::server::socket::run_rpc_cli(args) {
        print!("{output}");
        return true;
    }
    match answer(args) {
        Some(Ok(text)) => { print!("{text}"); true }
        Some(Err(error)) => { eprint!("{error}"); std::process::exit(1) }
        None => false,
    }
}

/// **The whole of the command line, as a function of its arguments** — so every rule below is
/// testable without a process. `None` means "no command was asked for; open the window".
fn answer(args: &[String]) -> Option<Answer> {
    let (cmd, invocation) = match parse(args)? {
        Ok(parsed) => parsed,
        Err(answer) => return Some(answer),
    };
    if cmd.launch.is_some() {
        return (cmd.run)(&invocation).err().map(Err);
    }
    Some((cmd.run)(&invocation))
}

fn parse(args: &[String]) -> Option<Result<(&'static Command, Invocation), Answer>> {
    if args.is_empty() { return None; }
    let words: Vec<_> = args.iter().map(String::as_str)
        .filter(|word| *word != JSON_FLAG && *word != JSON_SHORT).collect();
    let Some((cmd, consumed)) = COMMANDS.iter()
        .find_map(|cmd| cmd.prefix_len(&words).map(|len| (cmd, len))) else {
        let unknown = words.first().copied().unwrap_or(JSON_FLAG);
        let hint = nearest_command(unknown).map(|name| format!(" — did you mean '{name}'?")).unwrap_or_default();
        return Some(Err(Err(format!("unknown command '{unknown}'{hint}\n\nRun `heca --help` for the list.\n"))));
    };
    let positional = &words[consumed..];
    if positional.iter().any(|arg| *arg == "--help" || *arg == "-h") {
        return Some(Err(Ok(format!("{}\n{}\n", usage_of(cmd), cmd.about))));
    }
    let mut supplied = HashMap::new();
    let mut values = Vec::new();
    let mut it = positional.iter().copied();
    while let Some(word) = it.next() {
        if !is_flag(word) { values.push(word); continue; }
        let Some(option) = cmd.options.iter().find(|option| word.strip_prefix("--") == Some(option.name)) else {
            return Some(Err(Err(format!("{}: unknown option '{word}'\n\n{}\n", cmd.name, usage_of(cmd)))));
        };
        let Some(value) = it.next().filter(|value| !is_flag(value)) else {
            return Some(Err(Err(format!("{}: {word} needs a value\n", cmd.name))));
        };
        if supplied.insert(option.name.to_owned(), value.to_owned()).is_some() {
            return Some(Err(Err(format!("{}: {word} was supplied twice\n", cmd.name))));
        }
    }
    if values.len() > cmd.args.len() {
        return Some(Err(Err(format!("{} takes {} argument(s), got {}\n\n{}\n",
            cmd.name, cmd.args.len(), values.len(), usage_of(cmd)))));
    }
    supplied.extend(cmd.args.iter().zip(values).map(|(spec, value)| (spec.name.to_owned(), value.to_owned())));
    let specs: Vec<ArgSpec> = cmd.args.iter().chain(cmd.options).map(ArgSpec::from_descriptor).collect();
    let problems = check_args(&specs, &supplied);
    if !problems.is_empty() {
        let listed = problems.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
        return Some(Err(Err(format!("{}: {listed}\n\n{}\n", cmd.name, usage_of(cmd)))));
    }
    let json = args.iter().any(|arg| arg == JSON_FLAG || arg == JSON_SHORT);
    if json && !cmd.json { return Some(Err(Err(format!("{} does not support --json\n", cmd.name)))); }
    Some(Ok((cmd, Invocation { args: supplied, json })))
}

pub(crate) fn launch_mode(args: &[String]) -> LaunchMode {
    let Some(Ok((command, invocation))) = parse(args) else { return LaunchMode::Attached; };
    match command.launch {
        Some(LaunchKind::Standalone) => LaunchMode::Standalone,
        Some(LaunchKind::Session) => LaunchMode::Session(invocation.get("name").unwrap_or_default().to_owned()),
        Some(LaunchKind::New) => LaunchMode::New {
            path: invocation.get("path").map(str::to_owned), name: invocation.get("name").map(str::to_owned),
        },
        None => LaunchMode::Attached,
    }
}

/// Suggest a declared command with the same prefix.
fn nearest_command(typo: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .flat_map(|c| std::iter::once(c.name).chain(c.short))
        .map(|f| (shared_prefix(f, typo), f))
        .filter(|(n, _)| *n >= 4)
        .max_by_key(|(n, _)| *n)
        .map(|(_, f)| f)
}

fn shared_prefix(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

/// One command's usage line, built from what it declared.
fn usage_of(cmd: &Command) -> String {
    let mut usage = format!("Usage: heca {}", cmd.name);
    for a in cmd.args {
        usage.push_str(&if a.required {
            format!(" <{}>", a.name)
        } else {
            format!(" [{}]", a.name)
        });
    }
    if cmd.json {
        usage.push_str(" [--json]");
    }
    for option in cmd.options { usage.push_str(&format!(" [--{} <{}>]", option.name, option.name)); }
    usage
}

/// The help text, built from [`COMMANDS`] so it lists exactly what is implemented — arguments
/// included, from the same declaration the dispatcher checks against.
fn help() -> String {
    let usages: Vec<String> = COMMANDS
        .iter()
        .map(|c| {
            let tail = usage_of(c)
                .trim_start_matches("Usage: heca ")
                .trim_start_matches(c.name)
                .to_string();
            format!("{}{tail}", c.spelling())
        })
        .collect();
    let width = usages.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::from(
        "heca — a GPU-native workspace compositor\n\nUsage: heca [COMMAND]\n\nRun with no command to open the window.\n\nCommands:\n",
    );
    for (c, usage) in COMMANDS.iter().zip(&usages) {
        out.push_str(&format!("  {usage:<width$}  {}\n", c.about));
        // An argument's own description, where it has more to say than its name.
        for a in c.args.iter().chain(c.options) {
            out.push_str(&format!(
                "  {:<width$}    {}: {}\n",
                "", a.name, a.description
            ));
        }
    }
    out.push_str(
        "\nEvery command answers before the window opens, so a script can ask without a display.\n",
    );
    out
}
