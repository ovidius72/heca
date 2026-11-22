//! Asking the owner for the notification history, from a window, over the real owner loop.

use super::reconnect_tests::{Owner, attached, create};
use super::*;
use crate::notification::NotificationDraft;
use crate::server::NotificationCommand;

/// Poll until the owner has answered, or fail.
fn answered(client: &mut ClientState) -> Update {
    for _ in 0..20 {
        let update = client.poll().unwrap();
        if update.notification_history_answered {
            return update;
        }
    }
    panic!("the owner did not answer the request for the history");
}

#[test]
fn a_window_asks_the_owner_and_the_answer_opens_the_notifications_list() {
    let root = tempfile::tempdir().unwrap();
    let (_owner, window, control) = Owner::new();
    create(&control, root.path(), "A");
    let mut client = attached(window);

    client.notify(NotificationCommand::Raise(
        NotificationDraft::new("Restore failed").body("the folder moved"),
    ));
    client.notify(NotificationCommand::Raise(NotificationDraft::new("Saved")));
    client.notify(NotificationCommand::DismissLast);
    assert!(
        client.notification_history.is_none(),
        "nothing is held until it is asked for"
    );

    client.request_notification_history();
    let update = answered(&mut client);
    assert!(
        update.intents.is_empty(),
        "the list is opened by the window itself, not routed as a `command_palette` action: that is \
         refused while a palette or a floating pane has the screen, which is when a typed `!` asks",
    );

    let held = client
        .notification_history
        .take()
        .expect("the answer is held for the palette that lists it");
    let listed: Vec<(&str, &str)> = held
        .iter()
        .map(|record| (record.view.title.as_str(), record.standing.word()))
        .collect();
    assert_eq!(
        listed,
        [("Saved", "visible"), ("Restore failed", "dismissed")],
        "newest first"
    );
    assert_eq!(held[1].view.body.as_deref(), Some("the folder moved"));
    assert!(
        client.notification_history.is_none(),
        "taken once, not kept"
    );
}
