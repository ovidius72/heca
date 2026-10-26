use super::*;
use crate::notification::test_support::*;

#[test]
fn kind_label_is_lowercase_stable() {
    assert_eq!(NotificationSourceKind::App.as_label(), "app");
    assert_eq!(NotificationSourceKind::Config.as_label(), "config");
    assert_eq!(NotificationSourceKind::Pane.as_label(), "pane");
    assert_eq!(NotificationSourceKind::Provider.as_label(), "provider");
    assert_eq!(NotificationSourceKind::Plugin.as_label(), "plugin");
    assert_eq!(NotificationSourceKind::Command.as_label(), "command");
}

#[test]
fn no_agent_variant_exists() {
    // Compile-time: there is no Agent variant. This exhaustive match with no
    // Agent arm fails to compile if one is added — a guard rail, not a goal.
    fn has_no_agent(k: NotificationSourceKind) {
        match k {
            NotificationSourceKind::App
            | NotificationSourceKind::Config
            | NotificationSourceKind::Pane
            | NotificationSourceKind::Provider
            | NotificationSourceKind::Plugin
            | NotificationSourceKind::Command => {}
        }
    }
    has_no_agent(NotificationSourceKind::App);
}

#[test]
fn source_carries_kind_and_opaque_id() {
    let s = NotificationSource::new(NotificationSourceKind::Plugin, "git-watch");
    assert_eq!(s.kind(), NotificationSourceKind::Plugin);
    assert_eq!(s.id(), Some("git-watch"));
}

#[test]
fn app_helper_sets_app_kind() {
    let s = NotificationSource::app("config");
    assert_eq!(s.kind(), NotificationSourceKind::App);
    assert_eq!(s.id(), Some("config"));
}

#[test]
fn of_helper_leaves_id_none() {
    let s = NotificationSource::of(NotificationSourceKind::Command);
    assert_eq!(s.kind(), NotificationSourceKind::Command);
    assert!(s.id().is_none());
}

#[test]
fn default_is_unspecified_app() {
    let s = NotificationSource::default();
    assert_eq!(s.kind(), NotificationSourceKind::App);
    assert!(s.id().is_none());
}

#[test]
fn dedup_segment_combines_kind_and_id() {
    let s = NotificationSource::new(NotificationSourceKind::Pane, "pane-4f2a");
    assert_eq!(s.dedup_segment(), "pane:pane-4f2a");
}

#[test]
fn dedup_segment_falls_back_to_kind_when_id_missing_or_empty() {
    assert_eq!(
        NotificationSource::of(NotificationSourceKind::App).dedup_segment(),
        "app"
    );
    assert_eq!(
        NotificationSource::new(NotificationSourceKind::App, "").dedup_segment(),
        "app"
    );
}

