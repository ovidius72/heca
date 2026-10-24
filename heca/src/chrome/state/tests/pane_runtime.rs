//! Each pane's live facts and terminal viewport, through signals and events.

use super::*;

#[test]
fn pane_runtime_round_trips_through_signals() {
    let s = state();
    let pane = PaneId(42);
    let runtime = PaneRuntime {
        program: Some("nvim".into()),
        status: ProcessStatus::Running,
        cwd: Some(PathBuf::from("/tmp/project")),
        exit_code: Some(7),
        git: Some(GitInfo {
            branch: Some("main".into()),
            ahead: 1,
            behind: 2,
            added: 3,
            modified: 4,
            deleted: 5,
            dirty: true,
        }),
        kind: ContentKind::Terminal,
        agent: Some(heca_core::agent::AgentState::new(
            "demo", "Demo", heca_core::agent::AgentStatus::new("reviewing"),
        )),
    };

    s.panes.set_runtime(pane, &runtime, Some("my pane"));

    let mirrored = s
        .panes
        .with(pane, |runtime| runtime.expect("pane runtime").snapshot());
    assert_eq!(mirrored, runtime);
    assert_eq!(s.panes.custom_name(pane), Some("my pane".to_string()));
}

#[test]
fn pane_runtime_events_emit_only_on_change() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s.events().subscribe("*", move |event| {
        seen_events.borrow_mut().push(event.name().to_string());
    });
    let pane = PaneId(9);

    // Each field changes once and is then written again unchanged: only the change announces.
    let git = GitInfo {
        branch: Some("feat".into()),
        ahead: 0,
        behind: 0,
        added: 1,
        modified: 0,
        deleted: 0,
        dirty: true,
    };
    let mut runtime = PaneRuntime::default();
    let steps: [&dyn Fn(&mut PaneRuntime); 4] = [
        &|r| r.program = Some("bash".into()),
        &|r| r.status = ProcessStatus::Running,
        &|r| r.cwd = Some(PathBuf::from("/repo")),
        &|r| r.git = Some(git.clone()),
    ];
    for step in steps {
        step(&mut runtime);
        s.panes.set_runtime(pane, &runtime, None);
        s.panes.set_runtime(pane, &runtime, None);
    }

    assert_eq!(
        seen.borrow().as_slice(),
        [
            "pane.process.changed",
            "pane.status.changed",
            "pane.cwd.changed",
            "pane.git.changed",
        ],
    );
}

#[test]
fn agent_events_carry_transitions_without_changing_process_status() {
    use heca_core::agent::{AgentState, AgentStatus};
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let captured = seen.clone();
    let _sub = s.events().subscribe("pane.agent.changed", move |event| {
        captured.borrow_mut().push(event.clone());
    });
    let pane = PaneId(19);
    let working = AgentState::new("demo", "Demo", AgentStatus::new("working"));
    let waiting = AgentState::new("demo", "Demo", AgentStatus::new("waiting_input"));
    let mut runtime = PaneRuntime { agent: Some(working.clone()), ..Default::default() };
    s.panes.set_runtime(pane, &runtime, None);
    s.panes.set_runtime(pane, &runtime, None);
    runtime.agent = Some(waiting.clone());
    s.panes.set_runtime(pane, &runtime, None);
    runtime.agent = None;
    s.panes.set_runtime(pane, &runtime, None);
    assert_eq!(seen.borrow().as_slice(), &[
        ChromeEvent::PaneAgentChanged { pane, previous: None, agent: Some(Box::new(working.clone())) },
        ChromeEvent::PaneAgentChanged { pane, previous: Some(Box::new(working)), agent: Some(Box::new(waiting.clone())) },
        ChromeEvent::PaneAgentChanged { pane, previous: Some(Box::new(waiting)), agent: None },
    ]);
    assert_eq!(s.panes.runtime(pane).unwrap().status, ProcessStatus::Idle);
}

#[test]
fn terminal_viewport_defaults_are_zeros() {
    let s = state();
    let pane = PaneId(10);
    // A pane that was never written via set_viewport returns None.
    assert_eq!(s.panes.terminal_viewport(pane), None);

    // After a viewport write, it returns the expected values.
    s.panes.set_viewport(pane, 42, false, 200);
    let vp = s.panes.terminal_viewport(pane).unwrap();
    assert_eq!(
        vp,
        crate::host::TerminalViewport {
            viewport_offset: 42,
            at_bottom: false,
            scrollback_rows: 200,
        }
    );
}

#[test]
fn terminal_viewport_emits_only_on_change() {
    let s = state();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let seen_events = seen.clone();
    let _sub = s.events().subscribe("*", move |event| {
        seen_events.borrow_mut().push(event.name().to_string());
    });
    let pane = PaneId(11);

    // First write -> terminal.viewport.changed emitted.
    s.panes.set_viewport(pane, 10, false, 50);
    // Identical write -> no event.
    s.panes.set_viewport(pane, 10, false, 50);
    // Single field change -> event fired.
    s.panes.set_viewport(pane, 11, false, 50);
    // All fields changed -> single event.
    s.panes.set_viewport(pane, 0, true, 100);
    // Back to the last values -> event fired (still a change from current).
    s.panes.set_viewport(pane, 11, false, 50);

    assert_eq!(
        seen.borrow().as_slice(),
        [
            "terminal.viewport.changed",
            "terminal.viewport.changed",
            "terminal.viewport.changed",
            "terminal.viewport.changed",
        ],
        "expected only 4 events: first write, offset change, all-field change, revert"
    );
}

#[test]
fn terminal_viewport_does_not_require_runtime_preinit() {
    let s = state();
    let pane = PaneId(12);
    // set_viewport on a fresh pane without set_runtime should work.
    s.panes.set_viewport(pane, 5, true, 30);
    let vp = s.panes.terminal_viewport(pane).unwrap();
    assert_eq!(vp.viewport_offset, 5);
    assert!(vp.at_bottom);
    assert_eq!(vp.scrollback_rows, 30);
}
