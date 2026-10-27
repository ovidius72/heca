use super::*;
use std::time::Duration;
use heca_core::layout::{LayoutOptions, Session, SessionId};
use crate::server::{ServerState, wire::{ClientLink, ClientMsg, ServerMsg, in_process_waking}};

#[test]
fn the_owner_loop_survives_a_client_detaching() {
    let (handle, inbox) = channel(ConnectionConfig::default());
    let worker = std::thread::spawn(move || {
        // ServerState contains Rc values: construct it here, never send it from a window.
        let state = ServerState::for_tests(Default::default());
        let session = Session::new(SessionId(1), LayoutOptions::default());
        inbox.run(ServerLoop::new(state, session));
    });
    for _ in 0..2 {
        let (reply_tx, reply_rx) = mpsc::channel();
        let reply_wake = ServerWaker::new(move || { let _ = reply_tx.send(()); });
        let (client, server) = in_process_waking(handle.waker(), reply_wake);
        handle.connect(server).unwrap();
        client.send(ClientMsg::Attach).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(message) = client.poll().unwrap() {
                assert!(matches!(message, ServerMsg::Attached { .. }));
                break;
            }
            assert!(Instant::now() < deadline, "owner did not process attach");
            reply_rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).expect("reply woke client");
        }
        client.send(ClientMsg::Detach).unwrap();
    }
    handle.stop().unwrap();
    worker.join().unwrap();
}

#[test]
fn wakeups_coalesce_when_the_inbox_is_full() {
    let config = ConnectionConfig {
        queue_capacity: std::num::NonZeroUsize::new(1).unwrap(),
        ..ConnectionConfig::default()
    };
    let (handle, inbox) = channel(config);
    let wake = handle.waker();
    wake.wake();
    wake.wake();
    assert!(matches!(inbox.0.try_recv().unwrap(), Event::Wake));
    assert!(matches!(inbox.0.try_recv(), Err(mpsc::TryRecvError::Empty)));
    handle.stop().unwrap();
    assert!(matches!(inbox.0.try_recv().unwrap(), Event::Stop));
}

#[test]
fn a_server_deadline_wakes_an_otherwise_idle_owner() {
    let (handle, inbox) = channel(ConnectionConfig::default());
    let (reply_tx, reply_rx) = mpsc::channel();
    let (client, link) = in_process_waking(handle.waker(), ServerWaker::new(move || {
        let _ = reply_tx.send(());
    }));
    handle.connect(link).unwrap();
    client.send(ClientMsg::Attach).unwrap();
    let worker = std::thread::spawn(move || {
        let mut state = ServerState::for_tests(Default::default());
        state.execute(
            crate::server::ServerAction::Configure(crate::server::NotificationSettings {
                auto_dismiss: Duration::from_millis(100),
                mode: Default::default(),
                max_visible: 5,
            }),
            Instant::now(),
        );
        state.execute(
            crate::server::ServerAction::Raise(
                crate::notification::NotificationDraft::new("timer")
                    .auto_dismiss(Duration::from_millis(100)),
            ),
            Instant::now(),
        );
        let session = Session::new(SessionId(1), LayoutOptions::default());
        inbox.run(ServerLoop::new(state, session));
    });
    // Attach sends the current projection (the "timer" card). What proves the *deadline* woke the
    // idle owner, with no further client traffic, is the later projection that comes back empty
    // once the card expired.
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if matches!(client.poll().unwrap(), Some(ServerMsg::Notifications(views)) if views.is_empty()) {
            break;
        }
        reply_rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("the notification deadline must wake the owner without client traffic");
    }
    handle.stop().unwrap();
    worker.join().unwrap();
}

/// **`heca --kill-server` ends the owner loop on its own.** The kill request comes in over the same
/// rpc channel a script uses; the server answers one line and the loop stops without anyone calling
/// `handle.stop()`, so the process (and its panes and terminals) go with it.
#[test]
fn kill_server_ends_the_owner_loop() {
    let (handle, inbox) = channel(ConnectionConfig::default());
    let worker = std::thread::spawn(move || {
        let state = ServerState::for_tests(Default::default());
        let session = Session::new(SessionId(1), LayoutOptions::default());
        inbox.run(ServerLoop::new(state, session));
    });
    let (reply_tx, reply_rx) = mpsc::channel();
    handle
        .rpc(crate::server::run::KILL_SERVER.to_string(), reply_tx)
        .unwrap();
    assert_eq!(
        reply_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
        "heca: server stopped\n"
    );
    // No handle.stop(): the loop must end itself, so joining returns.
    worker.join().unwrap();
}
