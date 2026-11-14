//! Unattached RPC uses the same registry gate, including extension-emitted session actions.

use super::*;
use crate::{input::WmAction, rpc::RpcError};

impl ServerLoop {
    pub(super) fn rpc(&mut self, line: &str) -> String {
        if let Some(answer) = crate::rpc::introspect(&self.catalog, line) {
            return answer.unwrap_or_else(|error| format!("error: {error}"));
        }
        let mut output = None;
        let result = (|| {
            let (intent, dock) = crate::rpc::parse_rpc(line)?.into_intent(&self.catalog)?;
            if dock.is_some() {
                return Err(RpcError::NotRunnable(intent.action));
            }
            let store = self
                .last_index()
                .map(|i| self.sessions[i].store())
                .unwrap_or_else(|| crate::chrome::SharedChromeState::new(0.0, 0.0));
            let registry = self.registry.clone();
            let catalog = self.catalog.clone();
            let confirm = self
                .config
                .as_ref()
                .map(|config| config.config.confirm.clone())
                .unwrap_or_default();
            registry.execute_server_with(&catalog, &confirm, store, &intent, &mut |step| match step {
                crate::actions::ServerStep::Action(action) => self.run_rpc_action(action, &mut output),
                crate::actions::ServerStep::Service(intent) => self.run_rpc_service(intent).map(|()| Vec::new()),
            })
        })();
        match result {
            Ok(_) => output.unwrap_or_else(|| "ok".into()),
            Err(error) => format!("error: {error}"),
        }
    }

    /// A script's call to a service action: the same check every unattached call gets, then the
    /// service, in the most recent session. A deferred answer keeps `reply` until it is settled.
    pub(super) fn rpc_service(&mut self, intent: &crate::chrome::Intent, reply: std::sync::mpsc::Sender<String>) {
        let confirm = self.config.as_ref().map(|config| config.config.confirm.clone()).unwrap_or_default();
        let checked = crate::actions::check_server_intent(&self.catalog, &confirm, intent)
            .and_then(|_| self.last_index().ok_or(RpcError::NotInitialized));
        let index = match checked {
            Ok(index) => index,
            Err(error) => { let _ = reply.send(format!("error: {error}")); return; }
        };
        let session = &self.sessions[index];
        self.services.borrow_mut().invoke(session.info().id, None, session.config(), intent,
            super::super::services::Reply::Rpc(reply));
    }

    /// A service action reached by a chain a script started: in the most recent session.
    fn run_rpc_service(&mut self, intent: &crate::chrome::Intent) -> Result<(), RpcError> {
        let session = &self.sessions[self.last_index().ok_or(RpcError::NotInitialized)?];
        self.services.borrow_mut().run_step(session.info().id, session.config(), intent)
    }

    fn run_rpc_action(&mut self, action: WmAction, output: &mut Option<String>) -> Result<Vec<super::super::Change>, RpcError> {
        if super::super::sessions::owns(action.kind()) {
            let current = self.last_index().map(|i| self.sessions[i].info().name);
            let command = super::super::sessions::from_action(&action, current.as_deref())
                .map_err(RpcError::Failed)?;
            let reply = self.execute_session(command).map_err(RpcError::Failed)?;
            if let SessionReply::List(sessions) = reply {
                *output = Some(serde_json::to_string(&sessions).map_err(|error| RpcError::Failed(error.to_string()))?);
            }
            return Ok(Vec::new());
        }
        let i = self.last_index().ok_or(RpcError::NotInitialized)?;
        if !ServerState::runs(action.kind()) {
            return Err(RpcError::NotRunnable(format!("{action:?}")));
        }
        self.sessions[i].run_unattached(action)
    }
}
