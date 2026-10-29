//! App-owned notification domain model.
//!
//! This module defines what a notification *is* in the host application, above the
//! presentation seam. The grid-ui widget [`ToastSpec`](heca_grid_ui::widgets::ToastSpec)
//! (`heca_grid_ui::widgets::ToastSpec`) is a thin *projection* of these types and never
//! owns queue, lifecycle, timers, or dedup. Those are host responsibilities.
//!
//! What is here:
//! - [`NotificationSource`]: a descriptive, extensible origin for log, history and dedup,
//!   carrying no routing or policy semantics.
//! - lifecycle semantics + defaults.
//! - the canonical [`AppNotification`] + [`NotificationId`].
//! - actions as [`heca_view::Intent`].
//! - severity + `ToastSeverity` mapping.
//! - the `NotificationSystem` config enum.
//! - the `NotificationDraft` builder.
//! - the `AppNotification` → `ToastSpec` projection.
//!
//! # Design rules
//!
//! - Notification actions store a [`heca_view::Intent`] (name-keyed + args),
//!   never closures and never bare [`crate::input::WmAction`].
//! - The [`NotificationSource`] does **not** decide routing or
//!   [`ActionPolicy`](crate::app::interaction). It is descriptive metadata.
//! - There is deliberately **no `Agent`** source kind — that belongs to a
//!   separate feature. Adding it here would be a proposal, not a commit.
//! - No field holds a mutable reference to workspace/pane *indices*, which can
//!   become invalid after a notification is archived; opaque identifiers
//!   (strings) are used instead.

// Parts of the notification API (the draft builder, the store's history and look-ups) are for
// producers and views that are planned but not built; tests exercise them.
#![allow(dead_code)]

mod action;
mod model;
mod presentation;
mod runtime;
mod sink;
mod store;

pub use action::register_notify_action;
pub use model::*;
pub use presentation::{NotificationView, notification_view, toast_spec};
pub use runtime::NotificationRuntime;
pub use sink::{Notification, install_notification_sink};
pub(crate) use sink::collect_during as collect_notifications;
pub(crate) use sink::raise;
pub use store::*;

#[cfg(test)]
mod test_support;
