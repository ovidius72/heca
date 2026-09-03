//! Action registry — dispatch table for all window-manager actions.
//!
//! This module provides the `ActionRegistry` which maps `WmAction` discriminants
//! to named handler functions. It also preserves the static metadata catalog
//! (labels, categories, default bindings) for the command palette and docs.

use heca_grid_ui::Glyph;
use std::collections::HashMap;

/// Handler signature for all window-manager actions.
///
/// The `WmAction` parameter carries the full variant (including any embedded
/// arguments), so the same handler can serve both unit and parameterized
/// variants that share a discriminant.
pub type ActionHandler = fn(&mut crate::app_state::AppState, &crate::input::WmAction);

/// Category for grouping actions in the command palette and documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionCategory {
    /// Focus, sidebar navigation, workspace switching.
    Navigation,
    /// Splits, resizes, moves, swaps.
    Layout,
    /// Pane lifecycle: float, hide, close, rename, select.
    Pane,
    /// Workspace creation, renaming, switching.
    Workspace,
    /// Session-level: overview, save, load.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no session-level actions exist yet; used when overview/save/load land")
    )]
    Session,
    /// UI chrome: sidebar toggle, tab management.
    Chrome,
    /// System: command palette, quit.
    System,
}

impl ActionCategory {
    /// Human-readable category name for UI display (and the RPC-introspection category string).
    pub const fn label(self) -> &'static str {
        match self {
            ActionCategory::Navigation => "Navigation",
            ActionCategory::Layout => "Layout",
            ActionCategory::Pane => "Pane",
            ActionCategory::Workspace => "Workspace",
            ActionCategory::Session => "Session",
            ActionCategory::Chrome => "Chrome",
            ActionCategory::System => "System",
        }
    }

}

/// The glyph an action shows when it declares none of its own — **one neutral mark, for every
/// action**.
///
/// A per-*category* fallback was tried first and removed the same day (2026-07-30): it filled every
/// row, but then all four `focus_*` actions wore the same arrow, every `Layout` action the same
/// split, and a glyph that looks specific while meaning only "this is a Navigation action" reads as
/// wrong rather than as generic. A plain dot claims nothing. An action that wants meaning declares
/// its own icon, which always wins.
pub const GENERIC_ACTION_ICON: Glyph = Glyph::Circle;

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
/// in [`ActionDescriptor::args`] the same way [`ActionDescriptor`] is the static half of
/// [`ActionMeta`].
///
/// This exists so an action can *say* what it takes. Before it, the only record of an argument's
/// name was the string literal inside [`build_action`](crate::input::build_action)'s match arm — so
/// a misspelled argument was not a mistake anyone could detect: a required one made the whole
/// action fail to build (and, from a keybinding, silently never fire), and an optional one silently
/// took its default.
#[derive(Debug, Clone, Copy)]
pub struct ArgDescriptor {
    /// The argument's name — exactly the key [`build_action`](crate::input::build_action) reads.
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
        Self { name, kind, required: true, description, values: &[] }
    }

    /// An argument with a default — omitting it is legal, misspelling it is not.
    pub const fn optional(name: &'static str, kind: ArgKind, description: &'static str) -> Self {
        Self { name, kind, required: false, description, values: &[] }
    }

    /// A required argument limited to a fixed vocabulary. `values` must list every spelling the
    /// argument's `FromStr` accepts — [`EnumArg::VALUES`](crate::input::EnumArg::VALUES) provides
    /// that list next to the parser, so the two cannot drift.
    pub const fn required_enum(
        name: &'static str,
        values: &'static [&'static str],
        description: &'static str,
    ) -> Self {
        Self { name, kind: ArgKind::Enum, required: true, description, values }
    }

    /// A vocabulary-limited argument with a default.
    pub const fn optional_enum(
        name: &'static str,
        values: &'static [&'static str],
        description: &'static str,
    ) -> Self {
        Self { name, kind: ArgKind::Enum, required: false, description, values }
    }
}

/// Runtime, owned declaration of one action argument — what [`ActionMeta::args`] holds and what RPC
/// introspection serializes. The owned counterpart of [`ArgDescriptor`], for the same reason
/// [`ActionMeta`] is the owned counterpart of [`ActionDescriptor`]: an action registered at runtime
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
    /// alone (see [`builtin_policy`]) or for a test that needs a well-formed call.
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
    Unknown { name: String, did_you_mean: Option<String> },
    /// A required argument was not supplied. The action cannot be built.
    Missing { name: String },
    /// The value does not read as the declared kind.
    BadValue { name: String, value: String, expected: String },
}

impl std::fmt::Display for ArgProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgProblem::Unknown { name, did_you_mean: Some(near) } => {
                write!(f, "unknown argument '{name}' — did you mean '{near}'?")
            }
            ArgProblem::Unknown { name, did_you_mean: None } => {
                write!(f, "unknown argument '{name}'")
            }
            ArgProblem::Missing { name } => write!(f, "missing required argument '{name}'"),
            ArgProblem::BadValue { name, value, expected } => {
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
            None if spec.required => problems.push(ArgProblem::Missing { name: spec.name.clone() }),
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

/// Static descriptor for a window-manager action. The built-in set lives in
/// [`ActionRegistry::ALL`]; [`ActionCatalog`] loads them into the runtime, plugin-extensible
/// metadata surface every UI reads from.
#[derive(Debug, Clone, Copy)]
pub struct ActionDescriptor {
    /// Config key name (e.g. "focus_left").
    pub name: &'static str,
    /// Human-readable label for the command palette.
    pub label: &'static str,
    /// Short description of what the action does.
    pub description: &'static str,
    /// Category for grouping.
    pub category: ActionCategory,
    /// Centralized action icon. The single source of an action's [`Glyph`] —
    /// every surface that renders this action (pane-action bar, context menu,
    /// command palette) reads it from here instead of inventing its own. `None`
    /// for actions without an assigned icon yet.
    pub icon: Option<Glyph>,
    /// The arguments this action takes, in the order a reader should meet them. **Empty means it
    /// takes none** — there is no "not stated": a struct literal has to fill every field, so a new
    /// action cannot be added without answering the question, exactly as it cannot skip `policy`.
    pub args: &'static [ArgDescriptor],
}

/// Registry that maps action discriminants to handler functions.
///
/// Registry that maps action discriminants to handler functions.
///
/// Call `register()` during app initialization to wire up all actions,
/// then `execute()` at runtime to dispatch.
///
/// # Invariants
///
/// - Every `WmAction` variant must have a registered handler in
///   `build_registry()`. In debug builds, `execute()` panics if a handler
///   is missing. In release builds, missing handlers are silently skipped.
/// - Handlers are keyed by `Discriminant<WmAction>`, so all parameterized
///   variants of the same action share one handler (the handler destructures
///   the action to extract arguments).
pub struct ActionRegistry {
    handlers: HashMap<std::mem::Discriminant<crate::input::WmAction>, ActionHandler>,
    /// Handlers for **name-keyed** actions registered at runtime by providers (and later WASM
    /// plugins) — the actions that have no [`WmAction`](crate::input::WmAction) variant because the
    /// enum is closed and a plugin cannot extend it. Their *metadata* lives in the one
    /// [`ActionCatalog`], next to the built-ins; only the handler lives here. See [`Dispatch`].
    dyn_handlers: HashMap<String, DynHandler>,
}

/// How an action is run — the three back ends behind the one dispatch door
/// (`dispatch_view_intent`). There is no parallel dispatch path.
///
/// This is a *description* of the registry's contents, used by
/// [`ActionRegistry::dispatch_of`] to answer "how would this name run?" for introspection and
/// tests; the dispatch itself is keyed by discriminant (built-ins) or by name (the rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// A built-in: `fn(&mut AppState, &WmAction)` keyed by `Discriminant<WmAction>`. Parameterized
    /// variants share one handler (it destructures the action for its args).
    Native,
    /// Name-keyed **with** a native host handler — a first-party provider contributing an action of
    /// its own, before any WASM exists. Receives the `Intent`, so its args arrive as data.
    NativeDyn,
    /// Name-keyed with **no** host handler: the action is declared (it has metadata, a policy, a
    /// name) but the host cannot run it — it is forwarded to its owner across the plugin boundary
    /// (WASM, plugin-08). Dispatching one today is a no-op with a debug warning.
    Declarative,
}

/// The handler for a name-keyed action. Unlike [`ActionHandler`] (a bare `fn` pointer keyed by
/// discriminant), this is a closure — a provider closes over its own state — and it receives the
/// [`Intent`](crate::chrome::Intent), so its arguments arrive as serializable data rather than as
/// an enum variant's fields.
///
/// It takes `&mut AppState` deliberately: §2.3 forbids a provider from mutating app state from its
/// *build/observe* path, but an action handler **is** the sanctioned write path — dispatching an
/// action is exactly how a provider is supposed to change things.
pub type DynHandler = std::rc::Rc<dyn Fn(&mut crate::app_state::AppState, &crate::chrome::Intent)>;

/// Why a name-keyed action's id was already taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateAction {
    /// A compiled-in action owns the id. The declaration is **rejected**.
    ShadowsBuiltin,
    /// Another name-keyed action owned it; this one replaced it.
    ReplacedDynamic,
}

/// RAII handle for a registered dynamic action.
///
/// Held by the provider that registered the action (in `ProviderHandles`, alongside its event
/// subscriptions) so that unmounting the provider retires its actions. The handle carries only the
/// id: the registry and the catalog are reached from `HecaApp`/`AppState`, not from `Drop`; the
/// host calls [`unregister_dynamic`] with this id when it drops the provider — the same lifetime,
/// without wrapping the registry in a `RefCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionHandle(pub String);

/// Register a **name-keyed** action: its metadata joins the one [`ActionCatalog`] (so it gets an
/// icon, a label, introspection and a declared policy exactly like a built-in) and its handler —
/// when it has one — joins the [`ActionRegistry`].
///
/// This takes both halves because they live in different places for a borrow reason, not a design
/// one: an action handler is `fn(&mut AppState, …)` and gets **no** registry, so action *metadata*
/// must be reachable from `AppState` (the catalog), while the handler table must be borrowable
/// alongside `&mut AppState` (the registry). Metadata is still stored exactly once.
///
/// `handler: None` registers a [`Dispatch::Declarative`] action (declared, host cannot run it —
/// plugin-08 forwards it to its owner). Re-registering the same id replaces the previous entry (a
/// provider remounting). Returns the [`ActionHandle`] the provider keeps and hands back to
/// [`unregister_dynamic`] on unmount.
pub fn register_dynamic(
    registry: &mut ActionRegistry,
    catalog: &mut ActionCatalog,
    meta: ActionMeta,
    handler: Option<DynHandler>,
) -> Result<ActionHandle, DuplicateAction> {
    let id = meta.name.clone();
    // A rejected id must not get a handler either, or the key would run something whose metadata
    // says it is a different action.
    if let Err(dup @ DuplicateAction::ShadowsBuiltin) = catalog.insert_dynamic(meta) {
        return Err(dup);
    }
    match handler {
        Some(h) => {
            registry.dyn_handlers.insert(id.clone(), h);
        }
        None => {
            registry.dyn_handlers.remove(&id);
        }
    }
    Ok(ActionHandle(id))
}

/// A **native** action declared in one place: its `WmAction` variant (dispatched by discriminant),
/// its handler, and its metadata (including any [`confirm`](ActionMeta::confirm)) — the §5.6
/// native-dev ergonomics. The counterpart to [`register_dynamic`] for actions that *have* a variant.
///
/// This is what unifies the two things a built-in needs — a handler in the registry and metadata in
/// the catalog — into a single call, instead of a `registry.register(&action, handler)` here and a
/// `const ALL` descriptor there that can drift apart.
pub struct ActionSpec {
    /// The variant whose **discriminant** keys the handler (parameterized variants share one).
    pub action: crate::input::WmAction,
    pub handler: ActionHandler,
    pub meta: ActionMeta,
}

/// Register a native action from one [`ActionSpec`]: the handler joins the [`ActionRegistry`] (by
/// discriminant) and the metadata joins the [`ActionCatalog`] — one call, both halves, no drift.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "action-task-C native registration API; built-ins seed via ALL today, exercised by tests"
    )
)]
pub fn register(registry: &mut ActionRegistry, catalog: &mut ActionCatalog, spec: ActionSpec) {
    registry.register(&spec.action, spec.handler);
    catalog.insert(spec.meta);
}

