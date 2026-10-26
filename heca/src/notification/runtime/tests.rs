use super::*;
use crate::notification::test_support::*;

#[test]
fn the_runtime_holds_the_stack_while_the_pointer_rests_on_it() {
    let now = Instant::now();
    let mut runtime = NotificationRuntime::new(
        100,
        std::time::Duration::from_secs(4),
        heca_config::settings::NotificationSystem::App,
    );
    runtime
        .push(NotificationDraft::new("Read me"), now)
        .expect("the stack is empty, so the draft is accepted");

    assert!(!runtime.is_hovered());
    assert!(
        !runtime.set_hovered(true, now),
        "arriving costs nothing yet"
    );
    assert!(runtime.is_hovered());
    assert!(
        !runtime.set_hovered(true, now + std::time::Duration::from_secs(1)),
        "still resting — reported every frame, and idempotent",
    );

    // Nothing expires while it is held, however long the pointer stays.
    assert!(!runtime.expire_due(now + std::time::Duration::from_secs(60)));

    assert!(
        runtime.set_hovered(false, now + std::time::Duration::from_secs(3)),
        "leaving pays back the three seconds it cost",
    );
    assert!(!runtime.is_hovered());
    assert!(
        !runtime.expire_due(now + std::time::Duration::from_secs(5)),
        "one second still left of the four",
    );
    assert!(
        runtime.expire_due(now + std::time::Duration::from_secs(8)),
        "…and then it goes"
    );
}


#[test]
fn runtime_applies_the_configured_auto_dismiss_to_a_default_draft() {
    let now = Instant::now();
    let mut runtime = test_runtime(2500);
    runtime.push(NotificationDraft::info("hi"), now).unwrap();
    // The visible notification's deadline is `now + configured`, not the type default.
    assert_eq!(
        runtime.next_expiry(),
        Some(now + std::time::Duration::from_millis(2500))
    );
}

#[test]
fn runtime_preserves_an_explicit_draft_duration() {
    let now = Instant::now();
    let mut runtime = test_runtime(2500);
    runtime.push(NotificationDraft::info("Long build")
        .auto_dismiss(std::time::Duration::from_secs(10)), now).unwrap();
    assert_eq!(runtime.next_expiry(), Some(now + std::time::Duration::from_secs(10)));
}

#[test]
fn runtime_preserves_an_explicit_lifecycle_even_when_it_matches_the_fallback() {
    let now = Instant::now();
    let duration = std::time::Duration::from_millis(
        heca_config::settings::NotificationSystemConfig::default().auto_dismiss_ms,
    );
    let mut runtime = test_runtime(9000);
    runtime.push(NotificationDraft::info("Explicit")
        .lifecycle(NotificationLifecycle::auto(duration).dismissible(false)), now).unwrap();
    assert_eq!(runtime.next_expiry(), Some(now + duration));
    assert!(!runtime.visible_toasts()[0].dismissible);
}

#[test]
fn runtime_uses_the_config_fallback_when_no_duration_is_provided() {
    let config = heca_config::settings::NotificationSystemConfig::default();
    let now = Instant::now();
    let mut runtime = test_runtime(config.auto_dismiss_ms);
    runtime.push(NotificationDraft::info("Default"), now).unwrap();
    assert_eq!(runtime.next_expiry(), Some(now + std::time::Duration::from_millis(config.auto_dismiss_ms)));
}

#[test]
fn reload_changes_new_default_durations_but_not_explicit_ones() {
    let now = Instant::now();
    let mut runtime = test_runtime(1000);
    runtime.set_auto_dismiss(std::time::Duration::from_secs(9));
    let explicit = runtime.push(NotificationDraft::info("Explicit")
        .auto_dismiss(std::time::Duration::from_secs(3)), now).unwrap().notification_id();
    let configured = runtime.push(NotificationDraft::info("Configured"), now).unwrap().notification_id();
    assert_eq!(runtime.store.visible().find(|n| n.id == explicit).unwrap().expires_at(),
        Some(now + std::time::Duration::from_secs(3)));
    assert_eq!(runtime.store.visible().find(|n| n.id == configured).unwrap().expires_at(),
        Some(now + std::time::Duration::from_secs(9)));
}

#[test]
fn runtime_leaves_a_sticky_draft_alone() {
    let now = Instant::now();
    let mut runtime = test_runtime(2500);
    runtime
        .push(
            NotificationDraft::warning("careful").lifecycle(NotificationLifecycle::sticky()),
            now,
        )
        .unwrap();
    assert_eq!(runtime.next_expiry(), None);
}

#[test]
fn set_auto_dismiss_affects_only_notifications_raised_afterwards() {
    let now = Instant::now();
    let mut runtime = test_runtime(1000);
    runtime.push(NotificationDraft::info("first"), now).unwrap();
    runtime.set_auto_dismiss(std::time::Duration::from_millis(9000));
    // The already-visible first notification keeps its `now + 1s` deadline.
    assert_eq!(
        runtime.next_expiry(),
        Some(now + std::time::Duration::from_millis(1000))
    );
}

#[test]
fn mode_none_suppresses_and_mode_change_is_live() {
    use heca_config::settings::NotificationSystem;
    let mut runtime = test_runtime(4000);
    assert!(!runtime.suppressed());
    runtime.set_mode(NotificationSystem::None);
    assert!(runtime.suppressed());
    // `system` is reserved and delivers like `app` for now — not suppressed.
    runtime.set_mode(NotificationSystem::System);
    assert!(!runtime.suppressed());
}

#[test]
fn system_mode_announces_the_in_app_fallback_exactly_once() {
    use heca_config::settings::NotificationSystem;
    let mut runtime = test_runtime(4000);

    // app / none never produce the notice.
    assert!(runtime.system_fallback_notice().is_none());
    runtime.set_mode(NotificationSystem::None);
    assert!(runtime.system_fallback_notice().is_none());

    runtime.set_mode(NotificationSystem::System);
    let notice = runtime
        .system_fallback_notice()
        .expect("first raise under system mode gets the notice");
    assert_eq!(
        notice.dedup_key.as_deref(),
        Some("notification.system.unavailable")
    );
    assert!(
        runtime.system_fallback_notice().is_none(),
        "and only once per mode"
    );

    // switching away and back re-arms it.
    runtime.set_mode(NotificationSystem::App);
    runtime.set_mode(NotificationSystem::System);
    assert!(runtime.system_fallback_notice().is_some());
}
