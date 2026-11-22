//! The same action registry, operating on server-owned data without an AppState.

use super::{ActionCatalog, ActionRegistry};
use crate::{
    app::interaction::{Domain, InteractionSource, intent_args_as_strings, policy_allows_facts},
    chrome::{Intent, SharedChromeState},
    rpc::RpcError,
    server::{Asker, Change, ServerState},
};
use heca_core::layout::Session;

/// One piece of a server-side call that the registry cannot run itself: the caller owns what runs it.
pub(crate) enum ServerStep<'a> {
    /// A built-in action.
    Action(crate::input::WmAction),
    /// An extension service's action, which runs where the service is.
    Service(&'a Intent),
    /// A plain extension action that has just run, the registry running it itself. What it asked
    /// to follow it comes next, as steps of their own. Told so a caller can count it as run.
    Expanded,
}

/// **May this action run, and for whom?** — asked before each action of a call and before each
/// follow-up it expands into, with the action that asked for it. The caller owns who is asking, so a
/// follow-up is asked the same way as the first and can never do more than it could.
pub(crate) trait ServerGate {
    /// Whether the call was handed in by a service for someone else, and not asked by the user.
    fn is_external(&self) -> bool;
    /// May `intent` run? `parent` is the action that asked for it, when it is a follow-up.
    fn permit(&mut self, intent: &Intent, parent: Option<&str>) -> Result<(), RpcError>;
}

/// A gate that lets the user's own calls through, for a test of what is not about who asks.
#[cfg(test)]
pub(crate) struct AllowLocal;

#[cfg(test)]
impl ServerGate for AllowLocal {
    fn is_external(&self) -> bool {
        false
    }

    fn permit(&mut self, _: &Intent, _: Option<&str>) -> Result<(), RpcError> {
        Ok(())
    }
}

/// What a call is resolved against, the same for the action and every follow-up it expands into.
struct Resolving<'a> {
    catalog: &'a ActionCatalog,
    confirm: &'a heca_config::confirm::ConfirmConfig,
    store: SharedChromeState,
}

pub(crate) struct ServerInvocation<'a> {
    pub(crate) state: &'a mut ServerState,
    pub(crate) session: &'a mut Session,
    /// The session as the services are told of it.
    pub(crate) service_ref: crate::server::services::SessionRef,
    pub(crate) store: SharedChromeState,
    pub(crate) catalog: &'a ActionCatalog,
    pub(crate) confirm: &'a heca_config::confirm::ConfirmConfig,
    pub(crate) asker: Asker,
    pub(crate) services: &'a crate::server::services::SharedServices,
    pub(crate) config: Option<&'a heca_config::loader::AppConfig>,
    /// Who is asking.
    pub(crate) call: &'a crate::server::services::Call,
    /// How many actions of the call have run, for a caller that must tell "refused" from "failed".
    pub(crate) ran: usize,
}

impl ActionRegistry {
    /// Resolve by the one catalog, apply the shared policy and execute only server-owned work.
    /// Without a window an enabled confirmation cannot be answered, so it is refused, not skipped.
    pub(crate) fn execute_server_intent(
        &self,
        cx: &mut ServerInvocation<'_>,
        intent: &Intent,
    ) -> Result<Vec<Change>, RpcError> {
        let in_session = cx.service_ref;
        let mut gate =
            crate::server::services::gate(cx.services, cx.call, move |_| Some(in_session));
        let ran = &mut cx.ran;
        self.execute_server_with(
            cx.catalog,
            cx.confirm,
            cx.store.clone(),
            &mut gate,
            intent,
            &mut |step| {
                let changes = match step {
                    ServerStep::Action(action)
                        if ServerState::runs(action.kind())
                            && !crate::server::sessions::owns(action.kind()) =>
                    {
                        Ok(cx.state.run(cx.session, cx.asker, &action))
                    }
                    ServerStep::Service(intent) => cx
                        .services
                        .borrow_mut()
                        .run_step(cx.service_ref, cx.call, cx.config, intent)
                        .map(|()| Vec::new()),
                    ServerStep::Expanded => Ok(Vec::new()),
                    ServerStep::Action(action) => {
                        Err(RpcError::NotRunnable(crate::actions::describe(&action).0))
                    }
                };
                *ran += usize::from(changes.is_ok());
                changes
            },
        )
    }

