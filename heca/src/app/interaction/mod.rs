//! Central interaction policy layer.
//!
//! Every user-initiated action that changes WM state must flow through
//! `dispatch_action()` — the single chokepoint that decides whether an
//! interaction is allowed based on the current focus domain, input mode,
//! and interaction source.
//!
//! # Architecture
//!
//! ```text
//! User input → InteractionIntent → route_interaction() → RouteDecision
//!                                                        ├─ Allow(intent) → registry.execute()
//!                                                        └─ Block          → no-op
//! ```
//!
//! For keyboard actions:
//! ```text
//! KeyCombo → WmAction → dispatch_action(state, Keyboard, &action)
//! ```
//!
//! For mouse/sidebar actions:
//! ```text
//! Click/Drag → InteractionIntent::FocusPane { .. } → dispatch_action(state, MouseContent, &WmAction)
//! ```
//!
//! Handler-to-handler calls bypass the router and use `registry.execute()` directly.
//!
//! # Floating domain policy
//!
//! When `FocusDomain::Floating` is active, only `FocusedPaneLocal` actions
//! (Float/Unfloat, ClosePane, RenamePane) are allowed. Everything else is
//! blocked — tiled layout actions, sidebar, workspace switching, command
//! palette, mouse drag, and pane selection overlays.
//!
//! The only escape from floating is `prefix+f` (Float toggle) or closing the
//! floating pane (ClosePane).

mod dispatch;
mod domain;
mod focus;
mod policy;
mod route;
#[cfg(test)]
mod tests;
mod types;
mod view_intent;

pub(crate) use types::*;
pub use policy::ActionPolicy;
pub(crate) use policy::{action_allowed_when_floating, action_policy};
pub(crate) use focus::*;
pub(crate) use dispatch::*;
pub(crate) use view_intent::*;
