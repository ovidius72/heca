//! Named action handlers — one per `WmAction` variant, grouped by what they act on.
//!
//! Each handler is a plain `fn(&mut AppState, &WmAction)` that performs the action.
//! Parameterized variants destructure their fields from the enum; unit variants ignore the
//! `_action` parameter. [`crate::app::registry`] pairs each action with its handler; every
//! handler is re-exported here, so nothing outside this folder names a file inside it.

mod confirm;
mod docks;
mod links;
mod move_swap;
mod navigation;
mod pane;
mod pick;
mod rename;
mod scroll;
mod selection;
mod split_resize;
mod system;
mod take;
mod terminal;
mod workspace;

pub(crate) use confirm::*;
pub use docks::*;
pub use links::*;
pub use move_swap::*;
pub use navigation::*;
pub use pane::*;
pub use pick::*;
pub use rename::*;
pub use scroll::*;
pub use selection::*;
pub use split_resize::*;
pub use system::*;
pub use take::*;
pub use terminal::*;
pub use workspace::*;
