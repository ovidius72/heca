use super::*;
use crate::app::backend_store::{Program, TerminalSpec};
use crate::server::sessions::RecordedAgent;
use heca_core::{
    agent::{AgentState, AgentStatus},
    backend::{FakeBackend, ProcessLaunch},
    layout::{Pane, PaneId, SessionId},
    runtime::{PaneRuntime, ProcessStatus},
};

#[test]
fn qualified_terminal_names_work_through_headless_rpc_without_changing_identity() {
    use crate::app::backend_store::TerminalOwner;
    let mut state = ServerState::for_tests(Default::default());
    let owner = TerminalOwner::Named("demo.logs".into());
    let spec = TerminalSpec {
        program: Program::Shell,
        cwd: None,
        grid: (20, 6),
    };
    let id = state
        .backends
        .ensure(owner.clone(), spec.clone(), |_| {
            Ok(Box::new(FakeBackend::new(20, 6)))
        })
        .unwrap()
        .id();
    let mut runtime = SessionRuntime::new(state, Session::new(SessionId(42), Default::default()));
    for reference in ["demo.logs", &id.0.to_string()] {
        assert_eq!(
            runtime.rpc(&format!(
                "terminal_rename_to terminal={reference} name=Release"
            )),
            "ok"
        );
        assert_eq!(runtime.state.backends.custom_title(id), Some("Release"));
        assert_eq!(runtime.state.backends.find_owner(&owner), Some(id));
        assert_eq!(runtime.state.backends.spec_of(id), Some(&spec));
    }
    for reference in ["demo.missing", "Release", "9999"] {
        assert!(
            runtime
                .rpc(&format!(
                    "terminal_rename_to terminal={reference} name=Wrong"
                ))
                .starts_with("error:")
        );
        assert_eq!(runtime.state.backends.custom_title(id), Some("Release"));
    }
    assert_eq!(
        runtime.rpc("terminal_rename_to terminal=demo.logs name="),
        "ok"
    );
    assert_eq!(runtime.state.backends.custom_title(id), None);
}

#[test]
fn terminal_title_is_shared_replayed_on_attach_and_retained_on_restart() {
    use crate::app::backend_store::TerminalOwner;
    use crate::input::WmAction;
    use crate::server::wire::{ClientLink, in_process};
    let mut state = ServerState::for_tests(Default::default());
    state.backends.id_for(TerminalOwner::Named("unused".into()));
    let owner = TerminalOwner::Named("demo.logs".into());
    let id = state
        .backends
        .ensure(
            owner.clone(),
            TerminalSpec {
                program: Program::Shell,
                cwd: None,
                grid: (20, 6),
            },
            |_| Ok(Box::new(FakeBackend::new(20, 6))),
        )
        .unwrap()
        .id();
    let mut runtime = SessionRuntime::new(state, Session::new(SessionId(42), Default::default()));
    let mut clients = Vec::new();
    let mut connections = Vec::new();
    for client_id in [1, 2] {
        let (client, server) = in_process();
        let connection = Rc::new(Connection::new(client_id, Box::new(server)));
        connection.queue(ClientMsg::Attach);
        runtime.add_connection(connection.clone());
        connections.push(connection);
        clients.push(client);
    }
    runtime.pump(Instant::now());
    for client in &clients {
        while client.poll().unwrap().is_some() {}
    }
    // The sender's numeric id is not the server's id: owner normalization is authoritative.
    connections[0].queue(ClientMsg::Act {
        action: WmAction::TerminalRenameTo {
            terminal: "900".into(),
            name: "Build logs".into(),
        },
        asker: runtime.rpc_asker,
        request_id: None,
        terminal_owner: Some(owner.clone()),
    });
    runtime.pump(Instant::now());
    assert_eq!(runtime.state.backends.custom_title(id), Some("Build logs"));
    for client in &clients {
        let mut received = false;
        while let Some(message) = client.poll().unwrap() {
            if matches!(message, ServerMsg::Changed(Change::TerminalTitleChanged { title: Some(ref title), .. }) if title == "Build logs")
            {
                received = true;
            }
        }
        assert!(received, "both windows receive the shared label");
    }
    let (third, server) = in_process();
    let connection = Rc::new(Connection::new(3, Box::new(server)));
    connection.queue(ClientMsg::Attach);
    runtime.add_connection(connection);
    runtime.pump(Instant::now());
    let mut replayed = false;
    while let Some(message) = third.poll().unwrap() {
        if let ServerMsg::Terminal(frame) = message
            && frame.owner == owner
        {
            replayed = frame.display_title.as_deref() == Some("Build logs");
        }
    }
    assert!(
        replayed,
        "new attachments receive the title with the terminal snapshot"
    );
    runtime.terminate();
    assert_eq!(
        runtime.recorded_terminals()[0].display_title.as_deref(),
        Some("Build logs")
    );
    runtime.restart().unwrap();
    let new_id = runtime.state.backends.find_owner(&owner).unwrap();
    assert_eq!(
        runtime.state.backends.custom_title(new_id),
        Some("Build logs")
    );
}

