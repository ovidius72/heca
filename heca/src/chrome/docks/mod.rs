//! **Docks** — the containers placed in the window's regions: what a dock contributes, where it
//! sits, which one holds the keyboard, and the host that seats and starts them.

pub(super) mod contribution;
pub(super) mod focus;
pub(super) mod host;
pub(super) mod regions;
pub(crate) mod startup_queue;
