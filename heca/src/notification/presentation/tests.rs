use super::*;
use crate::notification::test_support::*;
use heca_grid_ui::widgets::ToastSeverity;

#[test]
fn queued_and_history_notifications_do_not_project() {
    let queued =
        NotificationDraft::new("Queued").build(NotificationId::from_raw(20), Instant::now());
    assert!(notification_view(&queued).is_none());

    let mut history = queued;
    history.placement = NotificationPlacement::History {
        cause: NotificationArchiveCause::Dismissed,
        archived_at: Instant::now(),
    };
    assert!(notification_view(&history).is_none());
}

#[test]
fn visible_notification_projects_presentation_data_and_keyed_actions() {
    let mut notification = NotificationDraft::new("Build failed")
        .source(NotificationSource::app("test"))
        .severity(NotificationSeverity::Error)
        .body("compiler exited with code 1")
        .action(NotificationAction::new("Retry", Intent::new("build.retry")))
        .build(NotificationId::from_raw(21), Instant::now());
    notification.placement = NotificationPlacement::Visible {
        visible_since: Instant::now(),
        expires_at: None,
    };

    let view = notification_view(&notification).expect("visible notification projects");
    // The view keeps the whole action, Intent and all — that is what the action relay resolves a
    // pressed key back through on the window side.
    assert_eq!(view.id.as_u64(), 21);
    assert_eq!(view.title, "Build failed");
    assert_eq!(view.severity, NotificationSeverity::Error);
    assert_eq!(view.action("build.retry").map(|a| a.label.as_str()), Some("Retry"));

    // The window builds the grid-ui spec from the view; only the action's name crosses into it.
    let spec = toast_spec(&view);
    assert_eq!(spec.id, 21);
    assert_eq!(spec.title, "Build failed");
    assert_eq!(spec.body.as_deref(), Some("compiler exited with code 1"));
    assert_eq!(spec.severity, ToastSeverity::Danger);
    assert_eq!(spec.actions.len(), 1);
    assert_eq!(spec.actions[0].key, "build.retry");
    assert_eq!(spec.actions[0].label, "Retry");
}

#[test]
fn a_timed_notification_carries_its_lifetime_to_the_spec_and_a_sticky_one_does_not() {
    // A timed (auto-dismiss) card: the lifetime reaches the spec, so the stack draws a countdown.
    let mut timed = NotificationDraft::new("Saved")
        .auto_dismiss(std::time::Duration::from_secs(4))
        .build(NotificationId::from_raw(40), Instant::now());
    timed.placement = NotificationPlacement::Visible {
        visible_since: Instant::now(),
        expires_at: None,
    };
    let view = notification_view(&timed).expect("visible");
    assert_eq!(view.auto_dismiss, Some(std::time::Duration::from_secs(4)));
    assert_eq!(toast_spec(&view).countdown, Some(std::time::Duration::from_secs(4)));

    // A sticky card carries no span, so no bar.
    let mut sticky = NotificationDraft::new("Fix me")
        .sticky()
        .build(NotificationId::from_raw(41), Instant::now());
    sticky.placement = NotificationPlacement::Visible {
        visible_since: Instant::now(),
        expires_at: None,
    };
    let sticky_view = notification_view(&sticky).expect("visible");
    assert_eq!(sticky_view.auto_dismiss, None);
    assert_eq!(toast_spec(&sticky_view).countdown, None);
}

#[test]
fn a_view_survives_the_wire_with_its_intent() {
    let mut notification = NotificationDraft::new("Config reload failed")
        .severity(NotificationSeverity::Error)
        .action(
            NotificationAction::new("Retry", Intent::new("reload_config")).dismiss_after(true),
        )
        .build(NotificationId::from_raw(30), Instant::now());
    notification.placement = NotificationPlacement::Visible {
        visible_since: Instant::now(),
        expires_at: None,
    };

    let view = notification_view(&notification).expect("visible");
    let json = serde_json::to_string(&view).expect("serializes");
    let back: NotificationView = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(view, back);
    // The Intent and its post-dispatch policy ride across — a ToastSpec could carry neither.
    let action = back.action("reload_config").expect("action by its intent name");
    assert_eq!(action.intent.action, "reload_config");
    assert!(action.dismiss_after);
}

#[test]
fn same_id_payload_update_produces_a_different_spec() {
    let id = NotificationId::from_raw(22);
    let now = Instant::now();
    let mut first = NotificationDraft::new("Connecting").build(id, now);
    first.placement = NotificationPlacement::Visible {
        visible_since: now,
        expires_at: None,
    };
    let mut second = NotificationDraft::new("Connected")
        .severity(NotificationSeverity::Success)
        .build(id, now);
    second.placement = NotificationPlacement::Visible {
        visible_since: now,
        expires_at: None,
    };

    let first_spec = toast_spec(&notification_view(&first).unwrap());
    let second_spec = toast_spec(&notification_view(&second).unwrap());
    assert_eq!(first_spec.id, second_spec.id);
    assert_ne!(first_spec, second_spec);
    assert_eq!(second_spec.severity, ToastSeverity::Success);
}
