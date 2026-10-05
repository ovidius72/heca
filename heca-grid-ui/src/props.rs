//! **New props for an existing child** — how a parent hands a widget it already holds the facts it
//! should now show, without rebuilding it.
//!
//! A widget says what it takes with [`on_props`](crate::builders::ComponentExt::on_props), typed to
//! the model it understands. Whoever owns the tree hands it a value with
//! [`Component::set_props`](crate::component::Component::set_props); the answer says whether the
//! widget took it, so a value of the wrong type is a refusal the caller can see, never a silent
//! no-op.

use std::any::Any;

/// What a widget does with the props it is handed. Held by the widget's [`Base`](crate::Base).
pub struct PropsSlot(Box<Take>);

/// Takes the props if they are of the type it declared, and says whether it did.
type Take = dyn FnMut(&dyn Any) -> bool;

impl PropsSlot {
    /// A slot that takes values of type `T`.
    pub fn of<T: Any>(mut take: impl FnMut(&T) + 'static) -> Self {
        Self(Box::new(move |props| match props.downcast_ref::<T>() {
            Some(props) => {
                take(props);
                true
            }
            None => false,
        }))
    }

    /// Hand `props` over. `false` when they are not the type this widget takes.
    pub fn take(&mut self, props: &dyn Any) -> bool {
        (self.0)(props)
    }
}

#[cfg(test)]
mod tests;
