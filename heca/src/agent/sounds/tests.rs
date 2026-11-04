use super::*;
use heca_core::agent::{AgentState, AgentStatus};
use heca_config::settings::SettingsConfig;

fn select(status: &str, config: &SettingsConfig, generation: u64) -> Option<Sound> {
    for_status(status, config.notification_sound, &config.agents, &crate::audio::library(), generation).unwrap()
}

fn configured() -> SettingsConfig {
    let mut config = SettingsConfig { notification_sound: true, ..Default::default() };
    config.agents.statuses.get_mut("finished").unwrap().sound = Some("finish.wav".into());
    config
}

#[test]
fn a_status_transition_selects_the_configured_sound() {
    let config = configured();
    assert_eq!(select("finished", &config, 3), Some(Sound {
        source: Audio::load("finish.wav"), volume: config.agents.sounds.volume, generation: 3,
    }));
}

#[test]
fn repeated_status_and_task_changes_are_silent() {
    let events = ChromeEventBus::default();
    let sounds = AgentSounds::new(&events);
    let config = configured();
    sounds.configure(&config.agents, config.notification_sound);
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    sounds.policy.borrow_mut().worker = Some(sender);
    let mut report = AgentState::new("demo", "Demo", AgentStatus::new("finished"));
    report.task = Some("A different task".into());
    events.emit(ChromeEvent::PaneAgentChanged { pane: heca_core::layout::PaneId(1), previous: None, agent: Some(Box::new(report)) });
    assert_eq!(receiver.try_iter().count(), 0, "only server-granted feedback can play");
}

#[test]
fn disabled_playback_and_unconfigured_statuses_are_silent() {
    let mut config = configured();
    config.notification_sound = false;
    assert_eq!(select("finished", &config, 0), None);
    config.notification_sound = true;
    assert_eq!(select("custom", &config, 0), None);
}

#[test]
fn added_statuses_use_the_same_sound_policy() {
    let mut config = configured();
    config.agents.statuses.insert("reviewing".into(), heca_config::settings::AgentStatusConfig {
        sound: Some("review.wav".into()), ..Default::default()
    });
    assert_eq!(select("reviewing", &config, 0).unwrap().source, Audio::load("review.wav"));
}

#[test]
fn the_event_subscriber_queues_one_sound_and_obeys_a_config_reload() {
    let events = ChromeEventBus::default();
    let sounds = AgentSounds::new(&events);
    let mut config = configured();
    sounds.configure(&config.agents, config.notification_sound);
    let (sender, receiver) = std::sync::mpsc::sync_channel(4);
    sounds.policy.borrow_mut().worker = Some(sender);
    let pane = heca_core::layout::PaneId(1);
    let feedback = || ChromeEvent::PaneFeedback { session: heca_core::layout::SessionId(1), pane, revision: 42, status: "finished".into() };
    events.emit(feedback());
    assert_eq!(receiver.try_iter().count(), 1);
    config.notification_sound = false;
    sounds.configure(&config.agents, config.notification_sound);
    events.emit(feedback());
    assert_eq!(receiver.try_iter().count(), 0);
}

#[test]
fn bundled_defaults_and_per_status_mute_and_volume_follow_configuration() {
    let mut config = SettingsConfig::default();
    assert!(config.notification_sound);
    let source = select("finished", &config, 0).unwrap();
    assert!(matches!(source.source, Audio::Bytes(_)));
    assert_eq!(source.volume, config.agents.sounds.volume);
    config.notification_sound = false;
    assert!(select("finished", &config, 0).is_none());
    config.notification_sound = true;
    let sound = config.agents.statuses.get_mut("finished").unwrap().sound.as_mut().unwrap();
    sound.volume = Some(0.2);
    assert_eq!(select("finished", &config, 0).unwrap().volume, 0.2);
    config.agents.statuses.get_mut("finished").unwrap().sound.as_mut().unwrap().enabled = false;
    assert!(select("finished", &config, 0).is_none());
    assert!(select("working", &config, 0).is_none(), "ordinary progress stays silent");
}

#[test]
fn a_custom_status_can_select_an_added_asset_and_missing_assets_report_an_error() {
    let mut config = configured();
    config.agents.statuses.get_mut("finished").unwrap().sound = Some(crate::audio::Sound::asset("plugin.ready"));
    let audio = Audio::from_bytes(b"fixture bytes");
    let library = SoundLibrary::new().sound("plugin.ready", audio.clone());
    assert_eq!(for_status("finished", config.notification_sound, &config.agents, &library, 0).unwrap().unwrap().source, audio);
    assert!(for_status("finished", config.notification_sound, &config.agents, &SoundLibrary::new(), 0).unwrap_err().contains("plugin.ready"));
}
