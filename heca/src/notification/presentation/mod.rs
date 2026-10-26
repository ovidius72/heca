use super::AppNotification;
use heca_grid_ui::widgets::ToastSpec;

/// Project a visible notification into the grid-ui presentation contract.
///
/// Only presentation data crosses the seam: id, title/body, mapped severity,
/// action key+label pairs, and dismissibility. Pickability is automatic on
/// the widget side now — there is no hint-target registration for the host to
/// thread through here. The `Intent`, queue, dedup policy, and timer state
/// stay in the app domain. `None` means the notification is not currently
/// visible (`Queued` or `History`).
///
/// Same-id updates naturally produce a new `ToastSpec`; `ToastStack` reconciles
/// that payload in place.
pub fn project_visible_toast(notification: &AppNotification) -> Option<ToastSpec> {
    if !notification.is_visible() {
        return None;
    }

    let mut spec = ToastSpec::new(notification.id.as_u64(), notification.title.clone())
        .severity(notification.severity.as_toast_severity())
        .dismissible(notification.lifecycle.is_dismissible());

    if let Some(body) = &notification.body {
        spec = spec.body(body.clone());
    }
    for action in &notification.actions {
        // Keyed by the action's own intent name — `on_action(id, key)` looks it
        // back up the same way (`AppNotification::action_for_visible`).
        spec = spec.action_with(
            heca_grid_ui::widgets::ToastAction::new(
                action.intent.action.clone(),
                action.label.clone(),
            )
            .variant(action.variant),
        );
    }

    Some(spec)
}

#[cfg(test)]
mod tests;
