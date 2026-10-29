//! Session-local action execution and forwarding to an attached window.

use super::*;

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
            let request_id = self.next_rpc;
            self.next_rpc += 1;
            if client
                .link
                .send(ServerMsg::Rpc { request_id, line })
                .is_err()
            {
                let _ = reply.send("error: attached client disconnected".into());
                return;
            }
            let timeout = self.connection.request_timeout_ms.get();
            self.pending_rpc.insert(
                request_id,
                PendingRpc {
                    client: client.id,
                    reply,
                    deadline: Instant::now() + std::time::Duration::from_millis(timeout),
                },
            );
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
        let result = (|| {
            let (intent, dock) = crate::rpc::parse_rpc(line)?.into_intent(&self.catalog)?;
            if dock.is_some() {
                return Err(crate::rpc::RpcError::NotRunnable(intent.action));
            }
            self.registry.execute_server_intent(
                &mut crate::actions::ServerInvocation {
                    state: &mut self.state,
                    session: &mut self.session,
                    store: self.store.clone(),
                    catalog: &self.catalog,
                    confirm: &self.confirm,
                    asker: self.rpc_asker,
                },
                &intent,
            )
        })();
        match result {
            Ok(changes) => {
                let _ = self.broadcast(&changes);
                "ok".into()
            }
            Err(error) => format!("error: {error}"),
        }
    }
}
