use super::*;
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use heca_core::layout::{LayoutOptions, Session, SessionId};
use crate::server::Change;

fn limits() -> TransportLimits {
    TransportLimits {
        max_frame_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        queue_capacity: NonZeroUsize::new(8).unwrap(),
        ..Default::default()
    }
}

fn wake() -> ServerWaker { ServerWaker::new(|| {}) }

fn wait<T>(mut next: impl FnMut() -> io::Result<Option<T>>) -> io::Result<T> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(message) = next()? { return Ok(message); }
        assert!(Instant::now() < deadline, "socket worker did not respond");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_same_protocol_crosses_a_real_socket() {
    let (a, b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    let server = SocketServer::new(b, limits(), wake()).unwrap();
    client.send(ClientMsg::Attach).unwrap();
    assert!(matches!(wait(|| server.poll()).unwrap(), ClientMsg::Attach));
    let content = Session::new(SessionId(1), LayoutOptions::default()).content();
    server.send(ServerMsg::Attached { content: content.clone() }).unwrap();
    match wait(|| client.poll()).unwrap() {
        ServerMsg::Attached { content: received } => assert_eq!(received, content),
        other => panic!("unexpected reply: {other:?}"),
    }
    server.send(ServerMsg::Changed(Change::LayoutChanged)).unwrap();
    assert!(matches!(wait(|| client.poll()).unwrap(), ServerMsg::Changed(Change::LayoutChanged)));
    client.send(ClientMsg::Detach).unwrap();
    assert!(matches!(wait(|| server.poll()).unwrap(), ClientMsg::Detach));
}

#[test]
fn dropping_a_socket_peer_closes_and_joins_its_workers() {
    let (a, b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    let server = SocketServer::new(b, limits(), wake()).unwrap();
    drop(client);
    assert!(wait(|| server.poll()).is_err());
    assert!(server.send(ServerMsg::Changed(Change::LayoutChanged)).is_err());
}

#[test]
fn an_oversized_header_closes_the_connection_before_payload_allocation() {
    use std::io::Write;
    let (a, mut b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    b.write_all(&u32::MAX.to_be_bytes()).unwrap();
    assert_eq!(wait(|| client.poll()).unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn dropping_a_peer_interrupts_an_incomplete_frame() {
    use std::io::Write;
    let (a, mut b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    b.write_all(&[0]).unwrap();
    drop(client);
    // Drop joins the blocked reader; reaching this line proves shutdown interrupted its read.
}

#[test]
fn a_full_queue_reports_backpressure_instead_of_blocking() {
    let (sender, _receiver) = queue::channel(NonZeroUsize::new(1).unwrap());
    sender.send(ClientMsg::Attach).unwrap();
    assert_eq!(sender.send(ClientMsg::Detach).unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn an_incoming_flood_closes_instead_of_dropping_messages_silently() {
    let (a, mut b) = UnixStream::pair().unwrap();
    let limits = TransportLimits { queue_capacity: NonZeroUsize::new(1).unwrap(), ..limits() };
    let server = SocketServer::new(a, limits, wake()).unwrap();
    frame::write(&mut b, &ClientMsg::Attach, limits.max_frame_bytes).unwrap();
    frame::write(&mut b, &ClientMsg::Attach, limits.max_frame_bytes).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while server.connection_status().is_ok() {
        assert!(Instant::now() < deadline, "flood did not close connection");
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(server.poll().unwrap(), Some(ClientMsg::Attach)));
    assert_eq!(server.poll().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn action_and_asker_are_serialized_together() {
    use crate::input::WmAction;
    use crate::server::Asker;
    use heca_core::layout::{PaneId, Rectangle, Size};
    let (a, b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    let server = SocketServer::new(b, limits(), wake()).unwrap();
    let asker = Asker { focused_pane: Some(PaneId(3)), workspace: 2, column: 1,
        area: Rectangle::from_size(Size::new(800.0, 600.0)), cell: (8.0, 14.0) };
    client.send(ClientMsg::Act { action: WmAction::SplitHorizontal, asker, request_id: None, terminal_owner: None }).unwrap();
    match wait(|| server.poll()).unwrap() {
        ClientMsg::Act { action, asker: received, .. } => {
            assert_eq!(action, WmAction::SplitHorizontal);
            assert_eq!(received, asker);
        }
        other => panic!("unexpected message: {other:?}"),
    }
}

#[test]
fn incoming_messages_and_disconnects_wake_the_owner() {
    let (a, mut b) = UnixStream::pair().unwrap();
    let (tx, rx) = mpsc::channel();
    let waker = ServerWaker::new(move || { let _ = tx.send(()); });
    let server = SocketServer::new(a, limits(), waker).unwrap();
    frame::write(&mut b, &ClientMsg::Attach, limits().max_frame_bytes).unwrap();
    rx.recv_timeout(Duration::from_secs(3)).expect("incoming message wakes");
    assert!(matches!(server.poll().unwrap(), Some(ClientMsg::Attach)));
    drop(b);
    rx.recv_timeout(Duration::from_secs(3)).expect("disconnect wakes");
    assert!(wait(|| server.poll()).is_err());
}
