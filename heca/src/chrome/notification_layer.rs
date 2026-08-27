//! Mount the notification toast stack — F009/T203.
//!
//! **This is host plumbing, not the capability a plugin meets.** A plugin (or RPC, or a
//! keybinding) raises a notification through the `notify` action (`crate::notification`,
//! F009/T491/P055) — it never touches a layer, a `LayerId`, or anything in this file. What lives
//! here is purely "how does a raised notification end up drawn on screen", which is the host's
//! job alone.
//!
//! The stack is the first [`LayerKind::Persistent`] **overlay** layer in the app (every other
//! registered layer today is `OnDemand` — a modal, a menu, the palette). `covers_content: false`
//! is not cosmetic: get it wrong and a visible toast puts the app in `Domain::Overlay`, which
//! permits only `Global` actions — `prefix+Enter` splitting a pane would silently stop working
//! for as long as any notification was on screen. `modal: false` is equally load-bearing: a
//! toast must never take the keyboard.
//!
//! Neither callback touches `AppState` — each builds an [`Intent`] and fires it through the
//! layer's own emitter, exactly as `chrome::fires` does for a native row (§3 below).

use heca_grid_ui::widgets::{KeyHintGroup, ToastPosition, ToastStack};
use heca_view::{Intent, PropValue};

use super::{layer_emitter, LayerKind};
use crate::app::interaction::InteractionIntent;
use crate::app_state::AppState;

/// Mount the toast stack once, at startup. Call after `AppState` exists (needs
/// `state.notifications` / `state.notification_pick_open`).
///
/// Wrapped in a [`KeyHintGroup`] bound to `state.notification_pick_open` (F009/T492's
/// `notification.pick` action flips that signal) — an **additional**, precise picker over the
/// visible toast actions/×, on top of (never instead of) their global `prefix+/` letters, which
/// they already carry for free since T486 put the cards in the tree.
pub(crate) fn mount_notification_stack(state: &mut AppState) {
    let id = state.layers.reserve_id();
    let emit = layer_emitter(&state.event_proxy, id);

    let stack = ToastStack::new(state.notifications.visible_toasts)
        .position(ToastPosition::TopRight)
        .on_dismiss({
            let emit = emit.clone();
            move |toast_id| {
                let intent = Intent::new("notification_dismiss_one")
                    .arg("id", PropValue::Int(toast_id as i64));
                emit.fire(InteractionIntent::View(intent));
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
                emit.fire(InteractionIntent::View(intent));
            }
        });

    let root: Box<dyn heca_grid_ui::Component> = Box::new(
        KeyHintGroup::new_boxed(Box::new(stack)).open_when(state.notification_pick_open),
    );

    state.layers.insert(
        id,
        None,          // child of the root, not of any opener — it's ambient, not an overlay reply
        LayerKind::Persistent,
        false,         // modal: a toast never captures the keyboard on its own
        false,         // covers_content: must NOT block Global actions behind it
        root,
    );
    state.notification_layer_id = Some(id);
}
