//! The host side of a component's declared actions (F003/P085/T353).
//!
//! A component says *what* it can do ([`Provider::actions`]) and *how* to do it
//! ([`Provider::perform`]). Everything between those two — putting the declaration into the one
//! action catalog, deciding which placement a call is aimed at, keeping `&mut AppState` out of the
//! component's hands, and applying what it asks for afterwards — is written **once**, here, for
//! every component that will ever exist.
//!
//! That is the whole point of the shape: a component author writes a `kind()`, some `ActionMeta`s
//! and a short `perform`, and gets the palette entry, the icon and label, `describe-action`, RPC,
//! the `prefix+/` hint and the destructive-confirm gate without writing a line of host code.

use std::collections::HashMap;
use std::rc::Rc;

use crate::actions::{ActionRegistry, DuplicateAction, register_dynamic, unregister_dynamic};
use crate::app::conflicts::{ActionConflict, Conflicts};
use crate::keymap::KeymapRegistry;
use crate::app_state::AppState;
use crate::chrome::Intent;
use crate::providers::ProviderCx;
use heca_grid_ui::reactive::SignalGet;

/// Put every mounted component's declared actions into the registry and the catalog.
///
/// The **host** registers them, never the provider: a provider does not mutate app state (§2.3), and
/// the registry/catalog are app state. Each action is registered with one shared handler
/// ([`route_to_owner`]) rather than a closure per action — see [`Provider::perform`] for why.
///
/// Handles are kept on the mount, so its actions die with it ([`retire_provider_actions`]).
/// Re-registering the same id replaces the previous entry, which is what a remount should do.
///
/// [`Provider::perform`]: crate::providers::Provider::perform
pub(crate) fn register_provider_actions(
    state: &mut AppState,
    registry: &mut ActionRegistry,
    conflicts: &mut Conflicts,
) {
    // Read the declarations first, then mutate: the providers live inside `state.chrome_host`, so
    // collecting up front is what lets the catalog and the host both be borrowed below.
    let declared = declarations(state);

    for (mount, kind, metas) in declared {
        for mut meta in metas {
            let id = meta.name.clone();
            // Who declared it is the **host's** answer, not the component's: stamped here from the
            // provider actually being registered, so an author can neither claim another
            // component's name nor forget to say their own (F003/P085/T358).
            meta.owner = Some(kind.clone());
            match register_dynamic(
                registry,
                &mut state.action_catalog,
                meta,
                Some(Rc::new(route_to_owner)),
            ) {
                Ok(handle) => {
                    if let Some(handles) = state.chrome_host.handles_mut(&mount) {
                        handles.keep_action(handle);
                    }
                }
                Err(duplicate) => conflicts.action(ActionConflict {
                    id: id.clone(),
                    kept: match duplicate {
                        DuplicateAction::ShadowsBuiltin => "the built-in".to_string(),
                        DuplicateAction::ReplacedDynamic => format!("component '{kind}'"),
                    },
                    rejected: format!("component '{kind}' (mount '{mount}')"),
                    shadows_builtin: matches!(duplicate, DuplicateAction::ShadowsBuiltin),
                }),
            }
        }
    }
}

/// Bind every mounted component's **runtime-registered** keys into its layer (F003/P086/T366).
///
/// This is the **plugin** path, and only the plugin path. A component shipped with heca writes its
/// keys in `keybindings.default.toml` under `[[keys.component]]`, where the user can see them next
/// to everything else and change them; a plugin has no file in that merge, so runtime registration
/// is its only way in. Anything in the config file wins over what is registered here.
///
/// Called at mount **and after a reload**: a reload throws the layers away and rebuilds them from a
/// file that knows nothing about a plugin that mounted afterwards, so without this second call
/// reloading would silently unbind every plugin key. Actions themselves are untouched — they live
/// in the catalog, which a reload does not rebuild.
pub(crate) fn bind_provider_keybindings(
    state: &AppState,
    component_keymaps: &mut HashMap<String, KeymapRegistry>,
    index: &mut crate::keymap::BindingIndex,
) {
    for provider in state.chrome_host.mounted_providers() {
        let kind = provider.kind().to_string();
        for (action, keys) in provider.keybindings() {
            crate::app::registry::register_component_keybinding(
                component_keymaps,
                index,
                &kind,
                &keys,
                &action,
            );
        }
    }
}

