use super::*;
use crate::notification::test_support::*;

#[test]
fn app_notification_builders_set_fields() {
    let id = NotificationId::from_raw(7);
    let n = AppNotification::new(
        id,
        NotificationSource::new(NotificationSourceKind::Plugin, "git"),
        NotificationSeverity::Error,
        "Push failed",
        Instant::now(),
    )
    .dedup_key("git-push")
    .body("rejected by remote")
    .lifecycle(NotificationLifecycle::sticky())
    .action(NotificationAction::dismissing(
        "Push",
        Intent::new("git.push"),
    ));
    assert_eq!(n.dedup_key.as_deref(), Some("git-push"));
    assert_eq!(n.body.as_deref(), Some("rejected by remote"));
    assert!(n.lifecycle.is_dismissible());
    assert_eq!(n.actions.len(), 1);
    assert_eq!(n.actions[0].intent.action, "git.push");
    assert_eq!(n.actions[0].label, "Push");
    assert!(n.actions[0].dismiss_after);
}

#[test]
fn app_notification_visible_carries_expiry() {
    let mut n = AppNotification::new(
        NotificationId::from_raw(2),
        NotificationSource::app("t"),
        NotificationSeverity::Info,
        "x",
        Instant::now(),
    );
    let deadline = Instant::now() + std::time::Duration::from_secs(5);
    n.placement = NotificationPlacement::Visible {
        visible_since: deadline - std::time::Duration::from_secs(5),
        expires_at: Some(deadline),
    };
    assert!(n.is_visible());
    assert_eq!(n.expires_at(), Some(deadline));
}

#[test]
fn app_notification_history_placement() {
    let mut n = AppNotification::new(
        NotificationId::from_raw(3),
        NotificationSource::app("t"),
        NotificationSeverity::Info,
        "x",
        Instant::now(),
    );
    n.placement = NotificationPlacement::History {
        cause: NotificationArchiveCause::Dismissed,
        archived_at: Instant::now(),
    };
    assert!(n.is_history());
    assert!(!n.is_visible());
    assert!(n.expires_at().is_none()); // History has no expiry
}

#[test]
fn app_notification_clones_for_projection() {
    let n = AppNotification::new(
        NotificationId::from_raw(4),
        NotificationSource::app("t"),
        NotificationSeverity::Success,
        "Done",
        Instant::now(),
    );
    let proj = n.clone(); // cheap clone for projection
    assert_eq!(proj.id, n.id);
    assert_eq!(proj.title, n.title);
    // original untouched by projection
    assert_eq!(n.placement, NotificationPlacement::Queued);
}
