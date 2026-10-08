//! **What a window asks the server to do** — plain data, no closures, no window.
//!
//! A [`ServerAction`] is a command that changes state every window shares. It is the same
//! vocabulary the rest of heca names actions with (`notify`, `notification_dismiss_one`, …): the
//! handler that a key, a click, an RPC call or a plugin reaches builds one and hands it to
//! [`ServerState::execute`](super::ServerState::execute); nothing else reaches the state it changes.
//! Data, so the same command can cross a socket when a window and the server stop sharing a
//! process (F012/P099).

use std::time::Duration;

use crate::notification::{NotificationDraft, NotificationId};

/// How notifications behave, as `[settings.notification_system]` says — handed over whole on start
/// and again on every config reload, so a setting cannot be applied in one place and forgotten in
/// the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NotificationSettings {
    /// How long a notification that auto-dismisses stays up.
    pub(crate) auto_dismiss: Duration,
    /// Where one is delivered: the app, the system (not built yet), or nowhere.
    pub(crate) mode: heca_config::settings::NotificationSystem,
    /// How many cards are up at once; the rest queue.
    pub(crate) max_visible: usize,
}

/// One command to the server.
#[derive(Debug)]
pub(crate) enum ServerAction {
    /// Raise a notification. Dropped when the mode says nowhere.
    Raise(NotificationDraft),
    /// Dismiss one notification by id.
    DismissOne(NotificationId),
    /// Dismiss the first eligible visible notification, in stable order.
    DismissLast,
    /// Dismiss every visible notification.
    DismissAll,
    /// Whether the pointer rests on a notification card. A held stack keeps its deadlines still, and
    /// the time that cost is handed back when the pointer leaves.
    Hover(bool),
    /// New notification settings (startup, reload).
    Configure(NotificationSettings),
}