/// Copy `mount`'s cursor onto every **other seating of the same component** (F003/P086/T365).
///
/// The host gives each placement its own `container_cursor`, which is right for a component that
/// keeps one cursor per placement. The workspaces component does not: it has **one** tree with one
/// cursor, so two views of it must light the same row — a click or a `j` in the left dock leaving
/// the right one pointing somewhere else is the model disagreeing with itself.
///
/// Done here, after the component has written its own, rather than by teaching the render to read a
/// shared cursor: the projection is deliberately keyed by `(mount, nav_key)` so that a component
/// which *does* keep per-placement cursors still gets them, and that must not be special-cased for
/// one component.
///
/// A component keeping genuinely independent cursors per placement is a later change to its own
/// model; this mirrors what is true today.
fn mirror_cursor_to_siblings(state: &mut AppState, mount: &str) {
    let Some(kind) = state.chrome_host.provider(mount).map(|p| p.kind().to_string()) else {
        return;
    };
    let cursor = state.chrome_state.container_cursor(mount).get_untracked();
    let siblings: Vec<String> = state
        .chrome_host
        .mounted_providers()
        .filter(|p| p.kind() == kind && p.id() != mount)
        .map(|p| p.id().to_string())
        .collect();
    for sibling in siblings {
        state
            .chrome_state
            .set_container_cursor(&sibling, cursor.clone());
    }
}

/// Move a placement's cursor to `key` — the host half of a click landing on a row
/// (F003/P086/T365).
///
/// Two writes, because there are two readers: the **generic** per-mount cursor is what the row
/// outlines draw from and what any host or plugin can read, and the component's own model is what
/// `j`/`k` step through. Setting only the first would move the highlight and leave the next keypress
/// continuing from the row the user had *before* they clicked.
///
/// Shaped like [`route_to_owner`] for the same reason: the provider lives inside
/// `state.chrome_host`, so no `&mut AppState` may be alive while it runs.
pub(crate) fn move_provider_cursor(state: &mut AppState, mount: &str, key: &str) {
    state
        .chrome_state
        .set_container_cursor(mount, Some(key.to_string()));
    let mut cx = ProviderCx::new(mount, state.chrome_state.clone());
    let queued = match state.chrome_host.provider(mount) {
        Some(provider) => {
            provider.cursor_moved(key, &mut cx);
            cx.drain()
        }
        None => return,
    };
    mirror_cursor_to_siblings(state, mount);
    // Same door as `route_to_owner`'s: whatever the component asks for on the way is policy-routed
    // and confirm-gated identically.
    for queued_intent in queued {
        let _ = state
            .event_proxy
            .send_event(crate::app::events::AppEvent::ChromeIntent {
                source: crate::app::interaction::InteractionSource::Provider,
                intent: crate::app::interaction::InteractionIntent::View(queued_intent),
            });
    }
}

/// `(mount, kind, declared actions)` for every mounted component that declares any.
fn declarations(state: &AppState) -> Vec<(String, String, Vec<crate::actions::ActionMeta>)> {
    state
        .chrome_host
        .mounted_providers()
        .map(|p| (p.id().to_string(), p.kind().to_string(), p.actions()))
        .filter(|(_, _, metas)| !metas.is_empty())
        .collect()
}

/// Retire everything a mount declared — its actions leave the catalog and the registry with it.
///
/// The mirror of [`register_provider_actions`] for a single mount. It is explicit rather than a
/// `Drop` impl because retiring needs the registry *and* the catalog, and neither is reachable from
/// a destructor without wrapping the registry in a `RefCell`.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "unmount seam: nothing unmounts a container at runtime yet (moves keep the mount alive); exercised by tests"
    )
)]
pub(crate) fn retire_provider_actions(
    state: &mut AppState,
    registry: &mut ActionRegistry,
    mount: &str,
) {
    let handles: Vec<crate::actions::ActionHandle> = state
        .chrome_host
        .handles_mut(mount)
        .map(|h| std::mem::take(&mut h.actions))
        .unwrap_or_default();
    for handle in handles {
        unregister_dynamic(registry, &mut state.action_catalog, &handle.0);
    }
}

