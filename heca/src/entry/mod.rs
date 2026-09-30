//! **How another crate adds to heca** — the lines a program built on heca writes before it calls
//! [`run`](crate::run).
//!
//! ```ignore
//! fn main() {
//!     heca::regions("sidebar.left").append(Planner::new("planner")); // a dock
//!     let pro = heca::extension("pro");                               // who this program is
//!     pro.action("start_worker").label("Start worker").run(start);    // `pro.start_worker`
//!     pro.layer("planner").view(planner_overlay);                     // `pro.planner`
//!     heca::run();
//! }
//! ```
//!
//! Nothing here is a second mechanism. Each door fills the one that already existed — a dock goes
//! into the region list, an action into the [`ActionCatalog`](crate::actions::ActionCatalog) and
//! registry every built-in uses — so what is added is reachable from a key, a menu, the palette and
//! RPC with nothing written for any of them.
//!
//! **What is added is a default.** Where a dock sits and whether a header button shows is the
//! user's to decide: a dock they move stays where they put it, and `[settings] show_*_sidebar` hides
//! a whole side whatever was added to it. (Placement is not saved across runs yet, so a
//! move lasts for the session.)
//!
//! **Which side each door lives on.** heca is going to run as a server holding all state, with each
//! window a client that only draws (F012/P104). The doors are split by that line, so the split
//! routes them rather than redesigning them:
//!
//! | side | doors | why |
//! |---|---|---|
//! | server | [`action`] | it changes state, so it runs where the state is |
//! | client | docks (`regions`), layers, pane decoration | it is drawn, so it lives in a window |
//!
//! Every door queues its declarations and the host takes them once, as it starts. A call after
//! that is said out loud and dropped, never ignored quietly.

mod action;
mod extension;
mod layer;

pub(crate) use action::register_queued_actions;
pub use extension::{Extension, extension};
pub(crate) use layer::{AddedLayers, rebuild as rebuild_added_layer, register_queued_layers};
pub use layer::{LayerBuilder, LayerCx};
