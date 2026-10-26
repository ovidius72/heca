use super::*;
use crate::notification::test_support::*;

#[test]
fn queued_and_history_notifications_do_not_project() {
    let queued =
        NotificationDraft::new("Queued").build(NotificationId::from_raw(20), Instant::now());
    assert!(project_visible_toast(&queued).is_none());

    let mut history = queued;
    history.placement = NotificationPlacement::History {
        cause: NotificationArchiveCause::Dismissed,
        archived_at: Instant::now(),
    };
    assert!(project_visible_toast(&history).is_none());
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

    let spec = project_visible_toast(&notification).expect("visible notification projects");
    assert_eq!(spec.id, 21);
    assert_eq!(spec.title, "Build failed");
    assert_eq!(spec.body.as_deref(), Some("compiler exited with code 1"));
    assert_eq!(spec.severity, ToastSeverity::Danger);
    assert_eq!(spec.actions.len(), 1);
    assert_eq!(spec.actions[0].key, "build.retry");
    assert_eq!(spec.actions[0].label, "Retry");
    // The Intent itself does not cross into ToastSpec — only its name, as the key.
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

    let first_spec = project_visible_toast(&first).unwrap();
    let second_spec = project_visible_toast(&second).unwrap();
    assert_eq!(first_spec.id, second_spec.id);
    assert_ne!(first_spec, second_spec);
    assert_eq!(second_spec.severity, ToastSeverity::Success);
}
