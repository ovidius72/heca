use std::rc::Rc;

use heca_config::{programs::ProgramsConfig, settings::{AgentStatusConfig, AgentsConfig}};
use heca_core::{agent::{AgentState, AgentStatus}, layout::PaneId, runtime::PaneRuntime};
use heca_grid_ui::{Component, DrawCommand, builders::{LayoutExt, Parent}};
use heca_grid_ui::reactive::{signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::Length;
use heca_grid_ui::theme::Theme;
use heca_grid_ui::widgets::Flex;

use crate::chrome::{PaneFacts, panes::pane_items::LineCx};

#[test]
fn status_icons_and_tones_change_together_without_rebuilding_the_row() {
    use heca_grid_ui::widgets::Glyph;
    let theme = Theme::default();
    let agent = signal(Some(AgentState::new("custom", "Program", AgentStatus::new("working"))));
    let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || PaneFacts::of(
        PaneId(1), &ProgramsConfig::default(), "shell", None,
        Some(&PaneRuntime { agent: agent.get(), ..Default::default() }),
    ));
    let statuses = signal(Rc::new(AgentsConfig::default().statuses));
    let mut root = Flex::column().width(Length::FULL)
        .child(super::build(&LineCx { theme: &theme, facts, agent_statuses: statuses }));
    for (state, label, glyph, color) in [
        ("working", "Working", Glyph::Lightning, theme.colors.accent),
        ("waiting_input", "Waiting…", Glyph::ChatCircleDots, theme.colors.warning),
        ("waiting_permission", "Waiting…", Glyph::Lock, theme.colors.warning),
        ("finished", "Finished", Glyph::Check, theme.colors.success),
        ("success", "Success", Glyph::CheckCircle, theme.colors.success),
        ("error", "Error", Glyph::XCircle, theme.colors.danger),
    ] {
        agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new(state));
        let scene = crate::chrome::paint_chrome_root(&mut root, 640.0, 240.0, &theme);
        for expected in [label.to_owned(), glyph.primary_char().unwrap().to_string()] {
            assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text)
                if text.text == expected && text.color == color)), "wrong icon/tone for {state}: {expected}");
        }
    }
    statuses.update(|statuses| {
        Rc::make_mut(statuses).insert("custom_ready".into(), AgentStatusConfig {
            label: Some("Ready".into()), icon: Some("check_circle".into()),
            tone: Some("success".into()), ..Default::default()
        });
    });
    agent.update(|agent| agent.as_mut().unwrap().status = AgentStatus::new("custom_ready"));
    let scene = crate::chrome::paint_chrome_root(&mut root, 640.0, 240.0, &theme);
    assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text)
        if text.text == "Ready" && text.color == theme.colors.success)));
}

#[test]
fn the_line_follows_reports_and_config_without_a_rebuild() {
    let theme = Theme::default();
    let agent = signal(None::<AgentState>);
    let statuses = signal(Rc::new(AgentsConfig::default().statuses));
    let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || PaneFacts::of(
        PaneId(1), &ProgramsConfig::default(), "shell", None,
        Some(&PaneRuntime { agent: agent.get(), ..Default::default() }),
    ));
    let line = super::build(&LineCx { theme: &theme, facts, agent_statuses: statuses });
    let mut root = Flex::column().width(Length::FULL).child(line);
    let draw = |root: &mut Flex| {
        crate::chrome::paint_chrome_root(root, 340.0, 240.0, &theme).iter()
            .filter_map(|command| match command {
                DrawCommand::Text(text) => Some(text.text.clone()),
                _ => None,
            }).collect::<Vec<_>>()
    };
    assert!(draw(&mut root).is_empty(), "non-agent panes have no extra line");
    let mut report = AgentState::new("custom", "My program", AgentStatus::new("reviewing"));
    report.task = Some("Review parser".into());
    agent.set(Some(report));
    let text = draw(&mut root);
    for expected in ["My program", "reviewing", "Review parser"] {
        assert!(text.iter().any(|text| text == expected), "missing {expected}: {text:?}");
    }
    statuses.update(|statuses| {
        Rc::make_mut(statuses).insert("reviewing".into(), AgentStatusConfig {
            label: Some("Checking".into()), ..Default::default()
        });
    });
    assert!(draw(&mut root).iter().any(|text| text == "Checking"));
    agent.set(None);
    assert!(draw(&mut root).is_empty());
}

#[test]
fn long_reported_text_stays_inside_a_narrow_sidebar() {
    let theme = Theme::default();
    let mut report = AgentState::new("custom", "A very long program name", AgentStatus::new("working"));
    report.task = Some("A task title long enough to extend far beyond a narrow sidebar".into());
    let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || PaneFacts::of(
        PaneId(1), &ProgramsConfig::default(), "shell", None,
        Some(&PaneRuntime { agent: Some(report.clone()), ..Default::default() }),
    ));
    let line = super::build(&LineCx {
        theme: &theme, facts,
        agent_statuses: signal(Rc::new(AgentsConfig::default().statuses)),
    });
    let mut root = Flex::column().width(Length::FULL).child(line);
    for width in [150.0, 240.0, 420.0] {
        let scene = crate::chrome::paint_chrome_root(&mut root, width, 240.0, &theme);
        for command in scene.iter() {
            if let DrawCommand::Text(text) = command {
                assert!(text.rect.loc.x >= 0.0 && text.rect.loc.x < f64::from(width), "text starts outside: {text:?}");
                assert!(text.rect.loc.x + text.rect.size.w <= f64::from(width), "text exceeds its sidebar: {text:?}");
            }
        }
        assert!(root.base().bounds.size.w <= f64::from(width));
    }
}