/// Retire a name-keyed action — drops both its handler and its metadata. `true` if it was
/// registered. Built-ins cannot be retired (their names are not removable from the catalog).
pub fn unregister_dynamic(
    registry: &mut ActionRegistry,
    catalog: &mut ActionCatalog,
    id: &str,
) -> bool {
    let had_handler = registry.dyn_handlers.remove(id).is_some();
    let had_meta = catalog.remove_dynamic(id);
    had_handler || had_meta
}

impl ActionRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            dyn_handlers: HashMap::new(),
        }
    }

    /// How the action named `name` would run, if at all — see [`Dispatch`]. `None` when the name is
    /// unknown to both back ends. `catalog` supplies the name→built-in resolution and the set of
    /// declared name-keyed actions.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "plugin-04 seam: RPC/palette introspection of how an action runs; exercised by tests today"
        )
    )]
    pub fn dispatch_of(&self, catalog: &ActionCatalog, name: &str) -> Option<Dispatch> {
        if crate::input::action_from_name(name).is_some() || catalog.is_builtin(name) {
            return Some(Dispatch::Native);
        }
        if self.dyn_handlers.contains_key(name) {
            return Some(Dispatch::NativeDyn);
        }
        catalog.find(name).map(|_| Dispatch::Declarative)
    }

    /// Run a name-keyed action, passing the intent through so the handler reads its own args.
    /// `false` when no *handler* is registered under `id` — either the id is unknown, or the action
    /// is [`Dispatch::Declarative`] (declared but host-unrunnable). Neither is a crash: a binding or
    /// a menu item may legitimately name an action whose provider is not mounted.
    pub fn execute_dynamic(
        &self,
        id: &str,
        state: &mut crate::app_state::AppState,
        intent: &crate::chrome::Intent,
    ) -> bool {
        let Some(handler) = self.dyn_handlers.get(id) else {
            return false;
        };
        // Clone the `Rc` so the borrow of `self` ends before the handler runs (it takes
        // `&mut AppState`).
        let handler = handler.clone();
        handler(state, intent);
        true
    }

    /// Register a handler for all variants that share `action`'s discriminant.
    pub fn register(&mut self, action: &crate::input::WmAction, handler: ActionHandler) {
        let disc = crate::input::action_discriminant(action);
        self.handlers.insert(disc, handler);
    }

    /// Execute the handler for `action`, if one is registered.
    ///
    /// In debug builds, panics if no handler is registered (this is a bug —
    /// every `WmAction` variant must have a handler in `build_registry()`).
    /// In release builds, silently does nothing.
    pub fn execute(&self, action: &crate::input::WmAction, state: &mut crate::app_state::AppState) {
        let disc = crate::input::action_discriminant(action);
        if let Some(handler) = self.handlers.get(&disc) {
            handler(state, action);
        } else {
            #[cfg(debug_assertions)]
            panic!("no handler registered for action: {:?}", action);
        }
    }

    /// Check whether a handler is registered for the given action.
    // Used in tests and debugging; kept for future RPC introspection.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "used by tests and reserved for future RPC introspection")
    )]
    pub fn has_handler(&self, action: &crate::input::WmAction) -> bool {
        let disc = crate::input::action_discriminant(action);
        self.handlers.contains_key(&disc)
    }
}

