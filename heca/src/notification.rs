//! App-owned notification domain model (F009 — Notification System).
//!
//! This module defines what a notification *is* in the host application, above the
//! presentation seam. The grid-ui widget [`ToastSpec`]
//! (`heca_grid_ui::widgets::ToastSpec`) is a thin *projection* of these types —
//! see [`project`](crate::notification::project) in the T193 task — and never
//! owns queue, lifecycle, timers, or dedup. Those are host responsibilities.
//!
//! Phase P061 builds the model in dependency order:
//! - T184 — [`NotificationSource`] (this task): a descriptive, extensible origin
//!   for log, history and dedup, carrying no routing or policy semantics.
//! - T185 — lifecycle semantics + defaults.
//! - T188 — canonical [`AppNotification`] + [`NotificationId`].
//! - T189 — actions as [`heca_view::Intent`].
//! - T190 — severity + `ToastSeverity` mapping.
//! - T191 — `NotificationSystem` config enum.
//! - T192 — `NotificationDraft` builder.
//! - T193 — `AppNotification` → `ToastSpec` projection.
//!
//! # Design rules (phase contract)
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
//
// Staged phase: P061 (F009) adds the notification domain types one task at a
// time (T184 source → T185 lifecycle → T188 canonical model → T189 actions →
// T190 severity → T191 config → T192 draft → T193 projection). Until T188
// wires `AppNotification` (which owns a `NotificationSource`), the public API
// below has no consumer in the binary, so the whole module is dead-code-clean
// by construction rather than unused. Drop this allow as the consumers land.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// The closed vocabulary of notification origins.
///
/// Extensible in the sense that a new variant is an additive, non-breaking
/// change for matching code that uses a non-exhaustive `_` arm; builders and
/// serialization treat it as a stable enum. Each kind pairs with an opaque
/// [`id`](NotificationSource::id) string that disambiguates within the kind
/// (a plugin id, a command name, a pane handle rendered as a string, …).
///
/// Notably **does not** include an `Agent` variant — agent-originated
/// notifications belong to a separate feature, not the core notification
/// domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSourceKind {
    /// Produced by heca itself (e.g. config reload result, internal status).
    App,
    /// Produced by a config-driven rule or default (e.g. a startup hint).
    Config,
    /// Produced on behalf of a tiled or floating pane (e.g. a bell, exit
    /// status). The pane is identified by an opaque handle string, not an
    /// index, because indices become stale after archival.
    Pane,
    /// Produced by a registered chrome provider (docker, git, …).
    Provider,
    /// Produced by a loaded plugin (future WASM host). The plugin id is the
    /// opaque `id`.
    Plugin,
    /// Produced by a user-invoked command (e.g. `:notify`, a bound action).
    Command,
}

impl Default for NotificationSourceKind {
    /// The default source is the app itself; explicit sources override it.
    fn default() -> Self {
        Self::App
    }
}

impl NotificationSourceKind {
    /// Human-readable label for log/history lines and dedup keys. Lowercase,
    /// stable, no spaces — suitable as the first segment of a dedup key.
    pub fn as_label(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Config => "config",
            Self::Pane => "pane",
            Self::Provider => "provider",
            Self::Plugin => "plugin",
            Self::Command => "command",
        }
    }
}

/// A descriptive, extensible origin for a notification.
///
/// Used for log lines, notification history, and dedup keys. It carries **no**
/// routing or [`ActionPolicy`](crate::app::interaction) semantics — a
/// `Pane` source does not by itself make a notification pane-scoped or
/// floating-only; policy is decided by the action the notification carries
/// (T189), not by where it came from.
///
/// The `id` is an opaque, caller-supplied string. For `Pane` it should be a
/// stable pane handle (not an index); for `Provider`/`Plugin` the registered
/// id; for `Command` the command name; for `App`/`Config` it may be a subsystem
/// tag or be left empty. It is never parsed by the notification domain.
///
/// # Examples
///
/// ```
/// use heca::notification::{NotificationSource, NotificationSourceKind};
///
/// let plugin = NotificationSource::new(NotificationSourceKind::Plugin, "git-watch");
/// let pane = NotificationSource::new(NotificationSourceKind::Pane, "pane-4f2a");
/// let app = NotificationSource::app("config");
///
/// assert_eq!(plugin.kind(), NotificationSourceKind::Plugin);
/// assert_eq!(plugin.id(), Some("git-watch"));
/// assert_eq!(app.kind(), NotificationSourceKind::App);
/// // dedup key first segment is the kind label:
/// assert_eq!(pane.kind().as_label(), "pane");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationSource {
    kind: NotificationSourceKind,
    /// Opaque, caller-supplied identifier within `kind`. Empty means
    /// "unspecified"; never parsed by the notification domain.
    id: Option<String>,
}

