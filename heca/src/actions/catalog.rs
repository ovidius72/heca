//! The catalog — what actions exist and what each says about itself: its label, icon, category,
//! arguments, policy and confirm. Every surface that renders an action reads it from here.

use super::builtins::builtins_by_category;
use super::confirm::builtin_confirm_specs;
use super::{ConfirmSpec, DuplicateAction, builtins};
use crate::args::{ArgSpec, sample_args};
use heca_grid_ui::Glyph;
use std::collections::HashMap;

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

/// **Which side of the server/client split an action runs on** (F012).
///
/// An action that changes what every window shares — the panes, columns and workspaces, their
/// names, the terminals behind them — is `Server`: it runs where that state is, whichever window
/// asked. An action that only changes what *this window* shows or is doing — a menu, a mode, a
/// dock, how far a view is scrolled — is `Client`. Declared on the action, with no default for a
/// built-in: the sort is a decision about each one, and an omitted answer would put it on the
/// side nobody chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    /// Runs on the server, for every window.
    Server,
    /// Runs in the window that asked.
    Client,
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
    /// close (action-task-E). [`check_args`](crate::args::check_args) compares a call against this list at both doors.
    pub args: Vec<ArgSpec>,
    /// Declarative confirmation requirement (action-interaction plan §4). `Some` means the central
    /// gate raises a confirm/response prompt before the action runs; `None` runs it straight.
    ///
    /// The guard lives **on the action**, so every surface that dispatches it — keyboard, a header
    /// button, a context-menu entry, RPC — confirms identically. A plugin declares its own the same
    /// way (through [`register_dynamic`](crate::actions::register_dynamic)).
    ///
    /// The confirm's toggle key is [`ConfirmSpec::config_name`], a separate field so a spec **may**
    /// be toggled under a name of its own — one spec can govern several `WmAction` variants
    /// (`ClosePane` + `ClosePaneById` confirm identically, §5.1). Every built-in nonetheless uses
    /// its own action name, so a user toggles `[confirm] <action> = false` and nothing else.
    pub(crate) confirm: Option<ConfirmSpec>,
}