#[test]
fn termination_retains_original_launch_and_native_harness_identity_for_persistence() {
    let folder = tempfile::tempdir().unwrap();
    let original = ProcessLaunch {
        executable: "/custom/bin/harness".into(),
        arguments: vec!["--label".into(), "two words".into()],
        cwd: Some(folder.path().to_owned()),
    };
    let mut agent = AgentState::new("custom-driver", "Custom", AgentStatus::new("working"));
    agent.launch = Some(original.clone());
    agent.session_id = Some("native-conversation-777".into());
    let mut backend = FakeBackend::new(20, 6);
    backend.set_runtime(PaneRuntime {
        program: Some("harness".into()),
        status: ProcessStatus::Running,
        agent: Some(agent.clone()),
        ..Default::default()
    });
    let mut state = ServerState::for_tests(Default::default());
    let pane = PaneId(7);
    state
        .backends
        .ensure(
            pane,
            TerminalSpec {
                program: Program::Command("shell-wrapper --status-file generated.json".into()),
                cwd: Some(folder.path().to_owned()),
                grid: (20, 6),
            },
            |_| Ok(Box::new(backend)),
        )
        .unwrap();
    let mut session = Session::new(SessionId(42), Default::default());
    session.add_column_with(0, 0, Pane::new(pane, "harness"));
    let mut runtime = SessionRuntime::new(state, session);
    let before = runtime.recorded_terminals();
    assert_eq!(before.len(), 1);
    assert!(before[0].running);
    assert_eq!(before[0].launch.as_ref(), Some(&original));
    assert_eq!(
        before[0].agent,
        Some(RecordedAgent {
            driver: "custom-driver".into(),
            session_id: Some("native-conversation-777".into())
        })
    );

    runtime.terminate();
    assert_eq!(runtime.info().status, SessionStatus::Terminated);
    assert!(runtime.state.backends.get(pane).is_none());
    let encoded = serde_json::to_vec(&runtime.recorded_terminals()).unwrap();
    let retained: Vec<crate::server::sessions::RecordedTerminal> =
        serde_json::from_slice(&encoded).unwrap();
    assert_eq!(retained[0].launch.as_ref(), Some(&original));
    let native = retained[0].agent.as_ref().unwrap();
    assert_eq!(native.driver, "custom-driver");
    assert_eq!(
        native.session_id.as_deref(),
        Some("native-conversation-777")
    );
    assert_ne!(native.session_id.as_deref(), Some("42"));
}

#[test]
fn attach_finishes_the_projection_after_terminal_snapshots() {
    use crate::server::wire::{ClientLink, in_process};
    let mut state = ServerState::for_tests(Default::default());
    let pane = PaneId(7);
    state
        .backends
        .ensure(
            pane,
            TerminalSpec {
                program: Program::Shell,
                cwd: None,
                grid: (20, 6),
            },
            |_| Ok(Box::new(FakeBackend::new(20, 6))),
        )
        .unwrap();
    let mut session = Session::new(SessionId(42), Default::default());
    session.add_column_with(0, 0, Pane::new(pane, "shell"));
    let mut runtime = SessionRuntime::new(state, session);
    let (client, server) = in_process();
    let connection = Rc::new(Connection::new(1, Box::new(server)));
    connection.queue(ClientMsg::Attach);
    runtime.add_connection(connection);
    runtime.pump(Instant::now());
    let mut messages = Vec::new();
    while let Some(message) = client.poll().unwrap() {
        messages.push(message);
    }
    let snapshot = messages
        .iter()
        .position(|m| matches!(m, ServerMsg::Terminal(_)))
        .unwrap();
    let ready = messages
        .iter()
        .position(|m| {
            matches!(
                m,
                ServerMsg::ProjectionReady {
                    session_id: SessionId(42)
                }
            )
        })
        .expect("attach has an explicit end even when delivery spans multiple client polls");
    assert!(snapshot < ready);

    runtime.terminate();
    while client.poll().unwrap().is_some() {}
    runtime.restart().unwrap();
    runtime.pump(Instant::now());
    let mut messages = Vec::new();
    while let Some(message) = client.poll().unwrap() {
        messages.push(message);
    }
    let snapshot = messages
        .iter()
        .position(|m| matches!(m, ServerMsg::Terminal(_)))
        .unwrap();
    let ready = messages
        .iter()
        .position(|m| matches!(m, ServerMsg::ProjectionReady { .. }))
        .unwrap();
    assert!(
        snapshot < ready,
        "restart also sends terminal snapshots before permitting presentation"
    );
}

