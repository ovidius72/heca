use super::*;
use crate::server::sessions::{SessionInfo, SessionStatus};
use heca_core::layout::{Session, SessionId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

struct RecordingLink {
    sent: Arc<Mutex<Vec<ClientMsg>>>,
    incoming: Arc<Mutex<VecDeque<ServerMsg>>>,
}

impl ClientLink for RecordingLink {
    fn send(&self, message: ClientMsg) -> io::Result<()> {
        self.sent.lock().unwrap().push(message);
        Ok(())
    }
    fn poll(&self) -> io::Result<Option<ServerMsg>> {
        Ok(self.incoming.lock().unwrap().pop_front())
    }
}

fn info(id: u64, name: &str) -> SessionInfo {
    SessionInfo {
        id: SessionId(id),
        name: name.into(),
        folder: None,
        label: None,
        status: SessionStatus::Active,
        windows: 1,
        agents: 0,
    }
}

pub(super) fn client(link: impl ClientLink + 'static) -> ClientState {
    let connection = heca_config::settings::ConnectionConfig::default();
    let wake = ServerWaker::new(|| {});
    let (outbox, requests) = outbox::channel(connection.queue_capacity.get(), wake.clone());
    ClientState {
        client_id: 1,
        window_ordinal: 1,
        feedback: Default::default(),
        session: info(1, "A"),
        sessions: Vec::new(),
        backends: BackendStore::new(),
        programs: Rc::new(ProgramsConfig::default()),
        link: Box::new(link),
        local: None,
        data: Default::default(),
        outbox,
        requests,
        pending: HashMap::new(),
        next_request: 1,
        images: Default::default(),
        image_limit: connection.max_image_bytes.get(),
        reads: Default::default(),
        failed: false,
        switching: false,
        detached: false,
        pending_selection: None,
        projection_deadline: None,
        wake,
        connection,
    }
}

#[test]
fn selection_sends_the_rpc_answer_before_attach_and_discards_old_terminal_requests() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let incoming = Arc::new(Mutex::new(VecDeque::new()));
    let mut client = client(RecordingLink {
        sent: sent.clone(),
        incoming: incoming.clone(),
    });
    let old_outbox = client.outbox.clone();
    client.switch_session("B".into()).unwrap();
    client.reply_rpc(9, "ok".into());
    old_outbox.send(ClientMsg::Reload);
    client.poll().unwrap();
    let messages = sent.lock().unwrap();
    assert_eq!(messages.len(), 2);
    assert!(
        matches!(&messages[0], ClientMsg::RpcReply { request_id: 9, answer } if answer == "ok")
    );
    assert!(matches!(&messages[1], ClientMsg::AttachTo { name } if name == "B"));
    drop(messages);
    incoming.lock().unwrap().push_back(ServerMsg::Attached {
        session: info(2, "B"),
        client_id: 1,
        window_ordinal: 2,
        content: Session::new(SessionId(2), Default::default()).content(),
    });
    incoming.lock().unwrap().push_back(ServerMsg::ProjectionReady { session_id: SessionId(2) });
    let update = client.poll().unwrap();
    assert!(update.session_attached);
    assert_eq!(client.session.id, SessionId(2));
    old_outbox.send(ClientMsg::Reload);
    assert!(
        old_outbox.check().is_err(),
        "old views must not send into the new session"
    );
    assert!(client.outbox.check().is_ok());
    assert!(!client.switching);
}

#[test]
fn closing_a_window_flushes_its_rpc_answer_before_detaching() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let client = client(RecordingLink { sent: sent.clone(), incoming: Default::default() });
    client.reply_rpc(17, "ok".into());
    drop(client);
    let messages = sent.lock().unwrap();
    assert!(matches!(&messages[0], ClientMsg::RpcReply { request_id: 17, answer } if answer == "ok"));
    assert!(matches!(&messages[1], ClientMsg::Detach));
}