// ── Built-in action metadata seed ──
// The compile-time catalog of built-in actions. `ActionCatalog::with_builtins()` loads this into
// the runtime, plugin-extensible catalog owned by `AppState`; every UI surface resolves metadata
// through that catalog, not this const directly.
impl ActionRegistry {
    /// The built-in action descriptors, in a stable order. Seed for [`ActionCatalog`].
    pub const ALL: &[ActionDescriptor] = &[
        // ── Navigation ──
        ActionDescriptor {
            name: "focus_left",
            label: "Focus Column Left",
            description: "Move focus to the column on the left.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretLeft),
            args: &[],
        },
        ActionDescriptor {
            name: "focus_right",
            label: "Focus Column Right",
            description: "Move focus to the column on the right.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretRight),
            args: &[],
        },
        ActionDescriptor {
            name: "focus_up",
            label: "Focus Pane Up",
            description: "Move focus to the pane above in the current column.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretUp),
            args: &[],
        },
        ActionDescriptor {
            name: "focus_down",
            label: "Focus Pane Down",
            description: "Move focus to the pane below in the current column.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretDown),
            args: &[],
        },
        ActionDescriptor {
            name: "next_pane",
            label: "Next Pane in Column",
            description: "Cycle focus forward through panes in the active column.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::ArrowLineRight),
            args: &[],
        },
        ActionDescriptor {
            name: "prev_pane",
            label: "Previous Pane in Column",
            description: "Cycle focus backward through panes in the active column.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::ArrowLineLeft),
            args: &[],
        },
        ActionDescriptor {
            name: "cursor_to",
            label: "Move Container Cursor",
            description: "Put a mounted container's cursor on a named row. Moves the cursor and nothing else.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[
                ArgDescriptor::required(
                    "mount",
                    ArgKind::Text,
                    "Id of the mounted container whose cursor moves.",
                ),
                ArgDescriptor::required(
                    "key",
                    ArgKind::Text,
                    "Identity of the row the cursor moves to.",
                ),
            ],
        },
        ActionDescriptor {
            name: "focus_dock",
            label: "Focus Dock",
            description: "Give chrome keyboard focus to a dock — press a letter to pick one, or name it.",
            category: ActionCategory::Chrome,
            icon: None,
            // OPTIONAL, deliberately: the bare binding opens the pick, and a caller that already
            // knows which dock it wants (RPC, a menu entry, a script) names it and skips the pick.
            // A *required* argument would make the bare binding illegal (F003/P010/T006).
            args: &[ArgDescriptor::optional(
                "dock",
                ArgKind::Text,
                "Id of the dock to focus; omit to pick one by letter.",
            )],
        },
        ActionDescriptor {
            name: "toggle_dock",
            label: "Toggle Dock Focus",
            description: "Give a dock the keyboard, or hand it back if that dock already has it.",
            category: ActionCategory::Chrome,
            icon: None,
            // The toggle belongs to the *gesture*: pressing a key again plainly means "undo that",
            // while a click, an RPC call and a palette entry all mean "focus it" and nothing more.
            // `global_focus` binds this; everything else binds `focus_dock` (F003/P082/T444).
            args: &[ArgDescriptor::optional(
                "dock",
                ArgKind::Text,
                "Id of the dock to toggle; omit to pick one by letter.",
            )],
        },
        ActionDescriptor {
            name: "clear_search_history",
            label: "Clear Search History",
            description: "Forget the past queries the command palette remembers.",
            category: ActionCategory::Chrome,
            icon: None,
            // OPTIONAL: bare forgets every search surface, which is what a key or a palette entry
            // means; a caller that knows which surface it wants names it. A *required* argument
            // would keep it out of the palette entirely.
            args: &[ArgDescriptor::optional(
                "scope",
                ArgKind::Text,
                "Which search surface to forget; omit for all of them.",
            )],
        },
        ActionDescriptor {
            name: "clear_search_ranking",
            label: "Clear Search Ranking",
            description: "Forget which commands you use, so the palette stops ordering by habit.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[ArgDescriptor::optional(
                "scope",
                ArgKind::Text,
                "Which search surface to forget; omit for all of them.",
            )],
        },
        ActionDescriptor {
            name: "unfocus_dock",
            label: "Release Dock Focus",
            description: "Give the keyboard back to the focused pane, releasing chrome focus.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "workspace_next",
            label: "Next Workspace",
            description: "Switch to the next workspace.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretDown),
            args: &[],
        },
        ActionDescriptor {
            name: "workspace_prev",
            label: "Previous Workspace",
            description: "Switch to the previous workspace.",
            category: ActionCategory::Navigation,
            icon: Some(Glyph::CaretUp),
            args: &[],
        },
        ActionDescriptor {
            name: "focus_toggle_local",
            label: "Last Pane",
            description: "Toggle between current and last-focused pane in the same workspace.",
            category: ActionCategory::Navigation,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "focus_toggle_global",
            label: "Last Workspace",
            description: "Toggle between current and last-visited workspace.",
            category: ActionCategory::Navigation,
            icon: None,
            args: &[],
        },
        // ── Layout ──
        ActionDescriptor {
            name: "split_horizontal",
            label: "New Column",
            description: "Create a new column to the right.",
            category: ActionCategory::Layout,
            // New column opens to the right (see the description); ColumnsPlusLeft stays
            // in the Glyph set for a future "add column to the left" action.
            icon: Some(Glyph::ColumnsPlusRight),
            args: &[],
        },
        ActionDescriptor {
            name: "split_vertical",
            label: "New Pane",
            description: "Add a new pane below the current one in the same column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::SquareHalfBottom),
            args: &[],
        },
        ActionDescriptor {
            // Add-pane-to-a-specific-column (the pane-header "+" button and the sidebar
            // column "New Pane" entry, which target a column by index — distinct from
            // `split_vertical` which splits the active column). Menu/button-only, so no
            // binding; the "+" button still shows the `v` hint via `pane_action_name`.
            name: "add_pane_to_column",
            label: "Add Pane to Column",
            description: "Add a new pane to this column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::FolderSimplePlus),
            args: &[
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace holding the column."),
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to add the pane to."),
            ],
        },
        ActionDescriptor {
            name: "zoom_column",
            label: "Toggle Column Zoom",
            description: "Toggle the active column between viewport-wide zoom and its previous width.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::FrameCorners),
            args: &[],
        },
        ActionDescriptor {
            name: "open_context_menu",
            label: "Open Context Menu",
            description: "Open the focused pane's context menu at the cursor.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::DotsThreeVertical),
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_view_left",
            label: "Scroll View Left",
            description: "Pan the horizontal view left to reach off-screen / overflowing columns.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_view_right",
            label: "Scroll View Right",
            description: "Pan the horizontal view right to reach off-screen / overflowing columns.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "resize_increase",
            label: "Increase Column Width",
            description: "Widen the active column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::Plus),
            args: &[],
        },
        ActionDescriptor {
            name: "resize_decrease",
            label: "Decrease Column Width",
            description: "Narrow the active column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::Minus),
            args: &[],
        },
        ActionDescriptor {
            name: "pane_height_increase",
            label: "Increase Pane Height",
            description: "Tallens the active pane within its column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::StackPlus),
            args: &[],
        },
        ActionDescriptor {
            name: "pane_height_decrease",
            label: "Decrease Pane Height",
            description: "Shortens the active pane within its column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::StackMinus),
            args: &[],
        },
        // ── Font zoom ──
        ActionDescriptor {
            name: "app_font_increase",
            label: "Increase App Font",
            description: "Increase the whole-app font: chrome/UI and every terminal pane.",
            category: ActionCategory::System,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "app_font_decrease",
            label: "Decrease App Font",
            description: "Decrease the whole-app font: chrome/UI and every terminal pane.",
            category: ActionCategory::System,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "app_font_reset",
            label: "Reset App Font",
            description: "Reset the whole-app font to the configured sizes.",
            category: ActionCategory::System,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "pane_terminal_font_increase",
            label: "Increase Terminal Font",
            description: "Increase the focused pane's terminal font size.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "pane_terminal_font_decrease",
            label: "Decrease Terminal Font",
            description: "Decrease the focused pane's terminal font size.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "pane_terminal_font_reset",
            label: "Reset Terminal Font",
            description: "Reset the focused pane to follow the app-wide font size.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "swap_left",
            label: "Swap Column Left",
            description: "Swap the active column with the one to its left.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "swap_right",
            label: "Swap Column Right",
            description: "Swap the active column with the one to its right.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "swap_up",
            label: "Swap Pane Up",
            description: "Swap the active pane with the one above.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "swap_down",
            label: "Swap Pane Down",
            description: "Swap the active pane with the one below.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "move_pane_left",
            label: "Move Pane to Column Left",
            description: "Move the active pane into the column on the left.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::ArrowLineLeft),
            args: &[ArgDescriptor::optional("pane_id", ArgKind::Int, "The pane to move; omit for the focused one.")],
        },
        ActionDescriptor {
            name: "move_pane_right",
            label: "Move Pane to Column Right",
            description: "Move the active pane into the column on the right.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::ArrowLineRight),
            args: &[ArgDescriptor::optional("pane_id", ArgKind::Int, "The pane to move; omit for the focused one.")],
        },
        ActionDescriptor {
            name: "zoom_column_at_index",
            label: "Zoom Column",
            description: "Toggle zoom on the named column, making it active first.",
            category: ActionCategory::Layout,
            // Same glyph as `zoom_column`: one act, two ways of naming its target.
            icon: Some(Glyph::FrameCorners),
            args: &[
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace holding the column."),
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to zoom."),
            ],
        },
        ActionDescriptor {
            name: "delete_column",
            label: "Delete Column",
            description: "Delete the named column and all its panes.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::Trash),
            args: &[
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace holding the column."),
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to delete."),
            ],
        },
        // ── Pane ──
        ActionDescriptor {
            name: "close",
            label: "Close Pane",
            description: "Close the active pane.",
            category: ActionCategory::Pane,
            // Remove/close pane; pairs with add-pane's FolderSimplePlus (both act on a
            // pane "slot" in a column).
            icon: Some(Glyph::FolderSimpleMinus),
            args: &[],
        },
        ActionDescriptor {
            name: "float",
            label: "Toggle Float",
            description: "Toggle the active pane between tiling and floating.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::Cards),
            args: &[],
        },
        ActionDescriptor {
            name: "pane_select",
            label: "Quick-Select Pane",
            description: "Press a letter to focus it.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "open_link",
            label: "Open Link",
            description: "Open the hyperlink in the OS default handler.",
            category: ActionCategory::Pane,
            // Constructed with a URL (mouse/HintKey/selection/menu); no global key.
            icon: Some(Glyph::ArrowRight),
            args: &[ArgDescriptor::required("url", ArgKind::Text, "The link to open.")],
        },
        ActionDescriptor {
            name: "follow_link",
            label: "Follow Link",
            description: "Press a letter to open the link.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::GitBranch),
            args: &[],
        },
        // Pick-mode prompts (`description`) double as the in-progress pick text shown
        // in `InputMode::pending_pick()` — single source of truth, not duplicated. The
        // "+ focus" variants make the focus-follow difference explicit.
        ActionDescriptor {
            name: "swap_pane",
            label: "Quick-Swap Pane",
            description: "Select a pane to swap with — focus stays where it is.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "swap_and_focus_pane",
            label: "Swap and Focus",
            description: "Select a pane to swap with, then follow focus to it.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "pane_take",
            label: "Take Pane",
            description: "Select a pane to pull into the active column — focus stays where it is.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "pane_take_and_focus",
            label: "Take and Focus",
            description: "Select a pane to pull into the active column, then focus it.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "move_pane_to_workspace_pick",
            label: "Move Pane to Workspace",
            description: "Select a workspace to move the active pane to.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "move_column_to_workspace_pick",
            label: "Move Column to Workspace",
            description: "Select a workspace to move the active column to.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "move_pane_to_column_pick",
            label: "Move Pane to Column",
            description: "Select a column to move the active pane into.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "rename_pane",
            label: "Rename Pane",
            description: "Rename the active pane/tab.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        ActionDescriptor {
            name: "reset_pane_name",
            label: "Use Process Name",
            description: "Clear the pane's custom name, reverting to the program name.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::Backspace),
            args: &[],
        },
        ActionDescriptor {
            name: "rename_column",
            label: "Rename Column",
            description: "Rename the active column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        // ── Workspace ──
        ActionDescriptor {
            name: "create_workspace",
            label: "Create Workspace",
            description: "Create a new workspace and switch to it.",
            category: ActionCategory::Workspace,
            icon: Some(Glyph::StackPlus),
            args: &[],
        },
        ActionDescriptor {
            name: "rename_workspace",
            label: "Rename Workspace",
            description: "Rename the current workspace.",
            category: ActionCategory::Workspace,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        ActionDescriptor {
            name: "reset_workspace_name",
            label: "Use Default Name",
            description: "Clear the workspace's custom name, reverting to \"Workspace N\".",
            category: ActionCategory::Workspace,
            icon: Some(Glyph::Backspace),
            args: &[],
        },
        ActionDescriptor {
            name: "delete_workspace",
            label: "Delete Workspace",
            description: "Delete a workspace and all its panes (not the last workspace).",
            category: ActionCategory::Workspace,
            icon: Some(Glyph::StackMinus),
            args: &[ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace to delete.")],
        },
        // ── Chrome ──
        ActionDescriptor {
            name: "sidebar_left",
            label: "Toggle Left Sidebar",
            description: "Show or hide the left sidebar.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "sidebar_right",
            label: "Toggle Right Sidebar",
            description: "Show or hide the right sidebar.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        // Chrome region show/hide mounted-gate (sidebar-fu-6). Unbound by default
        // (listed in the `UNBOUND` test allowlist); the user binds them in config.
        ActionDescriptor {
            name: "show_left_sidebar",
            label: "Show Left Sidebar",
            description: "Mount (show) the left sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "hide_left_sidebar",
            label: "Hide Left Sidebar",
            description: "Unmount (hide) the left sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_left_sidebar",
            label: "Toggle Left Sidebar Region",
            description: "Mount or unmount the left sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "show_right_sidebar",
            label: "Show Right Sidebar",
            description: "Mount (show) the right sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "hide_right_sidebar",
            label: "Hide Right Sidebar",
            description: "Unmount (hide) the right sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_right_sidebar",
            label: "Toggle Right Sidebar Region",
            description: "Mount or unmount the right sidebar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Sidebar),
            args: &[],
        },
        ActionDescriptor {
            name: "show_top_bar",
            label: "Show Top Bar",
            description: "Mount (show) the top bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        ActionDescriptor {
            name: "hide_top_bar",
            label: "Hide Top Bar",
            description: "Unmount (hide) the top bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_top_bar",
            label: "Toggle Top Bar Region",
            description: "Mount or unmount the top bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::NotePencil),
            args: &[],
        },
        ActionDescriptor {
            name: "show_bottom_bar",
            label: "Show Bottom Bar",
            description: "Mount (show) the bottom bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Pencil),
            args: &[],
        },
        ActionDescriptor {
            name: "hide_bottom_bar",
            label: "Hide Bottom Bar",
            description: "Unmount (hide) the bottom bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::Pencil),
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_bottom_bar",
            label: "Toggle Bottom Bar Region",
            description: "Mount or unmount the bottom bar region.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::PlusCircle),
            args: &[],
        },
        // ── Chrome container placement (plugin-04/T1) ──
        // The ONLY built-ins with dotted, namespaced names. Deliberate: this is the id scheme
        // plugins use, and container placement is the first host capability a plugin drives by
        // name. The other ~115 built-ins keep their snake_case config names — renaming them is a
        // migration nobody has decided on, so there are no snake_case aliases for these either
        // (one action, one name). All are parameterized, so they are built through `build_action`
        // and carry no default binding.
        ActionDescriptor {
            name: "chrome.container.move_to_region",
            label: "Move Container to Region",
            description: "Move a chrome container to another region (left/right sidebar, top/bottom bar).",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::ArrowLineRight),
            args: &[
                ArgDescriptor::required("container_id", ArgKind::Text, "Id of the container to move."),
                ArgDescriptor::required_enum("region", <crate::chrome::RegionId as crate::input::EnumArg>::VALUES, "The region to move it into."),
            ],
        },
        ActionDescriptor {
            name: "chrome.container.move_left_sidebar",
            label: "Move Container to Left Sidebar",
            description: "Move a chrome container into the left sidebar.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::ArrowLineLeft),
            args: &[ArgDescriptor::required("container_id", ArgKind::Text, "Id of the container to move.")],
        },
        ActionDescriptor {
            name: "chrome.container.move_right_sidebar",
            label: "Move Container to Right Sidebar",
            description: "Move a chrome container into the right sidebar.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::ArrowLineRight),
            args: &[ArgDescriptor::required("container_id", ArgKind::Text, "Id of the container to move.")],
        },
        ActionDescriptor {
            name: "chrome.container.reorder_before",
            label: "Reorder Container Before",
            description: "Move a chrome container before another in its region (omit the target to move it to the end).",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[
                ArgDescriptor::required("container_id", ArgKind::Text, "Id of the container to move."),
                ArgDescriptor::optional("before_id", ArgKind::Text, "Id of the container to sit before; omit to move it to the end."),
            ],
        },
        ActionDescriptor {
            name: "chrome.container.reorder_after",
            label: "Reorder Container After",
            description: "Move a chrome container after another in its region.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[
                ArgDescriptor::required("container_id", ArgKind::Text, "Id of the container to move."),
                ArgDescriptor::required("after_id", ArgKind::Text, "Id of the container to sit after."),
            ],
        },
        ActionDescriptor {
            name: "collapse_current_workspace",
            label: "Collapse Workspace Row",
            description: "Collapse the active workspace row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "expand_current_workspace",
            label: "Expand Workspace Row",
            description: "Expand the active workspace row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_current_workspace_collapsed",
            label: "Toggle Workspace Row",
            description: "Toggle the active workspace row collapsed state in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "collapse_current_column",
            label: "Collapse Column Row",
            description: "Collapse the focused tiled column row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "expand_current_column",
            label: "Expand Column Row",
            description: "Expand the focused tiled column row in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_current_column_collapsed",
            label: "Toggle Column Row",
            description: "Toggle the focused tiled column row collapsed state in the sidebar tree UI.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        // ── System ──
        ActionDescriptor {
            name: "command_palette",
            label: "Command Palette",
            description: "Search every action — the app's and every mounted component's — and run one.",
            category: ActionCategory::System,
            icon: Some(Glyph::Search),
            // Both OPTIONAL: bare is the actions list with an empty query, which is what a plain
            // binding means. A *required* argument would take this action out of the palette's own
            // listing, which offers only what it can run with nothing supplied.
            args: &[
                ArgDescriptor::optional(
                    "mode",
                    ArgKind::Text,
                    "Which list to open in: pane or workspace; omit for actions.",
                ),
                ArgDescriptor::optional(
                    "query",
                    ArgKind::Text,
                    "Text to start the search with; omit to open empty.",
                ),
            ],
        },
        ActionDescriptor {
            name: "close_overlay",
            label: "Close Overlay",
            description: "Dismiss the front-most overlay — the exposé, a dialog, a menu.",
            category: ActionCategory::Chrome,
            icon: None,
            // No arguments at all: the overlay it closes is the one in front, because an OverlayId
            // is a runtime counter nothing outside the process could name.
            args: &[],
        },
        ActionDescriptor {
            name: "show_layer",
            label: "Show Layer",
            description: "Bring an addressable layer into the stack — an expose, a plugin panel.",
            category: ActionCategory::Chrome,
            icon: None,
            // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
            // name, and a *required* argument would keep this out of the palette entirely.
            args: &[
                ArgDescriptor::optional(
                    "name",
                    ArgKind::Text,
                    "Layer to show, as owner.short (e.g. heca.expose).",
                ),
                ArgDescriptor::optional(
                    "dock",
                    ArgKind::Text,
                    "Which placement, when a component is seated twice; omit to use the focused one.",
                ),
            ],
        },
        ActionDescriptor {
            name: "hide_layer",
            label: "Hide Layer",
            description: "Take an addressable layer back out of the stack.",
            category: ActionCategory::Chrome,
            icon: None,
            // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
            // name, and a *required* argument would keep this out of the palette entirely.
            args: &[
                ArgDescriptor::optional(
                    "name",
                    ArgKind::Text,
                    "Layer to hide, as owner.short (e.g. heca.expose).",
                ),
                ArgDescriptor::optional(
                    "dock",
                    ArgKind::Text,
                    "Which placement, when a component is seated twice; omit to use the focused one.",
                ),
            ],
        },
        ActionDescriptor {
            name: "toggle_layer",
            label: "Toggle Layer",
            description: "Show an addressable layer if hidden, hide it if shown.",
            category: ActionCategory::Chrome,
            icon: None,
            // Both OPTIONAL: a `LayerId` is a runtime counter nothing outside the process could
            // name, and a *required* argument would keep this out of the palette entirely.
            args: &[
                ArgDescriptor::optional(
                    "name",
                    ArgKind::Text,
                    "Layer to toggle, as owner.short (e.g. heca.expose).",
                ),
                ArgDescriptor::optional(
                    "dock",
                    ArgKind::Text,
                    "Which placement, when a component is seated twice; omit to use the focused one.",
                ),
            ],
        },
        ActionDescriptor {
            name: "reload_config",
            label: "Reload Config",
            description: "Reload keymaps, theme, and settings from config.toml without restarting.",
            category: ActionCategory::System,
            icon: Some(Glyph::Gear),
            args: &[],
        },
        // ── Notifications (F009) ──
        ActionDescriptor {
            name: "notification_dismiss_one",
            label: "Dismiss Notification",
            description: "Dismiss a visible notification by id, when its lifecycle permits it.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::XSquare),
            args: &[ArgDescriptor::required("id", ArgKind::Int, "The notification's runtime id.")],
        },
        ActionDescriptor {
            name: "notification_dismiss_all",
            label: "Dismiss All Notifications",
            description: "Dismiss every currently visible notification.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::XSquare),
            args: &[],
        },
        ActionDescriptor {
            name: "notification_dismiss_last",
            label: "Dismiss Last Notification",
            description: "Dismiss the first eligible visible notification in stable toast order.",
            category: ActionCategory::Chrome,
            icon: Some(Glyph::XSquare),
            args: &[],
        },
        ActionDescriptor {
            name: "notification_pick",
            label: "Pick Notification Action",
            description: "Open a scoped picker over the visible toast actions/dismiss affordances, in addition to their global prefix+/ letters.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "notification_action_relay",
            label: "Run Notification Action",
            description: "Internal: the toast's inline action button cannot carry its own Intent (the notification, and therefore the Intent, does not exist yet when the button is built at mount time), so it names this relay by id + key instead — an address, not a smuggled closure — and the relay resolves it against the store and dispatches it. Not meant to be bound directly.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[
                ArgDescriptor::required("id", ArgKind::Int, "The notification's runtime id."),
                ArgDescriptor::required("key", ArgKind::Text, "The action's key (its own intent name)."),
            ],
        },
        // ── Scrollback (host terminal viewport) ──
        ActionDescriptor {
            name: "scrollback_page_up",
            label: "Scrollback Page Up",
            description: "Scroll the terminal viewport up by one page and enter selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scrollback_page_down",
            label: "Scrollback Page Down",
            description: "Scroll the terminal viewport down by one page and enter selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scrollback_line_up",
            label: "Scrollback Line Up",
            description: "Scroll the terminal viewport up by a configurable number of lines (selection mode).",
            category: ActionCategory::Pane,
            icon: None,
            args: &[ArgDescriptor::optional("amount", ArgKind::Int, "Notches to scroll; each is multiplied by `terminal_wheel_scroll_lines`. Default 1.")],
        },
        ActionDescriptor {
            name: "scrollback_line_down",
            label: "Scrollback Line Down",
            description: "Scroll the terminal viewport down by a configurable number of lines (selection mode).",
            category: ActionCategory::Pane,
            icon: None,
            args: &[ArgDescriptor::optional("amount", ArgKind::Int, "Notches to scroll; each is multiplied by `terminal_wheel_scroll_lines`. Default 1.")],
        },
        ActionDescriptor {
            name: "scrollback_to_top",
            label: "Scrollback to Top",
            description: "Jump the terminal viewport to the top of scrollback history.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::CaretUp),
            args: &[],
        },
        ActionDescriptor {
            name: "scrollback_to_bottom",
            label: "Scrollback to Bottom",
            description: "Snap the terminal viewport to the live bottom (latest output).",
            category: ActionCategory::Pane,
            icon: Some(Glyph::CaretDown),
            args: &[],
        },
        ActionDescriptor {
            name: "exit_scrollback",
            label: "Exit Scrollback",
            description: "Snap to the live bottom, clear selection, and exit selection mode.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::XCircle),
            args: &[],
        },
        // ── Direct scroll (non-prefix, no selection mode entry) ──
        ActionDescriptor {
            name: "scroll_page_up",
            label: "Scroll Page Up",
            description: "Scroll the terminal viewport up by one page immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_page_down",
            label: "Scroll Page Down",
            description: "Scroll the terminal viewport down by one page immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_line_up",
            label: "Scroll Line Up",
            description: "Scroll the terminal viewport up by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_line_down",
            label: "Scroll Line Down",
            description: "Scroll the terminal viewport down by a configurable number of lines immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_to_top",
            label: "Scroll to Top",
            description: "Jump the terminal viewport to the top of scrollback history immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::CaretUp),
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_to_bottom",
            label: "Scroll to Bottom",
            description: "Snap the terminal viewport to the live bottom immediately. Stays in Normal mode, repeatable.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::CaretDown),
            args: &[],
        },
        // ── Horizontal scroll (a chrome container's scroll area; a pane has one axis) ──
        ActionDescriptor {
            name: "scroll_page_left",
            label: "Scroll Page Left",
            description: "Scroll the focused chrome container one page left. Does nothing when no dock holds chrome focus — a terminal viewport has no horizontal axis.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_page_right",
            label: "Scroll Page Right",
            description: "Scroll the focused chrome container one page right. Does nothing when no dock holds chrome focus — a terminal viewport has no horizontal axis.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_to_left_edge",
            label: "Scroll to Left Edge",
            description: "Jump the focused chrome container to its left edge. Does nothing when no dock holds chrome focus.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_to_right_edge",
            label: "Scroll to Right Edge",
            description: "Jump the focused chrome container to its right edge. Does nothing when no dock holds chrome focus.",
            category: ActionCategory::Chrome,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "scroll_to_offset",
            label: "Scroll to Offset",
            description: "Jump the terminal viewport to an explicit offset in rows above the live bottom. Used by the GUI scrollbar and RPC; no default keybinding.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[ArgDescriptor::required("rows", ArgKind::Int, "Rows above the live bottom to jump to.")],
        },
        // ── Selection (host capability) ──
        ActionDescriptor {
            name: "enter_selection_mode",
            label: "Enter Selection Mode",
            description: "Enter the host-owned selection input mode. Selection data is driven by surface adapters (mouse, keyboard, RPC).",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "selection_left",
            label: "Selection Left",
            description: "Move the active selection focus one cell left in selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "selection_right",
            label: "Selection Right",
            description: "Move the active selection focus one cell right in selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "selection_up",
            label: "Selection Up",
            description: "Move the active selection focus one row up in selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "selection_down",
            label: "Selection Down",
            description: "Move the active selection focus one row down in selection mode.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "clear_selection",
            label: "Clear Selection",
            description: "Clear the active selection and exit selection mode if active.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "copy_selection",
            label: "Copy Selection",
            description: "Copy the active selection text to the system clipboard.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "paste_clipboard",
            label: "Paste Clipboard",
            description: "Paste system clipboard content into the focused pane (bracketed-paste aware).",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "begin_selection",
            label: "Begin Selection",
            description: "Start a selection from the caret position in selection mode. No-op if a selection already exists — clear first to restart.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "toggle_selection_endpoint",
            label: "Toggle Selection Endpoint",
            description: "Swap which end of the selection is active so movement grows from the other side.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[],
        },
        ActionDescriptor {
            name: "open_link_at_caret",
            label: "Open Link at Caret",
            description: "Open the hyperlink under the selection caret.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::GitBranch),
            args: &[],
        },
        ActionDescriptor {
            name: "search_scrollback",
            label: "Search Scrollback",
            description: "Type to search the scrollback; Enter keeps matches, Esc cancels.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::Search),
            args: &[],
        },
        ActionDescriptor {
            name: "search_next_match",
            label: "Next Search Match",
            description: "Jump to the next scrollback-search match.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::Search),
            args: &[],
        },
        ActionDescriptor {
            name: "search_prev_match",
            label: "Previous Search Match",
            description: "Jump to the previous scrollback-search match.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::CaretDown),
            args: &[],
        },

        // Column movement — bound by default, but they had no descriptor at all until
        // action-task-E went looking for actions the catalog could not see.
        ActionDescriptor {
            name: "move_column_up",
            label: "Move Column to Workspace Above",
            description: "Move the focused column one position earlier.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::CaretUp),
            args: &[],
        },
        ActionDescriptor {
            name: "move_column_down",
            label: "Move Column to Workspace Below",
            description: "Move the focused column one position later.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[],
        },

        // ── Actions that name their target (action-task-E) ──
        //
        // These are built by name + arguments through `build_action` — from a `config.toml`
        // binding with an `args` table, a menu entry's `Intent`, a mouse handler, or RPC. They
        // never carry a bare default keybinding, because a key cannot supply a pane id.
        //
        // Until action-task-E they were the only dispatchable actions with **no descriptor at
        // all**: `list-actions` could not see them, they had no label or category, and nothing
        // anywhere said what arguments they took. The `args` below are that missing half; the
        // labels and categories come with it, since metadata lives in one place per action.
        ActionDescriptor {
            name: "focus_pane",
            label: "Focus Pane",
            description: "Move focus to a specific pane by id.",
            category: ActionCategory::Navigation,
            icon: None,
            args: &[ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to focus.")],
        },
        ActionDescriptor {
            name: "focus_workspace",
            label: "Focus Workspace",
            description: "Switch to a specific workspace by index.",
            category: ActionCategory::Navigation,
            icon: None,
            args: &[ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace to switch to.")],
        },
        ActionDescriptor {
            name: "swap",
            label: "Swap Panes",
            description: "Swap the positions of two panes.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("a_id", ArgKind::Int, "The first pane."),
                ArgDescriptor::required("b_id", ArgKind::Int, "The second pane, which takes the first one's place."),
            ],
        },
        ActionDescriptor {
            name: "move",
            label: "Move Pane to Column",
            description: "Move a pane into another column of the current workspace.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
                ArgDescriptor::required("target_col", ArgKind::Int, "Index of the column to move it into."),
            ],
        },
        ActionDescriptor {
            name: "move_pane_to_workspace",
            label: "Move Pane to Workspace",
            description: "Move a pane into another workspace.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the destination workspace."),
            ],
        },
        ActionDescriptor {
            name: "move_pane_to_column",
            label: "Move Pane to Column in Workspace",
            description: "Move a pane into a specific column of a specific workspace.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to move."),
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the destination workspace."),
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the destination column."),
            ],
        },
        ActionDescriptor {
            name: "move_column",
            label: "Move Column",
            description: "Move a column to another position, in this workspace or another one.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("src_ws", ArgKind::Int, "Index of the workspace the column is in."),
                ArgDescriptor::required("src_col", ArgKind::Int, "Index of the column to move."),
                ArgDescriptor::required("dst_ws", ArgKind::Int, "Index of the destination workspace."),
                ArgDescriptor::required("dst_idx", ArgKind::Int, "Position to insert it at."),
                ArgDescriptor::optional("focus", ArgKind::Bool, "Follow the column with focus. Default true."),
            ],
        },
        ActionDescriptor {
            name: "swap_columns",
            label: "Swap Columns",
            description: "Swap the positions of two columns.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("a_ws", ArgKind::Int, "Workspace of the first column."),
                ArgDescriptor::required("a_col", ArgKind::Int, "Index of the first column."),
                ArgDescriptor::required("b_ws", ArgKind::Int, "Workspace of the second column."),
                ArgDescriptor::required("b_col", ArgKind::Int, "Index of the second column."),
            ],
        },
        ActionDescriptor {
            name: "resize",
            label: "Resize",
            description: "Move the boundary the focused column or pane owns, along one axis.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required_enum("target", <crate::input::ResizeTarget as crate::input::EnumArg>::VALUES, "What to resize."),
                ArgDescriptor::required_enum("axis", <crate::input::ResizeAxis as crate::input::EnumArg>::VALUES, "Which axis to resize along."),
                ArgDescriptor::required("amount", ArgKind::Float, "How far to move the boundary, along the axis: +x is right, +y is DOWN. A pane's boundary is the one below it, or the one above when it is last — so the divider moves the same way whichever pane is active."),
            ],
        },
        ActionDescriptor {
            name: "resize_to",
            label: "Resize To",
            description: "Resize the focused column or pane to an explicit size.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required_enum("target", <crate::input::ResizeTarget as crate::input::EnumArg>::VALUES, "What to resize."),
                ArgDescriptor::required("width", ArgKind::Float, "The new width."),
                ArgDescriptor::required("height", ArgKind::Float, "The new height."),
            ],
        },
        ActionDescriptor {
            name: "float_at",
            label: "Float Pane at Position",
            description: "Float a pane at an explicit position and size.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to float."),
                ArgDescriptor::required("x", ArgKind::Float, "Left edge."),
                ArgDescriptor::required("y", ArgKind::Float, "Top edge."),
                ArgDescriptor::required("width", ArgKind::Float, "Width of the floating pane."),
                ArgDescriptor::required("height", ArgKind::Float, "Height of the floating pane."),
            ],
        },
        ActionDescriptor {
            name: "close_pane_by_id",
            label: "Close Pane by Id",
            description: "Close a specific pane by id, whether or not it is focused.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::XSquare),
            args: &[ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to close.")],
        },
        ActionDescriptor {
            name: "rename_target",
            label: "Rename Pane To",
            description: "Set a pane's name directly, without opening the rename prompt.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::NotePencil),
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to rename."),
                ArgDescriptor::required("name", ArgKind::Text, "The new name."),
            ],
        },
        ActionDescriptor {
            name: "rename_pane_by_id",
            label: "Rename Pane",
            description: "Open the rename prompt for a specific pane.",
            category: ActionCategory::Pane,
            icon: Some(Glyph::NotePencil),
            args: &[ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to rename.")],
        },
        ActionDescriptor {
            name: "rename_workspace_by_idx",
            label: "Rename Workspace",
            description: "Open the rename prompt for a specific workspace.",
            category: ActionCategory::Workspace,
            icon: Some(Glyph::NotePencil),
            args: &[ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace to rename.")],
        },
        ActionDescriptor {
            name: "rename_column_by_idx",
            label: "Rename Column",
            description: "Open the rename prompt for a specific column.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::NotePencil),
            args: &[
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace holding the column."),
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to rename."),
            ],
        },
        ActionDescriptor {
            name: "reset_pane_name_by_id",
            label: "Reset Pane Name",
            description: "Drop a pane's custom name so it follows its process again.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[ArgDescriptor::required("pane_id", ArgKind::Int, "The pane whose name to reset.")],
        },
        ActionDescriptor {
            name: "reset_workspace_name_by_idx",
            label: "Reset Workspace Name",
            description: "Drop a workspace's custom name so it follows its default again.",
            category: ActionCategory::Workspace,
            icon: None,
            args: &[ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace whose name to reset.")],
        },
        ActionDescriptor {
            name: "take_pane",
            label: "Take Pane",
            description: "Pull a pane out of wherever it is and into the focused column.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("pane_id", ArgKind::Int, "The pane to take."),
                ArgDescriptor::optional("focus_after", ArgKind::Bool, "Focus the pane once it arrives. Default false."),
            ],
        },
        ActionDescriptor {
            name: "add_column_to_workspace",
            label: "Add Column to Workspace",
            description: "Add a column to a specific workspace.",
            category: ActionCategory::Layout,
            icon: Some(Glyph::FolderSimplePlus),
            args: &[ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the workspace to add the column to.")],
        },
        ActionDescriptor {
            name: "app_font_zoom",
            label: "App Font Zoom",
            description: "Step the whole app's font size up, down, or back to the configured size.",
            category: ActionCategory::System,
            icon: None,
            args: &[ArgDescriptor::required_enum("step", <crate::input::FontZoomStep as crate::input::EnumArg>::VALUES, "Which way to step.")],
        },
        ActionDescriptor {
            name: "pane_terminal_font_zoom",
            label: "Pane Font Zoom",
            description: "Step one terminal pane's font size up, down, or back to following the app.",
            category: ActionCategory::Pane,
            icon: None,
            args: &[
                ArgDescriptor::optional("pane_id", ArgKind::Int, "The pane to zoom; omit for the focused one."),
                ArgDescriptor::required_enum("step", <crate::input::FontZoomStep as crate::input::EnumArg>::VALUES, "Which way to step."),
            ],
        },
        ActionDescriptor {
            name: "spawn_command",
            label: "Spawn Command",
            description: "Open a new pane running a command.",
            category: ActionCategory::System,
            icon: None,
            args: &[
                ArgDescriptor::required("command", ArgKind::Text, "The command line to run."),
                ArgDescriptor::optional_enum("kind", <crate::input::SpawnKind as crate::input::EnumArg>::VALUES, "What kind of pane to open. Default terminal."),
                ArgDescriptor::optional("float", ArgKind::Bool, "Open it as a floating pane. Default false."),
                ArgDescriptor::optional("close_pane", ArgKind::Bool, "Close the pane when the command exits. Default false."),
                ArgDescriptor::optional("keep_on_error", ArgKind::Bool, "Keep the pane open when the command fails. Default false."),
                ArgDescriptor::optional("keep_on_success", ArgKind::Bool, "Keep the pane open when the command succeeds. Default false."),
            ],
        },
        ActionDescriptor {
            name: "move_column_to_workspace",
            label: "Move Column to Workspace",
            description: "Move a column into another workspace.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to move."),
                ArgDescriptor::required("ws_idx", ArgKind::Int, "Index of the destination workspace."),
                ArgDescriptor::optional("focus", ArgKind::Bool, "Follow the column with focus. Default true."),
            ],
        },
        ActionDescriptor {
            name: "resize_column_by",
            label: "Resize Column By",
            description: "Change one column's width by a fraction of the working width. The mouse divider drag and RPC use this; the keyboard resize acts on the focused column.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column to resize."),
                ArgDescriptor::required("delta", ArgKind::Float, "Fraction of the working width to add; negative shrinks."),
            ],
        },
        ActionDescriptor {
            name: "resize_pane_height_by",
            label: "Resize Pane Height By",
            description: "Change one stacked pane's height by a number of logical pixels.",
            category: ActionCategory::Layout,
            icon: None,
            args: &[
                ArgDescriptor::required("col_idx", ArgKind::Int, "Index of the column holding the pane."),
                ArgDescriptor::required("pane_idx", ArgKind::Int, "Index of the pane within the column."),
                ArgDescriptor::required("delta", ArgKind::Float, "Logical pixels to add; negative shrinks."),
            ],
        },
    ];

}