#[test]
fn the_identity_uses_reported_model_and_session_title_then_the_pane_name() {
    let theme = Theme::default();
    let agent = signal(Some(AgentState::new("custom", "Program", AgentStatus::new("working"))));
    let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || PaneFacts::of(
        PaneId(1), &ProgramsConfig::default(), "shell", Some("My pane"),
        Some(&PaneRuntime { agent: agent.get(), ..Default::default() }),
    ));
    let mut root = Flex::column().width(Length::FULL).child(super::build(&LineCx {
        theme: &theme, facts,
        agent_statuses: signal(Rc::new(AgentsConfig::default().statuses)),
    }));
    let words = |root: &mut Flex| crate::chrome::paint_chrome_root(root, 480.0, 240.0, &theme).iter()
        .filter_map(|cmd| match cmd { DrawCommand::Text(text) => Some(text.text.clone()), _ => None })
        .collect::<Vec<_>>();
    assert!(words(&mut root).iter().any(|text| text == "My pane"));
    agent.update(|agent| {
        let agent = agent.as_mut().unwrap();
        agent.model = Some("Actual model".into());
        agent.session_title = Some("Actual session".into());
    });
    let painted = words(&mut root);
    assert!(painted.iter().any(|text| text == "Actual model"));
    assert!(painted.iter().any(|text| text == "Actual session"));
    assert!(!painted.iter().any(|text| text == "My pane" || text == "Program"));
}

#[test]
fn arbitrary_driver_artwork_uses_the_shared_image_widget_and_clears_in_place() {
    use heca_core::agent::AgentIcon;
    use heca_grid_ui::scene::{HostDraw, Scene};
    let theme = Theme { font_size: 22.0, ..Default::default() };
    let mut report = AgentState::new("external-example", "External", AgentStatus::new("working"));
    report.session_title = Some("Session title".into());
    report.icon = Some(AgentIcon::svg(include_str!("../../../../../heca-renderer/assets/showcase/image.svg")));
    let agent = signal(Some(report));
    let facts: Rc<dyn Fn() -> PaneFacts> = Rc::new(move || PaneFacts::of(
        PaneId(1), &ProgramsConfig::default(), "shell", None,
        Some(&PaneRuntime { agent: agent.get(), ..Default::default() }),
    ));
    let mut root = Flex::column().width(Length::FULL).child(super::build(&LineCx {
        theme: &theme, facts,
        agent_statuses: signal(Rc::new(AgentsConfig::default().statuses)),
    }));
    let images = |scene: &Scene| scene.iter().filter(|cmd| matches!(cmd, DrawCommand::Host(host) if matches!(host.draw, HostDraw::Image { .. }))).count();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let scene = crate::chrome::paint_chrome_root(&mut root, 360.0, 240.0, &theme);
        if images(&scene) == 1 {
            for width in [320.0, 480.0] {
                let scene = crate::chrome::paint_chrome_root(&mut root, width, 240.0, &theme);
                let text = |name| scene.iter().find_map(|cmd| match cmd {
                    DrawCommand::Text(text) if text.text == name => Some(text.rect), _ => None,
                }).unwrap_or_else(|| panic!("missing complete {name} at {width}px: {scene:?}"));
                let primary = text("External");
                let status = text("Working");
                let session = text("Session title");
                assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text)
                    if text.text == "Session title" && text.color == theme.colors.foreground)),
                    "the session name must keep full foreground contrast");
                let logo = scene.iter().find_map(|cmd| match cmd {
                    DrawCommand::Host(host) if matches!(host.draw, HostDraw::Image { .. }) => Some(host.rect),
                    _ => None,
                }).unwrap();
                let center_y = |rect: heca_core::layout::Rectangle| rect.loc.y + rect.size.h / 2.0;
                assert!((center_y(logo) - center_y(primary)).abs() <= 1.0);
                assert!((center_y(status) - center_y(primary)).abs() <= 1.0);
                assert!(session.loc.y > primary.loc.y);
                assert_eq!(session.loc.x, primary.loc.x);
            }
            break
        }
        assert!(std::time::Instant::now() < deadline, "driver artwork never reached the shared scene");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    agent.update(|agent| agent.as_mut().unwrap().icon = None);
    let scene = crate::chrome::paint_chrome_root(&mut root, 360.0, 240.0, &theme);
    assert_eq!(images(&scene), 0);
    assert!(scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(text) if text.text == "External")));
}