#[test]
fn reconnecting_with_the_same_session_id_does_not_keep_old_feedback_acknowledgements() {
    use crate::server::feedback::PaneFeedback;
    use heca_core::layout::PaneId;
    let chrome = crate::chrome::SharedChromeState::new(0.0, 0.0);
    let agents = heca_config::settings::AgentsConfig::default();
    let feedback = |revision| PaneFeedback {
        pane_id: PaneId(7), feedback_revision: revision,
        seen_through_revision: 0, status: "error".into(),
    };
    let mut client = client(RecordingLink { sent: Default::default(), incoming: Default::default() });
    client.feedback.receive(vec![feedback(42)]);
    client.sync_feedback(Some(PaneId(7)), &chrome.panes, &agents, 8000, false);
    client.failed = true;
    let old_outbox = client.outbox.clone();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let link = RecordingLink {
        sent: sent.clone(),
        incoming: Arc::new(Mutex::new(VecDeque::from([
            ServerMsg::Attached {
                session: info(1, "A"), client_id: 9, window_ordinal: 4,
                content: Session::new(SessionId(1), Default::default()).content(),
            },
            ServerMsg::Feedback { session_id: SessionId(1), panes: vec![feedback(1)] },
            ServerMsg::ProjectionReady { session_id: SessionId(1) },
        ]))),
    };
    let (_send, ready) = mpsc::sync_channel(1);
    client.reconnect_link(Box::new(link), &ready).unwrap();
    assert!(client.poll().unwrap().session_attached);
    assert_eq!(client.client_id, 9);
    assert_eq!(client.window_ordinal, 4);
    client.sync_feedback(Some(PaneId(7)), &chrome.panes, &agents, 8000, false);
    client.poll().unwrap();
    old_outbox.send(ClientMsg::Reload);
    assert!(old_outbox.check().is_err());
    drop(client);
    let messages = sent.lock().unwrap();
    assert!(matches!(&messages[0], ClientMsg::AttachTo { name } if name == "A"));
    assert!(messages.iter().any(|m| matches!(m, ClientMsg::AcknowledgeFeedback { revision: 1, .. })));
    assert!(!messages.iter().any(|m| matches!(m, ClientMsg::AcknowledgeFeedback { revision: 42, .. })));
}

#[test]
fn failed_reconnect_keeps_the_intended_session_without_fallback() {
    for reply in [
        ServerMsg::Sessions { request_id: 0, result: Err("session not found".into()) },
        ServerMsg::Attached {
            session: info(2, "another-session"), client_id: 9, window_ordinal: 1,
            content: Session::new(SessionId(2), Default::default()).content(),
        },
    ] {
        let mut client = client(RecordingLink { sent: Default::default(), incoming: Default::default() });
        client.failed = true;
        let sent = Arc::new(Mutex::new(Vec::new()));
        let link = RecordingLink { sent: sent.clone(), incoming: Arc::new(Mutex::new(VecDeque::from([reply]))) };
        let (_send, ready) = mpsc::sync_channel(1);
        assert!(client.reconnect_link(Box::new(link), &ready).is_err());
        assert!(client.failed);
        assert_eq!(client.session, info(1, "A"));
        assert!(client.outbox.check().is_ok());
        assert!(matches!(sent.lock().unwrap().as_slice(), [ClientMsg::AttachTo { name }] if name == "A"));
    }
}

#[test]
fn session_projection_is_not_presented_between_attach_and_the_last_snapshot() {
    let incoming = Arc::new(Mutex::new(VecDeque::new()));
    let mut client = client(RecordingLink { sent: Default::default(), incoming: incoming.clone() });
    incoming.lock().unwrap().push_back(ServerMsg::Attached {
        session: info(2, "B"), client_id: 1, window_ordinal: 2,
        content: Session::new(SessionId(2), Default::default()).content(),
    });
    assert!(client.poll().unwrap().session_attached);
    assert!(!client.projection_ready(), "an attach alone must not expose an incomplete frame");
    incoming.lock().unwrap().push_back(ServerMsg::ProjectionReady { session_id: SessionId(1) });
    client.poll().unwrap();
    assert!(!client.projection_ready(), "another session cannot finish this snapshot");
    use heca_core::backend::PaneBackend;
    let owner = TerminalOwner::Pane(heca_core::layout::PaneId(7));
    let backend = heca_core::backend::FakeBackend::new(20, 6);
    let frame = crate::server::wire::terminal::TerminalFrame {
        id: 1, owner: owner.clone(), title: "shell".into(),
        spec: TerminalSpec { program: crate::app::backend_store::Program::Shell, cwd: None, grid: (20, 6) },
        screen: backend.terminal_snapshot().unwrap().to_wire(),
        damage: TerminalDamage::Full, runtime: Default::default(), mouse_grabbed: false,
    };
    incoming.lock().unwrap().push_back(ServerMsg::Terminal(Box::new(frame)));
    client.poll().unwrap();
    assert!(!client.projection_ready(), "an early terminal snapshot must not end the batch");
    assert!(client.backends.find_owner(&owner).is_some());
    incoming.lock().unwrap().push_back(ServerMsg::ProjectionReady { session_id: SessionId(2) });
    let update = client.poll().unwrap();
    assert!(client.projection_ready());
    assert!(update.effects.has_data, "completing the snapshot must wake rendering");
}

