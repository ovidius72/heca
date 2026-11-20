//! A real listener in front of a real server loop, reached the way a script and a window reach it.

use super::*;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use heca_config::settings::ConnectionConfig;
use heca_core::layout::{LayoutOptions, Session, SessionId};
use crate::server::{ServerState, ServerWaker, runtime::channel, wire::{ClientLink, ClientMsg, ServerMsg, await_message}};

/// A server loop on its own thread, with a listener at a fresh address. `kill_server` ends it.
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
            let _ = request_to(&self.address, "kill_server");
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

/// `heca --kill-server`: `kill_server` is answered, and the server ends.
#[test]
fn kill_server_is_answered_and_ends_the_server() {
    let mut served = Served::new();
    assert_eq!(request_to(&served.address, "kill_server").unwrap().trim_end(), "heca: server stopped");
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

/// A window of another version is refused with an answer it can read, and the server keeps serving.
#[test]
fn a_window_of_another_version_is_refused_and_the_server_keeps_serving() {
    let served = Served::new();
    let mut old = UnixStream::connect(&served.address).unwrap();
    old.write_all(b"@heca-attach 1\n").unwrap();
    old.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut answer = String::new();
    BufReader::new(old).read_line(&mut answer).unwrap();
    assert_eq!(answer, "@heca-attach refused 2\n");
    assert!(request_to(&served.address, "no_such_action").unwrap().starts_with("error:"));
}

/// A server that does not answer the attach line with "ok" — an older one takes it for a script line
/// — stops the window with a message saying what to do.
#[cfg(unix)]
#[test]
fn a_server_of_another_version_stops_the_window_with_what_to_do() {
    let folder = tempfile::tempdir().unwrap();
    let address = folder.path().join("old.sock");
    let listener = std::os::unix::net::UnixListener::bind(&address).unwrap();
    let older = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut line = String::new();
        BufReader::new(stream.try_clone().unwrap()).read_line(&mut line).unwrap();
        stream.write_all(b"error: unknown action\n").unwrap();
    });
    let wake = ServerWaker::new(|| {});
    let error = crate::server::wire::socket::SocketClient::connect(&address, ConnectionConfig::default(), wake)
        .map(|_| ()).unwrap_err();
    older.join().unwrap();
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(error.to_string().contains("heca --kill-server"), "got: {error}");
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
    assert_eq!(request_to(&served.address, "kill_server").unwrap().trim_end(), "heca: server stopped");
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

/// What a script sent, byte for byte, and the one line the server answered.
fn raw_request(served: &Served, bytes: &[u8]) -> String {
    let mut stream = UnixStream::connect(&served.address).unwrap();
    stream.write_all(bytes).unwrap();
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    answer
}

/// The line terminator ends the request and is not part of it, `\n` or `\r\n`; a line break left
/// inside the line is refused.
#[test]
fn only_the_terminator_is_dropped_from_a_script_line() {
    let served = Served::new();
    assert_eq!(raw_request(&served, b"no_such_action\n").trim_end(), "error: unknown command: no_such_action");
    assert_eq!(raw_request(&served, b"no_such_action\r\n").trim_end(), "error: unknown command: no_such_action");
    let answer = raw_request(&served, b"no_such_action a\rb\n");
    assert!(answer.starts_with("error: cannot read the line"), "got: {answer}");
}

/// A quote that never closes is answered before anything runs.
#[test]
fn an_open_quote_is_answered_with_an_error() {
    let served = Served::new();
    let answer = request_to(&served.address, r#"rename_session_to name="open"#).unwrap();
    assert!(answer.starts_with("error: cannot read the line"), "got: {answer}");
}
