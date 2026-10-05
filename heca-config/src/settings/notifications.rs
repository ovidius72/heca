//! `[settings.notification_system]` — how in-app toast notifications behave.

use serde::{Deserialize, Serialize};

/// Default auto-dismiss delay for an in-app notification, in milliseconds (4 s).
pub(super) fn default_notification_auto_dismiss_ms() -> u64 {
    4000
}

/// Default number of notification cards on screen at once.
///
/// Small on purpose: the cards are transient and the stack is not a log. Overflow is not lost — it
/// queues and joins the column as slots free — so raising this trades reading room against how much
/// of the window the stack is allowed to cover.
pub(super) fn default_notification_max_visible() -> usize {
    5
}

/// Default most lines a notification's title or body may take before it ends with `…`.
///
/// A card wraps rather than cuts, so a long path or error shows whole; this only stops a runaway
/// message from covering the window.
pub(super) fn default_notification_max_lines() -> usize {
    8
}


/// Where a raised notification is delivered.
///
/// `system` is **reserved** — the OS-notification backend is not built, so it falls back to
/// `app` until it lands. `none` suppresses every notification (nothing is raised at all).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSystem {
    /// In-app toast stack. **The default.**
    #[default]
    App,
    /// OS / desktop notifications. Reserved — falls back to `App` until the backend exists.
    System,
    /// No notifications at all.
    None,
}

/// `[settings.notification_system]` — how in-app toast notifications behave.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotificationSystemConfig {
    /// Where a notification is delivered: `app` (default) / `system` (reserved) / `none`.
    #[serde(default)]
    pub mode: NotificationSystem,
    /// How long a notification stays on screen before it dismisses itself, in
    /// milliseconds. Applies to every notification that auto-dismisses; one a
    /// producer marks sticky ignores it, and a per-notification lifetime override
    /// still wins. Default: 4000.
    #[serde(default = "default_notification_auto_dismiss_ms")]
    pub auto_dismiss_ms: u64,
    /// How many notification cards are on screen at once. Further notifications queue in the order
    /// they were raised — nothing is dropped.
    ///
    /// Closing one slides the cards after it up so the column never shows a blank slot, and the
    /// next in line joins at the **end**. Default: 5. Clamped to at least 1.
    #[serde(default = "default_notification_max_visible")]
    pub max_visible: usize,
    /// The most lines a card's title and its body may each take. Text longer than a card is wide
    /// **wraps** onto further lines (a path folds after a `/`) instead of being cut; text that needs
    /// more than this ends with `…`. Default: 8. Clamped to at least 1.
    #[serde(default = "default_notification_max_lines")]
    pub max_lines: usize,
}

impl Default for NotificationSystemConfig {
    fn default() -> Self {
        Self {
            mode: NotificationSystem::default(),
            auto_dismiss_ms: default_notification_auto_dismiss_ms(),
            max_visible: default_notification_max_visible(),
            max_lines: default_notification_max_lines(),
        }
    }
}
