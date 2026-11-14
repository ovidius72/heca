//! A service as declared in `main`, waiting for a server to start it.

use std::cell::RefCell;
use std::rc::Rc;

use crate::actions::ActionMeta;
use crate::chrome::Intent;
use crate::server::services::{Hosted, Running, StartArgs};

use super::{ActionCx, ActionResult, Service, ServiceInbox, ServiceResult, StartCx};

/// A service's action handler: handed the service itself.
pub(crate) type ActionFn<S> =
    dyn Fn(&mut S, &mut ActionCx<'_, <S as Service>::Message>, &Intent) -> ActionResult;

type Factory<S> = dyn Fn(&mut StartCx<'_, <S as Service>::Message>) -> ServiceResult<S>;

pub(super) struct Declared<S: Service> {
    pub(super) name: String,
    pub(super) factory: Box<Factory<S>>,
    pub(super) actions: RefCell<Vec<(ActionMeta, Rc<ActionFn<S>>)>>,
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
        let service = (self.factory)(&mut StartCx { config: args.config, epoch: args.epoch, inbox: &inbox })?;
        let actions = self.actions.borrow().iter()
            .map(|(meta, run)| (meta.name.clone(), run.clone()))
            .collect();
        Ok(Box::new(Running::new(self.name.clone(), service, inbox, messages, actions, args.capacity)))
    }
}
