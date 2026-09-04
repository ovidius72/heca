//! Mount the notification toast stack — F009/T203.
//!
//! **This is host plumbing, not the capability a plugin meets.** A plugin (or RPC, or a
//! keybinding) raises a notification through the `notify` action (`crate::notification`,
//! F009/T491/P055) — it never touches this file. What lives here is purely "how does a raised
//! notification end up drawn on screen", which is the host's job alone.
//!
//! # It is placed, not registered (F003/P097/T494)
//!
//! The stack is **a child of the window root**, like any widget. That is the whole of how it gets
//! on screen, and it is why its × and its action buttons work at all: the one capture → target →
//! bubble walk delivers pointer events to every node in the tree, so a surface that is *in* the
//! tree needs no dispatch function, no registration and no id (`docs/surface-compositor.md` § 0.3).
//!
//! It used to be a `LayerKind::Persistent` entry in the layer registry, and the registry has no
//! input pass — so the stack was laid out, painted, and **received nothing**: clicking the × did
//! nothing, and hovering a toast highlighted the pane behind it. That defect is what
//! `docs/surface-compositor.md` § 0.2 is written about.
//!
//! Two declarations that were load-bearing as registry flags are simply gone, because in a tree
//! nobody asks them: it never captured the keyboard (`modal: false`) and it never obscured the
//! panes (`covers_content: false`, without which a visible toast put the app in `Domain::Overlay`
//! and silently stopped `prefix+Enter` from splitting a pane). A plain child occludes nothing and
//! takes no focus unless it asks to.
//!
//! Neither callback touches `AppState` — each builds an [`Intent`] and fires it through the
//! surface's own emitter.

use heca_grid_ui::widgets::{KeyHintGroup, ToastPosition, ToastStack};
use heca_view::{Intent, PropValue};

use super::layers::{layer_name, HOST_OWNER};
use super::{layer_emitter, place_surface};
use crate::app::interaction::SurfaceKey;
use crate::app_state::AppState;

/// **The stack's identity** — its key in the window root, and what its intents are stamped with.
///
/// One constant, because the surface and whatever needs to speak *as* it (the action relay in
/// `handlers.rs`) must derive the same [`SurfaceKey`]; two spellings would be two surfaces as far
/// as the interaction policy is concerned.
pub(crate) const SURFACE: &str = "notifications";

/// The stack's key, owner-stamped — `heca.notifications`. The owner half is never typed by an
/// author; [`layer_name`] stamps it, which is what stops anyone claiming another's namespace.
pub(crate) fn surface_name() -> String {
    layer_name(HOST_OWNER, SURFACE).expect("a constant short name, non-empty and free of dots")
}

/// The identity the interaction policy knows this surface by.
pub(crate) fn surface_key() -> SurfaceKey {
    SurfaceKey::of(&surface_name())
}

/// Mount the toast stack once, at startup. Call after `AppState` exists (needs
/// `state.notifications` / `state.notification_pick_open`).
///
/// Wrapped in a [`KeyHintGroup`] bound to `state.notification_pick_open` (F009/T492's
/// `notification.pick` action flips that signal) — an **additional**, precise picker over the
/// visible toast actions/×, on top of (never instead of) their global `prefix+/` letters, which
/// they already carry for free since T486 put the cards in the tree.
pub(crate) fn mount_notification_stack(state: &mut AppState) {
    let emit = layer_emitter(&state.event_proxy, surface_key());

    let stack = ToastStack::new(state.notifications.visible_toasts)
        .position(ToastPosition::TopRight)
        // **A card must not retire under the pointer reaching for its button.** The stack reports
        // the hover and nothing more; the runtime owns what it costs and hands the time back.
        .hovered_signal(state.notification_hovered)
        .on_dismiss({
            let emit = emit.clone();
            move |toast_id| {
                let intent = Intent::new("notification_dismiss_one")
                    .arg("id", PropValue::Int(toast_id as i64));
                emit.fire(crate::app::interaction::InteractionIntent::View(intent));
            }
        })
        .on_action({
            let emit = emit.clone();
            move |toast_id, key| {
                // The store owns the mapping from `key` back to a real `Intent`
                // (`NotificationRuntime::action_intent_for_visible`) — it cannot be resolved here,
                // at mount time, because the notification that will eventually carry it does not
                // exist yet. So this names the relay action, which does the lookup at fire time
                // and re-dispatches the real Intent — see handle_notification_action_relay.
                let intent = Intent::new("notification_action_relay")
                    .arg("id", PropValue::Int(toast_id as i64))
                    .arg("key", PropValue::Text(key.to_string()));
                emit.fire(crate::app::interaction::InteractionIntent::View(intent));
            }
        });

    let root: Box<dyn heca_grid_ui::Component> = Box::new(
        KeyHintGroup::new_boxed(Box::new(stack)).open_when(state.notification_pick_open),
    );

    place_surface(&mut state.window_root, &surface_name(), root);
}
