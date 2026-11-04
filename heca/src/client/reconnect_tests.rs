//! Reconnect against the real owner loop, without a window, daemon or process.

use super::*;
use crate::server::{ServerState, run::ServerLoop, sessions::{SessionCommand, SessionInfo, SessionReply},
    wire::InProcessClient};
use heca_core::layout::{Session, SessionId};
use std::thread::JoinHandle;

type Tick = mpsc::Sender<Option<mpsc::SyncSender<()>>>;

struct Owner {
    tick: Tick,
    thread: Option<JoinHandle<()>>,
}

struct Pumped {
    link: InProcessClient,
    tick: Tick,
}

impl Pumped {
    fn advance(&self) -> io::Result<()> {
        let (done, receive) = mpsc::sync_channel(1);
        self.tick.send(Some(done)).map_err(io::Error::other)?;
        receive.recv().map_err(io::Error::other)
    }
}

impl ClientLink for Pumped {
    fn send(&self, message: ClientMsg) -> io::Result<()> {
        self.link.send(message)?;
        self.advance()
    }
    fn poll(&self) -> io::Result<Option<ServerMsg>> {
        self.advance()?;
        self.link.poll()
    }
}

impl Owner {
    fn new() -> (Self, Pumped, Pumped) {
        let (tick, ticks) = mpsc::channel::<Option<mpsc::SyncSender<()>>>();
        let (ready, receive) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            let mut server = ServerLoop::new(ServerState::for_tests(Default::default()),
                Session::new(SessionId(100), Default::default()));
            ready.send((server.connect(), server.connect())).unwrap();
            while let Ok(Some(done)) = ticks.recv() {
                server.pump(Instant::now());
                let _ = done.send(());
            }
            server.shutdown();
        });
        let (window, control) = receive.recv().unwrap();
        (Self { tick: tick.clone(), thread: Some(thread) },
            Pumped { link: window, tick: tick.clone() }, Pumped { link: control, tick })
    }

    fn stop(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = self.tick.send(None);
            thread.join().unwrap();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) { self.stop(); }
}

fn command(control: &Pumped, command: SessionCommand) -> SessionReply {
    control.send(ClientMsg::Session { request_id: 7, command }).unwrap();
    while let Some(message) = control.poll().unwrap() {
        if let ServerMsg::Sessions { request_id: 7, result } = message { return result.unwrap(); }
    }
    panic!("the owner did not answer the session command");
}

fn create(control: &Pumped, root: &std::path::Path, name: &str) -> SessionInfo {
    match command(control, SessionCommand::Create { folder: root.join(name), name: Some(name.into()), label: None }) {
        SessionReply::Updated(session) => session,
        reply => panic!("unexpected create reply: {reply:?}"),
    }
}

fn attached(window: Pumped) -> ClientState {
    let mut client = super::tests::client(window);
    client.link.send(ClientMsg::AttachTo { name: "A".into() }).unwrap();
    assert!(client.poll().unwrap().session_attached);
    assert!(client.projection_ready());
    client
}

#[test]
fn reconnect_uses_the_intended_name_when_another_session_reuses_its_id_and_is_last() {
    let root = tempfile::tempdir().unwrap();
    let (mut old, window, control) = Owner::new();
    let original = create(&control, root.path(), "A");
    let mut client = attached(window);
    let stale_outbox = client.outbox.clone();
    old.stop();
    assert!(client.poll().is_err());
    assert!(client.failed);

    let (_new, window, control) = Owner::new();
    let other = create(&control, root.path(), "B");
    let restored = create(&control, root.path(), "A");
    assert_eq!(other.id, original.id, "the old numeric identity now belongs to another session");
    assert_ne!(restored.id, original.id);
    control.send(ClientMsg::AttachTo { name: "B".into() }).unwrap();
    while control.poll().unwrap().is_some() {}
    let (_send, ready) = mpsc::sync_channel(1);
    client.reconnect_link(Box::new(window), &ready).unwrap();
    assert!(client.poll().unwrap().session_attached);
    assert_eq!(client.session.id, restored.id);
    assert_eq!(client.session.name, "A");
    assert_eq!(client.session.folder, original.folder);
    assert!(!client.failed);
    assert!(client.projection_ready());
    stale_outbox.send(ClientMsg::Reload);
    assert!(stale_outbox.check().is_err());
    assert_eq!(client.sessions.iter().find(|session| session.name == "B").unwrap().windows, 1);
}

#[test]
fn reconnect_does_not_create_or_attach_a_default_when_the_intended_session_is_missing() {
    let root = tempfile::tempdir().unwrap();
    let (mut old, window, control) = Owner::new();
    let original = create(&control, root.path(), "A");
    let mut client = attached(window);
    old.stop();
    assert!(client.poll().is_err());
    let (_new, window, control) = Owner::new();
    create(&control, root.path(), "B");
    let SessionReply::List(before) = command(&control, SessionCommand::List) else { panic!("not a list") };
    let (_send, ready) = mpsc::sync_channel(1);
    assert!(client.reconnect_link(Box::new(window), &ready).is_err());
    assert!(client.failed);
    assert_eq!(client.session.name, original.name);
    assert_eq!(client.session.folder, original.folder);
    assert!(client.projection_ready(), "the blocking unavailable dialog must remain drawable");
    let SessionReply::List(after) = command(&control, SessionCommand::List) else { panic!("not a list") };
    assert_eq!(after, before);
    assert!(after.iter().all(|session| session.windows == 0));
}
