//! A session's key, as services see it: made once per session, kept through everything but a
//! delete, and always the key of the session a call or a window is in.

use super::service_tests::{config, drain, register_actions, rpc, server};
use super::*;
use crate::entry::service::{
    ActionResult, ProjectCx, Projection, Service, ServiceCx, ServiceResult, SessionKey,
};
use crate::server::identity::{ServerIdentity, ServerKind};
use crate::server::wire::{ClientLink, service::ServiceUpdateKind};
use heca_core::layout::LayoutOptions;
use std::cell::RefCell;

/// Shows each window its session's key.
struct Keyed;

impl Service for Keyed {
    type Message = ();
    type Snapshot = String;
    type Delta = String;
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> {
        Ok(())
    }

    fn project(&self, cx: &ProjectCx<'_>) -> Option<Projection<String, String>> {
        Some(Projection::Snapshot {
            revision: 0,
            value: format!("{} {}", cx.session(), cx.session_key()),
        })
    }
}

/// What each action call saw: its session's number and key.
type Seen = Rc<RefCell<Vec<(u64, SessionKey)>>>;

/// `demo.keys` with `demo.key`, which records what the call saw, and `demo.chain`, a plain action
/// that follows up with it.
fn declare() -> Seen {
    crate::entry::service::reset();
    let seen = Seen::default();
    let demo = crate::entry::extension("demo");
    let log = seen.clone();
    let keys = demo.service("keys", |_| Ok(Keyed));
    keys.action(demo.action("key"), move |_, cx, _| {
        log.borrow_mut().push((cx.session(), cx.session_key()));
        ActionResult::Done(None)
    });
    demo.action("chain").run(|cx, _| {
        cx.dispatch("demo.key", crate::chrome::PropMap::new());
        heca_grid_ui::event::Handled::Yes
    });
    seen
}

/// A server with sessions 1 and 2, the second the most recent, and no window.
fn two_sessions() -> ServerLoop {
    let mut server = ServerLoop::from_config(
        &config(""),
        ServerIdentity::for_tests(ServerKind::Background),
        ServerWaker::new(|| {}),
    )
    .unwrap()
    .with_session(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(1), LayoutOptions::default()),
    )
    .with_session(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(2), LayoutOptions::default()),
    );
    register_actions(&mut server);
    server
}

fn key_of(server: &ServerLoop, index: usize) -> SessionKey {
    server.sessions[index].service_ref().key
}

/// A window attached to the session at `index`, with what it was first sent already read.
fn window_in(server: &mut ServerLoop, index: usize) -> crate::server::wire::InProcessClient {
    let window = server.connect();
    window
        .send(ClientMsg::AttachTo {
            name: server.sessions[index].info().name,
        })
        .unwrap();
    server.pump(Instant::now());
    window
}

