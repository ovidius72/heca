use super::*;
use heca_config::settings::{AgentStatusConfig, AgentsConfig};
use heca_core::{agent::{AgentState, AgentStatus}, runtime::PaneRuntime, layout::PaneId};

fn facts(process: ProcessStatus, agent: Option<AgentState>) -> PaneRuntime {
    PaneRuntime { program: Some("demo".into()), status: process, agent, ..Default::default() }
}

#[test]
fn only_real_program_or_agent_transitions_flash() {
    let statuses = AgentsConfig::default().statuses;
    let working = AgentState::new("external", "External", AgentStatus::new("working"));
    let previous = State::of(facts(ProcessStatus::Running, Some(working.clone())));
    assert_eq!(transition(None, &previous, &statuses), None, "mounting is not a status change");
    let mut metadata = working.clone();
    metadata.model = Some("New model name".into());
    metadata.session_title = Some("New session title".into());
    metadata.task = Some("Another task description".into());
    let repeated = State::of(facts(ProcessStatus::Running, Some(metadata)));
    assert_eq!(transition(Some(&previous), &repeated, &statuses), None, "repeated or metadata-only reports stay quiet");
    for (status, tone) in [("waiting_input", Tone::Warning), ("waiting_permission", Tone::Warning),
        ("finished", Tone::Success), ("error", Tone::Danger)] {
        let mut next = working.clone();
        next.status = AgentStatus::new(status);
        let next = State::of(facts(ProcessStatus::Running, Some(next)));
        assert_eq!(transition(Some(&previous), &next, &statuses), Some(tone));
    }
    let idle = State::of(facts(ProcessStatus::Idle, None));
    for (process, tone) in [(ProcessStatus::Running, Tone::Accent), (ProcessStatus::Success, Tone::Success),
        (ProcessStatus::Error, Tone::Danger)] {
        assert_eq!(transition(Some(&idle), &State::of(facts(process, None)), &statuses), Some(tone));
    }
}

#[test]
fn plugin_defined_statuses_use_the_configured_theme_tone() {
    let mut statuses = AgentsConfig::default().statuses;
    statuses.insert("review_ready".into(), AgentStatusConfig { tone: Some("success".into()), ..Default::default() });
    let previous = State::of(facts(ProcessStatus::Running, None));
    let custom = AgentState::new("custom.driver", "Custom", AgentStatus::new("review_ready"));
    let current = State::of(facts(ProcessStatus::Running, Some(custom)));
    assert_eq!(transition(Some(&previous), &current, &statuses), Some(Tone::Success));
}

#[test]
fn process_discovery_after_the_first_agent_report_is_not_a_work_state_change() {
    let statuses = AgentsConfig::default().statuses;
    let shell = PaneRuntime { program: Some("preview-shell".into()), ..Default::default() };
    let initial = State::of(shell.clone());
    let reporting = PaneRuntime { agent: Some(AgentState::new("demo", "Demo", AgentStatus::new("working"))), ..shell };
    let reported = State::of(reporting.clone());
    assert_eq!(transition(Some(&initial), &reported, &statuses), None);
    let discovered = State::of(PaneRuntime { program: Some("demo".into()), status: ProcessStatus::Running, ..reporting });
    assert_eq!(transition(Some(&reported), &discovered, &statuses), None,
        "prefix+Enter startup reports work state before the process monitor catches up");
    let mut waiting = discovered;
    waiting.agent.as_mut().unwrap().1 = "waiting_input".into();
    assert_eq!(transition(Some(&reported), &waiting, &statuses), Some(Tone::Warning),
        "a real work-state change still flashes even when program discovery arrives with it");
    let running = State::of(facts(ProcessStatus::Running, None));
    let renamed = State::of(PaneRuntime { program: Some("another name".into()), ..facts(ProcessStatus::Running, None) });
    assert_eq!(transition(Some(&running), &renamed, &statuses), None);
}

