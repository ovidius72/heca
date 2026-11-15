//! A real listener in front of a real server loop, reached the way a script and a window reach it.

use super::*;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use heca_config::settings::ConnectionConfig;
use heca_core::layout::{LayoutOptions, Session, SessionId};
use crate::server::{ServerState, ServerWaker, runtime::channel, wire::{ClientLink, ClientMsg, ServerMsg, await_message}};

/// A server loop on its own thread, with a listener at a fresh address. `kill-server` ends it.
struct Served {
    address: std::path::PathBuf,
    _folder: tempfile::TempDir,
    listener: Option<RuntimeListener>,
    owner: Option<std::thread::JoinHandle<()>>,
}

impl Served {
    fn new() -> Self {
        let folder = tempfile::tempdir().unwrap();
        let address = folder.path().join("test.sock");
        let config = ConnectionConfig::default();
        let (handle, inbox) = channel(config);
        let owner = std::thread::spawn(move || {
            // The server state holds `Rc`s: it is built on the thread that owns it.
            let state = ServerState::for_tests(Default::default());
            inbox.run(crate::server::run::ServerLoop::new(state, Session::new(SessionId(1), LayoutOptions::default())));
        });
        let listener = RuntimeListener::bind(&address, handle, config).unwrap();
        Self { address, _folder: folder, listener: Some(listener), owner: Some(owner) }
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        // Ask the loop to stop if a test did not, then close the door, then wait for the loop.
        if self.owner.is_some() {
            let _ = request_to(&self.address, crate::server::run::KILL_SERVER);
        }
        self.listener.take();
        if let Some(owner) = self.owner.take() { owner.join().unwrap(); }
    }
}

/// `heca rpc <line>`: a line in, an answer back, over the socket.
#[test]
fn a_script_line_is_answered_over_the_socket() {
    let served = Served::new();
    let answer = request_to(&served.address, "no_such_action").unwrap();
    assert!(answer.starts_with("error:"), "got: {answer}");
}

/// `heca --kill-server`: the one protocol word is answered, and the server ends.
#[test]
fn kill_server_is_answered_and_ends_the_server() {
    let mut served = Served::new();
    assert_eq!(request_to(&served.address, crate::server::run::KILL_SERVER).unwrap().trim_end(), "heca: server stopped");
    served.owner.take().unwrap().join().unwrap();
}

/// A window: the attach line, then framed messages, through the same listener.
#[test]
fn a_window_attaches_over_the_socket() {
    let served = Served::new();
    let (ready, woke) = std::sync::mpsc::sync_channel(1);
    let wake = ServerWaker::new(move || { let _ = ready.try_send(()); });
    let config = ConnectionConfig::default();
    let client = crate::server::wire::socket::SocketClient::connect(&served.address, config, wake).unwrap();
    client.send(ClientMsg::Attach).unwrap();
    let attached = await_message(&client, &woke, config, || {}).unwrap();
    assert!(matches!(attached, ServerMsg::Attached { .. }), "got: {attached:?}");
    client.send(ClientMsg::Detach).unwrap();
}

/// `heca rpc --events`: the preface subscribes, and the stream ends when the server does.
#[test]
fn an_events_subscriber_is_kept_until_the_server_ends() {
    let mut served = Served::new();
    let mut subscriber = UnixStream::connect(&served.address).unwrap();
    subscriber.write_all(runtime::EVENTS_PREFACE).unwrap();
    subscriber.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    // Subscribing is queued on the server like any request; once a later line is answered it has
    // been taken in.
    assert!(request_to(&served.address, "no_such_action").unwrap().starts_with("error:"));
    assert_eq!(request_to(&served.address, crate::server::run::KILL_SERVER).unwrap().trim_end(), "heca: server stopped");
    served.owner.take().unwrap().join().unwrap();
    let lines = BufReader::new(subscriber).lines().collect::<Result<Vec<_>, _>>().unwrap();
    assert!(lines.iter().all(|line| !line.is_empty()), "got: {lines:?}");
}

/// The address is held by the first server: a second cannot take it, and the first still answers.
#[test]
fn a_second_server_on_the_same_address_is_refused() {
    let served = Served::new();
    let (handle, _inbox) = channel(ConnectionConfig::default());
    assert!(RuntimeListener::bind(&served.address, handle, ConnectionConfig::default()).is_err());
    assert!(request_to(&served.address, "no_such_action").unwrap().starts_with("error:"));
}
