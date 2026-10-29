use super::*;

impl SessionRuntime {
    pub(crate) fn info(&self) -> SessionInfo {
        let mut info = self.identity.clone();
        info.windows = self
            .clients
            .iter()
            .filter(|c| !c.is_closed() && matches!(c.attachment.get(), Attachment::Attached))
            .count();
        info.agents = if info.status == SessionStatus::Terminated {
            0
        } else {
            self.session
                .workspaces
                .iter()
                .flat_map(|w| w.panes())
                .filter(|p| p.runtime.agent.is_some() && p.runtime.exit_code.is_none())
                .count()
        };
        info
    }

    pub(crate) fn set_identity(&mut self, identity: SessionInfo) {
        self.session.id = identity.id;
        self.identity = identity;
    }

    #[cfg(test)]
    pub(crate) fn content(&self) -> heca_core::layout::SessionContent {
        self.session.content()
    }

    pub(crate) fn recorded_terminals(&self) -> Vec<crate::server::sessions::RecordedTerminal> {
        if self.identity.status == SessionStatus::Terminated {
            return self.saved_terminals.clone();
        }
        self.state
            .backends
            .owners()
            .filter_map(|(owner, id)| {
                let backend = self.state.backends.get_by_id(id)?;
                let runtime = backend.runtime();
                let agent = runtime.agent;
                Some(crate::server::sessions::RecordedTerminal {
                    running: runtime.status == heca_core::runtime::ProcessStatus::Running,
                    launch: agent
                        .as_ref()
                        .and_then(|agent| agent.launch.clone())
                        .or_else(|| backend.process_launch()),
                    owner,
                    spec: self.state.backends.spec_of(id)?.clone(),
                    agent,
                })
            })
            .collect()
    }

    pub(crate) fn terminate(&mut self) {
        self.saved_terminals = self.recorded_terminals();
        self.state.backends.clear();
        self.known_terminals.clear();
        self.images.clear();
        self.identity.status = SessionStatus::Terminated;
        let info = self.info();
        for client in &self.clients {
            let _ = client.link.send(ServerMsg::SessionTerminated {
                session: info.clone(),
            });
        }
    }

    pub(crate) fn configure_recovery(&mut self, settings: &heca_config::settings::SettingsConfig) {
        self.resume_programs = settings.resume_programs;
        self.resume_harness = settings.resume_harness;
    }

    pub(crate) fn restart(&mut self) -> Result<(), String> {
        if self.identity.status == SessionStatus::Active {
            return Ok(());
        }
        let mut launches = Vec::new();
        for launch in &self.saved_terminals {
            let mut spec = launch.spec.clone();
            if launch.agent.is_some() {
                if self.resume_harness {
                    return Err("this harness has no recorded native resume command yet".into());
                }
                spec.program = crate::app::backend_store::Program::Shell;
            } else if !self.resume_programs {
                spec.program = crate::app::backend_store::Program::Shell;
            } else if launch.running {
                let original = launch
                    .launch
                    .clone()
                    .ok_or("the running program's original launch data is unavailable")?;
                spec.cwd.clone_from(&original.cwd);
                spec.program = crate::app::backend_store::Program::Recorded(original);
            }
            launches.push((launch.owner.clone(), spec));
        }
        for (owner, spec) in launches {
            if let Err(error) = self
                .state
                .terminals()
                .start(owner, spec, self.rpc_asker.cell)
            {
                self.state.backends.clear();
                return Err(error.to_string());
            }
        }
        self.identity.status = SessionStatus::Active;
        let info = self.info();
        for client in &self.clients {
            let _ = client.link.send(ServerMsg::Attached {
                session: info.clone(),
                client_id: client.id,
                window_ordinal: client.window_ordinal.get(),
                content: self.session.content(),
            });
            let _ = client.link.send(ServerMsg::Feedback {
                session_id: self.session.id,
                panes: self.state.feedback.snapshot(),
            });
        }
        Ok(())
    }
}
