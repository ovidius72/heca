use super::*;
use heca_core::agent::AgentStatus;

fn select(previous: Option<&AgentState>, current: Option<&AgentState>, config: &AgentsConfig, generation: u64) -> Option<Sound> {
    transition(previous, current, config, &crate::audio::library(), generation).unwrap()
}

fn configured() -> AgentsConfig {
    let mut config = AgentsConfig::default();
    config.sounds.enabled = true;
    config.statuses.get_mut("finished").unwrap().sound = Some("finish.wav".into());
    config
}

#[test]
fn a_status_transition_selects_the_configured_sound() {
    let config = configured();
    let working = AgentState::new("demo", "Demo", AgentStatus::new("working"));
    let finished = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    assert_eq!(select(Some(&working), Some(&finished), &config, 3), Some(Sound {
        source: Audio::load("finish.wav"), volume: config.sounds.volume, generation: 3,
    }));
}

#[test]
fn repeated_status_and_task_changes_are_silent() {
    let config = configured();
    let previous = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    let mut next = previous.clone();
    next.task = Some("A different task".into());
    assert_eq!(select(Some(&previous), Some(&next), &config, 0), None);
}

#[test]
fn disabled_playback_and_unconfigured_statuses_are_silent() {
    let mut config = configured();
    let finished = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    config.sounds.enabled = false;
    assert_eq!(select(None, Some(&finished), &config, 0), None);
    config.sounds.enabled = true;
    let custom = AgentState::new("demo", "Demo", AgentStatus::new("custom"));
    assert_eq!(select(None, Some(&custom), &config, 0), None);
    assert_eq!(select(Some(&finished), None, &config, 0), None);
}

#[test]
fn added_statuses_use_the_same_sound_policy() {
    let mut config = configured();
    config.statuses.insert("reviewing".into(), heca_config::settings::AgentStatusConfig {
        sound: Some("review.wav".into()), ..Default::default()
    });
    let report = AgentState::new("external", "External", AgentStatus::new("reviewing"));
    assert_eq!(select(None, Some(&report), &config, 0).unwrap().source, Audio::load("review.wav"));
}

#[test]
fn the_event_subscriber_queues_one_sound_and_obeys_a_config_reload() {
    let events = ChromeEventBus::default();
    let sounds = AgentSounds::new(&events);
    let mut config = configured();
    sounds.configure(&config);
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    sounds.policy.borrow_mut().worker = Some(sender);
    let report = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    let pane = heca_core::layout::PaneId(1);
    events.emit(ChromeEvent::PaneAgentChanged { pane, previous: None, agent: Some(Box::new(report.clone())) });
    events.emit(ChromeEvent::PaneAgentChanged {
        pane, previous: Some(Box::new(report.clone())), agent: Some(Box::new(report.clone())),
    });
    assert_eq!(receiver.try_iter().count(), 1);
    config.sounds.enabled = false;
    sounds.configure(&config);
    events.emit(ChromeEvent::PaneAgentChanged { pane, previous: None, agent: Some(Box::new(report)) });
    assert_eq!(receiver.try_iter().count(), 0);
}

#[test]
fn bundled_defaults_and_per_status_mute_and_volume_follow_configuration() {
    let mut config = AgentsConfig::default();
    assert!(!config.sounds.enabled);
    let finished = AgentState::new("custom", "Custom", AgentStatus::new("finished"));
    assert!(select(None, Some(&finished), &config, 0).is_none());
    config.sounds.enabled = true;
    let source = select(None, Some(&finished), &config, 0).unwrap();
    assert!(matches!(source.source, Audio::Bytes(_)));
    assert_eq!(source.volume, config.sounds.volume);
    let sound = config.statuses.get_mut("finished").unwrap().sound.as_mut().unwrap();
    sound.volume = Some(0.2);
    assert_eq!(select(None, Some(&finished), &config, 0).unwrap().volume, 0.2);
    config.statuses.get_mut("finished").unwrap().sound.as_mut().unwrap().enabled = false;
    assert!(select(None, Some(&finished), &config, 0).is_none());
    let working = AgentState::new("custom", "Custom", AgentStatus::new("working"));
    assert!(select(None, Some(&working), &config, 0).is_none(), "ordinary progress stays silent");
}

#[test]
fn a_custom_status_can_select_an_added_asset_and_missing_assets_report_an_error() {
    let mut config = configured();
    config.statuses.get_mut("finished").unwrap().sound = Some(crate::audio::Sound::asset("plugin.ready"));
    let report = AgentState::new("custom", "Custom", AgentStatus::new("finished"));
    let audio = Audio::from_bytes(b"fixture bytes");
    let library = SoundLibrary::new().sound("plugin.ready", audio.clone());
    assert_eq!(transition(None, Some(&report), &config, &library, 0).unwrap().unwrap().source, audio);
    assert!(transition(None, Some(&report), &config, &SoundLibrary::new(), 0).unwrap_err().contains("plugin.ready"));
}
