use super::*;
use crate::server::wire::ClientLink;
use heca_core::{
    agent::{AgentState, AgentStatus},
    layout::{Pane, PaneId, SessionId},
};

fn feedbacks(client: &impl ClientLink) -> Vec<crate::server::feedback::PaneFeedback> {
    let mut result = Vec::new();
    while let Some(message) = client.poll().unwrap() {
        if let ServerMsg::Feedback { panes, .. } = message {
            result = panes;
        }
    }
    result
}

#[test]
fn two_clients_acknowledge_one_session_and_late_joiners_receive_seen_state() {
    let mut config = heca_config::settings::AgentsConfig::default();
    config.programs.insert(
        "custom".into(),
        heca_config::settings::AgentProgramConfig {
            enabled: Some(true),
            feedback_events: Some(vec!["NeedsInput".into()]),
            ..Default::default()
        },
    );
    let mut state = ServerState::for_tests(Default::default());
    let mut report = AgentState::new("custom", "Custom", AgentStatus::new("waiting_permission"))
        .event("NeedsInput");
    report.turn_id = Some("1".into());
    state.feedback.observe(PaneId(7), Some(&report), &config);
    report.turn_id = Some("2".into());
    state.feedback.observe(PaneId(7), Some(&report), &config);
    let mut session = Session::new(SessionId(1), Default::default());
    session.add_column_with(0, 0, Pane::new(PaneId(7), "agent"));
    let mut server = ServerLoop::new(state, session);
    let first = server.connect();
    let second = server.connect();
    first.send(ClientMsg::Attach).unwrap();
    second.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert!(feedbacks(&first)[0].pending());
    assert!(feedbacks(&second)[0].pending());
    first
        .send(ClientMsg::AcknowledgeFeedback {
            session_id: SessionId(99),
            pane_id: PaneId(7),
            revision: 2,
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(
        feedbacks(&second).is_empty(),
        "a foreign session acknowledgement cannot change this session"
    );
    first
        .send(ClientMsg::AcknowledgeFeedback {
            session_id: SessionId(1),
            pane_id: PaneId(7),
            revision: 1,
        })
        .unwrap();
    server.pump(Instant::now());
    assert!(feedbacks(&first)[0].pending());
    assert!(feedbacks(&second)[0].pending());
    first
        .send(ClientMsg::AcknowledgeFeedback {
            session_id: SessionId(1),
            pane_id: PaneId(7),
            revision: 2,
        })
        .unwrap();
    server.pump(Instant::now());
    for client in [&first, &second] {
        let projection = feedbacks(client);
        assert!(!projection[0].pending());
        assert_eq!(projection[0].status, "waiting_permission");
    }
    let third = server.connect();
    third.send(ClientMsg::Attach).unwrap();
    server.pump(Instant::now());
    assert!(!feedbacks(&third)[0].pending());
}

#[test]
fn only_one_attached_client_is_granted_a_feedback_sound() {
    let mut state = ServerState::for_tests(Default::default());
    state.feedback.completed(PaneId(7), 1);
    let mut session = Session::new(SessionId(1), Default::default());
    session.add_column_with(0, 0, Pane::new(PaneId(7), "shell"));
    let mut server = ServerLoop::new(state, session);
    let first = server.connect();
    let second = server.connect();
    for client in [&first, &second] { client.send(ClientMsg::Attach).unwrap(); }
    server.pump(Instant::now());
    for client in [&first, &second] { feedbacks(client); }
    first.send(ClientMsg::RequestFeedbackSound { session_id: SessionId(99), pane_id: PaneId(7), revision: 1 }).unwrap();
    for client in [&first, &second] {
        client.send(ClientMsg::RequestFeedbackSound { session_id: SessionId(1), pane_id: PaneId(7), revision: 1 }).unwrap();
    }
    server.pump(Instant::now());
    let mut grants = 0;
    for client in [&first, &second] {
        while let Some(message) = client.poll().unwrap() {
            if let ServerMsg::FeedbackSound { session_id, pane_id, revision } = message {
                assert_eq!((session_id, pane_id, revision), (SessionId(1), PaneId(7), 1));
                grants += 1;
            }
        }
    }
    assert_eq!(grants, 1);
}