#[test]
fn incomplete_projection_times_out_instead_of_freezing_rendering_forever() {
    let mut client = client(RecordingLink { sent: Default::default(), incoming: Default::default() });
    client.reset_projection();
    assert!(!client.projection_ready());
    assert_eq!(client.next_wake(), client.projection_deadline);
    client.projection_deadline = Some(Instant::now());
    assert_eq!(client.poll().err().unwrap().kind(), io::ErrorKind::TimedOut);
    assert!(client.projection_ready(), "the disconnected-session dialog must still render");
    assert!(client.next_wake().is_none());
}

#[test]
fn terminating_an_incomplete_projection_allows_the_unavailable_dialog_to_render() {
    let incoming = Arc::new(Mutex::new(VecDeque::new()));
    let mut client = client(RecordingLink { sent: Default::default(), incoming: incoming.clone() });
    client.reset_projection();
    let mut stopped = info(1, "A");
    stopped.status = SessionStatus::Terminated;
    incoming.lock().unwrap().push_back(ServerMsg::SessionTerminated { session: stopped });
    assert!(client.poll().unwrap().session_terminated);
    assert!(client.projection_ready());
}

#[test]
fn destination_reload_is_not_lost_while_its_projection_is_loading() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = client(RecordingLink { sent: sent.clone(), incoming: Default::default() });
    client.reset_projection();
    client.reload();
    client.poll().unwrap();
    assert!(!client.projection_ready());
    drop(client);
    assert!(matches!(sent.lock().unwrap().first(), Some(ClientMsg::Reload)));
}

#[test]
fn recreating_a_terminal_view_never_restores_its_original_launch_grid() {
    use heca_core::backend::PaneBackend;
    use crate::server::wire::terminal::{ProcessInput, TerminalFrame};
    for grid in [(37, 40), (136, 20)] {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let owner = TerminalOwner::Pane(heca_core::layout::PaneId(7));
        let backend = heca_core::backend::FakeBackend::new(grid.0, grid.1);
        let incoming = Arc::new(Mutex::new(VecDeque::from([
            ServerMsg::Terminal(Box::new(TerminalFrame {
                id: 1, owner: owner.clone(), title: "shell".into(),
                spec: TerminalSpec { program: crate::app::backend_store::Program::Shell,
                    cwd: None, grid: (80, 24) },
                screen: backend.terminal_snapshot().unwrap().to_wire(),
                damage: TerminalDamage::Full, runtime: Default::default(), mouse_grabbed: false,
            })),
        ])));
        let mut client = client(RecordingLink { sent: sent.clone(), incoming });
        client.poll().unwrap();
        let id = client.backends.find_owner(&owner).unwrap();
        assert_eq!(client.backends.spec_of(id).unwrap().grid, (80, 24), "the original launch spec stays unchanged");
        client.terminal_view(id, 2.0).unwrap();
        client.backends.get_mut_by_id(id).unwrap().set_size(grid.0, grid.1);
        client.poll().unwrap();
        drop(client);
        let messages = sent.lock().unwrap();
        let sizes: Vec<_> = messages.iter().filter_map(|message| match message {
            ClientMsg::Terminal { input: ProcessInput::Resize { cols, rows, .. }, .. } => Some((*cols, *rows)),
            _ => None,
        }).collect();
        assert_eq!(sizes, vec![grid], "reattach must preserve the live grid, not briefly restore 80x24");
    }
}

