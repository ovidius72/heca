//! Compile and exercise the notification API from outside the library.

use heca::{Intent, Notification, NotificationAction};
use std::time::Duration;

#[test]
fn external_native_producer_uses_the_fluent_builder() {
    // Without a running host, send is intentionally inert. Delivery is tested by the host.
    Notification::success("Build finished")
        .body("All tests passed")
        .dedup_key("build.result")
        .send();
    Notification::info("Build started").send();
    Notification::warning("Build has warnings").sticky().send();
    Notification::danger("Build failed").send();
}

#[test]
fn external_native_producer_can_optionally_set_the_duration() {
    Notification::success("Build finished")
        .auto_dismiss(Duration::from_secs(10))
        .send();
    Notification::info("Configured duration").send();
    Notification::warning("Acknowledge this")
        .auto_dismiss(Duration::from_secs(10)).sticky().send();
}

#[test]
fn external_native_producer_can_attach_a_named_action() {
    let action = NotificationAction::new("Retry", Intent::new("build.retry"))
        .dismiss_after(true);
    assert_eq!(action.intent.action, "build.retry");
    assert!(action.dismiss_after);
    Notification::danger("Build failed").action(action).send();
}
