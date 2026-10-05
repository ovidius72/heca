//! Actions — what exists and what it says about itself ([`catalog`]), what runs when one fires
//! ([`registry`]), and what asks the user first ([`confirm`]). The built-in set is [`builtins`].
//!
//! How an action declares its arguments is not action-specific and lives in [`crate::args`].

mod builtins;
mod catalog;
mod confirm;
mod registry;
mod sides;

#[cfg(test)]
pub(crate) use sides::Sides as SidesForTests;
#[cfg(test)]
pub(crate) fn sides_for_tests() -> SidesForTests {
    sides::Sides::from_builtins()
}
#[cfg(test)]
mod testing;

pub use builtins::builtins;
pub use catalog::{
    ActionCatalog, ActionCategory, ActionMeta, GENERIC_ACTION_ICON, Side, builtin_args,
};
pub use confirm::{ButtonRole, ConfirmSpec, Outcome, ResponseButton};
pub use registry::{
    ActionHandle, ActionRegistry, DuplicateAction, register_dynamic, unregister_dynamic,
};

// Read outside this module only by tests (RPC introspection).
#[cfg(test)]
pub use catalog::ActionInfo;
