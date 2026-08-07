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
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use heca_view::Intent;

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

/// A session-monotonic, never-reused notification identity.
///
/// Allocated via [`NotificationId::next`] from a process-wide atomic counter, so
/// ids strictly increase and are never recycled within a session — even after a
/// notification is dismissed and archived. This is what the store reconciles on
/// and what `on_dismiss`/`on_action` report back.
///
/// Deliberately **not** `Copy`-able across the host boundary in a way that lets
/// two owners mutate the same id; it is `Clone` (cheap, just a `u64`) for
/// projection, but the counter only moves forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NotificationId(u64);

impl NotificationId {
    /// The raw counter value. Mainly for logging/debug; do not parse semantics
    /// out of it.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Allocate the next id. Monotonic across the whole process; never reused
    /// within a session. The counter only ever increases.
    pub fn next() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        // Relaxed: we only need monotonicity per-call, not cross-thread ordering
        // of *which* id went to *which* thread — the host store serializes use.
        let v = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(v)
    }

    /// Construct an id from a known value. **Test-only** — production code must
    /// use [`next`](Self::next) so ids stay monotonic and unique.
    #[doc(hidden)]
    pub fn from_raw(v: u64) -> Self {
        Self(v)
    }
}

/// Where a notification currently sits in the host store.
///
/// Separated from the canonical [`AppNotification`] data so the store can
/// move a notification between buckets (queued → visible → history) without
/// rewriting its immutable identity/content. The data fields (source, title,
/// …) are set once at creation; only [`placement`](AppNotification::placement)
/// and [`expires_at`](AppNotification::expires_at) change over its lifetime.
///
/// - `Queued` — accepted but not yet promoted to visible (e.g. over the
///   max-visible cap; waiting for a slot).
/// - `Visible` — in the active toast stack; may carry an `expires_at` computed
///   lazily by the store at promotion time (T185 contract).
/// - `History` — dismissed/expired/archived; kept up to
///   `notification_history_limit` then dropped (FIFO).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPlacement {
    Queued,
    Visible { expires_at: Option<Instant> },
    History,
}

impl Default for NotificationPlacement {
    /// New notifications start `Queued`; the store promotes them to `Visible`.
    fn default() -> Self {
        Self::Queued
    }
}

/// The canonical notification the host store owns.
///
/// Immutable identity + content (set at creation) plus two mutable placement
/// fields the store owns: [`placement`](Self::placement) and
/// [`expires_at`](Self::expires_at) (the latter lives on `Visible` to keep the
/// serde model clean). Stores no `Toast` widget and no UI callback — those are
/// presentation concerns; this type only describes *what* the notification is.
///
/// The optional [`action`](Self::action) is a [`heca_view::Intent`] (a routing
/// name plus args), never a closure — per the phase contract. T189 formalizes
/// the action helpers for notifications; T188 just carries the field.
///
/// `Clone` is cheap-ish (title/body/action are `Arc`-shared on clone in a
/// follow-up; for now they are owned `String`s — clone only for projection, not
/// on every frame). Targeted projection to [`ToastSpec`] (T193) reads the
/// fields it needs without cloning the whole store.
#[derive(Debug, Clone, PartialEq)]
pub struct AppNotification {
    /// Session-monotonic, never-reused identity.
    pub id: NotificationId,
    /// Optional dedup key. When set, the store treats two notifications with the
    /// same key as the same logical event (updates in place rather than
    /// stacking). Constructed from the source + a caller-supplied suffix.
    pub dedup_key: Option<String>,
    /// Descriptive provenance (log/history/dedup). T184.
    pub source: NotificationSource,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: Option<String>,
    /// Lifecycle policy (lifetime + dismissibility). T185.
    pub lifecycle: NotificationLifecycle,
    /// Optional inline action, dispatched as an [`Intent`] (T189 formalizes).
    pub action: Option<Intent>,
    /// When the notification was created. Injected by the caller (not `now()`)
    /// so domain tests are deterministic — per T185's rule that instants stay
    /// out of the builder when tests need them.
    pub created_at: Instant,
    /// Current placement bucket. Mutated by the store; not by the builder.
    pub placement: NotificationPlacement,
}

impl AppNotification {
    /// Build a notification with the given identity + title. Prefer the
    /// [`NotificationDraft`] builder (T192) for ergonomics; this is the canonical
    /// constructor the builder reduces to.
    ///
    /// `created_at` is taken explicitly so tests stay deterministic (T185 rule).
    pub fn new(
        id: NotificationId,
        source: NotificationSource,
        severity: NotificationSeverity,
        title: impl Into<String>,
        created_at: Instant,
    ) -> Self {
        Self {
            id,
            dedup_key: None,
            source,
            severity,
            title: title.into(),
            body: None,
            lifecycle: NotificationLifecycle::default(),
            action: None,
            created_at,
            placement: NotificationPlacement::default(),
        }
    }

