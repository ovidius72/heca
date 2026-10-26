//! Notification data and lifecycle vocabulary.

mod action;
mod draft;
mod entry;
mod vocabulary;

pub use action::NotificationAction;
pub use draft::NotificationDraft;
pub use entry::{AppNotification, NotificationArchiveCause, NotificationId, NotificationPlacement};
pub use vocabulary::*;

#[cfg(test)]
mod tests_1;

#[cfg(test)]
mod tests_2;