#[test]
fn the_window_directory_lists_each_attached_window_by_its_reported_title() {
    use crate::server::sessions::WindowInfo;
    use crate::server::wire::{ClientLink, in_process};
    let mut runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(42), Default::default()),
    );
    // Two windows attach to the one session, each reporting the title it formatted for itself.
    let (client_a, server_a) = in_process();
    let conn_a = Rc::new(Connection::new(1, Box::new(server_a)));
    conn_a.queue(ClientMsg::Attach);
    conn_a.queue(ClientMsg::ReportWindowTitle("~/projects/heca".into()));
    runtime.add_connection(conn_a);
    let (client_b, server_b) = in_process();
    let conn_b = Rc::new(Connection::new(2, Box::new(server_b)));
    conn_b.queue(ClientMsg::Attach);
    conn_b.queue(ClientMsg::ReportWindowTitle("~/projects/heca #2".into()));
    runtime.add_connection(conn_b);
    runtime.pump(Instant::now());

    // The last directory each window received names both windows: stable id, presentation ordinal,
    // and the title that window reported.
    let latest_windows = |client: &dyn ClientLink| {
        let mut windows = None;
        while let Some(message) = client.poll().unwrap() {
            if let ServerMsg::Windows {
                session_id,
                windows: list,
            } = message
            {
                assert_eq!(session_id, SessionId(42));
                windows = Some(list);
            }
        }
        windows.expect("every attached window gets the directory")
    };
    let expected = vec![
        WindowInfo {
            id: 1,
            ordinal: 1,
            title: "~/projects/heca".into(),
        },
        WindowInfo {
            id: 2,
            ordinal: 2,
            title: "~/projects/heca #2".into(),
        },
    ];
    assert_eq!(latest_windows(&client_a), expected);
    assert_eq!(latest_windows(&client_b), expected);
}

#[test]
fn following_relays_the_leaders_view_and_tells_the_leader_it_is_followed() {
    use crate::server::wire::{ClientLink, in_process};
    use heca_core::layout::{Size, WindowView};
    let mut runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(42), Default::default()),
    );
    let view = WindowView::new(Size::new(800.0, 600.0), 1.0).snapshot();
    let (leader, leader_server) = in_process();
    let conn_leader = Rc::new(Connection::new(1, Box::new(leader_server)));
    conn_leader.queue(ClientMsg::Attach);
    // The leader reports the view it is looking at.
    conn_leader.queue(ClientMsg::ReportView(view.clone()));
    runtime.add_connection(conn_leader);
    let (follower, follower_server) = in_process();
    let conn_follower = Rc::new(Connection::new(2, Box::new(follower_server)));
    conn_follower.queue(ClientMsg::Attach);
    // The follower follows the leader.
    conn_follower.queue(ClientMsg::Follow { target: Some(1) });
    runtime.add_connection(conn_follower);
    runtime.pump(Instant::now());

    // The leader is told it is followed, so it knows to keep reporting its view.
    let mut leader_msgs = Vec::new();
    while let Some(message) = leader.poll().unwrap() {
        leader_msgs.push(message);
    }
    assert!(
        leader_msgs
            .iter()
            .any(|m| matches!(m, ServerMsg::Followed { active: true }))
    );

    // The follower receives the leader's view to mirror.
    let mut mirrored = None;
    while let Some(message) = follower.poll().unwrap() {
        if let ServerMsg::FollowView(snapshot) = message {
            mirrored = Some(snapshot);
        }
    }
    assert_eq!(mirrored, Some(view));
}

#[test]
fn a_sessions_broadcast_streams_each_change_to_event_subscribers() {
    use crate::server::{Change, EventFanout, StreamEvent};
    use std::sync::mpsc;
    let fanout = EventFanout::default();
    let (tx, rx) = mpsc::sync_channel(64);
    fanout.subscribe(tx);
    let mut runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(42), Default::default()),
    );
    runtime.install_event_fanout(fanout);

    // A change broadcast to the session's windows is also streamed to the subscriber, as a line it
    // can parse back into the same `Change` (wrapped as a `StreamEvent`) — nothing invented.
    let gone = runtime.broadcast(&[Change::LayoutChanged, Change::NamesChanged]);
    assert!(gone.is_empty(), "no windows attached, so none disconnect");
    let event = |rx: &mpsc::Receiver<String>| {
        serde_json::from_str::<StreamEvent>(&rx.recv().unwrap()).unwrap()
    };
    assert_eq!(event(&rx), StreamEvent::Change(Change::LayoutChanged));
    assert_eq!(event(&rx), StreamEvent::Change(Change::NamesChanged));
}