/// The bridge every declared action is dispatched through.
///
/// It answers the one question a component cannot answer for itself — *which placement did the user
/// mean?* — then calls into that one, hands it a read-and-emit context, and applies what it asked
/// for afterwards.
///
/// **The borrow is why this is shaped like this.** The provider lives *inside*
/// `state.chrome_host`, so no `&mut AppState` may be alive while `perform` runs. [`ProviderCx`]
/// therefore holds an `Rc` alias of the store and a queue — never a borrow of the state — and the
/// queue is applied after the call, when the state is free again.
fn route_to_owner(state: &mut AppState, intent: &Intent) {
    // **A call that names its seating is not resolved at all.** A row declares its gesture inside
    // one mounted container and says which, so seating the same container twice cannot send the
    // right dock's row to the left dock's copy. `owning_mount` answers only for a call with no
    // element behind it — a keybinding, a palette entry, a script without `--dock`.
    let named = match intent.args.get(SEAT_ARG) {
        Some(crate::chrome::PropValue::Text(id)) => Some(id.clone()),
        _ => None,
    };
    let addressed = named.is_some();
    let Some(mount) = named.or_else(|| owning_mount(state, &intent.action)) else {
        #[cfg(debug_assertions)]
        eprintln!(
            "[heca] provider action '{}' has no mounted owner",
            intent.action
        );
        return;
    };
    // The seat is the host's address for the call, not something the component asked for: it is
    // taken off here so `perform` receives the action's own arguments and nothing else. Same rule
    // the RPC verb states about `--dock`; a component reads its seating from `ProviderCx::mount`,
    // which is the one place it is true.
    let mut aimed = intent.clone();
    aimed.args.remove(SEAT_ARG);
    let mut cx = ProviderCx::new(&mount, state.chrome_state.clone());
    let queued = match state.chrome_host.provider(&mount) {
        // `&self` — the component reads its own domain state, which is the point of `perform`
        // taking an id rather than the host holding a closure over it.
        Some(provider) => {
            provider.perform(&aimed.action, &aimed, &mut cx);
            cx.drain()
        }
        None => {
            // An addressed call whose seating is gone declines instead of falling back to a guess:
            // the row that named it is gone too, so there is nothing the guess could be right about.
            #[cfg(debug_assertions)]
            if addressed {
                eprintln!(
                    "[heca] provider action '{}' named seating '{mount}', which is not mounted",
                    intent.action
                );
            }
            return;
        }
    };
    // `perform` commonly moves the component's own cursor (`publish_cursor`), so the seatings are
    // brought back into step here too — one rule, both writers.
    mirror_cursor_to_siblings(state, &mount);
    // What the component asked for goes out the same door its widgets' clicks do, so it is
    // policy-routed and confirm-gated identically — a component cannot reach past the gate by
    // asking for something from inside `perform`.
    for queued_intent in queued {
        let _ = state
            .event_proxy
            .send_event(crate::app::events::AppEvent::ChromeIntent {
                source: crate::app::interaction::InteractionSource::Provider,
                intent: crate::app::interaction::InteractionIntent::View(queued_intent),
            });
    }
}

/// **The argument that addresses a call at one seating.**
///
/// A widget built inside a mounted container knows which seating it is in, so the gesture it
/// declares says so and nothing has to be resolved back to an instance — the way a DOM handler acts
/// on the element it is bound to rather than on "whichever one of these has focus". A call that
/// genuinely names no element (a keybinding, a palette entry) omits it, and only then does
/// [`owning_mount`] answer.
///
/// **It is the host's address, never one of the action's arguments.** [`route_to_owner`] takes it
/// off before `perform`, and the dispatch path takes it off before judging the args against the
/// action's declaration — so a component never sees it and cannot be written to depend on it. It
/// answers the same question the RPC verb's `--dock <id>` does (`heca/src/rpc.rs`), which likewise
/// never reaches the action.
///
/// ⚠️ **The `@` is what keeps it out of the way, and it is not decoration.** This was spelled
/// `"dock"` for a few hours and collided with the real `dock` argument of the built-in `focus_dock`
/// action: the dispatch path stripped it as an address, `focus_dock` was left with no dock named,
/// and a hint that meant "focus THIS seat" opened the **dock picker** instead — a second set of
/// letters over every dock, and picking one toggled the focus away again. An action argument is a
/// plain identifier, so a leading `@` cannot be one (F004/P084/T409 follow-up).
pub(crate) const SEAT_ARG: &str = "@seat";

/// Which **placement** owns `action` when the call did not say — written down once, rather than
/// decided per call site.
///
/// In order:
/// 1. the mount that currently holds chrome focus, if it declares the action — the user is looking
///    at it, so that is what they meant;
/// 2. the mount that held focus most recently, if it declares the action — this is what makes a
///    palette entry or an RPC call land somewhere sensible when nothing is focused;
/// 3. the first mount that declares it, in the host's order — there is exactly one candidate in the
///    ordinary single-placement case, so this is the common answer, not a fallback.
///
/// `None` when nothing mounted declares it: the component that owned the action is gone, and the
/// call declines rather than guessing.
///
/// `pub(crate)` because it is the rule for **which placement a call means**, wherever the call comes
/// from — the `perform` bridge below, a palette entry, an RPC line without `--dock` (F003/P085/T358).
/// Written once so those three cannot answer it differently.
pub(crate) fn owning_mount(state: &AppState, action: &str) -> Option<String> {
    let declares = |mount: &str| {
        state
            .chrome_host
            .provider(mount)
            .is_some_and(|p| p.actions().iter().any(|m| m.name == action))
    };
    if let Some(focused) = state.chrome_state.focused_container()
        && declares(&focused)
    {
        return Some(focused);
    }
    if let Some(last) = state.chrome_state.last_focused_container()
        && declares(&last)
    {
        return Some(last);
    }
    state
        .chrome_host
        .mounted_providers()
        .find(|p| p.actions().iter().any(|m| m.name == action))
        .map(|p| p.id().to_string())
}
