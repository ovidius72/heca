//! The argument model — how anything in heca declares the arguments it takes, and checks a call
//! against that declaration.
//!
//! Two unrelated callers share it: every action ([`ActionMeta::args`](crate::actions::ActionMeta::args))
//! and the command line (`app/cli.rs`). Checking both with the same [`check_args`] is what makes a
//! bad keybinding and a bad `heca --describe-action` produce the same sentence.

/// The type of one action argument — what a supplied value has to look like.
///
/// Deliberately small: an argument arrives as text (a `config.toml` binding, an
/// [`Intent`](crate::chrome::Intent)'s [`PropValue`](crate::chrome::PropValue) flattened to a
/// string, an RPC word), so this says how to read that text, not how it is stored in the
/// [`WmAction`](crate::input::WmAction) variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgKind {
    /// A whole, non-negative number — every numeric argument in the built-in set is a `u64` id or a
    /// `usize` index.
    Int,
    /// A decimal number (sizes, positions, resize amounts).
    Float,
    /// `true` or `false`.
    Bool,
    /// Free text (a name, a URL, a command line).
    Text,
    /// One of a fixed set of spellings, listed in [`ArgDescriptor::values`] /
    /// [`ArgSpec::values`] — the vocabulary the argument's own `FromStr` accepts.
    Enum,
}

/// Static declaration of **one argument** an action takes. The built-in half of [`ArgSpec`], living
/// in [`ActionDescriptor::args`](crate::actions::ActionDescriptor::args) the same way [`ActionDescriptor`](crate::actions::ActionDescriptor) is the static half of
/// [`ActionMeta`](crate::actions::ActionMeta).
///
/// This exists so an action can *say* what it takes. Before it, the only record of an argument's
/// name was the string literal inside [`resolve_action`](crate::input::resolve_action)'s match arm — so
/// a misspelled argument was not a mistake anyone could detect: a required one made the whole
/// action fail to build (and, from a keybinding, silently never fire), and an optional one silently
/// took its default.
#[derive(Debug, Clone, Copy)]
pub struct ArgDescriptor {
    /// The argument's name — exactly the key [`resolve_action`](crate::input::resolve_action) reads.
    pub name: &'static str,
    pub kind: ArgKind,
    /// `false` when the action supplies a default for a missing value.
    pub required: bool,
    pub description: &'static str,
    /// For [`ArgKind::Enum`]: every accepted spelling, aliases included. Empty for other kinds.
    pub values: &'static [&'static str],
}

impl ArgDescriptor {
    /// An argument the action cannot run without.
    pub const fn required(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            kind,
            required: true,
            description,
            values: &[],
        }
    }

    /// An argument with a default — omitting it is legal, misspelling it is not.
    pub const fn optional(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self {
            name,
            kind,
            required: false,
            description,
            values: &[],
        }
    }

    /// A required argument limited to a fixed vocabulary. `values` must list every spelling the
    /// argument's `FromStr` accepts — [`EnumArg::VALUES`](crate::args::EnumArg::VALUES) provides
    /// that list next to the parser, so the two cannot drift.
    pub const fn required_enum(
        name: &'static str,
        values: &'static [&'static str],
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind: ArgKind::Enum,
            required: true,
            description,
            values,
        }
    }

    /// A vocabulary-limited argument with a default.
    pub const fn optional_enum(
        name: &'static str,
        values: &'static [&'static str],
        description: &'static str,
    ) -> Self {
        Self {
            name,
            kind: ArgKind::Enum,
            required: false,
            description,
            values,
        }
    }
}

/// Runtime, owned declaration of one action argument — what [`ActionMeta::args`](crate::actions::ActionMeta::args) holds and what RPC
/// introspection serializes. The owned counterpart of [`ArgDescriptor`], for the same reason
/// [`ActionMeta`](crate::actions::ActionMeta) is the owned counterpart of [`ActionDescriptor`](crate::actions::ActionDescriptor): an action registered at runtime
/// by a provider or a plugin has no `'static` strings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ArgSpec {
    pub name: String,
    pub kind: ArgKind,
    pub required: bool,
    pub description: String,
    /// For [`ArgKind::Enum`]: every accepted spelling. Empty for other kinds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
}

impl ArgSpec {
    /// Load a built-in's static declaration into the owned runtime form.
    pub fn from_descriptor(d: &ArgDescriptor) -> Self {
        Self {
            name: d.name.to_string(),
            kind: d.kind,
            required: d.required,
            description: d.description.to_string(),
            values: d.values.iter().map(|v| v.to_string()).collect(),
        }
    }