impl NotificationSource {
    /// A source of `kind` with an opaque identifier.
    pub fn new(kind: NotificationSourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: Some(id.into()),
        }
    }

    /// A source of `kind` with no specific identifier.
    pub fn of(kind: NotificationSourceKind) -> Self {
        Self { kind, id: None }
    }

    /// An `App` source tagged with an optional subsystem name.
    pub fn app(subsystem: impl Into<String>) -> Self {
        Self::new(NotificationSourceKind::App, subsystem)
    }

    /// The origin kind.
    pub fn kind(&self) -> NotificationSourceKind {
        self.kind
    }

    /// The opaque identifier within the kind, if any.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// A stable, non-empty dedup key segment: `<kind>[:<id>]`. Suitable as the
    /// first segment of a full dedup key; later tasks may append a sub-key.
    pub fn dedup_segment(&self) -> String {
        match &self.id {
            Some(id) if !id.is_empty() => format!("{}:{}", self.kind.as_label(), id),
            _ => self.kind.as_label().to_string(),
        }
    }
}

impl Default for NotificationSource {
    /// Defaults to an unspecified `App` source, matching
    /// [`NotificationSourceKind::default`].
    fn default() -> Self {
        Self::of(NotificationSourceKind::default())
    }
}

/// How long a notification stays visible once the store promotes it to the
/// active queue.
///
/// This is the *configured* lifetime only — it carries no `Instant` and no
/// `expires_at`. The host store computes `expires_at` lazily, at the moment it
/// promotes a notification to visible (see T185 contract: "the timer starts
/// when the store promotes the notification to visible, not at push time").
/// Keeping the instant out of this type lets the domain builder stay pure and
/// deterministic in tests.
///
/// `Sticky` notifications never expire on their own but can still be closed by
/// the user when [`dismissible`](NotificationLifecycle::dismissible) is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationLifetime {
    /// Auto-dismiss after the given number of seconds once visible.
    Auto(std::time::Duration),
    /// Stay until explicitly dismissed (or replaced). Does not expire.
    Sticky,
}

impl Default for NotificationLifetime {
    /// Default lifetime is 5 s of auto-dismiss (matches the `Info` severity
    /// default; see [`default_lifetime_for_severity`]).
    fn default() -> Self {
        Self::Auto(std::time::Duration::from_secs(5))
    }
}

/// Lifecycle policy for a notification: when it leaves the active queue and
/// whether the user can close it.
///
/// Split from [`NotificationLifetime`] because dismissibility is orthogonal
/// to lifetime: a `Sticky` notification can be dismissible or not, and an
/// `Auto` notification can be made non-dismissible (it then expires on its own
/// but cannot be closed early).
///
/// Carries no `Instant`. The host store computes expiry lazily on promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NotificationLifecycle {
    lifetime: NotificationLifetime,
    /// Whether the × dismiss affordance is shown and the notification can be
    /// closed by the user. Sticky + non-dismissible = stays until replaced.
    dismissible: bool,
}

impl NotificationLifecycle {
    /// A lifecycle with the given lifetime that is dismissible.
    pub fn new(lifetime: NotificationLifetime) -> Self {
        Self {
            lifetime,
            dismissible: true,
        }
    }

    /// A dismissible, auto-dismiss lifecycle.
    pub fn auto(duration: std::time::Duration) -> Self {
        Self::new(NotificationLifetime::Auto(duration))
    }

    /// A dismissible sticky lifecycle (stays until closed).
    pub fn sticky() -> Self {
        Self::new(NotificationLifetime::Sticky)
    }

    /// A non-dismissible sticky lifecycle (stays until replaced).
    pub fn sticky_persistent() -> Self {
        Self {
            lifetime: NotificationLifetime::Sticky,
            dismissible: false,
        }
    }

