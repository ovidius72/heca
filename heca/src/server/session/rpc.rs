//! Session-local action execution and forwarding to an attached window.

use super::*;
use crate::server::services::Call;

impl SessionRuntime {
    pub(crate) fn store(&self) -> crate::chrome::SharedChromeState {
        self.store.clone()
    }

    pub(crate) fn run_unattached(
        &mut self,
        action: crate::input::WmAction,
    ) -> Result<Vec<Change>, crate::rpc::RpcError> {
        if self.identity.status == SessionStatus::Terminated {
            return Err(crate::rpc::RpcError::Failed("session is terminated".into()));
        }
        let changes = self.act(action, self.rpc_asker);
        let _ = self.broadcast(&changes);
        Ok(changes)
    }
    pub(crate) fn begin_rpc(&mut self, line: String, reply: std::sync::mpsc::Sender<String>) {
        if let Some(client) = self
            .clients
            .iter()
            .find(|client| matches!(client.attachment.get(), Attachment::Attached))
        {
            client.begin_rpc(line, reply, self.connection);
        } else {
            let answer = self.rpc(&line);
            let _ = reply.send(answer);
        }
    }

    pub(crate) fn configure_rpc(
        &mut self,
        asker: Asker,
        confirm: heca_config::confirm::ConfirmConfig,
        connection: heca_config::settings::ConnectionConfig,
    ) {
        self.rpc_asker = asker;
        self.confirm = confirm;
        self.connection = connection;
    }

    pub(super) fn rpc(&mut self, line: &str) -> String {
        if let Some(result) = crate::rpc::introspect(&self.catalog, line) {
            return result.unwrap_or_else(|error| format!("error: {error}"));
        }
        match self.run_line(&Call::local(None), line).result {
            Ok(changes) => {
                let _ = self.broadcast(&changes);
                "ok".into()
            }
            Err(error) => format!("error: {error}"),
        }
    }

    /// **Run a request a service handed in**, as `call`, in this session: the same line, parser,
    /// catalog, confirmations, question of who may ask and execution as a script's. `Err` says why
    /// it stopped and whether anything had run before.
    pub(crate) fn run_request(&mut self, call: &Call, line: &str) -> Result<(), (String, bool)> {
        let Executed { result, ran } = self.run_line(call, line);
        match result {
            Ok(changes) => {
                let _ = self.broadcast(&changes);
                Ok(())
            }
            // A policy's own words, not the script's "not permitted: " in front of them.
            Err(crate::rpc::RpcError::Denied(reason)) => Err((reason, ran > 0)),
            Err(error) => Err((error.to_string(), ran > 0)),
        }
    }

    /// **May the window `client` run this?** Asked when its message is about to run, so a rule that
    /// changed applies to what was already on its way.
    pub(super) fn window_may(
        &self,
        client: u64,
        action: &str,
        args: &crate::chrome::PropMap,
    ) -> Result<(), String> {
        self.services.borrow_mut().authorize(
            &Call::local(Some(client)),
            Some(self.service_ref()),
            action,
            args,
            None,
        )
    }

    /// The one way a line becomes an action in this session: parsed against the catalog, checked,
    /// put to every action policy as `call`, and run — each follow-up of a chain as the same `call`.
    fn run_line(&mut self, call: &Call, line: &str) -> Executed {
        let service_ref = self.service_ref();
        let mut invocation = crate::actions::ServerInvocation {
            state: &mut self.state,
            session: &mut self.session,
            service_ref,
            store: self.store.clone(),
            catalog: &self.catalog,
            confirm: &self.confirm,
            asker: self.rpc_asker,
            services: &self.services,
            config: self.config.as_deref(),
            call,
            ran: 0,
        };
        let result = (|| {
            let (intent, dock) = crate::rpc::parse_rpc(line)?.into_intent(invocation.catalog)?;
            if dock.is_some() {
                return Err(crate::rpc::RpcError::NotRunnable(intent.action));
            }
            self.registry
                .execute_server_intent(&mut invocation, &intent)
        })();
        Executed {
            result,
            ran: invocation.ran,
        }
    }
}

/// What running a line did: how it ended, and how many of its actions had run.
struct Executed {
    result: Result<Vec<Change>, crate::rpc::RpcError>,
    ran: usize,
}
