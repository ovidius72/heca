//! **An extension's saved state, through a real server**: an action saves an entry of its session,
//! a listener hears `state.changed`, the entry comes back after the server restarts, and it goes
//! when the session is deleted — and every window is shown the new value with nothing written by
//! the extension to say its view changed.

use super::restore_tests::{boot, make, restart, session};
use super::service_tests::{attached, register_actions, rpc, updates};
use crate::server::wire::service::ServiceUpdateKind;
use super::*;
use crate::entry::service::{ActionResult, ProjectCx, Projection, Scope, Service, ServiceCx, ServiceResult};
use std::cell::Cell;

struct Rooms;

impl Service for Rooms {
    type Message = ();
    type Snapshot = Vec<String>;
    type Delta = ();
    const SCHEMA_VERSION: u32 = 1;
    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> { Ok(()) }
    /// The window's room order, read from the saved state. Its revision never moves: the service
    /// never says its view changed, so only heca re-showing it after a save can bring a new order.
    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<Vec<String>, ()>> {
        Some(match cx.since() {
            Some(_) => Projection::Unchanged,
            None => Projection::Snapshot { revision: 1, value: cx.saved(Scope::Session, "room-order", "").unwrap_or_default() },
        })
    }
}

/// The service: `demo.reorder` saves the session's room order, `demo.order` answers it.
fn declare() -> Rc<Cell<usize>> {
    crate::entry::service::reset();
    let heard = Rc::new(Cell::new(0));
    let demo = crate::entry::extension("demo");
    let rooms = demo.service("rooms", |_| Ok(Rooms));
    let count = heard.clone();
    rooms.on("state.changed", move |_, _, ev| {
        let change = ev.as_state().expect("state.changed carries the entry");
        assert_eq!(change.key, "demo.room-order", "heca puts the extension's name in front");
        count.set(count.get() + 1);
    });
    rooms.action(demo.action("reorder"), |_, cx, _| {
        match cx.save(Scope::Session, "room-order", "", &vec!["lobby", "kitchen"]) {
            Ok(()) => ActionResult::Done(None),
            Err(error) => ActionResult::Refused(error.to_string()),
        }
    });
    rooms.action(demo.action("order"), |_, cx, _| {
        let order: Option<Vec<String>> = cx.saved(Scope::Session, "room-order", "");
        ActionResult::Done(Some(order.map_or_else(|| "none".into(), |order| order.join(","))))
    });
    heard
}

fn ask(server: &mut ServerLoop, line: &str) -> String {
    let answer = rpc(server, line);
    for _ in 0..50 {
        server.pump(Instant::now());
        if let Ok(line) = answer.try_recv() { return line; }
    }
    panic!("no answer to {line}");
}

#[test]
fn a_session_entry_is_heard_kept_across_a_restart_and_gone_with_the_session() {
    let heard = declare();
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    register_actions(&mut server);
    make(&mut server, folders.path(), "Work");
    assert_eq!(ask(&mut server, "demo.order"), "none", "nothing saved yet");
    ask(&mut server, "demo.reorder");
    assert_eq!(heard.get(), 1, "state.changed, once");
    assert_eq!(ask(&mut server, "demo.order"), "lobby,kitchen");

    let mut server = restart(server, root.path(), "");
    register_actions(&mut server);
    assert_eq!(ask(&mut server, "demo.order"), "lobby,kitchen", "back after a restart");

    let key = session(&mut server, "Work").service_ref().key;
    server.execute_session(SessionCommand::Delete { name: "Work".into() }).unwrap();
    assert!(server.services.borrow().saved_entries(key).is_empty(), "gone with the session");
}

#[test]
fn a_save_shows_every_window_the_new_value() {
    declare();
    let root = tempfile::tempdir().unwrap();
    let folders = tempfile::tempdir().unwrap();
    let mut server = boot(root.path(), "");
    register_actions(&mut server);
    make(&mut server, folders.path(), "Work");
    let window = attached(&mut server);
    server.pump(Instant::now());
    let _ = updates(&window);

    ask(&mut server, "demo.reorder");
    server.pump(Instant::now());
    let order: serde_json::Value = vec!["lobby", "kitchen"].into();
    assert!(updates(&window).iter().any(|(_, kind)| *kind == ServiceUpdateKind::Snapshot(order.clone())),
        "the window is shown the saved order");
}