    /// The same validation, the same question of who may ask, and the same dynamic-action
    /// expansion for a session or its server owner. **Every action of a call, and every follow-up
    /// it expands into, is put to `gate` before it runs** — the one place a script, a helper, an
    /// integration's request and a chain are all judged.
    pub(crate) fn execute_server_with(
        &self,
        catalog: &ActionCatalog,
        confirm: &heca_config::confirm::ConfirmConfig,
        store: SharedChromeState,
        gate: &mut impl ServerGate,
        intent: &Intent,
        run: &mut impl FnMut(ServerStep<'_>) -> Result<Vec<Change>, RpcError>,
    ) -> Result<Vec<Change>, RpcError> {
        self.execute_chain(
            &Resolving {
                catalog,
                confirm,
                store,
            },
            gate,
            intent,
            None,
            run,
        )
    }

    fn execute_chain(
        &self,
        resolving: &Resolving<'_>,
        gate: &mut impl ServerGate,
        intent: &Intent,
        parent: Option<&str>,
        run: &mut impl FnMut(ServerStep<'_>) -> Result<Vec<Change>, RpcError>,
    ) -> Result<Vec<Change>, RpcError> {
        let Resolving {
            catalog,
            confirm,
            store,
        } = resolving;
        let name = &intent.action;
        let action = check_server_intent(catalog, confirm, intent, gate.is_external())?;
        gate.permit(intent, parent)?;
        if let Some(action) = action {
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
        let emitted = self
            .execute_server_dynamic(name, store.clone(), intent)
            .ok_or_else(|| RpcError::NotRunnable(name.clone()))?;
        run(ServerStep::Expanded)?;
        let mut changes = Vec::new();
        for emitted in emitted {
            changes.extend(self.execute_chain(resolving, gate, &emitted, Some(name), run)?);
        }
        Ok(changes)
    }
}

/// **May a script with no window run this?** The catalog knows it, its arguments are complete, the
/// policy allows it with no overlay, dock or floating focus, and it needs no confirmation nobody
/// could answer. A call handed in for someone else (`external`) is refused for any action that
/// declares a confirmation, whatever the user's `[confirm]` settings say. Returns the built-in it resolves to, if it is one.
fn check_server_intent(
    catalog: &ActionCatalog,
    confirm: &heca_config::confirm::ConfirmConfig,
    intent: &Intent,
    external: bool,
) -> Result<Option<crate::input::WmAction>, RpcError> {
    let name = &intent.action;
    let meta = catalog
        .find(name)
        .ok_or_else(|| RpcError::UnknownCommand(name.clone()))?;
    let args = intent_args_as_strings(intent);
    let problems = crate::args::check_args(&meta.args, &args);
    if !problems.is_empty() {
        let problems = problems.iter().map(ToString::to_string).collect();
        return Err(RpcError::WrongArgs {
            name: name.clone(),
            problems,
        });
    }
    let action = crate::input::resolve_action(name, &args);
    if catalog.is_builtin(name) && action.is_none() {
        return Err(RpcError::MissingArgs(name.clone()));
    }
    if !policy_allows_facts(
        Domain::Tiled,
        InteractionSource::Rpc,
        meta.policy,
        action.as_ref(),
        None,
    ) {
        return Err(RpcError::Blocked(name.clone()));
    }
    if let Some(spec) = &meta.confirm {
        // Nobody can answer a prompt here. The user's own call is refused while its prompt is on.
        // A request handed in for someone else is refused whenever the action *declares* one:
        // turning the prompt off is the user's choice for themselves, not for whoever asks through
        // an integration.
        if external {
            return Err(RpcError::Denied(format!(
                "'{name}' asks for a confirmation, which a request handed in by a service cannot give"
            )));
        }
        if spec.forced || confirm.enabled(&spec.config_name, spec.default_enabled) {
            return Err(RpcError::Blocked(name.clone()));
        }
    }
    Ok(action)
}
