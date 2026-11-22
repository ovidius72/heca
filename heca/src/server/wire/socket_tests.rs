use super::*;
use crate::server::Change;
use crate::server::wire::frame;
use heca_core::layout::{LayoutOptions, Session, SessionId};
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn limits() -> TransportLimits {
    TransportLimits {
        max_frame_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        queue_capacity: NonZeroUsize::new(8).unwrap(),
        ..Default::default()
    }
}

fn wake() -> ServerWaker {
    ServerWaker::new(|| {})
}

fn wait<T>(mut next: impl FnMut() -> io::Result<Option<T>>) -> io::Result<T> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(message) = next()? {
            return Ok(message);
        }
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
    let session = crate::server::sessions::SessionInfo::unnamed(
        SessionId(1),
        &heca_config::settings::SettingsConfig::default().session_name_format,
        None,
    );
    server
        .send(ServerMsg::Attached {
            session,
            client_id: 1,
            window_ordinal: 1,
            content: content.clone(),
        })
        .unwrap();
    match wait(|| client.poll()).unwrap() {
        ServerMsg::Attached {
            content: received, ..
        } => assert_eq!(received, content),
        other => panic!("unexpected reply: {other:?}"),
    }
    server
        .send(ServerMsg::Changed(Change::LayoutChanged))
        .unwrap();
    assert!(matches!(
        wait(|| client.poll()).unwrap(),
        ServerMsg::Changed(Change::LayoutChanged)
    ));
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
    assert!(
        server
            .send(ServerMsg::Changed(Change::LayoutChanged))
            .is_err()
    );
}

