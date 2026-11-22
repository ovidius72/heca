//! The sessions, as services read them: every context hands the same list, and `session.changed`
//! reaches every listener — and the scripts' stream — from the one place it is raised.

use super::service_tests::{config, register_actions, rpc};
use super::*;
use crate::entry::service::{
    ActionResult, ProjectCx, Projection, ServerEvent, Service, ServiceCx, ServiceResult,
    SessionChange, SessionKey, SessionState,
};
use crate::server::identity::{ServerIdentity, ServerKind};
use heca_core::layout::LayoutOptions;
use std::cell::RefCell;

struct Quiet;

impl Service for Quiet {
    type Message = ();
    type Snapshot = ();
    type Delta = ();
    const SCHEMA_VERSION: u32 = 1;

    fn receive(&mut self, _: &mut ServiceCx<'_, ()>, _: ()) -> ServiceResult<()> {
        Ok(())
    }

    fn project(&self, _: &ProjectCx<'_>) -> Option<Projection<(), ()>> {
        None
    }
}

/// What the listener heard, as `(kind, session name, how many sessions after)`, and what the
/// `demo.where` action read: its own session's folder and the names on the same project.
#[derive(Default)]
struct Heard {
    events: Vec<(SessionChange, String, usize)>,
    every: usize,
    read: Option<(Option<std::path::PathBuf>, Vec<String>)>,
}

fn declare() -> Rc<RefCell<Heard>> {
    crate::entry::service::reset();
    let heard = Rc::new(RefCell::new(Heard::default()));
    let demo = crate::entry::extension("demo");
    let rooms = demo.service("rooms", |_| Ok(Quiet));
    let log = heard.clone();
    rooms.on("session.changed", move |_, _, ev| {
        let ev = ev
            .as_session()
            .expect("session.changed carries a session event");
        log.borrow_mut().events.push((
            ev.kind(),
            ev.session().name().to_owned(),
            ev.sessions().iter().count(),
        ));
    });
    let log = heard.clone();
    rooms.on("*", move |_, _, ev: &ServerEvent| {
        assert_eq!(ev.name(), "session.changed");
        log.borrow_mut().every += 1;
    });
    let log = heard.clone();
    rooms.action(demo.action("where"), move |_, cx, _| {
        let mine: SessionKey = cx.session_key();
        let folder = cx
            .sessions()
            .folder_of(mine)
            .map(std::path::Path::to_path_buf);
        let names = cx
            .sessions()
            .same_project(mine)
            .map(|s| s.name().to_owned())
            .collect();
        log.borrow_mut().read = Some((folder, names));
        ActionResult::Done(None)
    });
    heard
}

fn one_session() -> ServerLoop {
    let mut server = ServerLoop::from_config(
        &config(""),
        ServerIdentity::for_tests(ServerKind::Background),
        ServerWaker::new(|| {}),
    )
    .unwrap()
    .with_session(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(1), LayoutOptions::default()),
    );
    register_actions(&mut server);
    server
}

fn create(server: &mut ServerLoop, folder: &std::path::Path, name: &str) {
    server
        .execute_session(SessionCommand::Create {
            folder: folder.into(),
            name: Some(name.into()),
            label: None,
        })
        .unwrap();
}

/// **Every change is heard once, with what happened, the session and the list after it** — and a
/// `"*"` listener hears the same events.
#[test]
fn a_listener_hears_every_session_change_with_its_kind() {
    let heard = declare();
    let mut server = one_session();
    let dir = tempfile::tempdir().unwrap();
    create(&mut server, dir.path(), "a");
    server
        .execute_session(SessionCommand::Rename {
            name: "a".into(),
            new_name: "b".into(),
        })
        .unwrap();
    server
        .execute_session(SessionCommand::Terminate { name: "b".into() })
        .unwrap();
    server
        .execute_session(SessionCommand::Terminate { name: "b".into() })
        .unwrap();
    server
        .execute_session(SessionCommand::Restart { name: "b".into() })
        .unwrap();
    server
        .execute_session(SessionCommand::Delete { name: "b".into() })
        .unwrap();
    server.execute_session(SessionCommand::List).unwrap();

    let heard = heard.borrow();
    assert_eq!(
        heard.events,
        [
            (SessionChange::Created, "a".into(), 2),
            (SessionChange::Renamed, "b".into(), 2),
            (SessionChange::Terminated, "b".into(), 2),
            (SessionChange::Restarted, "b".into(), 2),
            (SessionChange::Deleted, "b".into(), 1),
        ],
        "terminating a stopped session and listing change nothing, so they say nothing"
    );
    assert_eq!(heard.every, 5);
}

/// **An action reads its own session's folder and the sessions on the same project.**
#[test]
fn an_action_reads_its_sessions_project() {
    let heard = declare();
    let mut server = one_session();
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    create(&mut server, dir.path(), "feature-a");
    create(&mut server, elsewhere.path(), "other");
    create(&mut server, dir.path(), "feature-b");
    rpc(&mut server, "demo.where").try_recv().unwrap();

    let read = heard.borrow().read.clone().expect("the action ran");
    assert_eq!(
        read.0.as_deref(),
        Some(dir.path().canonicalize().unwrap().as_path()),
        "the most recent session, feature-b, is on the project folder"
    );
    assert_eq!(read.1, ["feature-a", "feature-b"]);
}

/// **The scripts' stream gets the same `session.changed`**, with the kind and the session.
#[test]
fn the_scripts_stream_carries_session_changed() {
    declare();
    let mut server = one_session();
    let (sender, lines) = std::sync::mpsc::sync_channel(16);
    server.events.subscribe(sender);
    let dir = tempfile::tempdir().unwrap();
    create(&mut server, dir.path(), "a");
    server
        .execute_session(SessionCommand::Terminate { name: "a".into() })
        .unwrap();

    let events: Vec<serde_json::Value> = lines
        .try_iter()
        .map(|line| serde_json::from_str(&line).unwrap())
        .filter(|event: &serde_json::Value| event.get("SessionChanged").is_some())
        .collect();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["SessionChanged"]["kind"], "created");
    assert_eq!(events[1]["SessionChanged"]["kind"], "terminated");
    assert_eq!(events[1]["SessionChanged"]["session"]["name"], "a");
    assert_eq!(
        events[1]["SessionChanged"]["session"]["state"],
        serde_json::json!(SessionState::Terminated)
    );
}
