//! Local animation cursors over the server's shared feedback projection.

use crate::server::feedback::PaneFeedback;
use heca_core::layout::PaneId;
use std::collections::BTreeMap;

#[derive(Default)]
struct Presented {
    revision: u64,
    acknowledged: u64,
    flashing: bool,
    sound_granted: u64,
    sound_played: u64,
}

#[derive(Default)]
pub(super) struct FeedbackView {
    panes: Vec<PaneFeedback>,
    presented: BTreeMap<PaneId, Presented>,
    retired: Vec<PaneId>,
}

#[derive(Debug, PartialEq)]
enum Effect {
    Clear(PaneId),
    Pulse(PaneId, String),
    Acknowledge(PaneId, u64),
    Sound(PaneId, u64, String),
}

impl FeedbackView {
    pub(super) fn grant_sound(&mut self, pane: PaneId, revision: u64) {
        let previous = self.presented.entry(pane).or_default();
        previous.sound_granted = previous.sound_granted.max(revision);
    }

    pub(super) fn receive(&mut self, panes: Vec<PaneFeedback>) {
        self.presented.retain(|pane, previous| {
            let live = panes.iter().any(|feedback| feedback.pane_id == *pane);
            if !live && previous.flashing {
                self.retired.push(*pane);
            }
            live
        });
        self.panes = panes;
    }

    fn project(&mut self, focused: Option<PaneId>) -> Vec<Effect> {
        let mut effects = self
            .retired
            .drain(..)
            .map(Effect::Clear)
            .collect::<Vec<_>>();
        for feedback in &self.panes {
            let pane = feedback.pane_id;
            let previous = self.presented.entry(pane).or_default();
            if !feedback.pending() || focused == Some(pane) {
                if previous.flashing {
                    effects.push(Effect::Clear(pane));
                }
                previous.flashing = false;
                if feedback.pending() && previous.acknowledged < feedback.feedback_revision {
                    effects.push(Effect::Acknowledge(pane, feedback.feedback_revision));
                    previous.acknowledged = feedback.feedback_revision;
                }
            } else if feedback.feedback_revision > previous.revision {
                effects.push(Effect::Pulse(pane, feedback.status.clone()));
                previous.flashing = true;
            }
            if feedback.pending() && focused != Some(pane)
                && previous.acknowledged < feedback.feedback_revision
                && previous.sound_granted == feedback.feedback_revision
                && previous.sound_played < feedback.feedback_revision
            {
                effects.push(Effect::Sound(pane, feedback.feedback_revision, feedback.status.clone()));
                previous.sound_played = feedback.feedback_revision;
            }
            previous.revision = previous.revision.max(feedback.feedback_revision);
        }
        effects
    }
}

impl super::ClientState {
    pub(crate) fn sync_feedback(
        &mut self,
        focused: Option<PaneId>,
        panes: &crate::chrome::PaneRuntimeStore,
        agents: &heca_config::settings::AgentsConfig,
        notification_flash: u64,
        notification_sound: bool,
    ) {
        use heca_grid_ui::{Tone, prop::PropName};
        if self.switching || self.failed {
            return;
        }
        for effect in self.feedback.project(focused) {
            match effect {
                Effect::Clear(pane) => panes.signals(pane).flash.clear(),
                Effect::Pulse(pane, status) => {
                    let tone = agents.statuses
                        .get(&status)
                        .and_then(|status| status.tone.as_deref())
                        .and_then(Tone::from_prop_name)
                        .unwrap_or(Tone::Accent);
                    let flash = panes.signals(pane).flash;
                    if notification_flash == 0 { flash.repeat(tone); }
                    else { flash.repeat_for(tone, std::time::Duration::from_millis(notification_flash)); }
                    if notification_sound
                        && let Some(feedback) = self.feedback.panes.iter().find(|feedback| feedback.pane_id == pane) {
                        self.outbox.send(crate::server::wire::ClientMsg::RequestFeedbackSound {
                            session_id: self.session.id, pane_id: pane, revision: feedback.feedback_revision,
                        });
                    }
                }
                Effect::Acknowledge(pane_id, revision) => {
                    self.outbox
                        .send(crate::server::wire::ClientMsg::AcknowledgeFeedback {
                            session_id: self.session.id,
                            pane_id,
                            revision,
                        })
                }
                Effect::Sound(pane, revision, status) => panes.play_feedback(self.session.id, pane, revision, status),
            }
        }
        let duration = (notification_flash != 0).then(|| std::time::Duration::from_millis(notification_flash));
        for feedback in &self.feedback.panes {
            if self.feedback.presented.get(&feedback.pane_id).is_some_and(|previous| previous.flashing) {
                panes.signals(feedback.pane_id).flash.set_repeat_duration(duration);
            }
        }
    }
}

#[cfg(test)]
mod tests;