#[test]
fn flushing_before_drop_writes_the_rpc_answer_and_detach_in_order() {
    for _ in 0..16 {
        let (a, mut reader) = UnixStream::pair().unwrap();
        reader
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let client = SocketClient::new(a, limits(), wake()).unwrap();
        client
            .send(ClientMsg::RpcReply {
                request_id: 17,
                answer: "ok".into(),
            })
            .unwrap();
        client.send(ClientMsg::Detach).unwrap();
        client.flush().unwrap();
        drop(client);

        assert!(
            matches!(frame::read::<ClientMsg>(&mut reader, limits().max_frame_bytes).unwrap(),
            Some(ClientMsg::RpcReply { request_id: 17, answer }) if answer == "ok")
        );
        assert!(matches!(
            frame::read::<ClientMsg>(&mut reader, limits().max_frame_bytes).unwrap(),
            Some(ClientMsg::Detach)
        ));
        assert!(
            frame::read::<ClientMsg>(&mut reader, limits().max_frame_bytes)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn an_oversized_header_closes_the_connection_before_payload_allocation() {
    use std::io::Write;
    let (a, mut b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    b.write_all(&u32::MAX.to_be_bytes()).unwrap();
    assert_eq!(
        wait(|| client.poll()).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
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
fn a_full_queue_refuses_what_the_server_sends_instead_of_blocking() {
    let (sender, _receiver) = queue::channel(NonZeroUsize::new(1).unwrap());
    sender.send(ServerMsg::Followed { active: true }).unwrap();
    assert_eq!(
        sender
            .send(ServerMsg::Followed { active: false })
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
}

#[test]
fn an_incoming_flood_waits_for_room_and_every_message_arrives_in_order() {
    use crate::server::wire::terminal::ProcessInput;
    let (a, b) = UnixStream::pair().unwrap();
    let limits = TransportLimits {
        queue_capacity: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    let server = SocketServer::new(a, limits, wake()).unwrap();
    let client = SocketClient::new(b, limits, wake()).unwrap();
    let owner = crate::app::backend_store::TerminalOwner::Pane(heca_core::layout::PaneId(1));
    let typed: Vec<u8> = (0..2000u32).map(|i| b'a' + (i % 26) as u8).collect();
    for byte in &typed {
        client
            .send(ClientMsg::Terminal {
                owner: owner.clone(),
                input: ProcessInput::Bytes(vec![*byte]),
            })
            .unwrap();
    }
    let mut received = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while received.len() < typed.len() {
        assert!(
            Instant::now() < deadline,
            "only {} of {} arrived",
            received.len(),
            typed.len()
        );
        match server.poll().unwrap() {
            Some(ClientMsg::Terminal {
                input: ProcessInput::Bytes(bytes),
                ..
            }) => received.extend(bytes),
            Some(other) => panic!("unexpected {other:?}"),
            None => thread::yield_now(),
        }
    }
    assert_eq!(received, typed);
    assert!(server.connection_status().is_ok() && client.connection_status().is_ok());
}

#[test]
fn action_and_asker_are_serialized_together() {
    use crate::input::WmAction;
    use crate::server::Asker;
    use heca_core::layout::{PaneId, Rectangle, Size};
    let (a, b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    let server = SocketServer::new(b, limits(), wake()).unwrap();
    let asker = Asker {
        focused_pane: Some(PaneId(3)),
        workspace: 2,
        column: 1,
        area: Rectangle::from_size(Size::new(800.0, 600.0)),
        cell: (8.0, 14.0),
    };
    client
        .send(ClientMsg::Act {
            action: WmAction::SplitHorizontal,
            asker,
            request_id: None,
            terminal_owner: None,
        })
        .unwrap();
    match wait(|| server.poll()).unwrap() {
        ClientMsg::Act {
            action,
            asker: received,
            ..
        } => {
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
    let waker = ServerWaker::new(move || {
        let _ = tx.send(());
    });
    let server = SocketServer::new(a, limits(), waker).unwrap();
    frame::write(&mut b, &ClientMsg::Attach, limits().max_frame_bytes).unwrap();
    rx.recv_timeout(Duration::from_secs(3))
        .expect("incoming message wakes");
    assert!(matches!(server.poll().unwrap(), Some(ClientMsg::Attach)));
    drop(b);
    rx.recv_timeout(Duration::from_secs(3))
        .expect("disconnect wakes");
    assert!(wait(|| server.poll()).is_err());
}

/// The notification history, typed on both ends of a real socket: records with a still-visible
/// notification carrying an action (an intent with arguments) and a dismissed one.
#[test]
fn the_notification_history_crosses_a_real_socket_as_typed_values() {
    use crate::notification::{NotificationAction, NotificationDraft, NotificationStore};
    let (a, b) = UnixStream::pair().unwrap();
    let client = SocketClient::new(a, limits(), wake()).unwrap();
    let server = SocketServer::new(b, limits(), wake()).unwrap();
    client.send(ClientMsg::NotificationHistory).unwrap();
    assert!(matches!(
        wait(|| server.poll()).unwrap(),
        ClientMsg::NotificationHistory
    ));

    let now = Instant::now();
    let mut store = crate::notification::NotificationStore::with_capacity(10, 10);
    let first = store
        .push(NotificationDraft::new("first").body("whole\nbody"), now)
        .unwrap()
        .notification_id();
    store
        .push(
            NotificationDraft::new("second").action(NotificationAction::new(
                "Retry",
                heca_view::Intent::new("reload_config")
                    .arg("why", heca_view::PropValue::Text("typo".into())),
            )),
            now + Duration::from_millis(1),
        )
        .unwrap();
    store.dismiss(first, now + Duration::from_secs(1));
    let records = store.history_records_for(
        1,
        now + Duration::from_secs(2),
        std::time::SystemTime::now(),
    );
    assert_eq!(records.len(), 2);
    let _ = NotificationStore::new(0);

    server
        .send(ServerMsg::NotificationHistory(records.clone()))
        .unwrap();
    match wait(|| client.poll()).unwrap() {
        ServerMsg::NotificationHistory(received) => assert_eq!(received, records),
        other => panic!("unexpected reply: {other:?}"),
    }
}

/// **Closing a connection whose reader is waiting for room returns** — the reader is not reading
/// the socket, so shutting the socket alone would leave it, and the close, waiting for ever.
#[test]
fn dropping_a_peer_whose_reader_waits_for_room_returns() {
    let (a, mut b) = UnixStream::pair().unwrap();
    let limits = TransportLimits {
        queue_capacity: NonZeroUsize::new(1).unwrap(),
        ..limits()
    };
    let server = SocketServer::new(a, limits, wake()).unwrap();
    for _ in 0..3 {
        frame::write(&mut b, &ClientMsg::Attach, limits.max_frame_bytes).unwrap();
    }
    // Nobody polls: the reader fills the queue and waits for room.
    thread::sleep(Duration::from_millis(50));
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        drop(server);
        let _ = done.send(());
    });
    assert!(
        finished.recv_timeout(Duration::from_secs(5)).is_ok(),
        "the close waited for room nobody makes"
    );
}