    /// Set an optional dedup key.
    pub fn dedup_key(mut self, key: impl Into<String>) -> Self {
        self.dedup_key = Some(key.into());
        self
    }

    /// Set an optional body.
    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Set the lifecycle policy.
    pub fn lifecycle(mut self, l: NotificationLifecycle) -> Self {
        self.lifecycle = l;
        self
    }

    /// Set the optional action (an [`Intent`]).
    pub fn action(mut self, intent: Intent) -> Self {
        self.action = Some(intent);
        self
    }

    /// Whether the notification is currently visible in the toast stack.
    pub fn is_visible(&self) -> bool {
        matches!(self.placement, NotificationPlacement::Visible { .. })
    }

    /// Whether the notification has been archived to history.
    pub fn is_history(&self) -> bool {
        matches!(self.placement, NotificationPlacement::History)
    }

    /// The computed expiry instant, if any (only `Visible` with a deadline).
    pub fn expires_at(&self) -> Option<Instant> {
        match self.placement {
            NotificationPlacement::Visible { expires_at } => expires_at,
            _ => None,
        }
    }
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

    // -- canonical model (T188) -------------------------------------------

    #[test]
    fn notification_ids_are_monotonic_and_unique() {
        let a = NotificationId::next();
        let b = NotificationId::next();
        let c = NotificationId::next();
        assert!(b.as_u64() > a.as_u64());
        assert!(c.as_u64() > b.as_u64());
        assert_ne!(a, b);
        assert_ne!(b, c);
    }

    #[test]
    fn notification_id_serializes_as_u64() {
        let id = NotificationId::from_raw(42);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "42");
        let back: NotificationId = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn placement_default_is_queued() {
        assert_eq!(NotificationPlacement::default(), NotificationPlacement::Queued);
    }

    #[test]
    fn app_notification_new_has_defaults() {
        let id = NotificationId::from_raw(1);
        let now = Instant::now();
        let n = AppNotification::new(
            id,
            NotificationSource::app("test"),
            NotificationSeverity::Info,
            "Build done",
            now,
        );
        assert_eq!(n.id, id);
        assert!(n.dedup_key.is_none());
        assert!(n.body.is_none());
        assert!(n.action.is_none());
        assert_eq!(n.placement, NotificationPlacement::Queued);
        assert!(!n.is_visible());
        assert!(!n.is_history());
        assert!(n.expires_at().is_none());
        assert_eq!(n.created_at, now);
    }

    #[test]
    fn app_notification_builders_set_fields() {
        let id = NotificationId::from_raw(7);
        let n = AppNotification::new(
            id,
            NotificationSource::new(NotificationSourceKind::Plugin, "git"),
            NotificationSeverity::Error,
            "Push failed",
            Instant::now(),
        )
        .dedup_key("git-push")
        .body("rejected by remote")
        .lifecycle(NotificationLifecycle::sticky())
        .action(Intent::new("git.push"));
        assert_eq!(n.dedup_key.as_deref(), Some("git-push"));
        assert_eq!(n.body.as_deref(), Some("rejected by remote"));
        assert!(n.lifecycle.is_dismissible());
        assert_eq!(n.action.as_ref().unwrap().action, "git.push");
    }

    #[test]
    fn app_notification_visible_carries_expiry() {
        let mut n = AppNotification::new(
            NotificationId::from_raw(2),
            NotificationSource::app("t"),
            NotificationSeverity::Info,
            "x",
            Instant::now(),
        );
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        n.placement = NotificationPlacement::Visible { expires_at: Some(deadline) };
        assert!(n.is_visible());
        assert_eq!(n.expires_at(), Some(deadline));
    }

    #[test]
    fn app_notification_history_placement() {
        let mut n = AppNotification::new(
            NotificationId::from_raw(3),
            NotificationSource::app("t"),
            NotificationSeverity::Info,
            "x",
            Instant::now(),
        );
        n.placement = NotificationPlacement::History;
        assert!(n.is_history());
        assert!(!n.is_visible());
        assert!(n.expires_at().is_none()); // History has no expiry
    }

    #[test]
    fn app_notification_clones_for_projection() {
        let n = AppNotification::new(
            NotificationId::from_raw(4),
            NotificationSource::app("t"),
            NotificationSeverity::Success,
            "Done",
            Instant::now(),
        );
        let proj = n.clone(); // cheap clone for projection (T193 will read fields)
        assert_eq!(proj.id, n.id);
        assert_eq!(proj.title, n.title);
        // original untouched by projection
        assert_eq!(n.placement, NotificationPlacement::Queued);
    }
}