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
    let choices = Choices::of(&missing, &Places::of(state, &meta.args, &missing, &given));
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

/// **What an argument that names something can be**, read from the app as it is now: the docks
/// mounted, the workspaces, the columns of the workspace the argument is about, the panes. Only the
/// list the argument asks for is filled.
#[derive(Default)]
struct Places {
    docks: Vec<String>,
    named: Vec<Choice>,
}

impl Places {
    fn of(state: &AppState, args: &[ArgSpec], arg: &ArgSpec, given: &[(String, String)]) -> Self {
        let layout = state.layout();
        let (programs, panes, add_name) = (
            &state.client.programs,
            &state.chrome_state.panes,
            state.settings.pane_renamed_add_process_name,
        );
        let named = match arg.kind {
            ArgKind::Workspace => named(workspace_entries(layout, None), |entry| {
                let idx = entry.id.strip_prefix("ws:")?;
                Some((idx.to_owned(), entry.current))
            }),
            ArgKind::Column => {
                let ws = workspace_for(args, arg, given)
                    .unwrap_or_else(|| layout.active_workspace_idx());
                named(
                    column_entries(layout, ws, programs, panes, add_name),
                    |entry| Some((entry.id.rsplit(':').next()?.to_owned(), entry.current)),
                )
            }
            ArgKind::Pane => named(
                pane_entries(layout, programs, panes, add_name, None),
                |entry| Some((entry.pane?.0.to_string(), entry.current)),
            ),
            _ => Vec::new(),
        };
        Self {
            docks: docks(state),
            named,
        }
    }
}

/// The workspace a column argument is about: the value of the nearest workspace argument declared
/// before it, when one was chosen.
fn workspace_for(args: &[ArgSpec], arg: &ArgSpec, given: &[(String, String)]) -> Option<usize> {
    let before = args.iter().position(|a| a.name == arg.name)?;
    args[..before]
        .iter()
        .rev()
        .find(|a| a.kind == ArgKind::Workspace)
        .and_then(|a| given.iter().find(|(name, _)| *name == a.name))
        .and_then(|(_, value)| value.parse().ok())
}

/// Palette rows as choices: each row's value (read out of it by `value`), its name and where it is,
/// and whether it is the one you are on — which is where the selection starts.
fn named(
    entries: Vec<PaletteEntry>,
    value: impl Fn(&PaletteEntry) -> Option<(String, bool)>,
) -> Vec<Choice> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let (value, current) = value(&entry)?;
            Some(Choice {
                value,
                label: entry.label,
                description: Some(entry.description),
                current,
            })
        })
        .collect()
}

/// The docks mounted right now, by id — what a dock argument offers.
fn docks(state: &AppState) -> Vec<String> {
    crate::RegionId::ALL
        .iter()
        .flat_map(|region| state.chrome_host.contributions(*region).iter())
        .map(|mounted| mounted.id().to_string())
        .collect()
}

/// One row an argument offers: the value it sets, what the user reads, and whether it is the one
/// they are on now.
#[derive(Clone, Debug, PartialEq)]
struct Choice {
    value: String,
    label: String,
    description: Option<String>,
    current: bool,
}

impl Choice {
    /// A row whose words are its value.
    fn word(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            label: value.clone(),
            value,
            description: None,
            current: false,
        }
    }
}

/// **The rows one argument offers.**
enum Choices {
    /// A list to pick from: its words, the docks, true/false, or the workspaces, columns or panes by
    /// name.
    List(Vec<Choice>),
    /// Anything typed: the typed text is the one row, once it reads as the argument's kind.
    Typed,
}

