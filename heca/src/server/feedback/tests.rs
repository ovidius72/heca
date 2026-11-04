use super::*;
use heca_config::settings::AgentProgramConfig;
use heca_core::agent::AgentStatus;

fn config() -> AgentsConfig {
    let mut config = AgentsConfig::default();
    config.programs.insert(
        "custom".into(),
        AgentProgramConfig {
            enabled: Some(true),
            feedback_events: Some(vec!["NeedsInput".into(), "Done".into()]),
            ..Default::default()
        },
    );
    config
}

fn report(event: &str, turn: &str) -> AgentState {
    let mut state =
        AgentState::new("custom", "Custom", AgentStatus::new("waiting_permission")).event(event);
    state.turn_id = Some(turn.into());
    state
}

#[test]
fn only_configured_events_of_enabled_integrations_raise_feedback() {
    let mut feedback = Feedback::default();
    let mut config = config();
    let pane = PaneId(7);
    for event in ["SessionStart", "Working", "Tool", "Compact", "Interrupted"] {
        feedback.observe(pane, Some(&report(event, "1")), &config);
    }
    assert!(feedback.snapshot().is_empty());
    config.programs.get_mut("custom").unwrap().enabled = Some(false);
    feedback.observe(pane, Some(&report("NeedsInput", "2")), &config);
    assert!(feedback.snapshot().is_empty());
    config.programs.get_mut("custom").unwrap().enabled = Some(true);
    feedback.observe(pane, Some(&report("NeedsInput", "3")), &config);
    assert!(feedback.snapshot()[0].pending());
}

#[test]
fn stale_duplicate_future_and_unknown_acknowledgements_cannot_erase_new_feedback() {
    let mut feedback = Feedback::default();
    let pane = PaneId(7);
    let config = config();
    feedback.observe(pane, Some(&report("NeedsInput", "1")), &config);
    let first = feedback.snapshot()[0].feedback_revision;
    feedback.observe(pane, Some(&report("NeedsInput", "2")), &config);
    let second = feedback.snapshot()[0].feedback_revision;
    feedback.acknowledge(pane, first);
    feedback.acknowledge(pane, first);
    feedback.acknowledge(pane, second + 1);
    feedback.acknowledge(PaneId(99), second);
    assert_eq!(feedback.snapshot()[0].seen_through_revision, first);
    assert!(feedback.snapshot()[0].pending());
    feedback.acknowledge(pane, second);
    assert!(!feedback.snapshot()[0].pending());
    assert_eq!(feedback.snapshot()[0].status, "waiting_permission");
}

#[test]
fn metadata_changes_and_replayed_reports_do_not_generate_new_revisions() {
    let mut feedback = Feedback::default();
    let mut state = report("NeedsInput", "1");
    feedback.observe(PaneId(7), Some(&state), &config());
    let original = feedback.snapshot();
    state.task = Some("A renamed task".into());
    state.model = Some("Another model".into());
    state.session_title = Some("Renamed session".into());
    feedback.observe(PaneId(7), Some(&state), &config());
    feedback.observe(PaneId(7), Some(&state), &config());
    assert_eq!(feedback.snapshot(), original);
}

#[test]
fn event_selection_can_silence_feedback_without_disabling_reporting() {
    let mut feedback = Feedback::default();
    let mut config = config();
    config.programs.get_mut("custom").unwrap().enabled_events = Some(vec!["Done".into()]);
    feedback.observe(PaneId(7), Some(&report("NeedsInput", "1")), &config);
    assert!(feedback.snapshot().is_empty());
    feedback.observe(PaneId(7), Some(&report("Done", "1")), &config);
    assert_eq!(feedback.snapshot().len(), 1);
}

#[test]
fn removing_a_pane_retires_its_projection_without_reusing_revisions() {
    let mut feedback = Feedback::default();
    feedback.observe(PaneId(7), Some(&report("Done", "1")), &config());
    let first = feedback.snapshot()[0].feedback_revision;
    feedback.retain(&HashSet::new());
    assert!(feedback.snapshot().is_empty());
    feedback.observe(PaneId(7), Some(&report("Done", "1")), &config());
    assert!(feedback.snapshot()[0].feedback_revision > first);
}

#[test]
fn runtime_snapshots_alone_do_not_generate_feedback() {
    use crate::app::backend_store::{BackendStore, Program, TerminalSpec};
    use heca_core::{
        backend::FakeBackend,
        layout::{Pane, Session, SessionId},
        runtime::{PaneRuntime, ProcessStatus},
    };
    let mut session = Session::new(SessionId(1), Default::default());
    session.add_column_with(0, 0, Pane::new(PaneId(7), "shell"));
    for program in ["ls", "git status", "false"] {
        for status in [
            ProcessStatus::Running,
            ProcessStatus::Success,
            ProcessStatus::Error,
        ] {
            let mut state = super::super::ServerState::for_tests(Default::default());
            let mut backend = FakeBackend::new(20, 6);
            backend.set_runtime(PaneRuntime {
                program: Some(program.into()),
                status,
                exit_code: Some(1),
                ..Default::default()
            });
            state.backends = BackendStore::new();
            state
                .backends
                .ensure(
                    PaneId(7),
                    TerminalSpec {
                        program: Program::Shell,
                        cwd: None,
                        grid: (20, 6),
                    },
                    |_| Ok(Box::new(backend)),
                )
                .unwrap();
            state.collect_feedback(&session);
            assert!(
                state.feedback.snapshot().is_empty(),
                "{program} has no new completion event"
            );
        }
    }
}

#[test]
fn shell_editor_and_other_program_completions_use_the_same_feedback_store() {
    use crate::app::backend_store::{Program, TerminalSpec};
    use heca_core::backend::{BackendAlert, FakeBackend};
    for program in ["ls", "false", "vim", "custom-program"] {
        let mut state = super::super::ServerState::for_tests(Default::default());
        let mut backend = FakeBackend::new(20, 6);
        backend.queue_alert(BackendAlert::ProcessCompleted { exit_code: 0 });
        backend.queue_alert(BackendAlert::ProcessCompleted { exit_code: 1 });
        backend.queue_alert(BackendAlert::ProcessCompleted { exit_code: 1 });
        state.backends.ensure(PaneId(7), TerminalSpec {
            program: Program::Command(program.into()), cwd: None, grid: (20, 6),
        }, |_| Ok(Box::new(backend))).unwrap();
        state.poll_backends();
        let projection = state.feedback.snapshot();
        assert_eq!(projection[0].feedback_revision, 3, "{program}: every completion is a new event");
        assert_eq!(projection[0].status, "error");
        assert!(projection[0].pending());
        state.poll_backends();
        assert_eq!(state.feedback.snapshot(), projection, "polling does not replay {program}");
        state.feedback.acknowledge(PaneId(7), 3);
        assert!(!state.feedback.snapshot()[0].pending());
    }
}

#[test]
fn one_client_can_claim_a_pending_revisions_sound_but_never_replay_seen_or_old_feedback() {
    let mut feedback = Feedback::default();
    let pane = PaneId(7);
    feedback.completed(pane, 1);
    assert!(!feedback.claim_sound(pane, 2));
    assert!(!feedback.claim_sound(PaneId(99), 1));
    assert!(feedback.claim_sound(pane, 1));
    assert!(!feedback.claim_sound(pane, 1));
    feedback.completed(pane, 0);
    assert!(!feedback.claim_sound(pane, 1));
    feedback.acknowledge(pane, 2);
    assert!(!feedback.claim_sound(pane, 2));
    feedback.completed(pane, 0);
    assert!(feedback.claim_sound(pane, 3));
}