#[test]
fn a_windows_presence_report_becomes_focus_and_workspace_events() {
    use crate::server::wire::in_process;
    use crate::server::{EventFanout, StreamEvent};
    use heca_core::layout::PaneId;
    use std::sync::mpsc;
    let fanout = EventFanout::default();
    let (tx, rx) = mpsc::sync_channel(64);
    fanout.subscribe(tx);
    let mut runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(42), Default::default()),
    );
    runtime.install_event_fanout(fanout);
    let (_client, server) = in_process();
    let conn = Rc::new(Connection::new(7, Box::new(server)));
    conn.queue(ClientMsg::Attach);
    conn.queue(ClientMsg::ReportPresence {
        workspace: 0,
        pane: Some(PaneId(3)),
    });
    // Only the workspace moved: a `Workspace` event, no new `Focus`.
    conn.queue(ClientMsg::ReportPresence {
        workspace: 1,
        pane: Some(PaneId(3)),
    });
    runtime.add_connection(conn);
    runtime.pump(Instant::now());

    let events: Vec<StreamEvent> = std::iter::from_fn(|| {
        rx.try_recv()
            .ok()
            .map(|line| serde_json::from_str(&line).unwrap())
    })
    .collect();
    // First report (both new) → Workspace{0} + Focus{pane 3}; second (workspace only) → Workspace{1}.
    assert!(events.contains(&StreamEvent::Workspace {
        window: 7,
        workspace: 0
    }));
    assert!(events.contains(&StreamEvent::Focus {
        window: 7,
        workspace: 0,
        pane: Some(PaneId(3))
    }));
    assert!(events.contains(&StreamEvent::Workspace {
        window: 7,
        workspace: 1
    }));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamEvent::Focus { .. }))
            .count(),
        1,
        "the pane did not change on the second report, so no second Focus",
    );
}

#[test]
fn an_empty_or_terminated_session_also_finishes_its_projection() {
    use crate::server::wire::{ClientLink, in_process};
    for terminated in [false, true] {
        let mut runtime = SessionRuntime::new(
            ServerState::for_tests(Default::default()),
            Session::new(SessionId(42), Default::default()),
        );
        if terminated {
            runtime.terminate();
        }
        let (client, server) = in_process();
        let connection = Rc::new(Connection::new(1, Box::new(server)));
        connection.queue(ClientMsg::Attach);
        runtime.add_connection(connection);
        runtime.pump(Instant::now());
        let mut ready = 0;
        while let Some(message) = client.poll().unwrap() {
            if matches!(
                message,
                ServerMsg::ProjectionReady {
                    session_id: SessionId(42)
                }
            ) {
                ready += 1;
            }
        }
        assert_eq!(ready, 1);
    }
}