    /// Set whether the user can dismiss this notification.
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// The configured lifetime (no `expires_at`; computed lazily on promotion).
    pub fn lifetime(&self) -> NotificationLifetime {
        self.lifetime
    }

    /// Whether the user can close this notification.
    pub fn is_dismissible(&self) -> bool {
        self.dismissible
    }

    /// Whether this notification expires on its own (only `Auto`).
    pub fn expires(&self) -> bool {
        matches!(self.lifetime, NotificationLifetime::Auto(_))
    }
}

impl Default for NotificationLifecycle {
    /// Defaults to a dismissible, 5 s auto-dismiss lifecycle.
    fn default() -> Self {
        Self::new(NotificationLifetime::default())
    }
}

/// Default lifetime per severity, used by T190's severity mapping and by the
/// `NotificationDraft` builder (T192). Lower severity stays longer; error-level
/// and sticky-by-default severities stay until dismissed.
///
/// Kept here (not in config) because it is domain semantics, not user-tunable
/// presentation. The user can override per-notification via the builder.
pub fn default_lifetime_for_severity(severity: NotificationSeverity) -> NotificationLifetime {
    match severity {
        // Errors and warnings are sticky — the user should acknowledge them.
        NotificationSeverity::Error | NotificationSeverity::Warning => NotificationLifetime::Sticky,
        // Success is a transient confirmation; auto-dismiss after 4 s.
        NotificationSeverity::Success => NotificationLifetime::Auto(std::time::Duration::from_secs(4)),
        // Info is the most common, transient; 5 s (the type default).
        NotificationSeverity::Info => NotificationLifetime::Auto(std::time::Duration::from_secs(5)),
    }
}