#[test]
fn source_serializes_with_snake_case_kind() {
    let s = NotificationSource::new(NotificationSourceKind::Provider, "docker");
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"kind\":\"provider\""), "got {json}");
    let back: NotificationSource = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn source_eq_hash_stable_across_clone() {
    let a = NotificationSource::new(NotificationSourceKind::Plugin, "x");
    let b = a.clone();
    assert_eq!(a, b);
    let mut set = std::collections::HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn lifecycle_default_is_dismissible_and_uses_the_config_fallback() {
    let l = NotificationLifecycle::default();
    assert!(l.is_dismissible());
    assert!(l.expires());
    assert_eq!(
        l.lifetime(),
        NotificationLifetime::Auto(std::time::Duration::from_millis(
            heca_config::settings::NotificationSystemConfig::default().auto_dismiss_ms,
        ))
    );
}

#[test]
fn sticky_is_dismissible_and_does_not_expire() {
    let l = NotificationLifecycle::sticky();
    assert!(l.is_dismissible());
    assert!(!l.expires());
    assert_eq!(l.lifetime(), NotificationLifetime::Sticky);
}

#[test]
fn sticky_persistent_is_not_dismissible_and_does_not_expire() {
    let l = NotificationLifecycle::sticky_persistent();
    assert!(!l.is_dismissible());
    assert!(!l.expires());
    assert_eq!(l.lifetime(), NotificationLifetime::Sticky);
}

#[test]
fn dismissible_can_be_turned_off() {
    let l = NotificationLifecycle::auto(std::time::Duration::from_secs(3)).dismissible(false);
    assert!(!l.is_dismissible());
    assert!(l.expires());
}

#[test]
fn auto_lifetime_expires_sticky_does_not() {
    assert!(NotificationLifecycle::auto(std::time::Duration::from_secs(1)).expires());
    assert!(!NotificationLifecycle::sticky().expires());
}

#[test]
fn lifecycle_carries_no_instant() {
    // The domain type carries no expires_at / Instant — the store computes
    // expiry lazily on promotion. Ensure the public API exposes no time field.
    let l = NotificationLifecycle::sticky();
    let s = format!("{l:?}");
    assert!(!s.contains("Instant"));
    assert!(!s.contains("expires_at"));
}

#[test]
fn severity_does_not_change_the_default_lifetime() {
    // Severity is tone, not importance — every severity gets the same
    // auto-dismiss default; a producer opts into sticky explicitly.
    for severity in [
        NotificationSeverity::Info,
        NotificationSeverity::Success,
        NotificationSeverity::Warning,
        NotificationSeverity::Error,
    ] {
        let n = NotificationDraft::new("x")
            .severity(severity)
            .build(NotificationId::from_raw(1), Instant::now());
        assert_eq!(n.lifecycle, NotificationLifecycle::default());
    }
}

#[test]
fn lifecycle_serializes_with_snake_case() {
    let auto = NotificationLifecycle::auto(std::time::Duration::from_secs(2));
    let json = serde_json::to_string(&auto).unwrap();
    assert!(json.contains("\"auto\""), "got {json}");
    assert!(json.contains("\"dismissible\":true"), "got {json}");
    // round-trip
    let back: NotificationLifecycle = serde_json::from_str(&json).unwrap();
    assert_eq!(back, auto);

    let sticky = NotificationLifecycle::sticky();
    let sjson = serde_json::to_string(&sticky).unwrap();
    assert!(sjson.contains("\"sticky\""), "got {sjson}");
    assert!(sjson.contains("\"dismissible\":true"), "got {sjson}");
    let sback: NotificationLifecycle = serde_json::from_str(&sjson).unwrap();
    assert_eq!(sback, sticky);
}

#[test]
fn notification_severity_maps_totally_to_toast_severity() {
    assert_eq!(
        NotificationSeverity::Info.as_toast_severity(),
        ToastSeverity::Info
    );
    assert_eq!(
        NotificationSeverity::Success.as_toast_severity(),
        ToastSeverity::Success
    );
    assert_eq!(
        NotificationSeverity::Warning.as_toast_severity(),
        ToastSeverity::Warning
    );
    assert_eq!(
        NotificationSeverity::Error.as_toast_severity(),
        ToastSeverity::Danger
    );
}

#[test]
fn notification_severity_from_matches_adapter() {
    let severities = [
        NotificationSeverity::Info,
        NotificationSeverity::Success,
        NotificationSeverity::Warning,
        NotificationSeverity::Error,
    ];
    for severity in severities {
        assert_eq!(ToastSeverity::from(severity), severity.as_toast_severity());
    }
}

#[test]
fn notification_action_defaults_to_non_dismissive_retry() {
    let action = NotificationAction::new("Retry", Intent::new("build.retry"));
    assert_eq!(action.label, "Retry");
    assert_eq!(action.intent.action, "build.retry");
    assert!(!action.dismiss_after);
}

#[test]
fn notification_action_preserves_intent_arguments() {
    let intent = Intent::new("deploy.retry").arg("attempt", heca_view::PropValue::Int(2));
    let action = NotificationAction::new("Retry", intent).dismiss_after(true);
    assert!(action.dismiss_after);
    assert_eq!(
        action.intent.args.get("attempt"),
        Some(&heca_view::PropValue::Int(2))
    );
}

#[test]
fn notification_action_serializes_as_data_without_callbacks() {
    let action = NotificationAction::dismissing("Open", Intent::new("open_log"))
        .variant(heca_grid_ui::widgets::ButtonVariant::Ghost);
    let json = serde_json::to_string(&action).unwrap();
    assert!(json.contains("open_log"));
    assert!(json.contains("dismiss_after"));
    assert!(
        json.contains("\"ghost\""),
        "variant crosses as its name, not the enum: {json}"
    );
    let back: NotificationAction = serde_json::from_str(&json).unwrap();
    assert_eq!(back, action);
}

#[test]
fn an_unknown_variant_name_degrades_to_the_default_rather_than_failing() {
    let json = r#"{"label":"Open","intent":{"action":"open_log"},"variant":"not_a_real_variant","dismiss_after":false}"#;
    let action: NotificationAction = serde_json::from_str(json).unwrap();
    assert_eq!(
        action.variant,
        heca_grid_ui::widgets::ButtonVariant::default()
    );
}

#[test]
fn notification_draft_has_domain_defaults() {
    let draft = NotificationDraft::new("Hello");
    let now = Instant::now();
    let notification = draft.build(NotificationId::from_raw(10), now);
    assert_eq!(notification.title, "Hello");
    assert_eq!(notification.source.kind(), NotificationSourceKind::App);
    assert_eq!(notification.severity, NotificationSeverity::Info);
    assert_eq!(notification.lifecycle, NotificationLifecycle::default());
    assert_eq!(notification.placement, NotificationPlacement::Queued);
    assert_eq!(notification.created_at, now);
}

#[test]
fn notification_draft_sets_all_optional_fields() {
    let now = Instant::now();
    let notification = NotificationDraft::new("Failed")
        .source(NotificationSource::new(
            NotificationSourceKind::Plugin,
            "git",
        ))
        .severity(NotificationSeverity::Error)
        .lifecycle(NotificationLifecycle::sticky())
        .dismissible(false)
        .body("remote rejected")
        .dedup_key("git-push")
        .action(NotificationAction::new("Retry", Intent::new("git.retry")))
        .build(NotificationId::from_raw(11), now);
    assert_eq!(notification.source.id(), Some("git"));
    assert_eq!(notification.severity, NotificationSeverity::Error);
    assert!(!notification.lifecycle.is_dismissible());
    assert_eq!(notification.body.as_deref(), Some("remote rejected"));
    assert_eq!(notification.dedup_key.as_deref(), Some("git-push"));
    assert_eq!(notification.actions.len(), 1);
    assert_eq!(notification.actions[0].intent.action, "git.retry");
}

#[test]
fn notification_draft_build_does_not_make_notification_visible() {
    let n =
        NotificationDraft::new("Queued").build(NotificationId::from_raw(12), Instant::now());
    assert_eq!(n.placement, NotificationPlacement::Queued);
    assert!(!n.is_visible());
    assert!(n.expires_at().is_none());
}

#[test]
fn notification_ids_are_monotonic_and_unique() {
    let a = NotificationId::next();
    let b = NotificationId::next();
    let c = NotificationId::next();
    assert!(b.as_u64() > a.as_u64());
    assert!(c.as_u64() > b.as_u64());
    assert_ne!(a, b);
    assert_ne!(b, c);
}

#[test]
fn notification_id_serializes_as_u64() {
    let id = NotificationId::from_raw(42);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "42");
    let back: NotificationId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn placement_default_is_queued() {
    assert_eq!(
        NotificationPlacement::default(),
        NotificationPlacement::Queued
    );
}

#[test]
fn app_notification_new_has_defaults() {
    let id = NotificationId::from_raw(1);
    let now = Instant::now();
    let n = AppNotification::new(
        id,
        NotificationSource::app("test"),
        NotificationSeverity::Info,
        "Build done",
        now,
    );
    assert_eq!(n.id, id);
    assert!(n.dedup_key.is_none());
    assert!(n.body.is_none());
    assert!(n.actions.is_empty());
    assert_eq!(n.placement, NotificationPlacement::Queued);
    assert!(!n.is_visible());
    assert!(!n.is_history());
    assert!(n.expires_at().is_none());
    assert_eq!(n.created_at, now);
}
