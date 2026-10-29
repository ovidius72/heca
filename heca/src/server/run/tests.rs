use super::*;
use crate::server::Change;
use crate::server::wire::ClientLink;
use heca_core::layout::{LayoutOptions, SessionId};

fn a_loop() -> ServerLoop {
    let session = Session::new(SessionId(1), LayoutOptions::default());
    ServerLoop::new(ServerState::for_tests(Default::default()), session)
}

#[test]
fn deleting_the_last_session_keeps_actual_use_order_instead_of_creation_order() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    create(&mut server, &folder.path().join("c"), "C");
    let client = server.connect();
    client
        .send(ClientMsg::AttachTo {
            name: b.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    drain(&client);
    client
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    drain(&client);
    server
        .execute_session(SessionCommand::Delete { name: a.name })
        .unwrap();
    let next = server.connect();
    next.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(next.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == b.id)
    );
}

#[test]
fn selecting_another_session_reuses_the_same_connection_id_and_preserves_other_clients() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    let moving = server.connect();
    let staying = server.connect();
    moving
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    staying
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    drain(&moving);
    drain(&staying);
    let id = server.clients[0].id;
    moving.send(ClientMsg::AttachTo { name: b.name }).unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(moving.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == b.id)
    );
    assert_eq!(server.clients[0].id, id);
    assert_eq!(server.next_client, 3);
    assert_eq!(
        server.sessions[server.named(&a.name).unwrap()]
            .info()
            .windows,
        1
    );
    assert!(
        drain(&staying)
            .iter()
            .all(|message| matches!(message, ServerMsg::Sessions { .. }))
    );
}

#[test]
fn an_unattached_extensions_session_action_uses_the_existing_registry_gate() {
    let folder = tempfile::tempdir().unwrap();
    let destination = folder.path().join("extension");
    let mut server = a_loop();
    let mut registry = crate::actions::ActionRegistry::new();
    let mut catalog = crate::actions::ActionCatalog::with_builtins();
    let create = crate::chrome::Intent::new("create_session")
        .arg(
            "folder",
            crate::chrome::PropValue::Text(destination.display().to_string()),
        )
        .arg(
            "name",
            crate::chrome::PropValue::Text("extension-session".into()),
        );
    crate::actions::register_server_dynamic(
        &mut registry,
        &mut catalog,
        crate::actions::ActionMeta::new("test.make_session"),
        Rc::new(move |_, _| vec![create.clone()]),
    )
    .unwrap();
    server.install_actions(registry, catalog);
    assert_eq!(server.rpc("test.make_session"), "ok");
    assert!(server.named("extension-session").is_ok());
    assert_eq!(
        server.rpc("terminate_session session=extension-session"),
        "ok"
    );
    assert_eq!(
        server.sessions[server.named("extension-session").unwrap()]
            .info()
            .status,
        SessionStatus::Terminated
    );
    assert!(server.rpc("rename_session_to").starts_with("error:"));
}

/// A client that attaches is sent the whole content to draw from — the server's current session,
/// exactly as [`Session::content`] projects it.
#[test]
fn attaching_sends_the_content() {
    let mut server = a_loop();
    let client = server.connect();
    let expected = server_content(&server);

    client.send(ClientMsg::Attach).expect("connected");
    server.pump(Instant::now());

    match client.poll().expect("connected") {
        Some(ServerMsg::Attached { content, .. }) => assert_eq!(content, expected),
        other => panic!("attach did not return the content: {other:?}"),
    }
}

/// A detached client stops being sent to: after it detaches, a later change reaches no one and
/// the loop does not panic reaching for a client that is gone.
#[test]
fn a_detached_client_is_dropped() {
    let mut server = a_loop();
    let client = server.connect();
    client.send(ClientMsg::Detach).expect("connected");
    server.pump(Instant::now());
    assert!(
        server.clients.is_empty(),
        "the detached client was not dropped"
    );
}

