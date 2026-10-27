//! The same action registry, operating on server-owned data without an AppState.

use heca_core::layout::Session;
use crate::{
    app::interaction::{Domain, InteractionSource, intent_args_as_strings, policy_allows_facts},
    chrome::{Intent, SharedChromeState},
    rpc::RpcError,
    server::{Asker, Change, ServerState},
};
use super::{ActionCatalog, ActionRegistry};

pub(crate) struct ServerInvocation<'a> {
    pub(crate) state: &'a mut ServerState,
    pub(crate) session: &'a mut Session,
    pub(crate) store: SharedChromeState,
    pub(crate) catalog: &'a ActionCatalog,
    pub(crate) confirm: &'a heca_config::confirm::ConfirmConfig,
    pub(crate) asker: Asker,
}

impl ActionRegistry {
    /// Resolve by the one catalog, apply the shared policy and execute only server-owned work.
    /// Without a window an enabled confirmation cannot be answered, so it is refused, not skipped.
    pub(crate) fn execute_server_intent(
        &self,
        cx: &mut ServerInvocation<'_>,
        intent: &Intent,
    ) -> Result<Vec<Change>, RpcError> {
        let name = &intent.action;
        let meta = cx.catalog.find(name).ok_or_else(|| RpcError::UnknownCommand(name.clone()))?;
        let args = intent_args_as_strings(intent);
        if !crate::args::check_args(&meta.args, &args).is_empty() {
            return Err(RpcError::MissingArgs(name.clone()));
        }
        let action = crate::input::resolve_action(name, &args);
        if cx.catalog.is_builtin(name) && action.is_none() {
            return Err(RpcError::MissingArgs(name.clone()));
        }
        // Unattached scripts have no overlay, dock or floating-window focus.
        if !policy_allows_facts(Domain::Tiled, InteractionSource::Rpc, meta.policy, action.as_ref(), None) {
            return Err(RpcError::Blocked(name.clone()));
        }
        if let Some(confirm) = &meta.confirm
            && (confirm.forced || cx.confirm.enabled(&confirm.config_name, confirm.default_enabled)) {
            return Err(RpcError::Blocked(name.clone()));
        }
        if let Some(action) = action {
            return if ServerState::runs(action.kind()) {
                Ok(cx.state.run(cx.session, cx.asker, &action))
            } else {
                Err(RpcError::NotRunnable(name.clone()))
            };
        }
        let emitted = self.execute_server_dynamic(name, cx.store.clone(), intent)
            .ok_or_else(|| RpcError::NotRunnable(name.clone()))?;
        let mut changes = Vec::new();
        for emitted in emitted {
            changes.extend(self.execute_server_intent(cx, &emitted)?);
        }
        Ok(changes)
    }
}