    /// A value this argument accepts, for building a representative action from a declaration
    /// alone (see [`ActionCatalog::with_builtins`](crate::actions::ActionCatalog::with_builtins)) or for a test that needs a well-formed call.
    pub fn sample_value(&self) -> String {
        match self.kind {
            ArgKind::Int => "0".to_string(),
            ArgKind::Float => "0".to_string(),
            ArgKind::Bool => "false".to_string(),
            ArgKind::Text => String::new(),
            // A vocabulary argument has no neutral value — the first spelling is the only one
            // guaranteed to parse.
            ArgKind::Enum => self.values.first().cloned().unwrap_or_default(),
        }
    }

    /// Whether `value` reads as this argument's [`kind`](ArgSpec::kind). Enum spellings are matched
    /// case-insensitively, because every argument `FromStr` in the app lowercases before matching.
    pub fn accepts(&self, value: &str) -> bool {
        match self.kind {
            ArgKind::Int => value.parse::<u64>().is_ok(),
            ArgKind::Float => value.parse::<f64>().is_ok(),
            ArgKind::Bool => value.parse::<bool>().is_ok(),
            ArgKind::Text => true,
            ArgKind::Enum => self.values.iter().any(|v| v.eq_ignore_ascii_case(value)),
        }
    }
}

/// Something wrong with the arguments handed to an action. Produced by [`check_args`] and reported
/// at whichever door found it — never thrown away, which is the whole point of declaring arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgProblem {
    /// A name the action does not take. Usually a misspelling of one it does, so the nearest
    /// declared name is offered when there is an obvious one.
    Unknown {
        name: String,
        did_you_mean: Option<String>,
    },
    /// A required argument was not supplied. The action cannot be built.
    Missing { name: String },
    /// The value does not read as the declared kind.
    BadValue {
        name: String,
        value: String,
        expected: String,
    },
}

impl std::fmt::Display for ArgProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgProblem::Unknown {
                name,
                did_you_mean: Some(near),
            } => {
                write!(f, "unknown argument '{name}' — did you mean '{near}'?")
            }
            ArgProblem::Unknown {
                name,
                did_you_mean: None,
            } => {
                write!(f, "unknown argument '{name}'")
            }
            ArgProblem::Missing { name } => write!(f, "missing required argument '{name}'"),
            ArgProblem::BadValue {
                name,
                value,
                expected,
            } => {
                write!(f, "argument '{name}' expected {expected}, got '{value}'")
            }
        }
    }
}

/// Compare the arguments actually supplied against what the action declares it takes.
///
/// **The one check**, used by every door that carries arguments: a `config.toml` binding at load
/// (`action_ref_from_config`) and a declarative [`Intent`](crate::chrome::Intent) at dispatch
/// (`dispatch_view_intent`). An empty result means the call is well-formed.
///
/// It reports rather than decides: a [`Missing`](ArgProblem::Missing) argument means the action
/// cannot run, while an [`Unknown`](ArgProblem::Unknown) or [`BadValue`](ArgProblem::BadValue) one
/// costs only itself — the same rule the declarative UI model applies to a widget property
/// (`docs/chrome-and-ui.md` Part I §6). The caller decides; this only makes sure nothing is silent.
pub fn check_args(
    specs: &[ArgSpec],
    supplied: &std::collections::HashMap<String, String>,
) -> Vec<ArgProblem> {
    let mut problems = Vec::new();
    for spec in specs {
        match supplied.get(&spec.name) {
            Some(value) if !spec.accepts(value) => problems.push(ArgProblem::BadValue {
                name: spec.name.clone(),
                value: value.clone(),
                expected: describe_expected(spec),
            }),
            None if spec.required => problems.push(ArgProblem::Missing {
                name: spec.name.clone(),
            }),
            _ => {}
        }
    }
    for name in supplied.keys() {
        if !specs.iter().any(|s| &s.name == name) {
            problems.push(ArgProblem::Unknown {
                name: name.clone(),
                did_you_mean: nearest_name(name, specs),
            });
        }
    }
    // Stable order so the same mistake reports identically every run (a `HashMap` iteration is not).
    problems.sort_by_key(|p| match p {
        ArgProblem::Missing { name }
        | ArgProblem::BadValue { name, .. }
        | ArgProblem::Unknown { name, .. } => name.clone(),
    });
    problems
}

fn describe_expected(spec: &ArgSpec) -> String {
    match spec.kind {
        ArgKind::Int => "a whole number".to_string(),
        ArgKind::Float => "a number".to_string(),
        ArgKind::Bool => "true or false".to_string(),
        ArgKind::Text => "text".to_string(),
        ArgKind::Enum => format!("one of {}", spec.values.join(", ")),
    }
}

/// The declared name closest to `name`, when one is close enough to be worth suggesting. A
/// misspelling is the common case, so guessing costs nothing and saves the reader the lookup.
fn nearest_name(name: &str, specs: &[ArgSpec]) -> Option<String> {
    let limit = (name.len() / 3).max(1);
    specs
        .iter()
        .map(|s| (edit_distance(name, &s.name), &s.name))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n.clone())
}

