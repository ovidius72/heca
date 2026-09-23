//! **The command line — one table that dispatches, documents and validates.**
//!
//! Every question heca answers before opening a window lives in [`COMMANDS`], and each entry
//! carries four things: the flag, what it *does*, the arguments it takes, and the function that
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
//! shape here would be a second thing to keep in step (Antonio, 2026-09-04: *"this would be a
//! general pattern on how to build arguments"*).
//!
//! These all run **before the window**: no GPU, no event loop, so a script can ask heca a question
//! without a display.

use crate::actions::{ActionCatalog, ArgDescriptor, ArgKind, ArgSpec, check_args};
use std::collections::HashMap;

/// One thing heca can be asked from the command line.
struct Command {
    /// The flag itself.
    flag: &'static str,
    /// **A one-letter form of the same command.** One letter, not two: `-la` reads as the combined
    /// `-l -a` everywhere else on the machine, and a flag that looks like a convention it does not
    /// follow is worse than no short form at all. `None` where no letter is free or obvious.
    short: Option<&'static str>,
    /// One line, shown by `--help`.
    about: &'static str,
    /// **What it takes, as data.** Positional: the words after the flag are matched to these in
    /// order. Declared with the same type an action's arguments use, and checked the same way.
    args: &'static [ArgDescriptor],
    /// Whether `--json` means anything to it.
    json: bool,
    /// Runs it, given the checked arguments.
    run: fn(&Invocation) -> String,
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
    /// Whether `arg` names this command, long form or short.
    fn named_by(&self, arg: &str) -> bool {
        arg == self.flag || self.short == Some(arg)
    }

    /// How it is written, both forms: `-a, --list-actions`.
    fn spelling(&self) -> String {
        match self.short {
            Some(short) => format!("{short}, {}", self.flag),
            None => format!("    {}", self.flag),
        }
    }
}

/// **Does this argument name a flag rather than a value?** A leading `-` — except on a negative
/// number, which is a value however it starts.
fn is_flag(arg: &str) -> bool {
    arg.starts_with('-') && !arg[1..].starts_with(|c: char| c.is_ascii_digit())
}

/// **Every command, once.** The order here is the order `--help` prints.
const COMMANDS: &[Command] = &[
    Command {
        flag: "--help",
        short: Some("-h"),
        about: "List these commands and exit.",
        args: &[],
        json: false,
        run: |_| help(),
    },
    Command {
        flag: crate::app::keys_show::FLAG,
        short: Some("-k"),
        about: "Every binding name and the key it resolves to, across all layers.",
        args: &[],
        json: true,
        run: run_keys_show,
    },
    Command {
        flag: "--list-actions",
        short: Some("-a"),
        about: "Every action heca knows, with its category and whether it takes arguments.",
        args: &[],
        json: true,
        run: run_list_actions,
    },
    Command {
        flag: "--describe-action",
        short: Some("-d"),
        about: "One action in full: what it does, and every argument with its accepted values.",
        args: &[ArgDescriptor::required(
            "name",
            ArgKind::Text,
            "The action to describe. `--list-actions` prints them all.",
        )],
        json: true,
        run: run_describe_action,
    },
];

/// Answer a command-line question and exit, if the arguments asked one.
///
/// Returns `true` when it handled the invocation and the caller should not open a window.
pub(crate) fn run_if_requested(args: &[String]) -> bool {
    match answer(args) {
        Some(text) => {
            print!("{text}");
            true
        }
        None => false,
    }
}

/// **The whole of the command line, as a function of its arguments** — so every rule below is
/// testable without a process. `None` means "no command was asked for; open the window".
fn answer(args: &[String]) -> Option<String> {
    let flags: Vec<&String> = args.iter().filter(|a| is_flag(a)).collect();
    let Some(cmd) = COMMANDS
        .iter()
        .find(|c| flags.iter().any(|a| c.named_by(a)))
    else {
        // **An unrecognised flag is answered, not ignored.** Opening the window on a typo makes a
        // misspelling look like a broken app — the same silence that made `column`+`y` do nothing.
        let unknown = flags
            .iter()
            .find(|a| **a != JSON_FLAG && !COMMANDS.iter().any(|c| c.flag == a.as_str()))?;
        let near = nearest_flag(unknown);
        let hint = near
            .map(|f| format!(" — did you mean '{f}'?"))
            .unwrap_or_default();
        return Some(format!(
            "unknown command '{unknown}'{hint}\n\nRun `heca --help` for the list.\n"
        ));
    };

    let specs: Vec<ArgSpec> = cmd.args.iter().map(ArgSpec::from_descriptor).collect();
    // Positional: the words after the flag that are not themselves flags, matched to what the
    // command declared, in order.
    let positional: Vec<&String> = args
        .iter()
        .skip_while(|a| !cmd.named_by(a))
        .skip(1)
        .take_while(|a| !is_flag(a))
        .collect();
    if positional.len() > specs.len() {
        return Some(format!(
            "{} takes {} argument(s), got {}\n\nRun `heca --help` for the list.\n",
            cmd.flag,
            specs.len(),
            positional.len()
        ));
    }
    let supplied: HashMap<String, String> = specs
        .iter()
        .zip(positional.iter())
        .map(|(spec, value)| (spec.name.clone(), (*value).clone()))
        .collect();

    // The same check an action's arguments go through, so the sentence a user reads is the same
    // sentence wherever they got it wrong.
    let problems = check_args(&specs, &supplied);
    if !problems.is_empty() {
        let listed = problems
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Some(format!("{}: {listed}\n\n{}\n", cmd.flag, usage_of(cmd)));
    }

    Some((cmd.run)(&Invocation {
        args: supplied,
        json: cmd.json && args.iter().any(|a| a == JSON_FLAG || a == JSON_SHORT),
    }))
}

