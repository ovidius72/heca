//! **The palette asks for an action's arguments** — one step per required argument the action has
//! not been given yet, then it runs the action.
//!
//! Choosing an action that needs something (a dock to move, a name to give) opens a step whose rows
//! fit the argument: its words for a fixed list, the docks mounted right now for a dock, true and
//! false for a yes/no, and the typed text itself for anything free. Each choice reopens the palette
//! for the next argument, carrying what was chosen, until nothing required is missing. It is the same
//! for every action — heca's own and every extension's — because it reads only what each action
//! declares in its `args`. A script asks the same way: `command_palette action=chrome.container.move_to_region`.

use heca_grid_ui::widgets::{Command, CommandPalette};

use super::*;
use crate::args::{ArgKind, ArgSpec};
use crate::rpc::line::{Arg, format_line, tokenize};

/// The arguments already chosen, read back from their `name=value` words. Anything else in the text
/// is not an argument and is left out.
pub(crate) fn palette_arguments_given(text: Option<&str>) -> Vec<(String, String)> {
    let Some(text) = text else { return Vec::new() };
    tokenize(text)
        .map(|words| {
            words
                .into_iter()
                .filter_map(|word| match word.arg {
                    Arg::Named { name, value } => Some((name, value)),
                    Arg::Positional(_) => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// **Ask for `action`'s next missing argument, or run it** when nothing required is missing.
pub(crate) fn ask_action_arguments(
    state: &mut AppState,
    action: &str,
    given: Vec<(String, String)>,
) -> crate::actions::ActionResult {
    let Some(meta) = state.action_catalog.find(action).cloned() else {
        return Err(format!("no action named '{action}'"));
    };
    let emit = ChromeIntentEmitter::new(&state.event_proxy, InteractionSource::Keyboard);
    let Some(missing) = next_missing(&meta.args, &given).cloned() else {
        let owners = owners(state);
        emit.fire(run_intent(&meta.name, &given, owners.get(&meta.name)));
        return Ok(());
    };
    let choices = Choices::of(&missing, docks(state));
    let label = meta.label.clone();
    let placeholder = format!("{label} › {}: {}", missing.name, missing.description);
    let palette = CommandPalette::new()
        .placeholder(&placeholder)
        .search(search_model(state, "arguments", state.settings.search_case))
        .source(move |query| choices.commands(&meta.name, &given, &missing, query, &emit));
    open_command_palette(state, None, None, Some(palette));
    Ok(())
}

/// The first required argument not given yet, in declaration order.
fn next_missing<'a>(args: &'a [ArgSpec], given: &[(String, String)]) -> Option<&'a ArgSpec> {
    args.iter()
        .find(|arg| arg.required && !given.iter().any(|(name, _)| *name == arg.name))
}

/// What running the action with everything chosen sends: aimed at the component that owns it, focus
/// first, exactly as an action row is.
fn run_intent(
    action: &str,
    given: &[(String, String)],
    owner: Option<&OwnerInfo>,
) -> InteractionIntent {
    let mut intent = Intent::new(action);
    for (name, value) in given {
        intent = intent.arg(name.clone(), PropValue::Text(value.clone()));
    }
    match owner {
        Some(owner) => InteractionIntent::FocusContainerThenAction {
            container: owner.mount.clone(),
            action: intent,
        },
        None => InteractionIntent::View(intent),
    }
}

/// The docks mounted right now, by id — what a dock argument offers.
fn docks(state: &AppState) -> Vec<String> {
    crate::RegionId::ALL
        .iter()
        .flat_map(|region| state.chrome_host.contributions(*region).iter())
        .map(|mounted| mounted.id().to_string())
        .collect()
}

/// **The rows one argument offers.**
enum Choices {
    /// A fixed list: its words, the docks, or true/false.
    Words(Vec<String>),
    /// Anything typed: the typed text is the one row, once it reads as the argument's kind.
    Typed,
}

impl Choices {
    fn of(arg: &ArgSpec, docks: Vec<String>) -> Self {
        match arg.kind {
            ArgKind::Enum => Self::Words(arg.values.clone()),
            ArgKind::Dock => Self::Words(docks),
            ArgKind::Bool => Self::Words(vec!["true".into(), "false".into()]),
            ArgKind::Text | ArgKind::Int | ArgKind::Float => Self::Typed,
        }
    }

    fn commands(
        &self,
        action: &str,
        given: &[(String, String)],
        arg: &ArgSpec,
        query: &str,
        emit: &ChromeIntentEmitter,
    ) -> Vec<Command> {
        let choose = |value: String| {
            let mut chosen = given.to_vec();
            chosen.push((arg.name.clone(), value));
            let emit = emit.clone();
            let next = ask_again(action, &chosen);
            move || emit.fire(next.clone())
        };
        match self {
            Self::Words(words) => words
                .iter()
                .map(|word| Command::new(word.clone(), choose(word.clone())))
                .collect(),
            Self::Typed => {
                let typed = query.trim();
                if typed.is_empty() || !arg.accepts(typed) {
                    return Vec::new();
                }
                vec![
                    Command::new(format!("{}: {typed}", arg.name), choose(typed.to_owned()))
                        .fallback(true),
                ]
            }
        }
    }
}

/// The palette again, for the same action with what has been chosen so far.
pub(super) fn ask_again(action: &str, chosen: &[(String, String)]) -> InteractionIntent {
    let words: Vec<Arg> = chosen
        .iter()
        .map(|(name, value)| Arg::Named {
            name: name.clone(),
            value: value.clone(),
        })
        .collect();
    // The words alone, spelt as the RPC line spells them, so a value with spaces reads back whole.
    let arguments = format_line(action, &words)
        .ok()
        .and_then(|line| line.split_once(' ').map(|(_, rest)| rest.to_owned()))
        .unwrap_or_default();
    InteractionIntent::View(
        Intent::new("command_palette")
            .arg("action", PropValue::Text(action.to_owned()))
            .arg("arguments", PropValue::Text(arguments)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arg(name: &str, kind: ArgKind, required: bool, values: &[&str]) -> ArgSpec {
        ArgSpec {
            name: name.into(),
            kind,
            required,
            description: String::new(),
            values: values.iter().map(|v| v.to_string()).collect(),
        }
    }

    /// **One step per required argument, in order**, skipping what was given and what is optional.
    #[test]
    fn the_next_step_is_the_first_required_argument_not_given() {
        let args = [
            arg("container_id", ArgKind::Dock, true, &[]),
            arg("note", ArgKind::Text, false, &[]),
            arg("region", ArgKind::Enum, true, &["left", "right"]),
        ];
        assert_eq!(
            next_missing(&args, &[]).map(|a| a.name.as_str()),
            Some("container_id")
        );
        let given = vec![("container_id".to_string(), "workspaces".to_string())];
        assert_eq!(
            next_missing(&args, &given).map(|a| a.name.as_str()),
            Some("region")
        );
        let given = vec![given[0].clone(), ("region".into(), "left".into())];
        assert!(next_missing(&args, &given).is_none(), "all given: it runs");
    }

    /// **What was chosen reads back whole**, a value with spaces included.
    #[test]
    fn chosen_arguments_round_trip_through_their_words() {
        let chosen = vec![
            ("name".to_string(), "my room".to_string()),
            ("container_id".into(), "heca-pro.rooms".into()),
        ];
        let InteractionIntent::View(intent) = ask_again("rename_thing", &chosen) else {
            panic!("a view intent")
        };
        let Some(PropValue::Text(words)) = intent.args.get("arguments") else {
            panic!("arguments")
        };
        assert_eq!(palette_arguments_given(Some(words)), chosen);
    }

    /// **The rows fit the argument**: a list's words, the mounted docks, true/false, or the typed
    /// text once it reads as the argument's kind.
    #[test]
    fn each_kind_offers_what_fits_it() {
        let docks = vec!["workspaces".to_string(), "heca-pro.rooms".to_string()];
        assert!(
            matches!(Choices::of(&arg("r", ArgKind::Enum, true, &["a", "b"]), docks.clone()), Choices::Words(w) if w == ["a", "b"])
        );
        assert!(
            matches!(Choices::of(&arg("d", ArgKind::Dock, true, &[]), docks.clone()), Choices::Words(w) if w == docks)
        );
        assert!(
            matches!(Choices::of(&arg("b", ArgKind::Bool, true, &[]), Vec::new()), Choices::Words(w) if w == ["true", "false"])
        );
        assert!(matches!(
            Choices::of(&arg("n", ArgKind::Int, true, &[]), Vec::new()),
            Choices::Typed
        ));
        let number = arg("n", ArgKind::Int, true, &[]);
        let emit = ChromeIntentEmitter::of(InteractionSource::Keyboard, |_, _| {});
        assert!(
            Choices::Typed
                .commands("a", &[], &number, "abc", &emit)
                .is_empty(),
            "not a number: no row"
        );
        assert_eq!(
            Choices::Typed
                .commands("a", &[], &number, "12", &emit)
                .len(),
            1
        );
        assert!(
            Choices::Typed
                .commands("a", &[], &number, "  ", &emit)
                .is_empty(),
            "nothing typed: no row"
        );
    }
}