/// Levenshtein distance, two rows at a time. Only ever run on argument names (a handful of short
/// strings) at config load or on a failed dispatch.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        cur[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// A well-formed argument map built from a declaration alone — every declared argument set to a
/// value of its own kind. Used to build a representative action from its metadata, and by the tests
/// that check a declaration against the code that reads it.
pub fn sample_args(args: &[ArgSpec]) -> std::collections::HashMap<String, String> {
    args.iter()
        .map(|a| (a.name.clone(), a.sample_value()))
        .collect()
}

/// A type used as an action argument that accepts a **fixed vocabulary** of spellings.
///
/// [`VALUES`](EnumArg::VALUES) lists every spelling the type's `FromStr` accepts, aliases included,
/// and lives next to that `FromStr` so the two are read and changed together. An
/// [`ArgDescriptor`](crate::args::ArgDescriptor) points at it rather than restating the list, so
/// an action's declared vocabulary is the parser's vocabulary by construction. The test
/// `every_enum_arg_value_parses` walks each list through its own parser.
pub trait EnumArg: std::str::FromStr {
    /// Every accepted spelling, canonical form first.
    const VALUES: &'static [&'static str];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::builtin_args;

    #[test]
    fn check_args_names_what_is_wrong() {
        let specs = builtin_args("move_column").unwrap();

        assert!(
            check_args(&specs, &sample_args(&specs)).is_empty(),
            "a well-formed call is quiet"
        );

        // A misspelling is named, and the intended argument is offered.
        let mut typo = sample_args(&specs);
        typo.remove("src_col");
        typo.insert("src_colum".to_string(), "0".to_string());
        let problems = check_args(&specs, &typo);
        assert!(problems.contains(&ArgProblem::Missing {
            name: "src_col".to_string()
        }));
        assert!(problems.contains(&ArgProblem::Unknown {
            name: "src_colum".to_string(),
            did_you_mean: Some("src_col".to_string()),
        }));

        // A value of the wrong shape is named too — the old path just dropped it.
        let mut bad = sample_args(&specs);
        bad.insert("focus".to_string(), "yes".to_string());
        assert!(check_args(&specs, &bad).iter().any(|p| matches!(
            p,
            ArgProblem::BadValue { name, .. } if name == "focus"
        )));
    }

    /// The defect in one test: an **optional** argument spelled wrong used to be pure silence —
    /// the action ran, on a default nobody asked for, in every build.
    #[test]
    fn a_misspelled_optional_argument_is_reported_even_though_the_action_still_runs() {
        let specs = builtin_args("take_pane").unwrap();
        let mut args = sample_args(&specs);
        args.remove("focus_after");
        args.insert("focus_afterr".to_string(), "true".to_string());

        assert!(
            crate::input::resolve_action("take_pane", &args).is_some(),
            "the action still builds — one bad argument costs only itself",
        );
        assert_eq!(
            check_args(&specs, &args),
            vec![ArgProblem::Unknown {
                name: "focus_afterr".to_string(),
                did_you_mean: Some("focus_after".to_string()),
            }],
            "…but nobody is left guessing why it did not take effect",
        );
    }

    /// The sentence a user actually reads. It is written by the `Display` impl and printed by the
    /// two doors, so pin it here — the printing itself is skipped under `cfg(test)`, exactly as
    /// keybinding-conflict logging is.
    #[test]
    fn a_problem_reads_as_a_sentence() {
        assert_eq!(
            ArgProblem::Unknown {
                name: "ws_idxx".to_string(),
                did_you_mean: Some("ws_idx".to_string()),
            }
            .to_string(),
            "unknown argument 'ws_idxx' — did you mean 'ws_idx'?",
        );
        assert_eq!(
            ArgProblem::Unknown {
                name: "colour".to_string(),
                did_you_mean: None
            }
            .to_string(),
            "unknown argument 'colour'",
        );
        assert_eq!(
            ArgProblem::Missing {
                name: "ws_idx".to_string()
            }
            .to_string(),
            "missing required argument 'ws_idx'",
        );
        assert_eq!(
            ArgProblem::BadValue {
                name: "focus".to_string(),
                value: "yes".to_string(),
                expected: "true or false".to_string(),
            }
            .to_string(),
            "argument 'focus' expected true or false, got 'yes'",
        );

        // An enum lists what it will accept, so the reader does not have to go looking.
        let specs = builtin_args("resize").unwrap();
        let mut bad = sample_args(&specs);
        bad.insert("edge".to_string(), "sideways".to_string());
        let message = check_args(&specs, &bad)
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        assert_eq!(
            message,
            "argument 'edge' expected one of auto, top, bottom, left, right, got 'sideways'",
        );
    }
}
