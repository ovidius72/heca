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

fn client(link: RecordingLink) -> ClientState {
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
        pending_selection: None,
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