/// **A terminal whose screen did not change is not snapshotted or sent again** — to a window that
/// already has it — while a window that attaches later still gets it, and a change of screen is
/// sent to everyone.
#[test]
fn an_unchanged_terminal_is_sent_only_to_a_window_that_does_not_have_it() {
    use crate::app::backend_store::TerminalOwner;
    use crate::server::wire::{ClientLink, in_process, terminal::ProcessInput};
    let mut state = ServerState::for_tests(Default::default());
    let owner = TerminalOwner::Named("demo.logs".into());
    state
        .backends
        .ensure(
            owner.clone(),
            TerminalSpec {
                program: Program::Shell,
                cwd: None,
                grid: (20, 6),
            },
            |_| Ok(Box::new(FakeBackend::new(20, 6))),
        )
        .unwrap();
    let mut runtime = SessionRuntime::new(state, Session::new(SessionId(42), Default::default()));
    let attach = |runtime: &mut SessionRuntime, id: u64| {
        let (client, server) = in_process();
        let connection = Rc::new(Connection::new(id, Box::new(server)));
        connection.queue(ClientMsg::Attach);
        runtime.add_connection(connection.clone());
        (client, connection)
    };
    let frames = |client: &crate::server::wire::InProcessClient| {
        let mut count = 0;
        while let Some(message) = client.poll().unwrap() {
            if matches!(message, ServerMsg::Terminal(ref frame) if frame.owner == owner) {
                count += 1;
            }
        }
        count
    };
    let (first, connection) = attach(&mut runtime, 1);
    runtime.pump(Instant::now());
    assert_eq!(frames(&first), 1, "attaching sends the terminal");

    // Input makes the server publish its terminals; the screen did not change.
    connection.queue(ClientMsg::Terminal {
        owner: owner.clone(),
        input: ProcessInput::Bytes(b"x".to_vec()),
    });
    runtime.pump(Instant::now());
    assert_eq!(frames(&first), 0, "an unchanged screen is not sent again");

    let (second, _connection) = attach(&mut runtime, 2);
    runtime.pump(Instant::now());
    assert_eq!(frames(&second), 1, "a window that attaches later gets it");
    assert_eq!(frames(&first), 0);

    connection.queue(ClientMsg::Terminal {
        owner: owner.clone(),
        input: ProcessInput::Resize {
            cols: 30,
            rows: 8,
            cell: (8.0, 16.0),
            scale: 1.0,
        },
    });
    runtime.pump(Instant::now());
    assert_eq!(frames(&first), 1, "a changed screen goes to every window");
    assert_eq!(frames(&second), 1);
}

/// **An action asked from a window gets the workspace that window shows**, by a key that stays the
/// same when the workspaces are reordered — the position changes, the key does not.
#[test]
fn a_windows_call_gets_the_workspace_it_shows_by_a_key_a_reorder_keeps() {
    use crate::server::wire::in_process;
    use heca_core::layout::PaneId;
    let mut session = Session::new(SessionId(42), Default::default());
    session.add_workspace();
    let shown = session.workspaces[1].id;
    let mut runtime = SessionRuntime::new(ServerState::for_tests(Default::default()), session);
    let (_client, server) = in_process();
    let conn = Rc::new(Connection::new(7, Box::new(server)));
    conn.queue(ClientMsg::Attach);
    conn.queue(ClientMsg::ReportPresence {
        workspace: 1,
        pane: None::<PaneId>,
    });
    runtime.add_connection(conn.clone());
    runtime.pump(Instant::now());
    let key = runtime.service_ref().key;
    let expected = Some(crate::entry::service::WorkspaceKey::new(key, shown));
    assert_eq!(
        runtime.service_ref_for(0).workspace,
        expected,
        "the window's workspace"
    );
    assert_eq!(
        runtime.service_ref().workspace,
        expected,
        "a script gets the one shown last"
    );

    // Reorder: the shown workspace moves to the front, and the window says so.
    runtime.session.workspaces.swap(0, 1);
    conn.queue(ClientMsg::ReportPresence {
        workspace: 0,
        pane: None,
    });
    runtime.pump(Instant::now());
    assert_eq!(
        runtime.service_ref_for(0).workspace,
        expected,
        "same workspace, same key, new position"
    );
}

/// The window learns its session's key from what the server sends it, so it names a workspace with
/// the same key an action gets.
#[test]
fn the_session_a_window_is_sent_carries_the_key_actions_use() {
    let runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(5), Default::default()),
    );
    let sent: crate::server::sessions::SessionInfo =
        serde_json::from_str(&serde_json::to_string(&runtime.info()).unwrap()).unwrap();
    assert_eq!(sent.key, Some(runtime.service_ref().key));
}

/// **A window takes the lowest number no window on the session holds now** — alone it is the first,
/// a second is #2, a number a window left goes to the next one, and nobody's number moves when
/// another leaves.
#[test]
fn a_window_takes_the_lowest_free_number_and_keeps_it() {
    use crate::server::wire::in_process;
    let mut runtime = SessionRuntime::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(42), Default::default()),
    );
    let join = |runtime: &mut SessionRuntime, id: u64| {
        let (_client, server) = in_process();
        let connection = Rc::new(Connection::new(id, Box::new(server)));
        runtime.add_connection(connection.clone());
        connection
    };
    let one = join(&mut runtime, 1);
    let two = join(&mut runtime, 2);
    let three = join(&mut runtime, 3);
    assert_eq!(
        [
            one.window_ordinal.get(),
            two.window_ordinal.get(),
            three.window_ordinal.get()
        ],
        [1, 2, 3]
    );
    runtime.detach_connection(2);
    assert_eq!(three.window_ordinal.get(), 3, "nobody else's number moves");
    let four = join(&mut runtime, 4);
    assert_eq!(four.window_ordinal.get(), 2, "the number left free");
}