/// Runtime metadata for **one action** — a built-in or a name-keyed one contributed by a provider
/// or plugin. The single entry type of the [`ActionCatalog`]: built-in and plugin actions have the
/// *same* shape, so every surface (icons, tooltips, command palette, RPC introspection) treats them
/// identically.
///
/// Owned (`String`, not `&'static str`) precisely so a runtime-registered action can join the
/// catalog. The zero-copy `&'static ActionDescriptor` form was the deliberate Phase-A shortcut
/// (action-interaction plan §8b) that deferred this ripple; this is that ripple.
///
/// `policy` is **required, no default**. A built-in gets it from [`action_policy`]'s exhaustive
/// `match` — forgetting to classify a new variant there is a *compile error*, and this field is
/// **computed** from that match at registration, never hand-written, so the match stays the single
/// authority. A name-keyed action has no variant, so nothing else would force the question; a
/// permissive default would mean "the author forgot" silently resolves to the most permissive
/// setting in the system. It is a plain field so an action cannot be registered without answering
/// it.
///
/// [`action_policy`]: crate::app::interaction::action_policy
#[derive(Clone)]
pub struct ActionMeta {
    /// Stable id — the identity used by config bindings, RPC, menus, and `Intent.action`.
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: ActionCategory,
    /// The **component** that declared this action — its `kind()`, e.g. `"workspaces"`. `None` for
    /// a built-in, which belongs to the app itself (F003/P085/T358).
    ///
    /// The *kind*, never a mount id: a component seated twice declares its actions once, and which
    /// **placement** a given call lands on is decided per call by `owning_mount`, not fixed at
    /// registration. It is stamped by the host in `register_provider_actions`, not written by the
    /// component — an author cannot claim to be someone else, and cannot forget.
    ///
    /// It is what lets the palette label an entry with its scope, and `list-actions` say who owns
    /// what instead of returning one flat list in which a component's verbs are indistinguishable
    /// from the app's.
    pub owner: Option<String>,
    /// Centralized action icon — the single source of an action's [`Glyph`]. Every surface that
    /// renders this action reads it from here instead of inventing its own.
    pub icon: Option<Glyph>,
    /// Allow/Block classification for the interaction router. Required — see the type docs.
    pub policy: crate::app::interaction::ActionPolicy,
    /// The arguments this action takes. **Required, and empty means it takes none** — for the same
    /// reason `policy` has no default: an omitted list would read as "takes nothing" and every
    /// argument handed to it would be silently unknown, which is the defect this field exists to
    /// close (action-task-E). [`check_args`] compares a call against this list at both doors.
    pub args: Vec<ArgSpec>,
    /// Declarative confirmation requirement (action-interaction plan §4). `Some` means the central
    /// gate raises a confirm/response prompt before the action runs; `None` runs it straight.
    ///
    /// The guard lives **on the action**, so every surface that dispatches it — keyboard, a header
    /// button, a context-menu entry, RPC — confirms identically. A plugin declares its own the same
    /// way (through [`register_dynamic`]).
    ///
    /// The confirm's toggle key is [`ConfirmSpec::config_name`], a separate field so a spec **may**
    /// be toggled under a name of its own — one spec can govern several `WmAction` variants
    /// (`ClosePane` + `ClosePaneById` confirm identically, §5.1). Every built-in nonetheless uses
    /// its own action name, so a user toggles `[confirm] <action> = false` and nothing else.
    pub confirm: Option<ConfirmSpec>,
}

