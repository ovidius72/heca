//! Input vocabulary — the actions the window manager can run ([`WmAction`]), the word lists their
//! arguments take, and the two ways to build one: from its name alone, or from its name and
//! arguments.

mod action;
mod build;
mod names;
mod vocabulary;

pub use action::{WmAction, WmActionKind};
pub use build::resolve_action;
use names::action_from_name;
pub use vocabulary::{FontZoomStep, RegionVisibility, ResizeEdge, ResizeTarget, SpawnKind};

#[cfg(test)]
mod tests;