/// Placeholder severity enum so the lifecycle defaults compile before T190
/// lands the real mapping. T190 replaces this with the canonical
/// `NotificationSeverity` + its `ToastSeverity` projection; until then the
/// lifecycle default depends on this closed set.
///
/// Deliberately mirrors `heca_grid_ui::widgets::ToastSeverity`'s variants so
/// T190 is a rename/re-export, not a new design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Error,
    Warning,
    Success,
    #[default]
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_label_is_lowercase_stable() {
        assert_eq!(NotificationSourceKind::App.as_label(), "app");
        assert_eq!(NotificationSourceKind::Config.as_label(), "config");
        assert_eq!(NotificationSourceKind::Pane.as_label(), "pane");
        assert_eq!(NotificationSourceKind::Provider.as_label(), "provider");
        assert_eq!(NotificationSourceKind::Plugin.as_label(), "plugin");
        assert_eq!(NotificationSourceKind::Command.as_label(), "command");
    }

    #[test]
    fn no_agent_variant_exists() {
        // Compile-time: there is no Agent variant. This exhaustive match with no
        // Agent arm fails to compile if one is added — a guard rail, not a goal.
        fn has_no_agent(k: NotificationSourceKind) {
            match k {
                NotificationSourceKind::App
                | NotificationSourceKind::Config
                | NotificationSourceKind::Pane
                | NotificationSourceKind::Provider
                | NotificationSourceKind::Plugin
                | NotificationSourceKind::Command => {}
            }
        }
        has_no_agent(NotificationSourceKind::App);
    }

    #[test]
    fn source_carries_kind_and_opaque_id() {
        let s = NotificationSource::new(NotificationSourceKind::Plugin, "git-watch");
        assert_eq!(s.kind(), NotificationSourceKind::Plugin);
        assert_eq!(s.id(), Some("git-watch"));
    }

    #[test]
    fn app_helper_sets_app_kind() {
        let s = NotificationSource::app("config");
        assert_eq!(s.kind(), NotificationSourceKind::App);
        assert_eq!(s.id(), Some("config"));
    }

    #[test]
    fn of_helper_leaves_id_none() {
        let s = NotificationSource::of(NotificationSourceKind::Command);
        assert_eq!(s.kind(), NotificationSourceKind::Command);
        assert!(s.id().is_none());
    }

    #[test]
    fn default_is_unspecified_app() {
        let s = NotificationSource::default();
        assert_eq!(s.kind(), NotificationSourceKind::App);
        assert!(s.id().is_none());
    }

    #[test]
    fn dedup_segment_combines_kind_and_id() {
        let s = NotificationSource::new(NotificationSourceKind::Pane, "pane-4f2a");
        assert_eq!(s.dedup_segment(), "pane:pane-4f2a");
    }

    #[test]
    fn dedup_segment_falls_back_to_kind_when_id_missing_or_empty() {
        assert_eq!(NotificationSource::of(NotificationSourceKind::App).dedup_segment(), "app");
        assert_eq!(
            NotificationSource::new(NotificationSourceKind::App, "").dedup_segment(),
            "app"
        );
    }

    #[test]
    fn source_serializes_with_snake_case_kind() {
        let s = NotificationSource::new(NotificationSourceKind::Provider, "docker");
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"kind\":\"provider\""), "got {json}");
        let back: NotificationSource = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn source_eq_hash_stable_across_clone() {
        let a = NotificationSource::new(NotificationSourceKind::Plugin, "x");
        let b = a.clone();
        assert_eq!(a, b);
        let mut set = std::collections::HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    // -- lifecycle (T185) -------------------------------------------------

    #[test]
    fn lifecycle_default_is_dismissible_auto_5s() {
        let l = NotificationLifecycle::default();
        assert!(l.is_dismissible());
        assert!(l.expires());
        assert_eq!(l.lifetime(), NotificationLifetime::Auto(std::time::Duration::from_secs(5)));
    }

    #[test]
    fn sticky_is_dismissible_and_does_not_expire() {
        let l = NotificationLifecycle::sticky();
        assert!(l.is_dismissible());
        assert!(!l.expires());
        assert_eq!(l.lifetime(), NotificationLifetime::Sticky);
    }

    #[test]
    fn sticky_persistent_is_not_dismissible_and_does_not_expire() {
        let l = NotificationLifecycle::sticky_persistent();
        assert!(!l.is_dismissible());
        assert!(!l.expires());
        assert_eq!(l.lifetime(), NotificationLifetime::Sticky);
    }

    #[test]
    fn dismissible_can_be_turned_off() {
        let l = NotificationLifecycle::auto(std::time::Duration::from_secs(3)).dismissible(false);
        assert!(!l.is_dismissible());
        assert!(l.expires());
    }

    #[test]
    fn auto_lifetime_expires_sticky_does_not() {
        assert!(NotificationLifecycle::auto(std::time::Duration::from_secs(1)).expires());
        assert!(!NotificationLifecycle::sticky().expires());
    }

    #[test]
    fn lifecycle_carries_no_instant() {
        // The domain type carries no expires_at / Instant — the store computes
        // expiry lazily on promotion. Ensure the public API exposes no time field.
        let l = NotificationLifecycle::sticky();
        let s = format!("{l:?}");
        assert!(!s.contains("Instant"));
        assert!(!s.contains("expires_at"));
    }

    #[test]
    fn default_lifetime_for_severity_maps_each_variant() {
        assert_eq!(
            default_lifetime_for_severity(NotificationSeverity::Error),
            NotificationLifetime::Sticky
        );
        assert_eq!(
            default_lifetime_for_severity(NotificationSeverity::Warning),
            NotificationLifetime::Sticky
        );
        assert_eq!(
            default_lifetime_for_severity(NotificationSeverity::Success),
            NotificationLifetime::Auto(std::time::Duration::from_secs(4))
        );
        assert_eq!(
            default_lifetime_for_severity(NotificationSeverity::Info),
            NotificationLifetime::Auto(std::time::Duration::from_secs(5))
        );
    }

    #[test]
    fn lifecycle_serializes_with_snake_case() {
        let auto = NotificationLifecycle::auto(std::time::Duration::from_secs(2));
        let json = serde_json::to_string(&auto).unwrap();
        assert!(json.contains("\"auto\""), "got {json}");
        assert!(json.contains("\"dismissible\":true"), "got {json}");
        // round-trip
        let back: NotificationLifecycle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, auto);

        let sticky = NotificationLifecycle::sticky();
        let sjson = serde_json::to_string(&sticky).unwrap();
        assert!(sjson.contains("\"sticky\""), "got {sjson}");
        assert!(sjson.contains("\"dismissible\":true"), "got {sjson}");
        let sback: NotificationLifecycle = serde_json::from_str(&sjson).unwrap();
        assert_eq!(sback, sticky);
    }
}