#[test]
fn a_terminal_without_a_received_screen_still_uses_its_launch_grid() {
    use heca_core::backend::PaneBackend;
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = client(RecordingLink { sent: sent.clone(), incoming: Default::default() });
    let spec = TerminalSpec { program: crate::app::backend_store::Program::Shell, cwd: None, grid: (80, 24) };
    let mut view = terminal::TerminalView::new(TerminalOwner::Pane(heca_core::layout::PaneId(7)),
        &spec, client.data.clone(), client.outbox.clone(), (8.0, 14.0));
    view.set_scale_factor(2.0);
    client.poll().unwrap();
    drop(client);
    assert!(matches!(sent.lock().unwrap().first(), Some(ClientMsg::Terminal {
        input: crate::server::wire::terminal::ProcessInput::Resize { cols: 80, rows: 24, .. }, ..
    })));
}

#[test]
fn changing_terminal_geometry_invalidates_its_retained_picture() {
    use heca_core::backend::PaneBackend;
    let client = client(RecordingLink { sent: Default::default(), incoming: Default::default() });
    let spec = TerminalSpec { program: crate::app::backend_store::Program::Shell, cwd: None, grid: (80, 24) };
    let mut view = terminal::TerminalView::new(TerminalOwner::Pane(heca_core::layout::PaneId(7)),
        &spec, client.data.clone(), client.outbox.clone(), (8.0, 14.0));
    assert_eq!(view.take_terminal_damage(), TerminalDamage::Full);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::None);
    view.set_cell_size(9.0, 16.0);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::Full, "new cell metrics change the picture before a server reply");
    view.set_cell_size(9.0, 16.0);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::None);
    view.set_size(90, 30);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::Full);
    view.set_size(90, 30);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::None);
    view.set_scale_factor(2.0);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::Full);
    view.set_scale_factor(2.0);
    assert_eq!(view.take_terminal_damage(), TerminalDamage::None);
}

#[test]
fn pending_terminal_geometry_is_sent_as_one_final_resize() {
    use heca_core::backend::PaneBackend;
    let sent = Arc::new(Mutex::new(Vec::new()));
    let mut client = client(RecordingLink { sent: sent.clone(), incoming: Default::default() });
    let spec = TerminalSpec { program: crate::app::backend_store::Program::Shell, cwd: None, grid: (80, 24) };
    let mut view = terminal::TerminalView::new(TerminalOwner::Pane(heca_core::layout::PaneId(7)),
        &spec, client.data.clone(), client.outbox.clone(), (8.0, 14.0));
    view.set_scale_factor(2.0);
    view.set_cell_size(9.0, 16.0);
    view.set_size(90, 30);
    client.poll().unwrap();
    drop(client);
    let messages = sent.lock().unwrap();
    let messages: Vec<_> = messages.iter().filter(|msg| !matches!(msg, ClientMsg::Detach)).collect();
    assert_eq!(messages.len(), 1, "intermediate metrics must not resize the running shell");
    assert!(matches!(&messages[0], ClientMsg::Terminal {
        input: crate::server::wire::terminal::ProcessInput::Resize { cols: 90, rows: 30, cell, scale: 2.0 }, ..
    } if *cell == (9.0, 16.0)));
}

#[test]
fn logical_detach_keeps_rpc_working_and_allows_explicit_session_selection() {
    let sent = Arc::new(Mutex::new(Vec::new()));
    let incoming = Arc::new(Mutex::new(VecDeque::new()));
    let mut client = client(RecordingLink { sent: sent.clone(), incoming: incoming.clone() });
    client.detach_session().unwrap();
    client.reply_rpc(8, "ok".into());
    client.poll().unwrap();
    {
        let messages = sent.lock().unwrap();
        assert!(matches!(&messages[0], ClientMsg::RpcReply { request_id: 8, .. }));
        assert!(matches!(&messages[1], ClientMsg::LeaveSession));
    }
    incoming.lock().unwrap().push_back(ServerMsg::SessionDetached { id: SessionId(1) });
    let update = client.poll().unwrap();
    assert!(update.session_detached && !update.session_deleted);
    assert!(client.detached && client.projection_ready());
    client.reply_rpc(9, "still here".into());
    client.switch_session("B".into()).unwrap();
    client.poll().unwrap();
    drop(client);
    let messages = sent.lock().unwrap();
    assert!(matches!(&messages[2], ClientMsg::RpcReply { request_id: 9, .. }));
    assert!(matches!(&messages[3], ClientMsg::AttachTo { name } if name == "B"));
}