/// The nearest real flag to one nobody has heard of — by shared prefix, which is what a typo of a
/// long flag looks like.
fn nearest_flag(typo: &str) -> Option<&'static str> {
    COMMANDS
        .iter()
        .flat_map(|c| std::iter::once(c.flag).chain(c.short))
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
    let mut usage = format!("Usage: heca {}", cmd.flag);
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
                .trim_start_matches(c.flag)
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
        for a in c.args {
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

fn run_keys_show(inv: &Invocation) -> String {
    let config = heca_config::loader::AppConfig::load();
    // Conflicts are reported by the app proper; here they would be noise on stdout a script has to
    // filter, and the question asked is "what is bound", not "what collided".
    let mut conflicts = crate::app::conflicts::Conflicts::default();
    let keymaps = crate::app::registry::build_keymaps(&config.config, &mut conflicts);
    crate::app::keys_show::render(&keymaps.by_action, inv.json)
}

/// A category as one plain word. `ActionInfo` carries it as a serialisable value whose debug form
/// is quoted; the listing wants the word, not a literal.
fn category(c: &impl std::fmt::Debug) -> String {
    format!("{c:?}").trim_matches('"').to_string()
}

fn run_list_actions(inv: &Invocation) -> String {
    let catalog = ActionCatalog::with_builtins();
    let mut all = catalog.describe_all();
    all.sort_by(|a, b| a.name.cmp(&b.name));
    if inv.json {
        return serde_json::to_string_pretty(&all).unwrap_or_default() + "\n";
    }
    let name_width = all
        .iter()
        .map(|a| a.name.len())
        .max()
        .unwrap_or(0)
        .clamp(8, 40);
    let mut out = format!(
        "{:<name_width$}  {:<10}  ARGS  DESCRIPTION\n",
        "ACTION", "CATEGORY"
    );
    for a in &all {
        // The argument count is what tells a reader whether to go and ask `--describe-action`.
        let args_col = if a.args.is_empty() {
            "-".to_string()
        } else {
            a.args.len().to_string()
        };
        out.push_str(&format!(
            "{:<name_width$}  {:<10}  {args_col:<4}  {}\n",
            a.name,
            category(&a.category),
            a.description,
        ));
    }
    out
}

fn run_describe_action(inv: &Invocation) -> String {
    // Declared required, so it is here: the dispatcher refused the invocation otherwise.
    let name = inv.get("name").unwrap_or_default();
    let catalog = ActionCatalog::with_builtins();
    let Some(info) = catalog.describe(name) else {
        return format!("no action named '{name}'. `heca --list-actions` prints them.\n");
    };
    if inv.json {
        return serde_json::to_string_pretty(&info).unwrap_or_default() + "\n";
    }
    let mut out = format!(
        "{}  ({})\n{}\n",
        info.name,
        category(&info.category),
        info.description
    );
    if info.args.is_empty() {
        out.push_str("\nTakes no arguments.\n");
        return out;
    }
    out.push_str("\nArguments:\n");
    for a in &info.args {
        let need = if a.required { "required" } else { "optional" };
        out.push_str(&format!("  {} ({need}, {:?})\n", a.name, a.kind));
        out.push_str(&format!("      {}\n", a.description));
        if !a.values.is_empty() {
            out.push_str(&format!("      one of: {}\n", a.values.join(", ")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(args: &[&str]) -> Option<String> {
        answer(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    /// **The help lists exactly what runs, arguments included.** Both are rendered from the table
    /// the dispatcher walks, so a command cannot be documented without existing, nor exist without
    /// being documented — which is the way a `--help` normally goes stale.
    #[test]
    fn the_help_lists_every_command_and_every_argument_it_declares() {
        let text = help();
        for c in COMMANDS {
            assert!(
                text.contains(c.flag),
                "`{}` runs but --help does not list it",
                c.flag
            );
            for a in c.args {
                assert!(
                    text.contains(a.name) && text.contains(a.description),
                    "`{}` declares `{}` but --help does not describe it",
                    c.flag,
                    a.name
                );
            }
        }
    }

    /// **Nothing is asked for ⇒ open the window.** The only invocation that is not a question.
    #[test]
    fn no_command_opens_the_window() {
        assert!(ask(&[]).is_none());
    }

    /// **A missing argument is a sentence, not silence** — and the same sentence an action gives,
    /// because it is the same check.
    #[test]
    fn a_missing_argument_says_which_one_and_how_to_call_it() {
        let out = ask(&["--describe-action"]).expect("the command was recognised");
        assert!(
            out.contains("missing required argument 'name'"),
            "got: {out}"
        );
        assert!(
            out.contains("Usage: heca --describe-action <name> [--json]"),
            "got: {out}"
        );
    }

    /// **A command cannot be handed more than it declared.**
    #[test]
    fn too_many_arguments_are_refused() {
        let out = ask(&["--describe-action", "resize", "extra"]).expect("recognised");
        assert!(out.contains("takes 1 argument(s), got 2"), "got: {out}");
    }

    /// **An unrecognised flag is answered, with the nearest real one offered.** Opening the window
    /// on a typo makes a misspelling look like a broken app.
    #[test]
    fn an_unknown_flag_is_answered_and_a_near_one_is_offered() {
        let out = ask(&["--list-action"]).expect("an unknown flag is still a question");
        assert!(
            out.contains("unknown command '--list-action'"),
            "got: {out}"
        );
        assert!(out.contains("did you mean '--list-actions'?"), "got: {out}");
        assert!(out.contains("--help"), "…and says where the list is");
    }

    /// **A one-letter form runs the same command**, and no two commands claim the same letter — a
    /// collision would make one of them unreachable, silently, with nothing failing.
    #[test]
    fn every_short_form_runs_its_command_and_none_collide() {
        let mut seen: Vec<&str> = Vec::new();
        for c in COMMANDS {
            let Some(short) = c.short else { continue };
            assert_eq!(
                short.len(),
                2,
                "`{short}` is not a single letter after its dash"
            );
            assert!(
                !seen.contains(&short),
                "`{short}` is claimed by two commands"
            );
            seen.push(short);
            assert!(
                ask(&[short]).is_some(),
                "`{short}` is declared for {} but names nothing",
                c.flag
            );
            assert!(help().contains(short), "…and --help does not show it");
        }
        // The same command, either spelling, same answer.
        assert_eq!(ask(&["-a"]), ask(&["--list-actions"]));
        assert_eq!(
            ask(&["-d", "resize"]),
            ask(&["--describe-action", "resize"])
        );
        // Including the JSON switch.
        assert_eq!(
            ask(&["-d", "resize", "-j"]),
            ask(&["--describe-action", "resize", "--json"])
        );
    }

    /// **A negative number is a value, not a flag** — so an argument may start with a dash without
    /// the dispatcher mistaking it for one.
    #[test]
    fn a_negative_number_is_read_as_an_argument() {
        let out = ask(&["--describe-action", "-40"]).expect("recognised");
        assert!(out.contains("no action named '-40'"), "got: {out}");
    }

    /// Every action is listed, and one can be asked about in full.
    #[test]
    fn actions_are_listable_and_describable() {
        let listed = ask(&["--list-actions"]).expect("recognised");
        assert!(listed.contains("resize"), "the listing names every action");
        assert!(
            listed.lines().count() > 20,
            "…and it is the whole catalog, not a sample"
        );

        let one = ask(&["--describe-action", "resize"]).expect("recognised");
        assert!(one.contains("edge"), "an argument is named");
        assert!(one.contains("optional"), "…and says whether it is required");
        assert!(
            one.contains("auto, top, bottom, left, right"),
            "…and lists what it accepts"
        );

        let json = ask(&["--describe-action", "resize", "--json"]).expect("recognised");
        assert!(
            json.trim_start().starts_with('{'),
            "--json gives a script JSON"
        );
    }

    /// A name nobody has heard of is answered, not ignored.
    #[test]
    fn an_unknown_action_says_so_and_says_where_to_look() {
        let out = ask(&["--describe-action", "nope"]).expect("recognised");
        assert!(out.contains("no action named 'nope'"));
        assert!(
            out.contains("--list-actions"),
            "and points at the way to find one"
        );
    }
}