#[test]
fn missing_terminal_answers_instead_of_leaving_request_pending() {
    let mut server = a_loop();
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    let attached = drain(&client);
    assert!(attached.iter().any(|message| matches!(message, ServerMsg::Attached { .. })));
    assert!(attached.iter().any(|message| matches!(message, ServerMsg::Notifications(_))));
    assert!(attached.iter().any(|message| matches!(message, ServerMsg::Feedback { .. })));
    assert!(attached.iter().any(|message| matches!(message,
        ServerMsg::Sessions { result: Ok(SessionReply::List(_)), .. })));
    client
        .send(ClientMsg::Act {
            action: crate::input::WmAction::TerminalKill {
                terminal: Some(1),
                pane_id: None,
            },
            asker: test_asker(),
            request_id: Some(7),
            terminal_owner: Some(crate::app::backend_store::TerminalOwner::Pane(
                heca_core::layout::PaneId(999),
            )),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(client.poll().unwrap(), Some(ServerMsg::Reply { request_id: 7, changes })
        if changes == vec![Change::Refused(super::super::Refusal::TerminalUnavailable)])
    );
    assert_eq!(server.clients.len(), 1);
}

#[test]
fn kill_server_drops_every_client_and_asks_the_loop_to_stop() {
    let mut server = a_loop();
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert_eq!(server.clients.len(), 1);

    let (reply, answer) = std::sync::mpsc::channel();
    server.begin_rpc(super::KILL_SERVER.to_string(), reply);
    assert!(server.wants_shutdown(), "the server was asked to stop");
    assert!(server.clients.is_empty(), "kill-server drops every client");
    assert_eq!(answer.recv().unwrap(), "heca: server stopped\n");
}

#[test]
fn a_dropped_client_is_removed_without_a_detach_message() {
    let mut server = a_loop();
    let client = server.connect();
    drop(client);
    server.pump(Instant::now());
    assert!(server.clients.is_empty());
}

#[test]
fn an_rpc_answer_received_before_eof_is_delivered_before_disconnect() {
    let mut server = a_loop();
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    drain(&client);

    let (reply, answer) = std::sync::mpsc::channel();
    server.begin_rpc("detach_session".into(), reply);
    let Some(ServerMsg::Rpc { request_id, .. }) = client.poll().unwrap() else {
        panic!("RPC was not forwarded to the attached window");
    };
    client.send(ClientMsg::RpcReply { request_id, answer: "ok".into() }).unwrap();
    drop(client);
    server.pump(Instant::now());

    assert_eq!(answer.try_recv().unwrap(), "ok");
    assert!(server.clients.is_empty());
}

#[test]
fn an_action_before_attach_closes_the_connection() {
    let mut server = a_loop();
    let client = server.connect();
    client
        .send(ClientMsg::Act {
            action: crate::input::WmAction::ReloadConfig,
            asker: test_asker(),
            request_id: None,
            terminal_owner: None,
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(server.clients.is_empty());
    assert!(client.poll().is_err());
}

#[test]
fn detach_prevents_later_queued_actions_from_running() {
    let mut server = a_loop();
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    client.send(ClientMsg::Detach).unwrap();
    client
        .send(ClientMsg::Act {
            action: crate::input::WmAction::ReloadConfig,
            asker: test_asker(),
            request_id: None,
            terminal_owner: None,
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(server.clients.is_empty());
}

fn test_asker() -> super::super::Asker {
    use heca_core::layout::{Rectangle, Size};
    super::super::Asker {
        focused_pane: None,
        workspace: 0,
        column: 0,
        area: Rectangle::from_size(Size::new(800.0, 600.0)),
        cell: (8.0, 14.0),
    }
}

#[cfg(unix)]
#[test]
fn a_socket_attaches_through_the_same_server_loop() {
    use super::super::wire::socket::{SocketClient, SocketServer, TransportLimits};
    use std::num::NonZeroUsize;
    use std::os::unix::net::UnixStream;
    use std::time::Duration;
    let mut server = a_loop();
    let (a, b) = UnixStream::pair().unwrap();
    let limits = TransportLimits {
        max_frame_bytes: NonZeroUsize::new(65536).unwrap(),
        queue_capacity: NonZeroUsize::new(8).unwrap(),
        ..Default::default()
    };
    let (wake_tx, wake_rx) = std::sync::mpsc::channel();
    let wake = super::super::ServerWaker::new(move || {
        let _ = wake_tx.send(());
    });
    server.add_client(SocketServer::new(a, limits, wake).unwrap());
    let (client_tx, client_rx) = std::sync::mpsc::channel();
    let client = SocketClient::new(
        b,
        limits,
        super::super::ServerWaker::new(move || {
            let _ = client_tx.send(());
        }),
    )
    .unwrap();
    client.send(ClientMsg::Attach).unwrap();
    wake_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("server woke");
    server.pump(Instant::now());
    client_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("client woke");
    match client.poll().unwrap() {
        Some(ServerMsg::Attached { content, .. }) => assert_eq!(content, server_content(&server)),
        other => panic!("unexpected reply: {other:?}"),
    }
    client.send(ClientMsg::Detach).unwrap();
    wake_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("detach woke server");
    server.pump(Instant::now());
    assert!(server.clients.is_empty());
}

fn server_content(server: &ServerLoop) -> heca_core::layout::SessionContent {
    server.sessions[0].content()
}

fn create(server: &mut ServerLoop, folder: &std::path::Path, name: &str) -> SessionInfo {
    match server
        .execute_session(SessionCommand::Create {
            folder: folder.into(),
            name: Some(name.into()),
            label: None,
        })
        .unwrap()
    {
        SessionReply::Updated(info) => info,
        other => panic!("create returned {other:?}"),
    }
}

fn drain(client: &impl ClientLink) -> Vec<ServerMsg> {
    let mut messages = Vec::new();
    while let Some(message) = client.poll().unwrap() {
        messages.push(message);
    }
    messages
}

#[test]
fn clients_choose_sessions_without_a_second_client_identity() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "project A");
    let b = create(&mut server, &folder.path().join("b"), "project B");
    let first = server.connect();
    let second = server.connect();
    let third = server.connect();
    first
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    second
        .send(ClientMsg::AttachTo {
            name: b.name.clone(),
        })
        .unwrap();
    third
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(first.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == a.id)
    );
    assert!(
        matches!(second.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == b.id)
    );
    assert!(
        matches!(third.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == a.id)
    );
    assert_eq!(
        server.clients.iter().map(|c| c.id).collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn notifications_do_not_cross_sessions_even_when_clients_share_one_server() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    let first = server.connect();
    let mirror = server.connect();
    let other = server.connect();
    first
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    mirror.send(ClientMsg::AttachTo { name: a.name }).unwrap();
    other.send(ClientMsg::AttachTo { name: b.name }).unwrap();
    server.pump(Instant::now());
    drain(&first);
    drain(&mirror);
    drain(&other);
    first
        .send(ClientMsg::Notify(
            crate::server::NotificationCommand::Raise(crate::notification::NotificationDraft::new(
                "only A",
            )),
        ))
        .unwrap();
    server.pump(Instant::now());
    assert!(
        drain(&first)
            .iter()
            .any(|m| matches!(m, ServerMsg::Notifications(v) if v.len() == 1))
    );
    assert!(
        drain(&mirror)
            .iter()
            .any(|m| matches!(m, ServerMsg::Notifications(v) if v.len() == 1))
    );
    assert!(drain(&other).iter().all(|message| matches!(
        message,
        ServerMsg::Sessions {
            result: Ok(SessionReply::List(_)),
            ..
        }
    )));
}

#[test]
fn terminating_keeps_clients_and_definition_until_explicit_restart() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    let first = server.connect();
    let other = server.connect();
    first
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    other.send(ClientMsg::AttachTo { name: b.name }).unwrap();
    server.pump(Instant::now());
    drain(&first);
    drain(&other);
    server
        .execute_session(SessionCommand::Terminate {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(first.poll().unwrap(), Some(ServerMsg::SessionTerminated { session })
        if session.id == a.id && session.status == SessionStatus::Terminated)
    );
    assert!(drain(&other).iter().all(|message| matches!(
        message,
        ServerMsg::Sessions {
            result: Ok(SessionReply::List(_)),
            ..
        }
    )));
    assert_eq!(server.clients.len(), 2);
    let i = server.named(&a.name).unwrap();
    assert_eq!(server.sessions[i].info().status, SessionStatus::Terminated);
    drain(&first);
    server
        .execute_session(SessionCommand::Restart { name: a.name })
        .unwrap();
    assert!(
        matches!(first.poll().unwrap(), Some(ServerMsg::Attached { session, .. })
        if session.id == a.id && session.status == SessionStatus::Active)
    );
}

#[test]
fn deleting_removes_the_definition_and_only_its_clients_without_deleting_project_files() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let project = folder.path().join("a");
    let a = create(&mut server, &project, "A");
    std::fs::write(project.join("keep.txt"), "user data").unwrap();
    let b = create(&mut server, &folder.path().join("b"), "B");
    let first = server.connect();
    let other = server.connect();
    first
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    other.send(ClientMsg::AttachTo { name: b.name }).unwrap();
    server.pump(Instant::now());
    drain(&first);
    drain(&other);
    server
        .execute_session(SessionCommand::Delete {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(matches!(first.poll().unwrap(), Some(ServerMsg::SessionDeleted { id }) if id == a.id));
    first.send(ClientMsg::Detach).unwrap();
    server.pump(Instant::now());
    assert!(first.poll().is_err());
    assert!(server.named(&a.name).is_err());
    assert_eq!(server.clients.len(), 1);
    assert!(drain(&other).iter().all(|message| matches!(
        message,
        ServerMsg::Sessions {
            result: Ok(SessionReply::List(_)),
            ..
        }
    )));
    assert_eq!(
        std::fs::read_to_string(project.join("keep.txt")).unwrap(),
        "user data"
    );
}

#[test]
fn duplicate_names_fail_before_creating_a_project_folder() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    create(&mut server, &folder.path().join("first"), "same");
    let unwanted = folder.path().join("must-not-be-created");
    assert!(
        server
            .execute_session(SessionCommand::Create {
                folder: unwanted.clone(),
                name: Some("same".into()),
                label: None,
            })
            .is_err()
    );
    assert!(!unwanted.exists());
}

#[test]
fn plain_attach_chooses_the_last_session_and_unknown_attach_creates_nothing() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    let chosen = server.connect();
    chosen
        .send(ClientMsg::AttachTo {
            name: a.name.clone(),
        })
        .unwrap();
    server.pump(Instant::now());
    drain(&chosen);
    let plain = server.connect();
    plain.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert!(
        matches!(plain.poll().unwrap(), Some(ServerMsg::Attached { session, .. }) if session.id == a.id)
    );
    let unknown = server.connect();
    unknown
        .send(ClientMsg::AttachTo {
            name: "not found".into(),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(matches!(
        unknown.poll().unwrap(),
        Some(ServerMsg::Sessions { result: Err(_), .. })
    ));
    assert_eq!(server.sessions.len(), 3);
    assert!(server.named(&b.name).is_ok());
}

#[test]
fn an_unattached_control_client_uses_the_same_protocol_to_list_sessions() {
    let mut server = a_loop();
    let client = server.connect();
    client
        .send(ClientMsg::Session {
            request_id: 9,
            command: SessionCommand::List,
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(matches!(client.poll().unwrap(), Some(ServerMsg::Sessions {
        request_id: 9, result: Ok(SessionReply::List(sessions)),
    }) if sessions.len() == 1 && sessions[0].windows == 0));
}

#[test]
fn session_metadata_retains_a_free_extension_label() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let result = server
        .execute_session(SessionCommand::Create {
            folder: folder.path().into(),
            name: Some("work".into()),
            label: Some("external feature reference".into()),
        })
        .unwrap();
    assert!(
        matches!(result, SessionReply::Updated(info) if info.label.as_deref() == Some("external feature reference"))
    );
}

#[test]
fn native_notifications_from_terminal_setup_stay_in_the_requesting_session() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = a_loop();
    let a = create(&mut server, &folder.path().join("a"), "A");
    let b = create(&mut server, &folder.path().join("b"), "B");
    let first = server.connect();
    let other = server.connect();
    first.send(ClientMsg::AttachTo { name: a.name }).unwrap();
    other.send(ClientMsg::AttachTo { name: b.name }).unwrap();
    server.pump(Instant::now());
    drain(&first);
    drain(&other);
    first
        .send(ClientMsg::EnsureTerminal {
            owner: crate::app::backend_store::TerminalOwner::Pane(heca_core::layout::PaneId(77)),
            spec: crate::app::backend_store::TerminalSpec {
                program: crate::app::backend_store::Program::Shell,
                cwd: Some(folder.path().join("not-a-folder")),
                grid: (80, 24),
            },
            cell: (8.0, 14.0),
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(
        drain(&first)
            .iter()
            .any(|m| matches!(m, ServerMsg::Notifications(v) if !v.is_empty()))
    );
    assert!(drain(&other).is_empty());
}
