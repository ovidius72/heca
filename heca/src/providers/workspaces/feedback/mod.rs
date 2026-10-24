//! Bind a pane's real status changes to the same flash API a plugin uses.

use std::rc::Rc;

use heca_config::settings::AgentStatuses;
use heca_core::runtime::ProcessStatus;
use heca_grid_ui::{Tone, prop::PropName, reactive::{Signal, SignalGet, SignalUpdate, create_effect}};

use heca_core::runtime::PaneRuntime;
use heca_core::layout::PaneId;
use crate::chrome::PaneRuntimeSignals;

#[derive(Debug, PartialEq)]
struct State {
    program: Option<String>,
    process: ProcessStatus,
    agent: Option<(String, String)>,
}

impl State {
    fn of(runtime: PaneRuntime) -> Self {
        Self { program: runtime.program, process: runtime.status,
            agent: runtime.agent.map(|agent| (agent.driver, agent.status.as_str().to_owned())) }
    }

    fn tone(&self, statuses: &AgentStatuses) -> Tone {
        if let Some((_, status)) = &self.agent {
            return statuses.get(status).and_then(|status| status.tone.as_deref())
                .and_then(Tone::from_prop_name).unwrap_or(Tone::Accent);
        }
        match self.process {
            ProcessStatus::Running => Tone::Accent,
            ProcessStatus::Idle => Tone::Muted,
            ProcessStatus::Success => Tone::Success,
            ProcessStatus::Error => Tone::Danger,
        }
    }
}

fn transition(previous: Option<&State>, current: &State, statuses: &AgentStatuses) -> Option<Tone> {
    let previous = previous?;
    // An agent's explicit work state wins over process discovery. Shell startup
    // can report that state before the process monitor learns its program name.
    if current.agent.is_some() && current.agent == previous.agent { return None }
    if current.agent.is_none() && previous.agent.is_none() && current.process == previous.process { return None }
    let tone = current.tone(statuses);
    let initial_program = previous.program.is_none() && previous.agent.is_none();
    let initial_agent = previous.agent.is_none() && current.agent.is_some();
    // A program becoming known is initialization. Its first explicit request
    // for attention or outcome still matters, including when it arrives first.
    if (initial_program || initial_agent) && matches!(tone, Tone::Accent | Tone::Muted) { return None }
    Some(tone)
}

pub(super) fn bind(runtime: PaneRuntimeSignals, pane: PaneId,
    statuses: Signal<Rc<AgentStatuses>>, active: Signal<Option<PaneId>>) {
    if runtime.feedback_bound.get_untracked() { return }
    runtime.feedback_bound.set(true);
    let flash = runtime.flash;
    create_effect(move |previous: Option<(State, bool)>| {
        let current = State::of(runtime.snapshot());
        let focused = active.get() == Some(pane);
        if focused && previous.as_ref().is_none_or(|(_, was_focused)| !was_focused) {
            flash.clear();
        } else if let Some(tone) = transition(previous.as_ref().map(|(state, _)| state),
            &current, statuses.get().as_ref()) {
            if focused { flash.trigger(tone); } else { flash.repeat(tone); }
        }
        (current, focused)
    });
}

#[cfg(test)]
mod tests;
