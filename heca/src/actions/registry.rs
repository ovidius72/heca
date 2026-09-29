//! The registry — what runs when an action fires. Built-ins are keyed by their `WmAction`
//! variant, name-keyed actions (providers, later plugins) by name; both run through `execute*`,
//! which follows every handler with what every action is followed by.

use super::{ActionCatalog, ActionMeta};
use std::collections::HashMap;

/// Handler signature for all window-manager actions.
///
/// The `WmAction` parameter carries the full variant (including any embedded
/// arguments), so the same handler can serve both unit and parameterized
/// variants that share a discriminant.
pub type ActionHandler = fn(&mut crate::app_state::AppState, &crate::input::WmAction);

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
/// - Handlers are keyed by [`WmActionKind`](crate::input::WmActionKind), so all parameterized
///   variants of the same action share one handler (the handler destructures
///   the action to extract arguments).
pub struct ActionRegistry {
    handlers: HashMap<crate::input::WmActionKind, ActionHandler>,
    /// Handlers for **name-keyed** actions registered at runtime by providers (and later WASM
    /// plugins) — the actions that have no [`WmAction`](crate::input::WmAction) variant because the
    /// enum is closed and a plugin cannot extend it. Their *metadata* lives in the one
    /// [`ActionCatalog`], next to the built-ins; only the handler lives here. One with no handler is declared but
    /// not runnable by the host — its owner is across the plugin boundary (plugin-08).
    dyn_handlers: HashMap<String, DynHandler>,
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
/// `handler: None` registers an action that is declared but that the host cannot run —
/// plugin-08 forwards it to its owner. Re-registering the same id replaces the previous entry (a
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

    /// Run a name-keyed action, passing the intent through so the handler reads its own args.
    /// `false` when no *handler* is registered under `id` — either the id is unknown, or the action
    /// is declared with no host handler. Neither is a crash: a binding or
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
        run_then_follow_up(state, |state| handler(state, intent));
        true
    }

    /// Register the handler for every action of `kind` — all of a parameterized variant's values
    /// share one handler, which reads its arguments off the action it is given.
    pub fn register(&mut self, kind: crate::input::WmActionKind, handler: ActionHandler) {
        self.handlers.insert(kind, handler);
    }

    /// Execute the handler for `action`, if one is registered, and then what every action is
    /// followed by ([`after_action`](crate::app::mutations::after_action)) — so no handler has to
    /// remember to ask for a redraw or to refresh what lists the session.
    ///
    /// In debug builds, panics if no handler is registered (this is a bug —
    /// every `WmAction` variant must have a handler in `build_registry()`).
    /// In release builds, silently does nothing.
    pub fn execute(&self, action: &crate::input::WmAction, state: &mut crate::app_state::AppState) {
        let disc = action.kind();
        if let Some(handler) = self.handlers.get(&disc) {
            run_then_follow_up(state, |state| handler(state, action));
        } else {
            #[cfg(debug_assertions)]
            panic!("no handler registered for action: {:?}", action);
        }
    }

    /// Whether a handler is registered for the given action.
    #[cfg(test)]
    pub fn has_handler(&self, action: &crate::input::WmAction) -> bool {
        let disc = action.kind();
        self.handlers.contains_key(&disc)
    }

    /// Whether the host can run the name-keyed action `id` — `false` for one that is only
    /// declared.
    #[cfg(test)]
    pub fn has_dynamic_handler(&self, id: &str) -> bool {
        self.dyn_handlers.contains_key(id)
    }
}

/// Run one handler, then what every action is followed by
/// ([`after_action`](crate::app::mutations::after_action)) — the one place both back ends go
/// through, so no handler has to ask for a redraw or refresh what lists the session.
fn run_then_follow_up(
    state: &mut crate::app_state::AppState,
    handler: impl FnOnce(&mut crate::app_state::AppState),
) {
    let before = state.session.shape();
    handler(state);
    crate::app::mutations::after_action(state, &before);
}

#[cfg(test)]
mod tests;
