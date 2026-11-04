//! Session-owned feedback. Clients animate the projection and acknowledge specific revisions.

use std::collections::{BTreeMap, HashMap, HashSet};

use heca_config::settings::AgentsConfig;
use heca_core::{agent::AgentState, layout::PaneId};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PaneFeedback {
    pub pane_id: PaneId,
    pub feedback_revision: u64,
    pub seen_through_revision: u64,
    pub status: String,
}

impl PaneFeedback {
    pub(crate) fn pending(&self) -> bool {
        self.feedback_revision > self.seen_through_revision
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Event {
    driver: String,
    session: Option<String>,
    turn: Option<String>,
    event: Option<String>,
    status: String,
}

impl From<&AgentState> for Event {
    fn from(agent: &AgentState) -> Self {
        Self {
            driver: agent.driver.clone(),
            session: agent.session_id.clone(),
            turn: agent.turn_id.clone(),
            event: agent.event.clone(),
            status: agent.status.as_str().to_owned(),
        }
    }
}

#[derive(Default)]
pub(crate) struct Feedback {
    panes: BTreeMap<PaneId, PaneFeedback>,
    last: HashMap<PaneId, Event>,
    sounded: HashMap<PaneId, u64>,
    revision: u64,
    dirty: bool,
}

impl Feedback {
    pub(crate) fn observe(
        &mut self,
        pane: PaneId,
        agent: Option<&AgentState>,
        config: &AgentsConfig,
    ) {
        let Some(agent) = agent else {
            self.last.remove(&pane);
            return;
        };
        let event = Event::from(agent);
        if self.last.get(&pane) == Some(&event) {
            return;
        }
        self.last.insert(pane, event);
        let enabled = config.enabled
            && config.programs.get(&agent.driver).is_some_and(|program| {
                program.enabled == Some(true)
                    && agent.event.as_ref().is_some_and(|event| {
                        program
                            .feedback_events
                            .as_ref()
                            .is_some_and(|events| events.contains(event))
                            && program
                                .enabled_events
                                .as_ref()
                                .is_none_or(|events| events.contains(event))
                    })
            });
        if !enabled {
            return;
        }
        self.raise(pane, agent.status.as_str().to_owned());
    }

    pub(crate) fn completed(&mut self, pane: PaneId, exit_code: i32) {
        self.raise(pane, heca_core::runtime::ProcessStatus::of_exit(exit_code).as_str().to_owned());
    }

    fn raise(&mut self, pane: PaneId, status: String) {
        let Some(revision) = self.revision.checked_add(1) else {
            return;
        };
        self.revision = revision;
        let seen = self
            .panes
            .get(&pane)
            .map_or(0, |feedback| feedback.seen_through_revision);
        self.panes.insert(
            pane,
            PaneFeedback {
                pane_id: pane,
                feedback_revision: revision,
                seen_through_revision: seen,
                status,
            },
        );
        self.dirty = true;
    }

    pub(crate) fn acknowledge(&mut self, pane: PaneId, revision: u64) {
        let Some(feedback) = self.panes.get_mut(&pane) else {
            return;
        };
        if revision > feedback.feedback_revision || revision <= feedback.seen_through_revision {
            return;
        }
        feedback.seen_through_revision = revision;
        self.dirty = true;
    }

    pub(crate) fn retain(&mut self, panes: &HashSet<PaneId>) {
        self.last.retain(|pane, _| panes.contains(pane));
        self.sounded.retain(|pane, _| panes.contains(pane));
        let before = self.panes.len();
        self.panes.retain(|pane, _| panes.contains(pane));
        self.dirty |= before != self.panes.len();
    }

    pub(crate) fn snapshot(&self) -> Vec<PaneFeedback> {
        self.panes.values().cloned().collect()
    }

    pub(crate) fn claim_sound(&mut self, pane: PaneId, revision: u64) -> bool {
        if !self.panes.get(&pane).is_some_and(|feedback|
            feedback.pending() && feedback.feedback_revision == revision)
            || self.sounded.get(&pane).is_some_and(|played| *played >= revision)
        { return false }
        self.sounded.insert(pane, revision);
        true
    }
    pub(crate) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }
}

impl super::ServerState {
    pub(crate) fn collect_feedback(&mut self, session: &heca_core::layout::Session) {
        let live = session
            .workspaces
            .iter()
            .flat_map(|workspace| workspace.panes())
            .map(|pane| pane.id)
            .collect::<HashSet<_>>();
        for pane in &live {
            if let Some(backend) = self.backends.get_mut(*pane) {
                for agent in backend.take_agent_updates() {
                    self.feedback.observe(
                        *pane,
                        agent.as_ref(),
                        &self.terminal_settings.agent_feedback,
                    );
                }
            }
        }
        self.feedback.retain(&live);
    }
}

#[cfg(test)]
mod tests;
