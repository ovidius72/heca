//! **Dispatching a described tree's intents** — a name-keyed intent is judged by the same rules as a built-in action.

use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::Intent as ViewIntent;
use crate::input::WmAction;
use super::types::{InteractionIntent, InteractionSource, RouteDecision};
use super::domain::domain_for;
use super::route::{policy_allows, route_interaction};
use super::dispatch::dispatch_action;


/// Resolve and dispatch a declarative [`ViewNode`](crate::chrome::ViewNode) intent.
///
/// A `view::Intent` carries an action *name* (the same identifier config keys and RPC use)
/// plus optional args.
///
/// **This is the one front door** (`pluggable-chrome-plugin-plan.md` §2.7.2: "No parallel dispatch
/// path"). A name resolves down one of two back ends, and both are policy-routed:
///
/// 1. **Built-in** — [`resolve_action`](crate::input::resolve_action) maps it to a [`WmAction`],
///    which goes through [`dispatch_action`] exactly like a keypress. Policy comes from
///    [`action_policy`]'s exhaustive match.
/// 2. **Name-keyed** (plugin-04) — an action registered at runtime by a provider/plugin, which has
///    no `WmAction` variant because the enum is closed. Policy comes from its **declared**
///    `DynActionMeta.policy`, and both paths converge on the same [`policy_allows`], so a plugin
///    action is judged by identical rules.
///
/// An unknown name is not a crash: a binding or a menu item may legitimately name an action whose
/// provider is not mounted.
///
/// **Args are carried on both paths.** A name-keyed handler reads them off the `Intent` itself. A
/// built-in **parameterized** variant is constructed from them via
/// [`resolve_action`](crate::input::resolve_action) — so `{"action":"resize","args":{…}}` produces the
/// very same `WmAction` a config binding would. Unit built-ins ignore args, as they always did.
pub(crate) fn dispatch_view_intent(
    state: &mut AppState,
    registry: &ActionRegistry,
    source: InteractionSource,
    intent: &ViewIntent,
) -> IntentOutcome {
    // 0. Judge the args against what the action DECLARES it takes, and say what is wrong. Without
    //    this the two failures below are indistinguishable and both silent: a misspelled required
    //    argument makes `build_action` return `None` (so the intent looks like an unknown action),
    //    and a misspelled optional one is simply dropped, leaving the action to run with a default
    //    nobody asked for.
    let mut args = intent_args_as_strings(intent);
    // **The seating is an address, not an argument.** A row declares its gesture inside one mounted
    // container and names it (`SEAT_ARG`) so nothing has to resolve the call back to an instance;
    // that is the host's business, so it is taken off before the args are judged against what the
    // action declares — otherwise every addressed call would report an argument the action does not
    // take. `route_to_owner` takes it off again before `perform`, and `build_action` below never
    // sees it either.
    args.remove(crate::providers::SEAT_ARG);
    report_arg_problems(&state.action_catalog, &intent.action, &args);

    // 1. Built-in. Parameterized variants are built from the intent's args (`build_action`, the same
    //    constructor a config binding uses); unit variants come straight from the name.
    if let Some(action) = builtin_of(&intent.action, &args) {
        dispatch_action(state, registry, source, &action);
        return IntentOutcome::Ran;
    }

    // A built-in the caller could not build is an **arity** failure, and saying so is the whole
    // point: without this it fell through to the name-keyed branch below, where the catalog does
    // hold its metadata, no dynamic handler exists, and the answer came back `NotRunnable` —
    // "component not mounted?" about one of the app's own compiled-in actions. `heca action
    // add_pane_to_column` said that; the palette listing it said nothing at all (F003/P085/T358).
    if state.action_catalog.is_builtin(&intent.action) {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: '{}' is a built-in whose required arguments were not supplied",
            intent.action
        );
        return IntentOutcome::MissingArgs;
    }

    // 2. **A widget on screen declares it** (F003/P082/T427). Between the built-ins and the
    //    provider catalog, because a surface's own verb is the more specific thing the name means
    //    while that surface is up — the same nearest-declaration rule keys and menus follow.
    //
    //    This is the seam a **layer** has and used not to: a dock declares its actions through
    //    `Provider::actions`, while an overlay could only bind verbs the app had already compiled
    //    in. It is why the exposé's picker had to borrow the built-in `hint_pick`, and why a plugin
    //    could contribute targets to heca's picker but never open one of its own.
    //
    //    Reachability *is* the gate here, and deliberately so: a widget-declared action is found
    //    only by walking the **visible** trees, so an unmounted surface's verb resolves to nothing
    //    exactly as an unmounted provider's does. It needs no policy of its own because it cannot
    //    be reached when its surface is not on screen.
    if crate::chrome::fire_widget_action(state, &intent.action) {
        return IntentOutcome::Ran;
    }

    // 3. Name-keyed (provider/plugin), routed by its DECLARED policy — the same `policy_allows` the
    //    built-in path reaches through `route_action`, so a plugin action is judged by identical
    //    rules.
    let Some(policy) = state.action_catalog.policy(&intent.action) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: view intent '{}' did not resolve to a known action",
            intent.action
        );
        return IntentOutcome::Unknown;
    };

    // The same gate the built-in path takes through `route_action` — a component's or plugin's
    // action is judged by identical rules, on the same domain (F003/P086/T371). The `top_modal`
    // check that used to sit here is gone: a modal covers the tiled area, which `Domain::Overlay`
    // already refuses.
    if !policy_allows(
        state.layout(),
        domain_for(state, source),
        source,
        policy,
        None,
    ) {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: blocked dynamic action '{}' from {source:?}",
            intent.action
        );
        return IntentOutcome::Blocked;
    }
    // A name-keyed action that declares a confirm asks first, here, on every surface that dispatches it.
    if crate::handlers::maybe_confirm_dynamic(state, intent) {
        return IntentOutcome::Ran;
    }
    if !registry.execute_dynamic(&intent.action, state, intent) {
        // Declared but host-unrunnable (registered with no handler): its owner lives across the plugin
        // boundary and forwarding lands with the WASM bridge (plugin-08). Never a crash.
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] interaction: action '{}' is declared but has no host handler",
            intent.action
        );
        return IntentOutcome::NotRunnable;
    }
    IntentOutcome::Ran
}

