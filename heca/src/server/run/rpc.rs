//! Unattached RPC uses the same registry gate, including extension-emitted session actions.

use super::*;
use crate::server::services::{Call, Reply, gate};
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
            let services = self.services.clone();
            let call = Call::local(None);
            let last = self
                .last_index()
                .map(|index| self.sessions[index].service_ref());
            // What belongs to no session (making, listing, renaming sessions) is asked as such.
            let mut gate = gate(&services, &call, |intent| {
                last.filter(|_| {
                    !crate::input::resolve_action(
                        &intent.action,
                        &crate::app::interaction::intent_args_as_strings(intent),
                    )
                    .is_some_and(|action| super::super::sessions::owns(action.kind()))
                })
            });
            registry.execute_server_with(
                &catalog,
                &confirm,
                store,
                &mut gate,
                &intent,
                &mut |step| match step {
                    crate::actions::ServerStep::Action(action) => {
                        self.run_rpc_action(action, &mut output)
                    }
                    crate::actions::ServerStep::Service(intent) => {
                        self.run_rpc_service(&call, intent).map(|()| Vec::new())
                    }
                    crate::actions::ServerStep::Expanded => Ok(Vec::new()),
                },
            )
        })();
        match result {
            Ok(_) => output.unwrap_or_else(|| "ok".into()),
            Err(error) => format!("error: {error}"),
        }
    }

    /// A script's call to a service action: the same check and the same question of who may ask
    /// every unattached call gets, then the service, in the most recent session. A deferred answer
    /// keeps `reply` until it is settled.
    pub(super) fn rpc_service(
        &mut self,
        intent: &crate::chrome::Intent,
        reply: std::sync::mpsc::Sender<String>,
    ) {
        let confirm = self
            .config
            .as_ref()
            .map(|config| config.config.confirm.clone())
            .unwrap_or_default();
        let registry = self.registry.clone();
        let catalog = self.catalog.clone();
        let call = Call::local(None);
        let session = self.last_index().map(|index| {
            (
                self.sessions[index].service_ref(),
                self.sessions[index].config(),
            )
        });
        let services = self.services.clone();
        let mut gate = gate(&services, &call, |_| session.map(|(session, _)| session));
        let store = crate::chrome::SharedChromeState::new(0.0, 0.0);
        let result = registry.execute_server_with(
            &catalog,
            &confirm,
            store,
            &mut gate,
            intent,
            &mut |step| match step {
                crate::actions::ServerStep::Service(intent) => match session {
                    Some((session, config)) => {
                        services.borrow_mut().invoke(
                            session,
                            &call,
                            config,
                            intent,
                            Reply::Rpc(reply.clone()),
                        );
                        Ok(Vec::new())
                    }
                    None => Err(RpcError::NotInitialized),
                },
                crate::actions::ServerStep::Action(_) | crate::actions::ServerStep::Expanded => {
                    Err(RpcError::NotRunnable(intent.action.clone()))
                }
            },
        );
        if let Err(error) = result {
            let _ = reply.send(format!("error: {error}"));
        }
    }

    /// A service action reached by a chain a script started: in the most recent session.
    fn run_rpc_service(
        &mut self,
        call: &Call,
        intent: &crate::chrome::Intent,
    ) -> Result<(), RpcError> {
        let session = &self.sessions[self.last_index().ok_or(RpcError::NotInitialized)?];
        self.services
            .borrow_mut()
            .run_step(session.service_ref(), call, session.config(), intent)
    }

    fn run_rpc_action(
        &mut self,
        action: WmAction,
        output: &mut Option<String>,
    ) -> Result<Vec<super::super::Change>, RpcError> {
        if matches!(action, WmAction::KillServer) {
            self.stop();
            *output = Some("heca: server stopped\n".into());
            return Ok(Vec::new());
        }
        if super::super::sessions::owns(action.kind()) {
            let current = self.last_index().map(|i| self.sessions[i].info().name);
            let command = super::super::sessions::from_action(&action, current.as_deref())
                .map_err(RpcError::Failed)?;
            let reply = self.execute_session(command).map_err(RpcError::Failed)?;
            if let SessionReply::List(sessions) = reply {
                *output = Some(
                    serde_json::to_string(&sessions)
                        .map_err(|error| RpcError::Failed(error.to_string()))?,
                );
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
