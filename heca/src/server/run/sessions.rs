//! Named-session mutation and construction; wire routing remains in the owner loop.

use super::*;

impl ServerLoop {
    pub(super) fn execute_session(
        &mut self,
        command: SessionCommand,
    ) -> Result<SessionReply, String> {
        match command {
            SessionCommand::List => Ok(SessionReply::List(
                self.sessions.iter().map(SessionRuntime::info).collect(),
            )),
            SessionCommand::Create {
                folder,
                name,
                label,
            } => self
                .create_session(folder, name, label)
                .map(SessionReply::Updated),
            SessionCommand::Rename { name, new_name } => {
                crate::server::sessions::validate_name(&new_name)?;
                if self
                    .sessions
                    .iter()
                    .any(|s| s.info().name == new_name && s.info().name != name)
                {
                    return Err("a session already has that name".into());
                }
                let i = self.named(&name)?;
                let mut info = self.sessions[i].info();
                info.name = new_name;
                self.sessions[i].set_identity(info);
                Ok(SessionReply::Updated(self.sessions[i].info()))
            }
            SessionCommand::Terminate { name } => {
                let i = self.named(&name)?;
                if self.sessions[i].info().status == SessionStatus::Active {
                    self.sessions[i].terminate();
                }
                Ok(SessionReply::Updated(self.sessions[i].info()))
            }
            SessionCommand::Restart { name } => {
                let i = self.named(&name)?;
                let (result, drafts) =
                    crate::notification::collect_notifications(|| self.sessions[i].restart());
                for draft in drafts {
                    self.sessions[i].raise(draft);
                }
                result?;
                Ok(SessionReply::Updated(self.sessions[i].info()))
            }
            SessionCommand::Delete { name } => {
                let i = self.named(&name)?;
                let id = self.sessions[i].info().id;
                for client in &self.clients {
                    if client.session.get() == Some(id) {
                        let _ = client.link.send(ServerMsg::SessionDeleted { id });
                        self.sessions[i].detach_connection(client.id);
                        client.session.set(None);
                        let limits = self
                            .config
                            .as_ref()
                            .map(|config| config.config.settings.connection)
                            .unwrap_or_default();
                        client.retire_at.set(Some(
                            Instant::now()
                                + std::time::Duration::from_millis(limits.request_timeout_ms.get()),
                        ));
                    }
                }
                self.sessions[i].shutdown();
                self.sessions.remove(i);
                if self.last_session == Some(id) {
                    self.recent_sessions.retain(|recent| *recent != id);
                    self.last_session = self.recent_sessions.last().copied();
                } else {
                    self.recent_sessions.retain(|recent| *recent != id);
                }
                Ok(SessionReply::Deleted(id))
            }
        }
    }

    pub(super) fn named(&self, name: &str) -> Result<usize, String> {
        self.sessions
            .iter()
            .position(|s| s.info().name == name)
            .ok_or_else(|| "session not found".into())
    }

    pub(super) fn create_session(
        &mut self,
        folder: std::path::PathBuf,
        name: Option<String>,
        label: Option<String>,
    ) -> Result<SessionInfo, String> {
        if !folder.is_absolute() {
            return Err("the session folder must be absolute".into());
        }
        if let Some(name) = &name {
            crate::server::sessions::validate_name(name)?;
            if self.sessions.iter().any(|s| s.info().name == *name) {
                return Err("a session already has that name".into());
            }
        }
        let settings = self
            .config
            .as_ref()
            .map(|c| c.config.settings.clone())
            .unwrap_or_default();
        if name.is_none() && !settings.session_name_format.contains("{number}") {
            return Err("session_name_format must contain {number}".into());
        }
        let id = loop {
            let id = self.next_session;
            self.next_session = id.checked_add(1).ok_or("no more session identifiers")?;
            let proposed = name.clone().unwrap_or_else(|| {
                settings
                    .session_name_format
                    .replace("{number}", &id.to_string())
            });
            if !self.sessions.iter().any(|s| s.info().name == proposed) {
                break (SessionId(id), proposed);
            }
        };
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let folder = folder.canonicalize().map_err(|e| e.to_string())?;
        let mut identity =
            SessionInfo::unnamed(id.0, &settings.session_name_format, Some(folder.clone()));
        identity.name = id.1;
        identity.label = label;
        let (runtime, drafts) =
            crate::notification::collect_notifications(|| self.make_session(identity.clone()));
        let mut runtime = runtime?;
        for draft in drafts {
            runtime.raise(draft);
        }
        runtime.install_actions(self.registry.clone(), self.catalog.clone());
        self.sessions.push(runtime);
        self.last_session = Some(identity.id);
        self.recent_sessions.push(identity.id);
        Ok(identity)
    }

    fn make_session(&self, identity: SessionInfo) -> Result<SessionRuntime, String> {
        if let (Some(config), Some(wake)) = (&self.config, &self.wake) {
            let project = identity
                .folder
                .as_deref()
                .map(heca_config::loader::AppConfig::load_for);
            return SessionRuntime::from_config(
                project.as_ref().unwrap_or(config),
                wake.clone(),
                identity,
            )
            .map_err(|e| e.to_string());
        }
        #[cfg(test)]
        {
            let layout = Session::new(identity.id, Default::default());
            let mut runtime =
                SessionRuntime::new(ServerState::for_tests(Default::default()), layout);
            runtime.set_identity(identity);
            Ok(runtime)
        }
        #[cfg(not(test))]
        Err(std::io::Error::other("session factory is not configured").to_string())
    }
}