fn shown(window: &impl ClientLink) -> Vec<serde_json::Value> {
    drain(window)
        .into_iter()
        .filter_map(|message| match message {
            ServerMsg::Service(update) => match update.kind {
                ServiceUpdateKind::Snapshot(value) => Some(value),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// **Every session is made with a key of its own.**
#[test]
fn two_sessions_get_different_keys() {
    declare();
    let server = two_sessions();
    assert_ne!(key_of(&server, 0), key_of(&server, 1));
    assert_eq!(
        key_of(&server, 0).to_string().len(),
        32,
        "128 bits, shown as hex"
    );
}

/// **Renaming, terminating and restarting a session change nothing about its key.**
#[test]
fn rename_terminate_and_restart_keep_the_key() {
    declare();
    let mut server = two_sessions();
    let (name, key) = (server.sessions[0].info().name, key_of(&server, 0));
    let other = key_of(&server, 1);
    server
        .execute_session(SessionCommand::Rename {
            name,
            new_name: "renamed".into(),
        })
        .unwrap();
    assert_eq!(key_of(&server, 0), key, "renamed");
    server
        .execute_session(SessionCommand::Terminate {
            name: "renamed".into(),
        })
        .unwrap();
    assert_eq!(key_of(&server, 0), key, "terminated");
    server
        .execute_session(SessionCommand::Restart {
            name: "renamed".into(),
        })
        .unwrap();
    assert_eq!(key_of(&server, 0), key, "restarted");
    assert_eq!(
        key_of(&server, 1),
        other,
        "and the other session's was never touched"
    );
}

/// **A session made on another server, with the same number, is not the same session.**
#[test]
fn a_new_server_reusing_the_number_gets_a_different_key() {
    declare();
    let (old, new) = (server(&config("")), server(&config("")));
    assert_eq!(
        old.sessions[0].info().id,
        new.sessions[0].info().id,
        "both are session 1"
    );
    assert_ne!(key_of(&old, 0), key_of(&new, 0));
}

/// **A call gets the key of the session it runs in** — a window's call, a script's call, and a
/// script's chain, by the server and by a session — checked in both sessions, so no path can pass by always naming the same one.
#[test]
fn every_path_reports_the_key_of_its_own_session() {
    let seen = declare();
    let mut server = two_sessions();
    let (first, second) = (key_of(&server, 0), key_of(&server, 1));
    let last = |seen: &Seen| seen.borrow().last().copied().unwrap();
    let call = |server: &mut ServerLoop, action: &str| {
        rpc(server, action).try_recv().unwrap();
    };

    call(&mut server, "demo.key");
    assert_eq!(
        last(&seen),
        (2, second),
        "a script's call, the most recent session being 2"
    );
    call(&mut server, "demo.chain");
    assert_eq!(
        last(&seen),
        (2, second),
        "a script's chain, with no window to run it"
    );

    for (index, (id, key)) in [(1, first), (2, second)].into_iter().enumerate() {
        let (reply, answer) = std::sync::mpsc::channel();
        server.sessions[index].begin_rpc("demo.chain".into(), reply);
        assert_eq!(answer.try_recv().unwrap(), "ok");
        assert_eq!(last(&seen), (id, key), "a chain run by session {id} itself");
    }

    let in_first = window_in(&mut server, 0);
    in_first
        .send(ClientMsg::Invoke {
            intent: crate::chrome::Intent::new("demo.key"),
        })
        .unwrap();
    server.pump(Instant::now());
    assert_eq!(
        last(&seen),
        (1, first),
        "a window's call, in the session it is attached to"
    );
    call(&mut server, "demo.key");
    assert_eq!(
        last(&seen),
        (1, first),
        "a script's call, the most recent session now being 1"
    );

    let in_second = window_in(&mut server, 1);
    in_second
        .send(ClientMsg::Invoke {
            intent: crate::chrome::Intent::new("demo.key"),
        })
        .unwrap();
    server.pump(Instant::now());
    assert_eq!(
        last(&seen),
        (2, second),
        "a window's call, in the other session"
    );
    assert_eq!(seen.borrow().len(), 7);
}

/// **Each window's projection is asked with its own session's key.**
#[test]
fn a_projection_gets_the_key_of_the_session_its_window_is_in() {
    declare();
    let mut server = two_sessions();
    let (first, second) = (key_of(&server, 0), key_of(&server, 1));
    let (a, b) = (window_in(&mut server, 0), window_in(&mut server, 1));
    assert_eq!(shown(&a), [serde_json::json!(format!("1 {first}"))]);
    assert_eq!(shown(&b), [serde_json::json!(format!("2 {second}"))]);
}

/// **A key can be stored and read back, and nothing but a real one reads back.**
#[test]
fn a_key_survives_serde_and_rejects_anything_else() {
    let key = SessionKey::generate();
    let text = serde_json::to_string(&key).unwrap();
    assert_eq!(serde_json::from_str::<SessionKey>(&text).unwrap(), key);
    for bad in [
        "\"\"",
        "\"abc\"",
        &format!("\"{}zz\"", &key.to_string()[..30]),
        &format!("\"{key}00\""),
        "7",
    ] {
        assert!(serde_json::from_str::<SessionKey>(bad).is_err(), "{bad}");
    }
}
