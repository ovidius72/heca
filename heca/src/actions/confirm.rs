//! The confirm model — an action that asks before it runs declares it here, as data: the body
//! message, the response buttons and what each does, whether the choice is forced, and the key
//! that turns the prompt off.

use super::ActionRegistry;

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
            role: if danger {
                ButtonRole::Danger
            } else {
                ButtonRole::Default
            },
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
pub(super) fn builtin_confirm_specs() -> Vec<(&'static str, ConfirmSpec)> {
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
    use crate::actions::testing::dyn_meta;
    use crate::actions::{ActionCatalog, register_dynamic};

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
}
