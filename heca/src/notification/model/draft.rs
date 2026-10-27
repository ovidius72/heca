use super::*;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Producer-facing builder for an [`AppNotification`].
///
/// The only required input is a title. Defaults are app-owned: source `App`,
/// severity `Info`, and the configured dismissible auto-dismiss lifecycle. The
/// draft deliberately does **not** allocate a [`NotificationId`], read
/// `Instant::now()`, choose a delivery channel, or project to `ToastSpec`.
/// The router/store supplies the id and creation instant at acceptance time;
/// this keeps producer code deterministic and prevents a draft from claiming
/// visibility before the store promotes it.
///
/// Serializable so a window that is a pure client can raise one over the wire: the draft is plain
/// data (no `Instant`, no closures), and the server stamps the acceptance time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationDraft {
    title: String,
    body: Option<String>,
    source: NotificationSource,
    severity: NotificationSeverity,
    pub(in crate::notification) lifecycle: NotificationLifecycle,
    explicit_lifetime: bool,
    pub(in crate::notification) dedup_key: Option<String>,
    actions: Vec<NotificationAction>,
}

impl NotificationDraft {
    /// Start a draft with the required title and domain defaults.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            source: NotificationSource::default(),
            severity: NotificationSeverity::default(),
            lifecycle: NotificationLifecycle::default(),
            explicit_lifetime: false,
            dedup_key: None,
            actions: Vec::new(),
        }
    }

    /// `Self::new(title).severity(NotificationSeverity::Info)` — the domain default, spelled out.
    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title).severity(NotificationSeverity::Info)
    }

    /// `Self::new(title).severity(NotificationSeverity::Success)`.
    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).severity(NotificationSeverity::Success)
    }

    /// `Self::new(title).severity(NotificationSeverity::Warning)`.
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).severity(NotificationSeverity::Warning)
    }

    /// `Self::new(title).severity(NotificationSeverity::Error)` (the wire/presentation spelling
    /// is "danger", matching `ToastSeverity::Danger` — see [`NotificationSeverity`]'s `EnumArg`).
    pub fn danger(title: impl Into<String>) -> Self {
        Self::new(title).severity(NotificationSeverity::Error)
    }

    /// Set the descriptive producer source.
    pub fn source(mut self, source: NotificationSource) -> Self {
        self.source = source;
        self
    }

    /// Set app-owned severity (presentation mapping happens later).
    pub fn severity(mut self, severity: NotificationSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the complete lifecycle policy.
    pub fn lifecycle(mut self, lifecycle: NotificationLifecycle) -> Self {
        self.lifecycle = lifecycle;
        self.explicit_lifetime = true;
        self
    }

    /// Set only dismissibility while retaining the configured lifetime.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.lifecycle = self.lifecycle.dismissible(dismissible);
        self
    }

    /// Sticky: no auto-dismiss timer, stays until the user (or a producer) dismisses it.
    /// `NotificationLifecycle::sticky()` spelled out on the draft, for producer ergonomics.
    pub fn sticky(mut self) -> Self {
        self.lifecycle = NotificationLifecycle::sticky();
        self.explicit_lifetime = true;
        self
    }

    /// Auto-dismiss after `duration`, overriding the configured default lifetime.
    pub fn auto_dismiss(mut self, duration: std::time::Duration) -> Self {
        self.lifecycle = NotificationLifecycle::auto(duration);
        self.explicit_lifetime = true;
        self
    }

    /// Apply the host's default only when the producer left the lifetime unspecified.
    pub(in crate::notification) fn with_default_auto_dismiss(mut self, duration: std::time::Duration) -> Self {
        if !self.explicit_lifetime {
            self.lifecycle = self.lifecycle.with_auto_duration(duration);
        }
        self
    }

    /// Set optional body text.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set an optional deduplication key.
    pub fn dedup_key(mut self, key: impl Into<String>) -> Self {
        self.dedup_key = Some(key.into());
        self
    }

    /// Append an inline name-keyed notification action. Callable more than
    /// once — each must name a distinct `intent.action` (a card may show
    /// several actions).
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Materialize the draft into a queued notification.
    ///
    /// The caller supplies both the monotonic id and creation instant; this
    /// method never calls `Instant::now()` and never marks the result visible.
    pub fn build(self, id: NotificationId, created_at: Instant) -> AppNotification {
        let mut notification =
            AppNotification::new(id, self.source, self.severity, self.title, created_at)
                .lifecycle(self.lifecycle);
        notification.body = self.body;
        notification.dedup_key = self.dedup_key;
        notification.actions = self.actions;
        notification
    }
}
