use super::*;
use crate::app::backend_store::{Program, TerminalSpec};
use heca_core::{
    agent::{AgentState, AgentStatus},
    backend::{FakeBackend, ProcessLaunch},
    layout::{Pane, PaneId, SessionId},
    runtime::{PaneRuntime, ProcessStatus},
};

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
        program: Some("harness".into()), status: ProcessStatus::Running,
        agent: Some(agent.clone()), ..Default::default()
    });
    let mut state = ServerState::for_tests(Default::default());
    let pane = PaneId(7);
    state.backends.ensure(pane, TerminalSpec {
        program: Program::Command("shell-wrapper --status-file generated.json".into()),
        cwd: Some(folder.path().to_owned()), grid: (20, 6),
    }, |_| Ok(Box::new(backend))).unwrap();
    let mut session = Session::new(SessionId(42), Default::default());
    session.add_column_with(0, 0, Pane::new(pane, "harness"));
    let mut runtime = SessionRuntime::new(state, session);
    let before = runtime.recorded_terminals();
    assert_eq!(before.len(), 1);
    assert!(before[0].running);
    assert_eq!(before[0].launch.as_ref(), Some(&original));
    assert_eq!(before[0].agent.as_ref(), Some(&agent));

    runtime.terminate();
    assert_eq!(runtime.info().status, SessionStatus::Terminated);
    assert!(runtime.state.backends.get(pane).is_none());
    let encoded = serde_json::to_vec(&runtime.recorded_terminals()).unwrap();
    let retained: Vec<crate::server::sessions::RecordedTerminal> = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(retained[0].launch.as_ref(), Some(&original));
    let native = retained[0].agent.as_ref().unwrap();
    assert_eq!(native.driver, "custom-driver");
    assert_eq!(native.session_id.as_deref(), Some("native-conversation-777"));
    assert_ne!(native.session_id.as_deref(), Some("42"));
}

#[test]
fn attach_finishes_the_projection_after_terminal_snapshots() {
    use crate::server::wire::{ClientLink, in_process};
    let mut state = ServerState::for_tests(Default::default());
    let pane = PaneId(7);
    state.backends.ensure(pane, TerminalSpec {
        program: Program::Shell, cwd: None, grid: (20, 6),
    }, |_| Ok(Box::new(FakeBackend::new(20, 6)))).unwrap();
    let mut session = Session::new(SessionId(42), Default::default());
    session.add_column_with(0, 0, Pane::new(pane, "shell"));
    let mut runtime = SessionRuntime::new(state, session);
    let (client, server) = in_process();
    let connection = Rc::new(Connection::new(1, Box::new(server)));
    connection.queue(ClientMsg::Attach);
    runtime.add_connection(connection);
    runtime.pump(Instant::now());
    let mut messages = Vec::new();
    while let Some(message) = client.poll().unwrap() { messages.push(message); }
    let snapshot = messages.iter().position(|m| matches!(m, ServerMsg::Terminal(_))).unwrap();
    let ready = messages.iter().position(|m| matches!(m, ServerMsg::ProjectionReady { session_id: SessionId(42) }))
        .expect("attach has an explicit end even when delivery spans multiple client polls");
    assert!(snapshot < ready);

    runtime.terminate();
    while client.poll().unwrap().is_some() {}
    runtime.restart().unwrap();
    runtime.pump(Instant::now());
    let mut messages = Vec::new();
    while let Some(message) = client.poll().unwrap() { messages.push(message); }
    let snapshot = messages.iter().position(|m| matches!(m, ServerMsg::Terminal(_))).unwrap();
    let ready = messages.iter().position(|m| matches!(m, ServerMsg::ProjectionReady { .. })).unwrap();
    assert!(snapshot < ready, "restart also sends terminal snapshots before permitting presentation");
}

#[test]
fn an_empty_or_terminated_session_also_finishes_its_projection() {
    use crate::server::wire::{ClientLink, in_process};
    for terminated in [false, true] {
        let mut runtime = SessionRuntime::new(ServerState::for_tests(Default::default()),
            Session::new(SessionId(42), Default::default()));
        if terminated { runtime.terminate(); }
        let (client, server) = in_process();
        let connection = Rc::new(Connection::new(1, Box::new(server)));
        connection.queue(ClientMsg::Attach);
        runtime.add_connection(connection);
        runtime.pump(Instant::now());
        let mut ready = 0;
        while let Some(message) = client.poll().unwrap() {
            if matches!(message, ServerMsg::ProjectionReady { session_id: SessionId(42) }) { ready += 1; }
        }
        assert_eq!(ready, 1);
    }
}
