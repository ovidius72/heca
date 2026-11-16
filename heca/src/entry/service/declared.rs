//! A service as declared in `main`, waiting for a server to start it.

use std::cell::RefCell;
use std::rc::Rc;

use crate::actions::ActionMeta;
use crate::chrome::Intent;
use crate::server::services::{Brought, Home, Hosted, Requests, Running, StartArgs};

use super::{ActionCx, ActionRequest, ActionResult, Effects, Permit, Service, ServiceCx, ServiceInbox, ServiceResult, StartCx};

/// A service's action handler: handed the service itself.
pub(crate) type ActionFn<S> =
    dyn Fn(&mut S, &mut ActionCx<'_, <S as Service>::Message>, &Intent) -> ActionResult;

/// A service's action policy: asked about every action that is about to run.
pub(crate) type PolicyFn<S> =
    dyn Fn(&mut S, &mut ServiceCx<'_, <S as Service>::Message>, &ActionRequest<'_>) -> Permit;

type Factory<S> = dyn Fn(&mut StartCx<'_, <S as Service>::Message>) -> ServiceResult<S>;

pub(super) struct Declared<S: Service> {
    pub(super) name: String,
    /// The extension it was declared under: the one table of the configuration it may read.
    pub(super) extension: String,
    pub(super) factory: Box<Factory<S>>,
    pub(super) actions: RefCell<Vec<(ActionMeta, Rc<ActionFn<S>>)>>,
    pub(super) policies: RefCell<Vec<Rc<PolicyFn<S>>>>,
}

/// One declared service, whatever its type — what a starting server reads.
pub(crate) trait Declaration {
    /// `<extension>.<short>`.
    fn name(&self) -> &str;
    /// What its actions say about themselves, for the one catalog.
    fn action_metas(&self) -> Vec<ActionMeta>;
    /// Run the factory for one server.
    fn start(&self, args: &StartArgs<'_>) -> ServiceResult<Box<dyn Hosted>>;
}

impl<S: Service> Declaration for Declared<S> {
    fn name(&self) -> &str {
        &self.name
    }

    fn action_metas(&self) -> Vec<ActionMeta> {
        self.actions.borrow().iter().map(|(meta, _)| meta.clone()).collect()
    }

    fn start(&self, args: &StartArgs<'_>) -> ServiceResult<Box<dyn Hosted>> {
        let (inbox, messages) = ServiceInbox::channel(args.capacity, args.wake.clone());
        let data_dir = args.identity.service_folder(&self.name)?;
        let service = (self.factory)(&mut StartCx {
            config: args.config,
            extension: &self.extension,
            epoch: args.epoch,
            inbox: &inbox,
            data_dir: &data_dir,
            catalog: args.catalog,
        })?;
        let actions = self.actions.borrow().iter()
            .map(|(meta, run)| (meta.name.clone(), run.clone()))
            .collect();
        let policies = self.policies.borrow().clone();
        let requests = Requests::new(&self.name, args.request_capacity, args.max_line, !policies.is_empty());
        let home = Home { name: self.name.clone(), extension: self.extension.clone(), data_dir };
        Ok(Box::new(Running::new(home, service, inbox, messages,
            Brought { actions, policies, effects: Effects::new(requests), capacity: args.capacity })))
    }
}