// Manual because `ConfirmSpec` is intentionally not `Debug` — it can hold a native `Callback`
// closure (§6), which is neither `Debug` nor serializable. Rendering it as a bool keeps `ActionMeta`
// printable without dragging that requirement onto the whole confirm model.
impl std::fmt::Debug for ActionMeta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ActionMeta")
            .field("name", &self.name)
            .field("label", &self.label)
            .field("description", &self.description)
            .field("category", &self.category)
            .field("icon", &self.icon)
            .field("policy", &self.policy)
            .field("args", &self.args)
            .field("confirm", &self.confirm.is_some())
            .finish()
    }
}

/// Runtime catalog of action metadata, owned by [`AppState`](crate::app_state::AppState).
///
/// **The single metadata home for every action**, built-in or plugin. Seeded from the built-in
/// [`ActionRegistry::ALL`] descriptors at startup and extended at runtime through
/// [`register_dynamic`]. Every UI surface resolves action metadata through here — icons, labels,
/// the pick prompts, the command palette, RPC introspection.
///
/// It lives on `AppState` (rather than inside `ActionRegistry`) for a borrow reason: an action
/// handler is `fn(&mut AppState, &WmAction)` and receives no registry, so metadata must be
/// reachable from the state. The registry holds the *handlers*; this holds the *metadata*. One of
/// each — never two of either.
pub struct ActionCatalog {
    by_name: HashMap<String, ActionMeta>,
    /// Names in stable order (built-ins in `ALL` order, then registration order).
    order: Vec<String>,
    /// The built-in names, so a dynamic action can never shadow or retire one.
    builtins: std::collections::HashSet<&'static str>,
}

impl ActionCatalog {
    /// Build the catalog seeded from the built-in [`ActionRegistry::ALL`] descriptors.
    ///
    /// Each built-in's `policy` is **computed** from [`action_policy`](crate::app::interaction::action_policy)
    /// via [`builtin_policy`] — never hand-written here — so the exhaustive `match` in
    /// `interaction.rs` remains the only authority on built-in policy and the two cannot drift.
    ///
    /// The three destructive confirm specs are attached to their **owner action's** meta — `close`,
    /// `delete_column`, `delete_workspace` — each toggled under that same name (§5.1; the spec's
    /// `config_name` may differ from the action name, but no built-in needs it to).
    pub fn with_builtins() -> Self {
        let mut catalog = Self {
            by_name: HashMap::new(),
            order: Vec::new(),
            builtins: ActionRegistry::ALL.iter().map(|d| d.name).collect(),
        };
        for d in ActionRegistry::ALL {
            let args: Vec<ArgSpec> = d.args.iter().map(ArgSpec::from_descriptor).collect();
            catalog.insert(ActionMeta {
                name: d.name.to_string(),
                label: d.label.to_string(),
                description: d.description.to_string(),
                category: d.category,
                // The app's own — a built-in belongs to no component.
                owner: None,
                icon: d.icon,
                policy: builtin_policy(d.name, &args),
                args,
                confirm: None,
            });
        }
        for (owner_action, spec) in builtin_confirm_specs() {
            if let Some(meta) = catalog.by_name.get_mut(owner_action) {
                meta.confirm = Some(spec);
            } else {
                debug_assert!(false, "confirm spec owner {owner_action:?} has no built-in action");
            }
        }
        catalog
    }

    /// Add or replace an action's metadata. Re-registering an id replaces it (a provider
    /// remounting) while keeping its position in the stable order.
    pub(crate) fn insert(&mut self, meta: ActionMeta) {
        if !self.by_name.contains_key(&meta.name) {
            self.order.push(meta.name.clone());
        }
        self.by_name.insert(meta.name.clone(), meta);
    }

    /// Insert a **name-keyed** action's metadata, refusing to shadow a built-in.
    ///
    /// A built-in's id is compiled in and its label, icon, policy and confirm are what every
    /// surface renders, so a component taking it over would silently change the meaning of a menu
    /// entry the component has nothing to do with. `false` means the id was rejected and the
    /// built-in still owns it; a duplicate between two *dynamic* declarers is allowed (a remount
    /// legitimately replaces itself) but recorded, because two different components claiming one id
    /// is a mistake either way.
    ///
    /// Returns whether the id was free — the caller records the collision.
    pub(crate) fn insert_dynamic(&mut self, meta: ActionMeta) -> Result<(), DuplicateAction> {
        if self.is_builtin(&meta.name) {
            return Err(DuplicateAction::ShadowsBuiltin);
        }
        let replaced = self.by_name.contains_key(&meta.name);
        self.insert(meta);
        if replaced {
            return Err(DuplicateAction::ReplacedDynamic);
        }
        Ok(())
    }