#[test]
fn adding_a_pane_preserves_the_old_flash_and_does_not_flash_the_new_pane() {
    use heca_grid_ui::{DrawCommand, FlashExt, Flex, Theme, builders::LayoutExt,
        reactive::{signal, SignalUpdate}};
    let chrome = crate::chrome::SharedChromeState::new(280.0, 260.0);
    let statuses = signal(Rc::new(AgentsConfig::default().statuses));
    let first = chrome.panes.signals(PaneId(1));
    let active = signal(None);
    bind(first, PaneId(1), statuses, active);
    let initial = facts(ProcessStatus::Running,
        Some(AgentState::new("demo", "Demo", AgentStatus::new("working"))));
    chrome.panes.set_runtime(PaneId(1), &initial, None);
    let mut old_row = Flex::row().width(320.0).height(40.0).flash(first.flash);
    let painted = |row: &mut Flex| crate::chrome::paint_chrome_root(row, 320.0, 80.0, &Theme::default())
        .iter().any(|cmd| matches!(cmd, DrawCommand::Rect(rect) if rect.border.is_some()));
    assert!(!painted(&mut old_row), "the first program and agent snapshot initializes silently");
    first.agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new("waiting_permission"));
    assert!(painted(&mut old_row));

    // Rebuilding the first row uses its pane-owned handle, not a fresh animation.
    let rebuilt = chrome.panes.signals(PaneId(1));
    let mut old_row = Flex::row().width(320.0).height(40.0).flash(rebuilt.flash);
    bind(rebuilt, PaneId(1), statuses, active);
    chrome.panes.set_runtime(PaneId(2), &PaneRuntime { program: Some("preview-shell".into()), ..Default::default() }, None);
    let second = chrome.panes.signals(PaneId(2));
    active.set(Some(PaneId(2)));
    bind(second, PaneId(2), statuses, active);
    let mut new_row = Flex::row().width(320.0).height(40.0).flash(second.flash);
    chrome.panes.set_agent(PaneId(2), initial.agent.clone());
    assert!(!painted(&mut new_row), "the initial agent report is silent before process discovery");
    chrome.panes.set_runtime(PaneId(2), &initial, None);
    bind(rebuilt, PaneId(1), statuses, signal(Some(PaneId(1))));
    assert!(painted(&mut old_row), "the original pane retains its pending attention");
    assert!(!painted(&mut new_row), "a new column never inherits the old pane's flash");
}

#[test]
fn a_bound_row_flashes_for_a_new_status_and_stays_quiet_for_metadata() {
    use heca_grid_ui::{DrawCommand, FlashExt, Flex, Length, Theme, builders::LayoutExt,
        reactive::{signal, SignalUpdate}};
    let chrome = crate::chrome::SharedChromeState::new(280.0, 260.0);
    chrome.panes.set_runtime(PaneId(1), &facts(ProcessStatus::Idle, None), None);
    let runtime = chrome.panes.signals(PaneId(1));
    let flash = runtime.flash;
    let active = signal(None);
    bind(runtime, PaneId(1), signal(Rc::new(AgentsConfig::default().statuses)), active);
    let mut row = Flex::row().width(Length::FULL).height(40.0).flash(flash);
    let theme = Theme::default();
    let painted_tone = |row: &mut Flex| {
        let scene = crate::chrome::paint_chrome_root(row, 320.0, 80.0, &theme);
        scene.iter().find_map(|cmd| match cmd {
            DrawCommand::Rect(rect) => rect.border.map(|border| border.color.with_alpha(255)), _ => None,
        })
    };
    assert_eq!(painted_tone(&mut row), None);
    runtime.status.set(ProcessStatus::Running);
    assert_eq!(painted_tone(&mut row), Some(theme.colors.accent.with_alpha(255)));
    active.set(Some(PaneId(1)));
    assert_eq!(painted_tone(&mut row), None, "focusing the pane acknowledges its repeating flash");
    active.set(None);
    assert_eq!(painted_tone(&mut row), None, "leaving a pane does not restart an acknowledged flash");
    runtime.agent.set(Some(AgentState::new("demo", "Demo", AgentStatus::new("waiting_input"))));
    assert!(painted_tone(&mut row).is_some());
    flash.clear();
    runtime.agent.update(|agent| agent.as_mut().unwrap().task = Some("New task text".into()));
    assert_eq!(painted_tone(&mut row), None);
    runtime.agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new("waiting_input"));
    assert_eq!(painted_tone(&mut row), None);
    runtime.agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new("finished"));
    assert_eq!(painted_tone(&mut row), Some(theme.colors.success.with_alpha(255)));
    active.set(Some(PaneId(1)));
    assert_eq!(painted_tone(&mut row), None);
    runtime.agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new("working"));
    assert_eq!(painted_tone(&mut row), Some(theme.colors.accent.with_alpha(255)),
        "a state change in the focused pane still gives a finite burst");
}
