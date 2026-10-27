use super::{
    AppNotification, NotificationAction, NotificationId, NotificationLifetime, NotificationSeverity,
};
use heca_grid_ui::widgets::{ToastAction, ToastSpec};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// **The serializable projection of a visible notification** — what the server sends a window.
///
/// The server owns the queue, lifecycle, timers and dedup; a window draws from this. It carries
/// only presentation data: the id, mapped-later severity, title/body, dismissibility, and the
/// actions (each a name-keyed [`heca_view::Intent`] with a label and variant). It deliberately
/// does **not** carry the grid-ui [`ToastSpec`](heca_grid_ui::widgets::ToastSpec): a widget type
/// has no business in serialized state, so the seam is this plain value and the window turns it
/// into a `ToastSpec` for itself ([`toast_spec`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationView {
    /// Stable identity — what the toast stack reports back on dismiss/action.
    pub id: NotificationId,
    /// App-owned severity; the window maps it to the paint-time token.
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: Option<String>,
    /// Whether the × dismiss affordance is shown.
    pub dismissible: bool,
    /// **The lifetime this notification auto-dismisses over**, when it has a timer — the span the
    /// card's countdown bar depletes across. `None` for a sticky notification, which shows no bar.
    pub auto_dismiss: Option<Duration>,
    /// Every action the card offers, in order; each dispatched by its own `intent.action` name.
    pub actions: Vec<NotificationAction>,
}

impl NotificationView {
    /// The action whose `intent.action` name is `key` — the same name
    /// `ToastStack::on_action(id, key)` reports back.
    pub fn action(&self, key: &str) -> Option<&NotificationAction> {
        self.actions.iter().find(|action| action.intent.action == key)
    }
}

/// Project a visible notification to the serializable [`NotificationView`] the server sends.
///
/// `None` when the notification is not currently visible (`Queued` or `History`). This runs on the
/// server side of the seam — it reads the canonical [`AppNotification`] and never builds a widget.
pub fn notification_view(notification: &AppNotification) -> Option<NotificationView> {
    if !notification.is_visible() {
        return None;
    }
    Some(NotificationView {
        id: notification.id,
        severity: notification.severity,
        title: notification.title.clone(),
        body: notification.body.clone(),
        dismissible: notification.lifecycle.is_dismissible(),
        auto_dismiss: match notification.lifecycle.lifetime() {
            NotificationLifetime::Auto(duration) => Some(duration),
            NotificationLifetime::Sticky => None,
        },
        actions: notification.actions.clone(),
    })
}

/// Build the grid-ui [`ToastSpec`] a window draws, from a received [`NotificationView`].
///
/// This is the window's half of the projection — the only place a `ToastSpec` is built from a
/// notification, so the cut, reflow and action wiring live in one place. Keyed by each action's
/// own `intent.action` name, which is what `on_action(id, key)` reports back.
pub fn toast_spec(view: &NotificationView) -> ToastSpec {
    let mut spec = ToastSpec::new(view.id.as_u64(), view.title.clone())
        .severity(view.severity.as_toast_severity())
        .dismissible(view.dismissible);

    if let Some(lifetime) = view.auto_dismiss {
        // A timed card gets a countdown bar; the stack turns the widget's `progress` prop on when a
        // span is present, and a sticky card (no span) shows none.
        spec = spec.countdown(lifetime);
    }
    if let Some(body) = &view.body {
        spec = spec.body(body.clone());
    }
    for action in &view.actions {
        spec = spec.action_with(
            ToastAction::new(action.intent.action.clone(), action.label.clone())
                .variant(action.variant),
        );
    }
    spec
}

#[cfg(test)]
mod tests;