    /// Remove a **dynamic** action's metadata. Built-ins are never removable, so a provider
    /// unmounting can't retire `close`. `true` if a dynamic entry was removed.
    pub(crate) fn remove_dynamic(&mut self, name: &str) -> bool {
        if self.is_builtin(name) || !self.by_name.contains_key(name) {
            return false;
        }
        self.by_name.remove(name);
        self.order.retain(|n| n != name);
        true
    }

    /// Whether `name` is one of the compiled-in built-in actions.
    pub fn is_builtin(&self, name: &str) -> bool {
        self.builtins.contains(name)
    }

    /// The declarative confirmation spec for an action, by its **action name** — the owner, e.g.
    /// `close`. `None` when the action needs no prompt.
    pub fn confirm_spec(&self, action_name: &str) -> Option<&ConfirmSpec> {
        self.find(action_name).and_then(|m| m.confirm.as_ref())
    }

    /// Look up an action's metadata by its name.
    pub fn find(&self, name: &str) -> Option<&ActionMeta> {
        self.by_name.get(name)
    }

    /// The declared interaction policy for a name-keyed action — how the router judges a plugin
    /// action by exactly the same rules as a built-in.
    pub fn policy(&self, name: &str) -> Option<crate::app::interaction::ActionPolicy> {
        self.find(name).map(|m| m.policy)
    }

    /// The icon [`Glyph`] for an action, by name — the single source of action iconography
    /// (pane-action bar, context menu, command palette all resolve through here).
    ///
    /// **Every action has one**: its own if it declared one, else [`GENERIC_ACTION_ICON`] — so no
    /// surface has to decide what to do with a blank. `None` only for a name nothing knows.
    pub fn icon(&self, name: &str) -> Option<Glyph> {
        self.find(name).map(|m| m.icon.unwrap_or(GENERIC_ACTION_ICON))
    }

    /// The human-readable label for an action, by name — so a caller names the action
    /// rather than re-spelling the label.
    pub fn label(&self, name: &str) -> Option<&str> {
        self.find(name).map(|m| m.label.as_str())
    }

    /// **Is this action destructive?** — the single source, read by every surface that renders it.
    ///
    /// An action declares this by carrying a [`ConfirmSpec`]: the central gate asks before running
    /// it because it cannot be undone, and that is the same fact a surface needs to draw it in the
    /// danger hue. Declared once, in `builtin_confirm_specs`, rather than each surface deciding
    /// from the action's *name* — which is how a pane header came to have `matches!(action, Close)`
    /// written into it, a styling rule keyed to a name that no other surface would ever share
    /// (Antonio, 2026-09-03).
    ///
    /// Same reasoning as [`icon`](Self::icon): every surface reads it from here instead of
    /// inventing its own, so a menu entry, a header button and the palette cannot drift.
    pub fn destructive(&self, name: &str) -> bool {
        self.find(name).is_some_and(|m| m.confirm.is_some())
    }

    /// Every action's metadata, in stable order — what a surface that **renders** actions walks
    /// (the command palette). Distinct from [`describe_all`](Self::describe_all), which projects to
    /// the serializable [`ActionInfo`] for the wire and drops the `Glyph`.
    pub(crate) fn all(&self) -> impl Iterator<Item = &ActionMeta> + '_ {
        self.order.iter().filter_map(|n| self.by_name.get(n))
    }

    /// All actions in a given category, in stable order.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "preserved for the command palette + RPC introspection")
    )]
    pub fn by_category(&self, category: ActionCategory) -> impl Iterator<Item = &ActionMeta> + '_ {
        self.order
            .iter()
            .filter_map(move |n| self.by_name.get(n))
            .filter(move |m| m.category == category)
    }

    /// Total number of catalogued actions.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "preserved for the command palette + RPC introspection")
    )]
    pub fn count(&self) -> usize {
        self.order.len()
    }

    /// Every action's metadata as a serializable [`ActionInfo`], in stable order — the RPC / command
    /// palette **introspection** surface (action-task-C). Built-in and plugin actions alike, so a
    /// tool can discover what a running heca (with its plugins) can do, by name.
    pub fn describe_all(&self) -> Vec<ActionInfo> {
        self.order
            .iter()
            .filter_map(|n| self.by_name.get(n))
            .map(ActionInfo::from_meta)
            .collect()
    }

    /// One action's metadata as a serializable [`ActionInfo`], by name — `None` if unknown.
    pub fn describe(&self, name: &str) -> Option<ActionInfo> {
        self.find(name).map(ActionInfo::from_meta)
    }
}

/// A serializable projection of an action's metadata — what RPC introspection returns. Enums are
/// rendered as their stable string names so the wire form is stable and language-neutral (an
/// [`Intent`](crate::chrome::Intent)-style contract). A native `Callback` outcome is **not**
/// serializable, so `confirm` reports only *that* a prompt exists and its toggle key, never the
/// outcome closures (§6).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionInfo {
    pub name: String,
    pub label: String,
    pub description: String,
    pub category: String,
    pub policy: String,
    /// The component that declared this action (its `kind()`), or `None` for one of the app's own
    /// built-ins — so a tool discovering a running heca can tell whose verb it is looking at
    /// (F003/P085/T358).
    pub owner: Option<String>,
    /// The arguments the action takes, in declaration order — what a caller has to supply to build
    /// it. Empty when it takes none. Without this a tool could discover an action's *name* and
    /// still have no way to learn how to call it.
    pub args: Vec<ArgSpec>,
    /// The confirm toggle key (`ConfirmSpec::config_name`) when the action prompts; `None` if it
    /// runs straight.
    pub confirm: Option<String>,
}

impl ActionInfo {
    fn from_meta(m: &ActionMeta) -> Self {
        use crate::app::interaction::ActionPolicy;
        let policy = match m.policy {
            ActionPolicy::Global => "global",
            ActionPolicy::AlwaysAllowed => "always_allowed",
            ActionPolicy::TiledOnly => "tiled_only",
            ActionPolicy::FocusedPaneLocal => "focused_pane_local",
            ActionPolicy::WorkspaceLevel => "workspace_level",
            ActionPolicy::SourceDependent => "source_dependent",
            ActionPolicy::ContainerFocused => "container_focused",
        };
        Self {
            name: m.name.clone(),
            label: m.label.clone(),
            description: m.description.clone(),
            category: m.category.label().to_string(),
            policy: policy.to_string(),
            owner: m.owner.clone(),
            args: m.args.clone(),
            confirm: m.confirm.as_ref().map(|c| c.config_name.clone()),
        }
    }
}

/// The interaction policy of a **built-in**, derived from the one authority:
/// [`action_policy`](crate::app::interaction::action_policy)'s exhaustive `match`.
///
/// Reading a policy needs a `WmAction`, and a unit action gives one straight from its name. A
/// parameterized one does not — it is built from arguments (mouse / HintKey / selection / context
/// menu / RPC / the GUI scrollbar / a config binding with an `args` table) — so this builds a
/// stand-in from the action's **own declared arguments**
/// ([`ArgSpec::sample_value`]) purely to read the policy off the same match.
///
/// That declaration is what removed the hand-written table that used to sit here: it named a
/// representative variant for six actions and `unreachable!`d on the rest, so the other twenty-three
/// argument-taking actions could not be catalogued at all. Now every one of them can.
///
/// Never classify an action here: classify it in `action_policy` and it lands here automatically.
/// A declaration that does not build its action fails at startup, where `with_builtins` runs.
fn builtin_policy(name: &str, args: &[ArgSpec]) -> crate::app::interaction::ActionPolicy {
    let action = crate::input::action_from_name(name)
        .or_else(|| crate::input::build_action(name, &sample_args(args)))
        .unwrap_or_else(|| {
            unreachable!(
                "built-in action {name:?} builds from neither its name nor its declared \
                 arguments: add it to `action_from_name`, or correct its `args` declaration \
                 so `build_action` accepts it"
            )
        });
    crate::app::interaction::action_policy(&action)
}

/// A well-formed argument map built from a declaration alone — every declared argument set to a
/// value of its own kind. Used to build a representative action from its metadata, and by the tests
/// that check a declaration against the code that reads it.
pub fn sample_args(args: &[ArgSpec]) -> std::collections::HashMap<String, String> {
    args.iter()
        .map(|a| (a.name.clone(), a.sample_value()))
        .collect()
}

/// What a **built-in** action declares it takes, by name — read straight from
/// [`ActionRegistry::ALL`] rather than from the [`ActionCatalog`].
///
/// The catalog is the metadata home, but config is read before any `AppState` exists
/// (`HecaApp::new` builds the keymap; `startup` builds the catalog), and a keybinding can only
/// name a built-in at load time anyway — a provider's action does not exist yet. So the config
/// door reads the static seed directly instead of a second catalog being built to serve it.
///
/// `None` for a name that is not a built-in, which is not an error: it may be an action a provider
/// or plugin registers later.
pub fn builtin_args(name: &str) -> Option<Vec<ArgSpec>> {
    ActionRegistry::ALL
        .iter()
        .find(|d| d.name == name)
        .map(|d| d.args.iter().map(ArgSpec::from_descriptor).collect())
}

impl Default for ActionCatalog {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
//  Declarative action confirmation / response (action-interaction plan, Phase B)
// ══════════════════════════════════════════════════════════════════════════════

/// A **native** outcome callback (see [`Outcome::Callback`]). Runs once when its response button is
/// chosen, receiving the live [`AppState`](crate::app_state::AppState) + [`ActionRegistry`].
///
/// **NATIVE-ONLY — read before using.** A closure is not serializable, so it can never cross the
/// WASM or RPC boundary; the plugin-facing builder does **not** expose it, and when an action's
/// metadata is serialized (RPC/introspection) a `Callback` outcome is rendered **opaquely**
/// (e.g. `"native"`), never silently dropped. **Prefer [`Outcome::Dispatch`]** (portable, testable,
/// RPC-drivable): reach for `Callback` only when the logic genuinely cannot be a named action — and
/// first ask whether a small native action + `Dispatch` is cleaner. It runs *after* the user chose,
/// so it executes directly and does not re-enter interaction policy; do not use it to smuggle
/// un-gated destructive work (compose `Dispatch`/`Proceed` for that). Captured state must be
/// `'static` (the `Rc`), like every [`open_modal`](crate::chrome::open_modal) completion.
pub type ConfirmCallback = std::rc::Rc<dyn Fn(&mut crate::app_state::AppState, &ActionRegistry)>;

/// The role of a confirmation response button — drives initial focus (Default/Cancel), the danger
/// tint (Danger), and which button Esc / scrim maps to (Cancel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonRole {
    /// The safe default — takes initial focus so Enter activates it.
    Default,
    /// The cancel choice — Esc / scrim (when dismissible) resolve to this button's outcome.
    Cancel,
    /// A destructive choice — rendered with the danger tint.
    Danger,
}

/// What choosing a response button does. Declarative variants (`Proceed`/`Cancel`/`Dispatch`) are
/// serializable and plugin/RPC-safe; [`Callback`](Outcome::Callback) is a native-only escape hatch.
#[derive(Clone)]
pub enum Outcome {
    /// Run the **original gated action** (the "yes, do it"). Executed via `registry.execute`, which
    /// bypasses the dispatch gate that raised the prompt (no loop).
    Proceed,
    /// Do nothing.
    Cancel,
    /// Dispatch **another** action (declarative — plugin/RPC-safe); it is policy-routed normally.
    // Used by plugin-declared confirmations (Phase C) + tests; the built-in specs use Proceed/Cancel.
    #[cfg_attr(not(test), allow(dead_code))]
    Dispatch(crate::input::WmAction),
    /// Run a native closure. **Native-only** — see [`ConfirmCallback`].
    #[cfg_attr(not(test), allow(dead_code))]
    Callback(ConfirmCallback),
}

/// One response button of a [`ConfirmSpec`].
#[derive(Clone)]
pub struct ResponseButton {
    /// Comes back in [`ModalResult::Action`](crate::chrome::ModalResult); also the tooltip action id.
    pub id: String,
    pub label: String,
    pub role: ButtonRole,
    pub outcome: Outcome,
}

