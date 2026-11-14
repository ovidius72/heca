//! The same action registry, operating on server-owned data without an AppState.

use heca_core::layout::Session;
use crate::{
    app::interaction::{Domain, InteractionSource, intent_args_as_strings, policy_allows_facts},
    chrome::{Intent, SharedChromeState},
    rpc::RpcError,
    server::{Asker, Change, ServerState},
};
use super::{ActionCatalog, ActionRegistry};

/// One piece of a server-side call that the registry cannot run itself: the caller owns what runs it.
pub(crate) enum ServerStep<'a> {
    /// A built-in action.
    Action(crate::input::WmAction),
    /// An extension service's action, which runs where the service is.
    Service(&'a Intent),
}

pub(crate) struct ServerInvocation<'a> {
    pub(crate) state: &'a mut ServerState,
    pub(crate) session: &'a mut Session,
    pub(crate) store: SharedChromeState,
    pub(crate) catalog: &'a ActionCatalog,
    pub(crate) confirm: &'a heca_config::confirm::ConfirmConfig,
    pub(crate) asker: Asker,
    pub(crate) services: &'a crate::server::services::SharedServices,
    pub(crate) config: Option<&'a heca_config::loader::AppConfig>,
}

impl ActionRegistry {
    /// Resolve by the one catalog, apply the shared policy and execute only server-owned work.
    /// Without a window an enabled confirmation cannot be answered, so it is refused, not skipped.
    pub(crate) fn execute_server_intent(
        &self,
        cx: &mut ServerInvocation<'_>,
        intent: &Intent,
    ) -> Result<Vec<Change>, RpcError> {
        self.execute_server_with(cx.catalog, cx.confirm, cx.store.clone(), intent, &mut |step| match step {
            ServerStep::Action(action) if ServerState::runs(action.kind()) && !crate::server::sessions::owns(action.kind()) =>
                Ok(cx.state.run(cx.session, cx.asker, &action)),
            ServerStep::Service(intent) =>
                cx.services.borrow_mut().run_step(cx.session.id, cx.config, intent).map(|()| Vec::new()),
            ServerStep::Action(_) => Err(RpcError::NotRunnable(intent.action.clone())),
        })
    }

    /// The same validation and dynamic-action expansion for a session or its server owner.
    pub(crate) fn execute_server_with(
        &self, catalog: &ActionCatalog, confirm: &heca_config::confirm::ConfirmConfig,
        store: SharedChromeState, intent: &Intent,
        run: &mut impl FnMut(ServerStep<'_>) -> Result<Vec<Change>, RpcError>,
    ) -> Result<Vec<Change>, RpcError> {
        let name = &intent.action;
        if let Some(action) = check_server_intent(catalog, confirm, intent)? {
            let changes = run(ServerStep::Action(action))?;
            if let Some(reason) = changes.iter().find_map(|change| match change {
                Change::Refused(reason) => Some(reason),
                _ => None,
            }) {
                return Err(RpcError::Failed(format!("{reason:?}")));
            }
            return Ok(changes);
        }
        if self.is_service(name) {
            return run(ServerStep::Service(intent));
        }
        let emitted = self.execute_server_dynamic(name, store.clone(), intent)
            .ok_or_else(|| RpcError::NotRunnable(name.clone()))?;
        let mut changes = Vec::new();
        for emitted in emitted {
            changes.extend(self.execute_server_with(catalog, confirm, store.clone(), &emitted, run)?);
        }
        Ok(changes)
    }
}

/// **May a script with no window run this?** The catalog knows it, its arguments are complete, the
/// policy allows it with no overlay, dock or floating focus, and it needs no confirmation nobody
/// could answer. Returns the built-in it resolves to, if it is one.
pub(crate) fn check_server_intent(
    catalog: &ActionCatalog, confirm: &heca_config::confirm::ConfirmConfig, intent: &Intent,
) -> Result<Option<crate::input::WmAction>, RpcError> {
    let name = &intent.action;
    let meta = catalog.find(name).ok_or_else(|| RpcError::UnknownCommand(name.clone()))?;
    let args = intent_args_as_strings(intent);
    if !crate::args::check_args(&meta.args, &args).is_empty() {
        return Err(RpcError::MissingArgs(name.clone()));
    }
    let action = crate::input::resolve_action(name, &args);
    if catalog.is_builtin(name) && action.is_none() {
        return Err(RpcError::MissingArgs(name.clone()));
    }
    if !policy_allows_facts(Domain::Tiled, InteractionSource::Rpc, meta.policy, action.as_ref(), None) {
        return Err(RpcError::Blocked(name.clone()));
    }
    if let Some(spec) = &meta.confirm
        && (spec.forced || confirm.enabled(&spec.config_name, spec.default_enabled)) {
        return Err(RpcError::Blocked(name.clone()));
    }
    Ok(action)
}
