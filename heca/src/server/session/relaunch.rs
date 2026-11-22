//! **What each terminal starts as when its session starts again** — after Restart, and when a saved
//! session is rebuilt. One rule for both, so a session behaves the same however it came back.
//!
//! The two switches are separate: `resume_programs` is for ordinary programs, `resume_harness` for
//! harnesses. A terminal whose switch is off, or that cannot be brought back as it was, gets the
//! user's own shell through the same start every other shell goes through. A harness is never
//! started as a new conversation in place of the one it had — including one heca was never told was
//! a harness, because agent reporting was off where it ran: it is recognised from what was recorded.

use heca_core::agent::AgentDrivers;

use super::SessionRuntime;
use crate::app::backend_store::{Program, TerminalSpec};
use crate::notification::Notification;
use crate::server::sessions::{RecordedAgent, RecordedTerminal};

impl SessionRuntime {
    /// Every driver heca knows, switched on or not: what tells a harness from an ordinary program.
    pub(super) fn known_agents(&self) -> AgentDrivers {
        let settings = self
            .config()
            .map(|config| config.config.settings.agents.clone())
            .unwrap_or_default();
        crate::agent::known(&settings)
    }

    /// How `recorded` is started again.
    ///
    /// # Errors
    /// An ordinary program that should come back as it was, but whose launch was never recorded.
    pub(super) fn relaunch(
        &self,
        recorded: &RecordedTerminal,
        known: &AgentDrivers,
    ) -> Result<TerminalSpec, String> {
        let mut spec = recorded.spec.clone();
        // What reported itself as a harness, or else what a driver says the recorded command is.
        let inferred = recorded.agent.is_none();
        let agent = recorded.agent.clone().or_else(|| {
            let driver = known.recognize_launch(recorded.launch.as_ref()?)?;
            Some(RecordedAgent {
                driver: driver.id().to_owned(),
                session_id: None,
            })
        });
        match &agent {
            Some(agent) if self.resume_harness => {
                self.resume_harness(recorded, agent, inferred, known, &mut spec)
            }
            Some(_) => spec.program = Program::Shell,
            None if !self.resume_programs => spec.program = Program::Shell,
            None if recorded.running => {
                let original = recorded
                    .launch
                    .clone()
                    .ok_or("the running program's original launch data is unavailable")?;
                spec.cwd.clone_from(&original.cwd);
                spec.program = Program::Recorded(original);
            }
            None => {}
        }
        // A program is not run in the wrong folder: with its folder gone, the pane gets a shell,
        // which the one shell start sends to the home folder and says so.
        if let Program::Recorded(launch) = &spec.program
            && let Some(folder) = launch.cwd.as_deref().filter(|folder| !folder.is_dir())
        {
            spec.cwd = Some(folder.to_owned());
            spec.program = Program::Shell;
        }
        Ok(spec)
    }

    /// Put the harness's own conversation back through its driver, or say why it cannot be and give
    /// the pane a shell in the harness's folder. `inferred` is a harness known only from its recorded
    /// command, so what is missing is reporting, and the notice says how to turn it on.
    fn resume_harness(
        &self,
        recorded: &RecordedTerminal,
        agent: &RecordedAgent,
        inferred: bool,
        known: &AgentDrivers,
        spec: &mut TerminalSpec,
    ) {
        let none = AgentDrivers::new();
        let drivers = self.state.agent_drivers().unwrap_or(&none);
        let resumed = recorded
            .launch
            .as_ref()
            .ok_or_else(|| "its original launch was not recorded".to_owned())
            .and_then(|original| {
                drivers
                    .resume(&agent.driver, agent.session_id.as_deref(), original)
                    .map_err(|refusal| refusal.to_string())
            });
        match resumed {
            Ok(launch) => {
                spec.cwd.clone_from(&launch.cwd);
                spec.program = Program::Recorded(launch);
            }
            Err(why) => {
                let name = known
                    .driver(&agent.driver)
                    .map_or(agent.driver.as_str(), |driver| driver.name());
                // **Say the cause that is true now.** Reporting switched off is one reason a harness
                // never said its session; with it on, the harness ran without reporting anyway — an
                // earlier heca started it directly, or the folder was not trusted.
                let reporting_on = drivers.driver(&agent.driver).is_some();
                let hint = match (inferred, reporting_on) {
                    (false, _) => String::new(),
                    (true, true) => format!(
                        " {name} ran without reporting to heca (started by an earlier heca, or in a folder \
                         that is not trusted), so heca could not record its session. It will next time."
                    ),
                    (true, false) => format!(
                        " Agent reporting was not on for {name} where it ran (it needs `enabled = true` under \
                         `[settings.agents.programs.{}]`, in a trusted project), so heca could not record its session.",
                        agent.driver,
                    ),
                };
                Notification::warning(format!("{name} was not resumed"))
                    .body(format!("{why}. A new conversation was not started; this pane has your shell instead.{hint}"))
                    .dedup_key(format!("session.resume:{}:{:?}", self.session.id.0, recorded.owner))
                    .send();
                spec.cwd = recorded
                    .launch
                    .as_ref()
                    .and_then(|launch| launch.cwd.clone())
                    .or(spec.cwd.take());
                spec.program = Program::Shell;
            }
        }
    }
}