impl ResponseButton {
    /// A `Cancel`-role button that does nothing (the safe default choice).
    pub fn cancel(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: ButtonRole::Cancel,
            outcome: Outcome::Cancel,
        }
    }

    /// A button that runs the original action ([`Outcome::Proceed`]); `danger` gives it the
    /// destructive tint + `Danger` role, otherwise the `Default` role.
    pub fn proceed(id: impl Into<String>, label: impl Into<String>, danger: bool) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: if danger { ButtonRole::Danger } else { ButtonRole::Default },
            outcome: Outcome::Proceed,
        }
    }

    /// A fully-specified button.
    // Used by plugin-declared confirmations (Phase C) + tests; the built-in specs use the
    // `cancel`/`proceed` constructors.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        role: ButtonRole,
        outcome: Outcome,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role,
            outcome,
        }
    }
}

/// Declarative confirmation / response requirement attached to an action. Pure data (the native
/// [`Outcome::Callback`] aside): the central gate converts it to a
/// [`ModalSpec`](crate::chrome::ModalSpec) at open time and runs the chosen button's [`Outcome`].
/// The action's **title** stays dynamic (computed by the gate from the concrete target); this spec
/// owns the reusable parts — the body message, the response buttons + outcomes, whether the choice
/// is forced, and the on/off config key.
#[derive(Clone)]
pub struct ConfirmSpec {
    /// Body message under the (dynamic) title — e.g. "This action cannot be undone."
    pub message: String,
    /// The response buttons (2 for yes/no, 3 for yes/no/cancel, N for anything).
    pub buttons: Vec<ResponseButton>,
    /// `false` = forced decision (Esc / scrim swallowed) — mirrors `Dialog::dismissible`.
    pub dismissible: bool,
    /// Config key under `[confirm]` that toggles this prompt on/off (Phase B maps the three
    /// built-ins to the existing `[settings] confirm_*` flags; the generic `[confirm]` table is
    /// Phase B2). Defaults to [`default_enabled`](ConfirmSpec::default_enabled) when unset.
    pub config_name: String,
    pub default_enabled: bool,
}

