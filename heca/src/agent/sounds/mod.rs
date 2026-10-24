//! One subscriber to status changes; file decoding and playback stay off the UI thread.

mod worker;

use std::{cell::RefCell, rc::Rc, sync::{Arc, atomic::{AtomicU64, Ordering}, mpsc::SyncSender}};

use heca_config::settings::AgentsConfig;
use heca_core::agent::AgentState;

use crate::chrome::{ChromeEvent, ChromeEventBus, ChromeSubscription};
use crate::audio::{Audio, SoundLibrary};

#[derive(Clone, Debug, PartialEq)]
struct Sound {
    source: Audio,
    volume: f32,
    generation: u64,
}

struct Policy {
    config: AgentsConfig,
    worker: Option<SyncSender<Sound>>,
    generation: Arc<AtomicU64>,
    library: SoundLibrary,
}

/// Owned by the runtime mirror. A clone of that store shares this same subscription.
pub(crate) struct AgentSounds {
    policy: Rc<RefCell<Policy>>,
    _subscription: ChromeSubscription,
}

impl AgentSounds {
    pub(crate) fn new(events: &ChromeEventBus) -> Self {
        let policy = Rc::new(RefCell::new(Policy {
            config: AgentsConfig::default(), worker: None, generation: Arc::new(AtomicU64::new(0)),
            library: crate::audio::library(),
        }));
        let subscribed = policy.clone();
        let subscription = events.subscribe("pane.agent.changed", move |event| {
            let ChromeEvent::PaneAgentChanged { previous, agent, .. } = event else { return };
            let mut policy = subscribed.borrow_mut();
            let generation = policy.generation.load(Ordering::Relaxed);
            let sound = match transition(previous.as_deref(), agent.as_deref(), &policy.config, &policy.library, generation) {
                Ok(Some(sound)) => sound,
                Ok(None) => return,
                Err(error) => { eprintln!("[heca] sound: {error}"); return }
            };
            if policy.worker.is_none() {
                match worker::start(policy.config.sounds.queue_capacity, policy.generation.clone()) {
                    Ok(sender) => policy.worker = Some(sender),
                    Err(error) => { eprintln!("[heca] cannot start sound playback: {error}"); return }
                }
            }
            if let Some(sender) = &policy.worker {
                // A busy audio device must never block a terminal update.
                if let Err(std::sync::mpsc::TrySendError::Disconnected(_)) = sender.try_send(sound) {
                    policy.worker = None;
                }
            }
        });
        Self { policy, _subscription: subscription }
    }

    pub(crate) fn configure(&self, config: &AgentsConfig) {
        let mut policy = self.policy.borrow_mut();
        if policy.config == *config { return }
        policy.generation.fetch_add(1, Ordering::Relaxed);
        policy.worker = None;
        policy.config = config.clone();
    }
}

impl std::fmt::Debug for AgentSounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentSounds").finish_non_exhaustive()
    }
}

fn transition(previous: Option<&AgentState>, current: Option<&AgentState>, config: &AgentsConfig, library: &SoundLibrary, generation: u64) -> Result<Option<Sound>, String> {
    if !config.sounds.enabled { return Ok(None) }
    let Some(current) = current else { return Ok(None) };
    if previous.is_some_and(|previous| previous.driver == current.driver && previous.status == current.status) {
        return Ok(None);
    }
    let Some(sound) = config.statuses.get(current.status.as_str()).and_then(|status| status.sound.as_ref()) else { return Ok(None) };
    if !sound.enabled { return Ok(None) }
    let volume = sound.volume.unwrap_or(config.sounds.volume);
    if !volume.is_finite() { return Ok(None) }
    Ok(Some(Sound { source: library.resolve(&sound.source)?, volume: volume.clamp(0.0, 1.0), generation }))
}

#[cfg(test)]
mod tests;
