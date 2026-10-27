use super::*;
use crate::notification::test_support::*;

#[test]
fn notification_send_hands_the_exact_draft_through_the_installed_sink() {
    // `Notification::…send()` is the whole public raise surface. Everything below it is
    // sealed: `NotificationRuntime::push` and `NotificationStore::push` are `pub(crate)`
    // (no producer or plugin can name them), `NotificationRuntime`'s `store` field is
    // private, and `raise` — the only caller of `push` outside these tests — is
    // `pub(crate)` too. So the one way a draft reaches the store is the sink installed
    // here, and this pins what crosses it.
    use std::cell::RefCell;
    use std::rc::Rc;

    let captured: Rc<RefCell<Vec<NotificationDraft>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = captured.clone();
    install_notification_sink(move |draft| sink.borrow_mut().push(draft));

    Notification::success("Build finished")
        .body("3 warnings in heca-grid-ui")
        .dedup_key("build")
        .action(
            NotificationAction::new("View log", Intent::new("open_pane_log"))
                .variant(heca_grid_ui::widgets::ButtonVariant::Ghost),
        )
        .send();

    let drafts = captured.borrow();
    assert_eq!(drafts.len(), 1, "one send, one draft on the sink");

    let built = drafts[0]
        .clone()
        .build(NotificationId::from_raw(1), Instant::now());
    assert_eq!(built.title, "Build finished");
    assert_eq!(built.severity, NotificationSeverity::Success);
    assert_eq!(built.body.as_deref(), Some("3 warnings in heca-grid-ui"));
    assert_eq!(built.dedup_key.as_deref(), Some("build"));
    assert_eq!(built.actions.len(), 1);
    assert_eq!(built.actions[0].intent.action, "open_pane_log");
    assert_eq!(
        built.actions[0].variant,
        heca_grid_ui::widgets::ButtonVariant::Ghost
    );
}

#[test]
fn notification_send_is_inert_until_a_sink_is_installed() {
    // A `.send()` before startup installs the sink — or in any headless test — is simply
    // dropped, never a panic. This is what lets a producer call `Notification::send`
    // unconditionally.
    Notification::info("no sink here").send();
}

#[test]
fn public_builder_preserves_optional_durations_through_send_and_acceptance() {
    use std::{cell::RefCell, rc::Rc, time::Duration};

    let now = Instant::now();
    let explicit = Duration::from_secs(10);
    let configured = Duration::from_millis(2500);
    for (notification, expected) in [
        (Notification::info("Configured"), Some(configured)),
        (Notification::info("Explicit").auto_dismiss(explicit).body("Done").dedup_key("build"), Some(explicit)),
        (Notification::warning("Sticky").auto_dismiss(explicit).sticky(), None),
        (Notification::info("Timed again").sticky().auto_dismiss(explicit), Some(explicit)),
        (Notification::info("Immediate").auto_dismiss(Duration::ZERO), Some(Duration::ZERO)),
    ] {
        let runtime = Rc::new(RefCell::new(test_runtime(2500)));
        let sink = runtime.clone();
        install_notification_sink(move |draft| { sink.borrow_mut().push(draft, now).unwrap(); });
        notification.send();
        assert_eq!(runtime.borrow().next_expiry(), expected.map(|duration| now + duration));
    }
}

#[test]
fn notification_sticky_opts_out_of_auto_dismiss_and_survives_the_runtime_rewrite() {
    let now = Instant::now();
    let mut runtime = test_runtime(4000);
    let draft = Notification::danger("Config reload failed")
        .body("expected `=` at line 3")
        .sticky()
        .draft;
    runtime.push(draft, now).unwrap();
    assert_eq!(
        runtime.next_expiry(),
        None,
        "a sticky notification never expires"
    );
}

#[test]
fn retry_dismisses_the_failure_toast_then_a_deduped_success_comes_back_fresh() {
    // The reload-config flow (heca/src/main.rs): the Retry action has `dismiss_after`, so
    // clicking it archives the sticky failure toast; the reload it triggers then raises a
    // success with the SAME dedup key, which re-promotes to a fresh visible card rather
    // than mutating the archived one in place.
    let now = Instant::now();
    let mut runtime = test_runtime(4000);

    let fail = Notification::danger("Config reload failed")
        .body("bad line 3")
        .dedup_key("config-reload")
        .sticky()
        .draft;
    let fail_id = runtime.push(fail, now).unwrap().notification_id();
    assert_eq!(runtime.visible_views().len(), 1);
    assert_eq!(
        runtime.next_expiry(),
        None,
        "sticky failure does not expire"
    );

    // Retry's `dismiss_after`.
    runtime.dismiss_one(fail_id, now);
    assert!(runtime.visible_views().is_empty(), "Retry cleared it");

    // The reload succeeded — success with the same dedup key.
    let ok = Notification::success("Configuration reloaded")
        .dedup_key("config-reload")
        .draft;
    runtime.push(ok, now).unwrap();
    let v = runtime.visible_views();
    assert_eq!(v.len(), 1, "a fresh success card is visible");
    assert_eq!(v[0].title, "Configuration reloaded");
    assert_eq!(v[0].severity, crate::notification::NotificationSeverity::Success);
    assert_eq!(
        runtime.next_expiry(),
        Some(now + std::time::Duration::from_millis(4000)),
        "and it auto-dismisses"
    );
    // A toast that auto-dismisses carries its lifetime in the projection, so the stack draws a
    // countdown bar. This is the real path: a plain `Notification::success(..)` with no explicit
    // duration gets the configured default applied at push.
    assert_eq!(
        v[0].auto_dismiss,
        Some(std::time::Duration::from_millis(4000)),
        "the timed toast's lifetime reaches the view for the countdown bar"
    );
}

#[test]
fn a_visible_actions_intent_is_resolvable_for_the_relay_to_re_dispatch() {
    // `on_action(id, key)` names the relay action; the relay looks the key
    // back up here and re-dispatches the *real* Intent through the router (registry/policy
    // path), never a direct handler. This is the lookup that makes that possible.
    let now = Instant::now();
    let mut runtime = test_runtime(4000);
    let draft = Notification::info("Build failed")
        .action(
            NotificationAction::new(
                "Open log",
                Intent::new("open_pane_log").arg("pane_id", heca_view::PropValue::Int(7)),
            )
            .dismiss_after(true),
        )
        .draft;
    let id = runtime.push(draft, now).unwrap().notification_id();

    let (intent, dismiss_after) = runtime
        .action_and_dismiss_after_for_visible(id, "open_pane_log")
        .expect("the visible card's action resolves by its intent name");
    assert_eq!(intent.action, "open_pane_log");
    assert_eq!(
        intent.args.get("pane_id"),
        Some(&heca_view::PropValue::Int(7))
    );
    assert!(dismiss_after);

    assert!(
        runtime
            .action_and_dismiss_after_for_visible(id, "not_an_action")
            .is_none(),
        "an unknown key resolves to nothing, not a panic"
    );
}