impl ActionMeta {
    /// An action called `name` — the id a key binding, a menu, RPC and `Intent.action` all use.
    ///
    /// Everything else has a default, so an author says only what is true of theirs: a label, an
    /// icon, that it takes an argument, that it cannot be undone. The defaults are the cautious
    /// ones — see [`policy`](Self::policy).
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: name.clone(),
            name,
            description: String::new(),
            category: ActionCategory::System,
            owner: None,
            icon: None,
            // Refused while a floating pane is active. An author who forgot to think about it
            // gets an action blocked in one place they can see, not one allowed everywhere they
            // did not.
            policy: crate::app::interaction::ActionPolicy::AlwaysAllowed,
            args: Vec::new(),
            confirm: None,
        }
    }

    /// The short name shown in menus and the palette. Defaults to the id.
    ///
    /// English text until the app-wide text system (T519) lands; then a plugin's strings come from
    /// the language files under its own namespace, and this is what shows when one is missing.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// One sentence for `describe-action` and the palette. An English fallback, like the label.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// The icon every surface shows for it — menu, header button, palette. Without one it wears
    /// [`GENERIC_ACTION_ICON`].
    pub fn icon(mut self, icon: Glyph) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Where the palette files it. Defaults to [`ActionCategory::System`].
    pub fn category(mut self, category: ActionCategory) -> Self {
        self.category = category;
        self
    }

    /// **When it may run** — see [`ActionPolicy`](crate::app::interaction::ActionPolicy). Defaults
    /// to `AlwaysAllowed`, which is refused while a floating pane is active; say `Global` for an
    /// app-level action with no effect on the layout that must stay reachable there.
    pub fn policy(mut self, policy: crate::app::interaction::ActionPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// One argument it takes. An action that declares none takes none, and a call that passes one
    /// is told so.
    pub fn arg(mut self, arg: ArgSpec) -> Self {
        self.args.push(arg);
        self
    }

    /// It cannot be undone: every surface asks first, and its buttons read in the danger hue. Say
    /// it only when it is true. The button that goes ahead is worded with the action's label.
    pub fn destructive(mut self) -> Self {
        // The verb is filled in when the action is registered, from the label as it is *then* —
        // so `.destructive().label("Delete")` and `.label("Delete").destructive()` agree.
        self.confirm = Some(ConfirmSpec::destructive(self.name.clone(), ""));
        self
    }
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
/// [`builtins`] descriptors at startup and extended at runtime through
/// [`register_dynamic`](crate::actions::register_dynamic). Every UI surface resolves action metadata through here — icons, labels,
/// the pick prompts, the command palette, RPC introspection.
///
/// It lives on `AppState` (rather than inside `ActionRegistry`) for a borrow reason: an action
/// handler is `fn(&mut AppState, &WmAction)` and receives no registry, so metadata must be
/// reachable from the state. The registry holds the *handlers*; this holds the *metadata*. One of
/// each — never two of either.
pub struct ActionCatalog {
    by_name: HashMap<String, ActionMeta>,
    /// Names in stable order (built-ins in [`builtins`] order, then registration order).
    order: Vec<String>,
    /// The built-in names, so a dynamic action can never shadow or retire one.
    builtins: std::collections::HashSet<&'static str>,
}

impl ActionCatalog {
    /// Build the catalog seeded from the built-in [`builtins`] descriptors.
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
            builtins: builtins().map(|d| d.name).collect(),
        };
        for (category, d) in builtins_by_category() {
            let args: Vec<ArgSpec> = d.args.iter().map(ArgSpec::from_descriptor).collect();
            catalog.insert(ActionMeta {
                name: d.name.to_string(),
                label: d.label.to_string(),
                description: d.description.to_string(),
                category,
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
                debug_assert!(
                    false,
                    "confirm spec owner {owner_action:?} has no built-in action"
                );
            }
        }
        catalog
    }

    /// Add or replace an action's metadata. Re-registering an id replaces it (a provider
    /// remounting) while keeping its position in the stable order.
    fn insert(&mut self, meta: ActionMeta) {
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
    pub(crate) fn confirm_spec(&self, action_name: &str) -> Option<&ConfirmSpec> {
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
        self.find(name)
            .map(|m| m.icon.unwrap_or(GENERIC_ACTION_ICON))
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
    /// Where a **built-in** runs: `server` changes what every window shares, `client` only this
    /// window. `None` for a name-keyed action (a dock's or a plugin's): it has no side of its own —
    /// it dispatches built-ins, and each one goes where it belongs.
    pub side: Option<Side>,
    pub label: String,
    pub description: String,
    pub category: String,
    pub policy: crate::app::interaction::ActionPolicy,
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
        Self {
            name: m.name.clone(),
            side: builtin_side(&m.name),
            label: m.label.clone(),
            description: m.description.clone(),
            category: m.category.label().to_string(),
            policy: m.policy,
            owner: m.owner.clone(),
            args: m.args.clone(),
            // A forced prompt has no toggle key to report: nothing can turn it off.
            confirm: m
                .confirm
                .as_ref()
                .filter(|c| !c.forced)
                .map(|c| c.config_name.clone()),
        }
    }
}

/// The side a **built-in** runs on, from its descriptor — the one place the side is declared.
/// `None` for anything that is not a built-in.
pub fn builtin_side(name: &str) -> Option<Side> {
    builtins().find(|d| d.name == name).map(|d| d.side)
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
    let action = crate::input::resolve_action(name, &sample_args(args)).unwrap_or_else(|| {
        unreachable!(
            "built-in action {name:?} builds from neither its name nor its declared \
                 arguments: add it to `action_from_name`, or correct its `args` declaration \
                 so `build_action` accepts it"
        )
    });
    crate::app::interaction::action_policy(&action)
}

/// What a **built-in** action declares it takes, by name — read straight from
/// [`builtins`] rather than from the [`ActionCatalog`].
///
/// The catalog is the metadata home, but config is read before any `AppState` exists
/// (`HecaApp::new` builds the keymap; `startup` builds the catalog), and a keybinding can only
/// name a built-in at load time anyway — a provider's action does not exist yet. So the config
/// door reads the static seed directly instead of a second catalog being built to serve it.
///
/// `None` for a name that is not a built-in, which is not an error: it may be an action a provider
/// or plugin registers later.
pub fn builtin_args(name: &str) -> Option<Vec<ArgSpec>> {
    builtins()
        .find(|d| d.name == name)
        .map(|d| d.args.iter().map(ArgSpec::from_descriptor).collect())
}

impl Default for ActionCatalog {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests;