/// **The `WmAction` a view-intent name means**, built from its args exactly as a `config.toml`
/// binding is.
///
/// Extracted so that *running* an intent and *judging* one resolve it the same way. Two copies of
/// this line would be two answers to "what does this name mean", and the one nobody exercises is
/// the one that drifts.
pub(super) fn builtin_of(name: &str, args: &std::collections::HashMap<String, String>) -> Option<WmAction> {
    crate::input::resolve_action(name, args)
}

/// **Would this intent be allowed to run right now?** — asked *before* a picker spends a letter on
/// it (F003/P082/T432).
///
/// A picker that offers letters which do nothing is a broken picker. With a floating pane active,
/// `prefix+/` lettered every pane and every sidebar row naming one, and pressing a letter did
/// nothing at all, because `ActionPolicy` correctly refuses a `FocusPane` that does not target the
/// active float. This is that refusal, asked one moment earlier.
///
/// It is deliberately the **same three arms** [`dispatch_view_intent`] resolves, in the same order,
/// and it shares [`builtin_of`] with it so the two cannot disagree about what a name means:
///
/// 1. a **built-in** → judged by [`route_interaction`], the one gate every surface goes through;
/// 2. a **name-keyed** action → judged by its declared policy, the same [`policy_allows`] call the
///    dispatcher makes;
/// 3. anything else → **allowed**. A verb declared by a widget on screen has no policy of its own
///    (reachability is its gate — it cannot be found when its surface is not visible), and a name
///    nothing knows is not this function's to refuse. `true` here means *"nothing to ask"*, never
///    *"permitted"*: withholding letters from everything the policy cannot see would be a worse
///    picker than the one this fixes.
pub(crate) fn view_intent_allowed(
    state: &AppState,
    source: InteractionSource,
    intent: &ViewIntent,
) -> bool {
    let mut args = intent_args_as_strings(intent);
    args.remove(crate::providers::SEAT_ARG);
    if let Some(action) = builtin_of(&intent.action, &args) {
        return matches!(
            route_interaction(state, source, InteractionIntent::ActivateAction(action)),
            RouteDecision::Allow(_)
        );
    }
    match state.action_catalog.policy(&intent.action) {
        Some(policy) => policy_allows(
            state.layout(),
            domain_for(state, source),
            source,
            policy,
            None,
        ),
        None => true,
    }
}

/// What became of a dispatched intent — the answer a **scripted** caller needs (F003/P086/T372).
///
/// A click can afford to fail silently; a script cannot be told "ok" when nothing happened. The
/// three failures are genuinely different: a name nothing knows, a name the domain refuses right
/// now, and an action whose owner is not mounted (or lives across the plugin boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IntentOutcome {
    /// Dispatched — a built-in ran, or a name-keyed handler did.
    Ran,
    /// No such action, in the built-ins or the catalog.
    Unknown,
    /// Known, but its policy does not permit it in the current domain.
    Blocked,
    /// A **built-in whose required arguments were not supplied** — `close_pane_by_id` with no
    /// `pane_id`, `add_pane_to_column` with no column. Its own answer, because the caller's mistake
    /// is fixable and none of the other three say what is wrong: the name is real, the domain has
    /// no opinion, and the action is perfectly runnable with arguments (F003/P085/T358).
    MissingArgs,
    /// Declared, but nothing here can run it: its component is not mounted, or it is a plugin's to
    /// run across a boundary that does not exist yet.
    NotRunnable,
}

/// Report a dispatched intent's argument mistakes against the action's declared
/// [`args`](crate::actions::ActionMeta::args).
///
/// Built-in and name-keyed actions alike — they share one catalog, so they are judged by one rule,
/// the same way [`policy_allows`] judges them by one rule. An action the catalog does not know is
/// not this function's business: the caller already reports an unresolved name.
///
/// This reports and does not decide. A missing required argument stops the action anyway (nothing
/// can build it); an unknown or malformed one costs only itself and the rest of the call still
/// stands — the rule the declarative UI model already applies to a widget property.
pub(super) fn report_arg_problems(
    catalog: &crate::actions::ActionCatalog,
    name: &str,
    args: &std::collections::HashMap<String, String>,
) {
    let Some(meta) = catalog.find(name) else {
        return;
    };
    for problem in crate::args::check_args(&meta.args, args) {
        eprintln!("[heca] action '{name}': {problem}");
    }
}

/// Flatten an [`Intent`](crate::chrome::Intent)'s typed args into the `name -> string` map
/// [`resolve_action`](crate::input::resolve_action) parses, so a declarative intent and a `config.toml`
/// binding construct a parameterized built-in through **one** code path.
pub(super) fn intent_args_as_strings(intent: &ViewIntent) -> std::collections::HashMap<String, String> {
    use crate::chrome::PropValue;
    intent
        .args
        .iter()
        .map(|(k, v)| {
            let s = match v {
                PropValue::Bool(b) => b.to_string(),
                PropValue::Int(i) => i.to_string(),
                PropValue::Float(f) => f.to_string(),
                PropValue::Text(t) | PropValue::Color(t) | PropValue::Glyph(t) => t.clone(),
                other => format!("{other:?}"),
            };
            (k.clone(), s)
        })
        .collect()
}
