//! The notification history, asked for over the wire: server-owned, so every window reads the same
//! list and a window that detached and came back loses nothing.

use super::service_tests::{attached, config, drain, server};
use super::*;
use crate::notification::{NotificationDraft, NotificationRecord};
use crate::server::NotificationCommand;
use crate::server::wire::ClientLink;

/// What a window is told when it asks for the history.
fn asked(window: &crate::server::wire::InProcessClient, server: &mut ServerLoop) -> Vec<NotificationRecord> {
    window.send(ClientMsg::NotificationHistory).unwrap();
    server.pump(Instant::now());
    let mut answers = drain(window).into_iter().filter_map(|message| match message {
        ServerMsg::NotificationHistory(records) => Some(records),
        _ => None,
    });
    let answer = answers.next().expect("the server answers a request for the history");
    assert!(answers.next().is_none(), "one request, one answer");
    answer
}

fn raise(window: &crate::server::wire::InProcessClient, server: &mut ServerLoop, draft: NotificationDraft) {
    window.send(ClientMsg::Notify(NotificationCommand::Raise(draft))).unwrap();
    server.pump(Instant::now());
}

fn titles(records: &[NotificationRecord]) -> Vec<(&str, &'static str)> {
    records.iter().map(|record| (record.view.title.as_str(), record.standing.word())).collect()
}

#[test]
fn a_window_that_asks_gets_the_history_newest_first_and_one_that_does_not_gets_none() {
    let mut server = server(&config(""));
    let window = attached(&mut server);
    let idle = attached(&mut server);
    drain(&window);
    drain(&idle);

    raise(&window, &mut server, NotificationDraft::new("first"));
    raise(&window, &mut server, NotificationDraft::new("second"));
    window.send(ClientMsg::Notify(NotificationCommand::DismissOne(first_id(&window, &mut server)))).unwrap();
    server.pump(Instant::now());
    drain(&idle);

    let records = asked(&window, &mut server);
    assert_eq!(titles(&records), [("second", "visible"), ("first", "dismissed")]);
    assert!(
        drain(&idle).iter().all(|message| !matches!(message, ServerMsg::NotificationHistory(_))),
        "the history is sent to the window that asked, never pushed",
    );
}

/// The id of the notification titled `first`, read from the history itself.
fn first_id(window: &crate::server::wire::InProcessClient, server: &mut ServerLoop) -> crate::notification::NotificationId {
    asked(window, server).iter().find(|record| record.view.title == "first").expect("raised").view.id
}

#[test]
fn every_window_reads_the_same_history_including_one_that_reattached() {
    let mut server = server(&config(""));
    let first = attached(&mut server);
    let second = attached(&mut server);
    raise(&first, &mut server, NotificationDraft::new("restore warning").body("the folder moved").sticky());
    raise(&first, &mut server, NotificationDraft::new("saved"));
    first.send(ClientMsg::Notify(NotificationCommand::DismissAll)).unwrap();
    server.pump(Instant::now());
    drain(&second);

    let before = asked(&first, &mut server);
    assert_eq!(before.len(), 2);
    assert_eq!(asked(&second, &mut server), before, "two windows, one list");

    // The first window goes away and comes back as a new connection.
    first.send(ClientMsg::Detach).unwrap();
    server.pump(Instant::now());
    drop(first);
    let again = attached(&mut server);
    let after = asked(&again, &mut server);
    assert_eq!(after, before, "the history belongs to the server, not to the window");
    assert_eq!(after[1].view.body.as_deref(), Some("the folder moved"), "with the whole body");
}

#[test]
fn a_notification_meant_for_one_window_is_not_in_the_others_history() {
    let mut server = server(&config(""));
    let owner = attached(&mut server);
    let other = attached(&mut server);
    raise(&owner, &mut server, NotificationDraft::new("for me").originator());
    raise(&owner, &mut server, NotificationDraft::new("for all"));

    assert_eq!(titles(&asked(&owner, &mut server)), [("for all", "visible"), ("for me", "visible")]);
    assert_eq!(titles(&asked(&other, &mut server)), [("for all", "visible")]);
}