impl Choices {
    fn of(arg: &ArgSpec, places: &Places) -> Self {
        let words = |words: &[String]| words.iter().map(Choice::word).collect();
        match arg.kind {
            ArgKind::Enum => Self::List(words(&arg.values)),
            ArgKind::Dock => Self::List(words(&places.docks)),
            ArgKind::Bool => Self::List(words(&["true".into(), "false".into()])),
            ArgKind::Workspace | ArgKind::Column | ArgKind::Pane => {
                Self::List(places.named.clone())
            }
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
            Self::List(choices) => choices
                .iter()
                .map(|choice| {
                    let mut row = Command::new(choice.label.clone(), choose(choice.value.clone()))
                        .completion(choice.value.clone())
                        .current(choice.current)
                        .preselect(choice.current);
                    if let Some(description) = &choice.description {
                        row = row.description(description.clone());
                    }
                    row
                })
                .collect(),
            Self::Typed => {
                let typed = query.trim();
                if typed.is_empty() {
                    // **A blank field says what it wants**: one row naming the argument and what to
                    // type. Choosing it asks the same question again.
                    let same = ask_again(action, given);
                    let emit = emit.clone();
                    return vec![
                        Command::new(
                            format!("{}: type {}", arg.name, arg.expected()),
                            move || emit.fire(same.clone()),
                        )
                        .description(arg.description.clone()),
                    ];
                }
                if !arg.accepts(typed) {
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
        let places = Places {
            docks: docks.clone(),
            named: Vec::new(),
        };
        let values = |choices: Choices| match choices {
            Choices::List(list) => list.into_iter().map(|c| c.value).collect::<Vec<_>>(),
            Choices::Typed => panic!("a list"),
        };
        assert_eq!(
            values(Choices::of(
                &arg("r", ArgKind::Enum, true, &["a", "b"]),
                &places
            )),
            ["a", "b"]
        );
        assert_eq!(
            values(Choices::of(&arg("d", ArgKind::Dock, true, &[]), &places)),
            docks
        );
        assert_eq!(
            values(Choices::of(&arg("b", ArgKind::Bool, true, &[]), &places)),
            ["true", "false"]
        );
        assert!(matches!(
            Choices::of(&arg("n", ArgKind::Int, true, &[]), &places),
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
        assert_eq!(
            Choices::Typed
                .commands("a", &[], &number, "  ", &emit)
                .len(),
            1,
            "nothing typed: one row saying what to type"
        );
    }

    /// **A column argument is about the workspace chosen before it** — the nearest workspace
    /// argument declared earlier — and the current one when none was chosen.
    #[test]
    fn a_column_follows_the_workspace_chosen_before_it() {
        let args = [
            arg("src_ws", ArgKind::Workspace, true, &[]),
            arg("src_col", ArgKind::Column, true, &[]),
            arg("dst_ws", ArgKind::Workspace, true, &[]),
            arg("dst_idx", ArgKind::Column, true, &[]),
        ];
        let given = vec![
            ("src_ws".to_string(), "1".to_string()),
            ("dst_ws".into(), "3".into()),
        ];
        assert_eq!(workspace_for(&args, &args[1], &given), Some(1));
        assert_eq!(
            workspace_for(&args, &args[3], &given),
            Some(3),
            "the nearest one before it"
        );
        assert_eq!(
            workspace_for(&args, &args[1], &[]),
            None,
            "none chosen: the current one"
        );
    }

    /// **Workspaces, columns and panes are offered by name**, each with the value it sets and
    /// whether it is the one you are on — the row the selection starts on.
    #[test]
    fn workspaces_columns_and_panes_are_offered_by_name() {
        let named = vec![
            Choice {
                value: "0".into(),
                label: "main".into(),
                description: Some("2 columns".into()),
                current: false,
            },
            Choice {
                value: "1".into(),
                label: "heca-pro".into(),
                description: None,
                current: true,
            },
        ];
        let places = Places {
            docks: Vec::new(),
            named: named.clone(),
        };
        for kind in [ArgKind::Workspace, ArgKind::Column, ArgKind::Pane] {
            let Choices::List(list) = Choices::of(&arg("x", kind, true, &[]), &places) else {
                panic!("a list for {kind:?}")
            };
            assert_eq!(list, named);
        }
        let emit = ChromeIntentEmitter::of(InteractionSource::Keyboard, |_, _| {});
        let rows = Choices::List(named).commands(
            "move_column",
            &[],
            &arg("src_ws", ArgKind::Workspace, true, &[]),
            "",
            &emit,
        );
        assert_eq!(rows.len(), 2);
    }
}