/// The built-in confirmation specs, each paired with the **owner action name** it attaches to. The
/// three destructive actions — close pane / delete column / delete workspace — each get a
/// `[Cancel] [<verb>]` forced prompt.
///
/// Note the `close` row: `ClosePane` and `ClosePaneById` both resolve to this **one** spec (§5.1),
/// which is what `config_name` is for — it stays a separate field so a spec can be toggled under a
/// name of its own. No built-in uses that freedom: every one of these is toggled by its own action
/// name, so `[confirm] close = false` disables the pane prompt.
///
/// A pane is **closed**, not deleted: that is the word its id, its binding name and its label all
/// use, and the "cannot be undone" line already carries the weight. Delete is kept for the two
/// containers below, which are a different kind of thing.
fn builtin_confirm_specs() -> Vec<(&'static str, ConfirmSpec)> {
    let mk = |config_name: &'static str, verb: &str| ConfirmSpec {
        message: "This action cannot be undone.".to_string(),
        buttons: vec![
            ResponseButton::cancel("cancel", "Cancel"),
            ResponseButton::proceed("confirm", verb, true),
        ],
        dismissible: false,
        config_name: config_name.to_string(),
        default_enabled: true,
    };
    vec![
        ("close", mk("close", "Close")),
        ("delete_column", mk("delete_column", "Delete")),
        ("delete_workspace", mk("delete_workspace", "Delete")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::WmAction;

    // ══════════════════════════════════════════════════════════════════════════
    //  Declared arguments (action-task-E)
    //
    //  An action's `args` are a *claim* about code that lives somewhere else —
    //  `build_action`'s match arm, which reads each name as a string literal. These four tests are
    //  what makes the claim true, in both directions: the three below walk every declaration and
    //  check the code agrees, and `every_wm_action_variant_is_reachable_by_name` (in `input.rs`,
    //  where the exhaustive variant list lives) walks every action and checks a declaration exists.
    //
    //  To see them work, misspell one name in a `build_action` arm — `pane_id` → `paneid` — and
    //  `every_declared_argument_is_read_by_the_action` fails on that action.
    // ══════════════════════════════════════════════════════════════════════════

    /// The declarations of every action that takes arguments, as the catalog holds them.
    fn declared_args() -> Vec<(&'static str, Vec<ArgSpec>)> {
        ActionRegistry::ALL
            .iter()
            .filter(|d| !d.args.is_empty())
            .map(|d| {
                (
                    d.name,
                    d.args.iter().map(ArgSpec::from_descriptor).collect(),
                )
            })
            .collect()
    }

    /// Supply exactly what an action declares and it builds. Fails when a declaration names an
    /// argument the code does not read, or misses one it requires.
    #[test]
    fn every_declared_argument_is_read_by_the_action() {
        for (name, specs) in declared_args() {
            let args = sample_args(&specs);
            assert!(
                crate::input::build_action(name, &args).is_some(),
                "{name} declares {:?} but build_action refuses that exact call — the declaration \
                 and the arm that reads it have drifted",
                specs.iter().map(|s| &s.name).collect::<Vec<_>>(),
            );
        }
    }

    /// `required: true` means it. Drop each required argument on its own and the action must
    /// refuse to build.
    #[test]
    fn every_required_argument_is_actually_required() {
        for (name, specs) in declared_args() {
            for spec in specs.iter().filter(|s| s.required) {
                let mut args = sample_args(&specs);
                args.remove(&spec.name);
                assert!(
                    crate::input::build_action(name, &args).is_none(),
                    "{name} builds without '{}', so that argument is not required — declare it \
                     optional, or the caller will never learn it was ignored",
                    spec.name,
                );
            }
        }
    }

    /// `required: false` means it too. An optional argument can be left out and the action still
    /// builds, on its declared default.
    #[test]
    fn every_optional_argument_is_actually_optional() {
        for (name, specs) in declared_args() {
            for spec in specs.iter().filter(|s| !s.required) {
                let mut args = sample_args(&specs);
                args.remove(&spec.name);
                assert!(
                    crate::input::build_action(name, &args).is_some(),
                    "{name} refuses to build without '{}', so that argument is required — say so, \
                     or a caller that omits it gets nothing and no reason",
                    spec.name,
                );
            }
        }
    }

    /// An action that **needs** a target must not be reachable from its bare name.
    ///
    /// This is the line that stops the defect coming back. `action_from_name` used to answer
    /// `delete_workspace` with `DeleteWorkspace { ws_idx: 0 }`, so binding that name to a key
    /// deleted the *first* workspace — not the focused one, not nothing. Eight actions did that.
    /// Now a required argument has to be supplied, and the caller hears about it if it is not.
    #[test]
    fn every_action_that_needs_a_target_refuses_to_default_it() {
        for d in ActionRegistry::ALL {
            if d.args.iter().any(|a| a.required) {
                assert!(
                    crate::input::action_from_name(d.name).is_none(),
                    "{} requires an argument but resolves from its bare name alone — that answer \
                     is a guess, and a silent one",
                    d.name,
                );
            }
        }
    }

    #[test]
    fn check_args_names_what_is_wrong() {
        let specs = builtin_args("move_column").unwrap();

        assert!(check_args(&specs, &sample_args(&specs)).is_empty(), "a well-formed call is quiet");

        // A misspelling is named, and the intended argument is offered.
        let mut typo = sample_args(&specs);
        typo.remove("src_col");
        typo.insert("src_colum".to_string(), "0".to_string());
        let problems = check_args(&specs, &typo);
        assert!(problems.contains(&ArgProblem::Missing { name: "src_col".to_string() }));
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
            crate::input::build_action("take_pane", &args).is_some(),
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
            ArgProblem::Unknown { name: "colour".to_string(), did_you_mean: None }.to_string(),
            "unknown argument 'colour'",
        );
        assert_eq!(
            ArgProblem::Missing { name: "ws_idx".to_string() }.to_string(),
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
        bad.insert("axis".to_string(), "sideways".to_string());
        let message = check_args(&specs, &bad)
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        assert_eq!(
            message,
            "argument 'axis' expected one of x, horizontal, width, y, vertical, height, got 'sideways'",
        );
    }

    /// Every action that takes arguments says so where a caller can read it — `list-actions` and
    /// `describe-action` carry the list, not just the name.
    #[test]
    fn introspection_carries_the_arguments() {
        let catalog = ActionCatalog::with_builtins();
        let info = catalog.describe("resize").unwrap();
        let names: Vec<&str> = info.args.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["target", "axis", "amount"]);

        let target = &info.args[0];
        assert_eq!(target.kind, ArgKind::Enum);
        assert!(target.values.contains(&"column".to_string()));
        assert!(target.required);

        let json = serde_json::to_string(&info).unwrap();
        let back: ActionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.args, info.args, "the declaration survives the wire");
    }

    #[test]
    fn test_registry_dispatch() {
        let mut registry = ActionRegistry::new();
        fn dummy_handler(state: &mut crate::app_state::AppState, _action: &WmAction) {
            state.needs_redraw = true;
        }
        registry.register(&WmAction::FocusLeft, dummy_handler);
        assert!(registry.has_handler(&WmAction::FocusLeft));
        assert!(!registry.has_handler(&WmAction::FocusRight));
    }

    #[test]
    fn test_registry_has_actions() {
        assert!(
            ActionCatalog::with_builtins().count() > 0,
            "catalog should not be empty"
        );
    }

    #[test]
    fn test_find_known_action() {
        let catalog = ActionCatalog::with_builtins();
        let desc = catalog.find("focus_left");
        assert!(desc.is_some(), "should find focus_left");
        let desc = desc.unwrap();
        assert_eq!(desc.label, "Focus Column Left");
        assert!(matches!(desc.category, ActionCategory::Navigation));
    }

    #[test]
    fn test_find_unknown_action() {
        assert!(ActionCatalog::with_builtins().find("nonexistent").is_none());
    }

    #[test]
    fn test_selection_action_descriptors_exist() {
        let catalog = ActionCatalog::with_builtins();
        for name in [
            "enter_selection_mode",
            "selection_left",
            "selection_right",
            "selection_up",
            "selection_down",
            "clear_selection",
            "copy_selection",
            "paste_clipboard",
        ] {
            let desc = catalog.find(name);
            assert!(desc.is_some(), "missing descriptor for {name}");
            let desc = desc.unwrap();
            assert!(!desc.label.is_empty(), "{name} label must be set");
            assert!(
                !desc.description.is_empty(),
                "{name} description must be set"
            );
        }
    }

    #[test]
    fn test_by_category() {
        let catalog = ActionCatalog::with_builtins();
        let nav_count = catalog.by_category(ActionCategory::Navigation).count();
        assert!(nav_count > 0, "should have navigation actions");

        let sys_count = catalog.by_category(ActionCategory::System).count();
        assert!(sys_count > 0, "should have system actions");
    }

    #[test]
    fn test_all_names_unique() {
        let mut names: Vec<&str> = ActionRegistry::ALL.iter().map(|d| d.name).collect();
        let original_len = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), original_len, "all action names must be unique");
    }

    // ── plugin-04 / T3: the one runtime registry ──

    /// The built-in metadata after seeding is IDENTICAL to the `const ALL` descriptors it came
    /// from — the owned-`String` ripple must not have changed a single value (the non-regression
    /// test the task asks for).
    #[test]
    fn builtin_metadata_survives_the_owned_ripple_unchanged() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(catalog.count(), ActionRegistry::ALL.len());
        for d in ActionRegistry::ALL {
            let m = catalog
                .find(d.name)
                .unwrap_or_else(|| panic!("missing meta for {}", d.name));
            assert_eq!(m.name, d.name);
            assert_eq!(m.label, d.label);
            assert_eq!(m.description, d.description);
            assert_eq!(m.category, d.category);
            assert_eq!(m.icon, d.icon);
            assert!(catalog.is_builtin(d.name));
        }
    }

    /// Every built-in's `policy` is COMPUTED from `action_policy`'s exhaustive match, never
    /// hand-written — so the match stays the single authority and the two cannot drift. This also
    /// proves `builtin_policy` resolves all 115 names (it would `unreachable!` otherwise).
    #[test]
    fn builtin_policy_is_derived_from_the_exhaustive_match() {
        use crate::app::interaction::{action_policy, ActionPolicy};
        let catalog = ActionCatalog::with_builtins();
        for d in ActionRegistry::ALL {
            let meta = catalog.find(d.name).unwrap();
            if let Some(action) = crate::input::action_from_name(d.name) {
                assert_eq!(
                    meta.policy,
                    action_policy(&action),
                    "{}: catalog policy diverged from action_policy()",
                    d.name
                );
            }
        }
        // Spot-check the two parameterized names that have no bare-name variant.
        assert_eq!(
            catalog.find("scroll_to_offset").unwrap().policy,
            ActionPolicy::FocusedPaneLocal
        );
        assert_eq!(
            catalog.find("open_link").unwrap().policy,
            action_policy(&WmAction::OpenLink { url: String::new() })
        );
    }

    fn dyn_meta(name: &str, policy: crate::app::interaction::ActionPolicy) -> ActionMeta {
        ActionMeta {
            name: name.to_string(),
            label: "Restart Container".to_string(),
            description: "Restart the selected Docker container.".to_string(),
            category: ActionCategory::System,
            owner: None,
            icon: Some(Glyph::Trash),
            policy,
            args: Vec::new(),
            confirm: None,
        }
    }

    /// A name-keyed action joins the SAME catalog as the built-ins — so it gets a label, an icon and
    /// introspection exactly like `close` does. This is the whole point of the owned ripple: before
    /// it, a plugin action could only be dispatched, never rendered.
    #[test]
    fn a_dynamic_action_lives_in_the_same_catalog_as_the_builtins() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let builtins = catalog.count();

        let handle = register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.docker.restart", ActionPolicy::Global),
            Some(std::rc::Rc::new(|_state, _intent| {})),
        );

        assert_eq!(handle, Ok(ActionHandle("plugin.docker.restart".to_string())));
        assert_eq!(catalog.count(), builtins + 1);
        assert_eq!(
            catalog.label("plugin.docker.restart"),
            Some("Restart Container")
        );
        assert_eq!(catalog.icon("plugin.docker.restart"), Some(Glyph::Trash));
        assert_eq!(
            catalog.policy("plugin.docker.restart"),
            Some(ActionPolicy::Global),
            "the router reads the DECLARED policy"
        );
        assert!(!catalog.is_builtin("plugin.docker.restart"));
        assert_eq!(
            registry.dispatch_of(&catalog, "plugin.docker.restart"),
            Some(Dispatch::NativeDyn)
        );
        // A built-in still reports as Native, through the same one door.
        assert_eq!(
            registry.dispatch_of(&catalog, "close"),
            Some(Dispatch::Native)
        );
        assert_eq!(registry.dispatch_of(&catalog, "nope.not.a.thing"), None);
    }

    /// **Every action renders with an icon** — its own, or the one generic mark.
    ///
    /// A per-category fallback was tried first and removed the same day: it filled every row, but
    /// all four `focus_*` then wore the same arrow, which reads as a wrong meaning rather than as
    /// no meaning. A plain circle claims nothing.
    #[test]
    fn an_action_without_its_own_icon_shows_the_generic_mark() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(
            catalog.icon("close"),
            Some(Glyph::FolderSimpleMinus),
            "an action that declares its own keeps it",
        );
        assert_eq!(
            catalog.icon("focus_toggle_local"),
            Some(GENERIC_ACTION_ICON),
            "…and one that declares none shows the generic mark, not a family glyph",
        );
        assert!(
            catalog.all().all(|m| catalog.icon(&m.name).is_some()),
            "no catalogued action may render blank",
        );
        assert_eq!(catalog.icon("nope.not.a.thing"), None, "only an unknown name has none");
    }

    /// **Introspection says whose verb it is** (F003/P085/T358). Without an owner, `list-actions`
    /// returns one flat list in which a component's actions are indistinguishable from the app's,
    /// and a tool has no way to group or scope them.
    #[test]
    fn introspection_names_the_component_that_declared_an_action() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let mut meta = dyn_meta("docker.restart_selected", ActionPolicy::ContainerFocused);
        // What `register_provider_actions` stamps from the provider being registered.
        meta.owner = Some("docker".to_string());
        let _ = register_dynamic(&mut registry, &mut catalog, meta, None);

        let info = catalog
            .describe("docker.restart_selected")
            .expect("a declared action is describable");
        assert_eq!(info.owner.as_deref(), Some("docker"));
        assert_eq!(
            catalog.describe("close").and_then(|i| i.owner),
            None,
            "a built-in belongs to the app itself",
        );
        // And it survives the wire form, which is the only reason it exists.
        let json = serde_json::to_string(&info).expect("ActionInfo is serializable");
        assert!(json.contains("\"owner\":\"docker\""), "{json}");
    }

    /// `unregister` (the provider unmounting and dropping its handle) retires BOTH halves — the
    /// handler and the metadata — so the id is no longer dispatchable or renderable.
    #[test]
    fn unregister_retires_both_the_handler_and_the_metadata() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let _ = register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.docker.restart", ActionPolicy::TiledOnly),
            Some(std::rc::Rc::new(|_state, _intent| {})),
        );

        assert!(unregister_dynamic(
            &mut registry,
            &mut catalog,
            "plugin.docker.restart"
        ));
        assert!(catalog.find("plugin.docker.restart").is_none());
        assert_eq!(registry.dispatch_of(&catalog, "plugin.docker.restart"), None);
        // Idempotent: retiring it twice is not an error.
        assert!(!unregister_dynamic(
            &mut registry,
            &mut catalog,
            "plugin.docker.restart"
        ));
    }

    /// Re-registering an id (a provider remounting) REPLACES the entry rather than duplicating it.
    #[test]
    fn re_registering_an_id_replaces_it() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let before = catalog.count();
        let _ = register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.x", ActionPolicy::Global),
            None,
        );
        let mut second = dyn_meta("plugin.x", ActionPolicy::TiledOnly);
        second.label = "Second".to_string();
        let _ = register_dynamic(&mut registry, &mut catalog, second, None);

        assert_eq!(catalog.count(), before + 1, "replaced, not duplicated");
        assert_eq!(catalog.label("plugin.x"), Some("Second"));
        assert_eq!(catalog.policy("plugin.x"), Some(ActionPolicy::TiledOnly));
    }

    /// A DECLARATIVE action (no host handler — its owner is a WASM plugin) is declared and
    /// policy-classified, but the host cannot run it. Dispatching it is a no-op, never a crash.
    #[test]
    fn a_declarative_action_is_declared_but_has_no_host_handler() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let _ = register_dynamic(
            &mut registry,
            &mut catalog,
            dyn_meta("plugin.wasm.thing", ActionPolicy::FocusedPaneLocal),
            None, // no host handler
        );
        assert_eq!(
            registry.dispatch_of(&catalog, "plugin.wasm.thing"),
            Some(Dispatch::Declarative)
        );
        assert_eq!(
            catalog.policy("plugin.wasm.thing"),
            Some(ActionPolicy::FocusedPaneLocal)
        );
    }

    /// A dynamic action can neither shadow nor retire a built-in.
    #[test]
    fn a_dynamic_action_cannot_retire_a_builtin() {
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        assert!(!unregister_dynamic(&mut registry, &mut catalog, "close"));
        assert!(catalog.find("close").is_some(), "built-in survives");
    }

    #[test]
    fn test_descriptors_are_populated() {
        for desc in ActionRegistry::ALL {
            assert!(!desc.name.is_empty(), "name must not be empty");
            assert!(!desc.label.is_empty(), "label must not be empty");
            assert!(
                !desc.description.is_empty(),
                "description must not be empty"
            );
            assert!(
                !desc.category.label().is_empty(),
                "category label must not be empty"
            );
        }
    }

    #[test]
    fn builtin_confirm_specs_are_declared_for_the_destructive_actions() {
        let catalog = ActionCatalog::with_builtins();
        // The confirm spec lives ON the owner action's meta (action-task-C), keyed by ACTION name
        // — not a parallel index keyed by toggle key. `config_name` stays a separate field (§5.1),
        // but every built-in is toggled under its own name, so the two agree here.
        for (owner, toggle_key) in [
            ("close", "close"),
            ("delete_column", "delete_column"),
            ("delete_workspace", "delete_workspace"),
        ] {
            let spec = catalog
                .confirm_spec(owner)
                .unwrap_or_else(|| panic!("missing confirm spec on meta for {owner}"));
            // Same spec is reachable directly off the meta.
            assert!(catalog.find(owner).unwrap().confirm.is_some());
            assert!(!spec.dismissible, "{owner} is a forced decision");
            assert!(spec.default_enabled);
            assert_eq!(spec.config_name, toggle_key, "{owner} toggle key");
            assert_eq!(spec.buttons.len(), 2, "{owner}: cancel + confirm");
            assert_eq!(spec.buttons[0].role, ButtonRole::Cancel);
            assert_eq!(spec.buttons[1].role, ButtonRole::Danger);
            assert!(matches!(spec.buttons[1].outcome, Outcome::Proceed));
        }
        // `delete_pane` was this spec's toggle key until the vocabulary was made to agree; it is
        // not an action, so it is not a confirm-spec lookup key.
        assert!(catalog.confirm_spec("delete_pane").is_none());
        // Non-destructive actions carry no confirm spec.
        assert!(catalog.confirm_spec("focus_left").is_none());
        assert!(catalog.find("focus_left").unwrap().confirm.is_none());
    }

    /// `register(ActionSpec)` wires BOTH halves of a native action in one call: the handler into the
    /// registry (dispatchable), and the meta — with its confirm — into the catalog (introspectable).
    #[test]
    fn register_native_wires_handler_and_meta_together() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog {
            by_name: HashMap::new(),
            order: Vec::new(),
            builtins: std::collections::HashSet::new(),
        };
        fn noop(state: &mut crate::app_state::AppState, _a: &WmAction) {
            state.needs_redraw = true;
        }
        let mut meta = dyn_meta("reload_config", ActionPolicy::Global);
        meta.confirm = None;
        register(
            &mut registry,
            &mut catalog,
            ActionSpec {
                action: WmAction::ReloadConfig,
                handler: noop,
                meta,
            },
        );
        assert!(registry.has_handler(&WmAction::ReloadConfig), "handler wired");
        assert!(catalog.find("reload_config").is_some(), "meta wired");
        assert_eq!(
            registry.dispatch_of(&catalog, "reload_config"),
            Some(Dispatch::Native)
        );
    }

    /// A plugin can declare its OWN confirm through `register_dynamic` — the spec rides on the meta,
    /// so it is reachable exactly like a built-in's, no parallel registration.
    #[test]
    fn a_dynamic_action_can_declare_its_own_confirm() {
        use crate::app::interaction::ActionPolicy;
        let mut registry = ActionRegistry::new();
        let mut catalog = ActionCatalog::with_builtins();
        let mut meta = dyn_meta("plugin.docker.remove", ActionPolicy::Global);
        meta.confirm = Some(ConfirmSpec {
            message: "Remove the container?".to_string(),
            buttons: vec![
                ResponseButton::cancel("cancel", "Cancel"),
                ResponseButton::proceed("confirm", "Remove", true),
            ],
            dismissible: false,
            config_name: "plugin.docker.remove".to_string(),
            default_enabled: true,
        });
        let _ = register_dynamic(&mut registry, &mut catalog, meta, None);

        let spec = catalog.confirm_spec("plugin.docker.remove").unwrap();
        assert_eq!(spec.config_name, "plugin.docker.remove");
        assert!(!spec.dismissible);
    }

    #[test]
    fn outcome_variants_compose() {
        // All four outcomes construct (the declarative three + the native Callback). This also
        // exercises the ResponseButton constructors + `new`.
        let fired = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let f = fired.clone();
        let buttons = [
            ResponseButton::cancel("cancel", "Cancel"),
            ResponseButton::proceed("ok", "OK", false),
            ResponseButton::new(
                "other",
                "Discard",
                ButtonRole::Default,
                Outcome::Dispatch(crate::input::WmAction::ReloadConfig),
            ),
            ResponseButton::new(
                "cb",
                "Run",
                ButtonRole::Default,
                Outcome::Callback(std::rc::Rc::new(move |_state, _reg| f.set(f.get() + 1))),
            ),
        ];
        assert_eq!(buttons.len(), 4);
        // The callback is a stored `Rc<dyn Fn>` — invoking it (as `run_outcome` would) runs once.
        if let Outcome::Callback(cb) = &buttons[3].outcome {
            // Can't build a full AppState/registry here, so just confirm the closure is wired;
            // end-to-end firing is covered by in-app verification + the gate's existing tests.
            let _ = cb; // callback is present and typed correctly
        } else {
            panic!("expected a Callback outcome");
        }
        assert_eq!(fired.get(), 0, "constructing does not fire the callback");
    }

    #[test]
    fn test_session_category_exists() {
        let count = ActionCatalog::with_builtins()
            .by_category(ActionCategory::Session)
            .count();
        assert_eq!(
            count, 0,
            "no actions in Session category yet, but variant is reserved"
        );
    }
}
