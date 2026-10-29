use super::*;
use crate::server::wire::ClientLink;
use heca_core::layout::SessionId;

fn identity(client: &impl ClientLink) -> (u64, u64, SessionId) {
    let mut identity = None;
    while let Some(message) = client.poll().unwrap() {
        if let ServerMsg::Attached {
            client_id,
            window_ordinal,
            session,
            ..
        } = message
        {
            identity = Some((client_id, window_ordinal, session.id));
        }
    }
    identity.expect("attach includes the connection identity and session ordinal")
}

#[test]
fn ordinals_start_at_one_allow_gaps_and_survive_restart_and_repeated_attach() {
    let mut server = ServerLoop::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(1), Default::default()),
    );
    let first = server.connect();
    let second = server.connect();
    first.send(ClientMsg::Attach).unwrap();
    second.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    let a = identity(&first);
    let b = identity(&second);
    assert_eq!((a.1, b.1), (1, 2));
    assert_ne!(a.0, b.0);
    first.send(ClientMsg::Detach).unwrap();
    server.pump(Instant::now());
    let third = server.connect();
    third.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    let c = identity(&third);
    assert_eq!(c.1, 3);
    while second.poll().unwrap().is_some() {}
    second.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert_eq!(identity(&second), b);
    let name = server.sessions[0].info().name;
    server
        .execute_session(SessionCommand::Terminate { name: name.clone() })
        .unwrap();
    server
        .execute_session(SessionCommand::Restart { name })
        .unwrap();
    assert_eq!(identity(&second), b);
    assert_eq!(identity(&third), c);
}

#[test]
fn switching_sessions_preserves_client_identity_and_uses_the_destination_ordinal() {
    let folder = tempfile::tempdir().unwrap();
    let mut server = ServerLoop::new(
        ServerState::for_tests(Default::default()),
        Session::new(SessionId(1), Default::default()),
    );
    let client = server.connect();
    client.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    let previous = identity(&client);
    server
        .execute_session(SessionCommand::Create {
            folder: folder.path().into(),
            name: Some("B".into()),
            label: None,
        })
        .unwrap();
    client
        .send(ClientMsg::AttachTo { name: "B".into() })
        .unwrap();
    server.pump(Instant::now());
    let current = identity(&client);
    assert_eq!(current.0, previous.0);
    assert_ne!(current.2, previous.2);
    assert_eq!(current.1, 1);
}
