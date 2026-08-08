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
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use heca_view::Intent;
use heca_grid_ui::hint::HintTargetId;
use heca_grid_ui::widgets::{ToastSeverity, ToastSpec};

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

/// Canonical app-owned notification severity.
///
/// This enum is deliberately independent of the grid-ui presentation enum;
/// severity drives domain defaults (including lifecycle) while the adapter
/// [`NotificationSeverity::as_toast_severity`] maps it to paint-time tokens.
/// It does not reorder notifications already visible in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSeverity {
    Error,
    Warning,
    Success,
    #[default]
    Info,
}

impl NotificationSeverity {
    /// Convert to the presentation severity used by `ToastSpec`/`Toast`.
    ///
    /// This is total and exhaustive: adding a new domain severity requires an
    /// explicit presentation decision instead of silently falling back.
    pub fn as_toast_severity(self) -> ToastSeverity {
        match self {
            Self::Info => ToastSeverity::Info,
            Self::Success => ToastSeverity::Success,
            Self::Warning => ToastSeverity::Warning,
            Self::Error => ToastSeverity::Danger,
        }
    }
}

impl From<NotificationSeverity> for ToastSeverity {
    fn from(severity: NotificationSeverity) -> Self {
        severity.as_toast_severity()
    }
}

/// A notification action: a user-facing label plus a serializable view intent.
///
/// The intent stores a name-keyed action id and optional `PropMap` arguments. It
/// is resolved only when activated through the app's `dispatch_view_intent`
/// path; this type never stores a closure and is not limited to `WmAction`.
/// `dismiss_after` controls the post-dispatch policy: keep it `false` for
/// retry-like actions, while dismissive actions close only according to the
/// dispatch result and never bypass `ActionRegistry`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationAction {
    /// Label rendered on the action affordance.
    pub label: String,
    /// Name-keyed action intent with optional arguments.
    pub intent: Intent,
    /// Whether a successful activation may dismiss the notification.
    pub dismiss_after: bool,
}

impl NotificationAction {
    /// Create a non-dismissing action (appropriate for Retry).
    pub fn new(label: impl Into<String>, intent: Intent) -> Self {
        Self {
            label: label.into(),
            intent,
            dismiss_after: false,
        }
    }

    /// Set whether successful activation may dismiss the notification.
    pub fn dismiss_after(mut self, dismiss: bool) -> Self {
        self.dismiss_after = dismiss;
        self
    }

    /// Convenience constructor for an action that dismisses on successful dispatch.
    pub fn dismissing(label: impl Into<String>, intent: Intent) -> Self {
        Self::new(label, intent).dismiss_after(true)
    }
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
    Visible {
        /// The instant at which the store promoted this entry into a slot.
        visible_since: Instant,
        /// Auto-dismiss deadline; absent for sticky notifications.
        expires_at: Option<Instant>,
    },
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
    /// Optional inline action, dispatched as a name-keyed [`Intent`] with a
    /// label and post-dispatch dismiss policy.
    pub action: Option<NotificationAction>,
    /// When the notification was created. Injected by the caller (not `now()`)
    /// so domain tests are deterministic — per T185's rule that instants stay
    /// out of the builder when tests need them.
    pub created_at: Instant,
    /// Current placement bucket. Mutated by the store; not by the builder.
    placement: NotificationPlacement,
}

/// The fixed number of simultaneous notification cards in the toast host.
pub const MAX_VISIBLE_NOTIFICATIONS: usize = 4;

/// Outcome of one store-owned state transition.
///
/// Consumers use [`visible_projection_changed`](Self::visible_projection_changed)
/// to decide whether to refresh their retained toast projection. They do not
/// inspect queues, slots, or lifecycle state to derive that result themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NotificationStoreUpdate {
    visible_projection_changed: bool,
}

/// Result of accepting a notification draft into [`NotificationStore`].
///
/// The host receives the canonical id and the already-computed projection
/// update. It never allocates ids or inspects store slots to infer refreshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationStorePush {
    notification_id: NotificationId,
    update: NotificationStoreUpdate,
}

impl NotificationStorePush {
    /// Canonical identity of the accepted notification.
    pub fn notification_id(self) -> NotificationId {
        self.notification_id
    }

    /// Projection update produced by accepting the draft.
    pub fn update(self) -> NotificationStoreUpdate {
        self.update
    }
}

/// Failure while accepting a notification draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationStoreError {
    /// Every representable [`NotificationId`] has already been assigned.
    IdExhausted,
}

impl std::fmt::Display for NotificationStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdExhausted => f.write_str("notification id counter is exhausted"),
        }
    }
}

impl std::error::Error for NotificationStoreError {}

impl NotificationStoreUpdate {
    /// Whether this transition changed the projection consumed by the toast host.
    pub fn visible_projection_changed(self) -> bool {
        self.visible_projection_changed
    }
}

/// Canonical, single-owner notification state.
///
/// The store owns every mutable lifecycle concern: the monotonic id counter,
/// canonical payloads, deduplication index, queued order, stable visible slots,
/// and bounded history. Callers never move an [`AppNotification`] between
/// placements directly; the transition helpers added by P056 tasks perform the
/// matching storage/index updates together.
///
/// # Invariants
///
/// - An id appears in exactly one placement: `queued`, one entry of `visible`,
///   or `history`.
/// - `visible` has exactly [`MAX_VISIBLE_NOTIFICATIONS`] stable slots. A slot
///   may become vacant but no existing visible id moves to fill it.
/// - Every id in a placement resolves in `notifications`, and every dedup index
///   entry resolves to exactly one canonical notification with that key.
/// - `next_id` only increases. IDs are never reused, including after history
///   eviction.
/// - History is insertion ordered and retains at most `history_limit` entries;
///   a zero limit retains none.
///
/// All time-aware transition APIs accept their `Instant` from the caller. This
/// type intentionally does not read the wall clock, keeping expiry behavior
/// deterministic and testable.
#[derive(Debug)]
pub struct NotificationStore {
    /// The next session-local id to allocate. `None` means `u64::MAX` was
    /// already allocated and no id can ever be reused.
    next_id: Option<u64>,
    /// Canonical payload for every queued, visible, or retained history id.
    notifications: HashMap<NotificationId, AppNotification>,
    /// Optional deduplication key to its single canonical notification.
    dedup_index: HashMap<String, NotificationId>,
    /// FIFO ids waiting for a visible slot. The front is promoted first.
    queued: VecDeque<NotificationId>,
    /// Stable presentation slots. Existing visible entries never reorder.
    visible: [Option<NotificationId>; MAX_VISIBLE_NOTIFICATIONS],
    /// FIFO archival order; the oldest entry is evicted first.
    history: VecDeque<NotificationId>,
    /// Maximum retained history entries; zero disables retention.
    history_limit: usize,
}

impl NotificationStore {
    /// Create an empty store with a bounded runtime history.
    ///
    /// `history_limit` is a runtime retention policy, not persisted state.
    pub fn new(history_limit: usize) -> Self {
        Self {
            next_id: Some(1),
            notifications: HashMap::new(),
            dedup_index: HashMap::new(),
            queued: VecDeque::new(),
            visible: [None; MAX_VISIBLE_NOTIFICATIONS],
            history: VecDeque::new(),
            history_limit,
        }
    }

    /// The configured maximum number of retained archived notifications.
    pub fn history_limit(&self) -> usize {
        self.history_limit
    }

    /// Whether the store holds no queued, visible, or retained notification.
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Archive every visible auto-dismiss notification due at `now`.
    ///
    /// Slots are cleared in place and then refilled FIFO from pending entries;
    /// occupied slots retain their positions. Sticky notifications have no
    /// deadline and are never selected.
    pub fn expire_due(&mut self, now: Instant) -> NotificationStoreUpdate {
        let (_, update) = self.transition_at(now, |store, now| {
            for slot in &mut store.visible {
                let Some(notification_id) = *slot else {
                    continue;
                };
                let due = store.notifications[&notification_id]
                    .expires_at()
                    .is_some_and(|expires_at| expires_at <= now);
                if due {
                    *slot = None;
                    let notification = store
                        .notifications
                        .get_mut(&notification_id)
                        .expect("visible notification must have canonical storage");
                    notification.placement = NotificationPlacement::History;
                    store.history.push_back(notification_id);
                }
            }
            ((), store.promote_pending(now))
        });
        update
    }

    /// Accept a producer draft into the canonical store.
    ///
    /// A new id is allocated only when no known deduplication key resolves to
    /// an existing notification. The full update/reprioritization policy for
    /// that existing entry belongs to T195; until then this method preserves
    /// its state and returns its canonical id without creating a duplicate.
    ///
    /// New entries first join the pending FIFO and are then promoted into every
    /// vacant stable slot, oldest first. Promotion starts an auto-dismiss timer
    /// from `now`; queued entries have no expiry.
    pub fn push(
        &mut self,
        draft: NotificationDraft,
        now: Instant,
    ) -> Result<NotificationStorePush, NotificationStoreError> {
        if let Some(key) = draft.dedup_key.as_deref()
            && let Some(&notification_id) = self.dedup_index.get(key)
        {
            return Ok(NotificationStorePush {
                notification_id,
                update: NotificationStoreUpdate::default(),
            });
        }

        let (result, update) = self.transition_at(now, |store, now| {
            let Some(raw_id) = store.next_id else {
                return (Err(NotificationStoreError::IdExhausted), false);
            };
            store.next_id = raw_id.checked_add(1);

            let notification_id = NotificationId::from_raw(raw_id);
            let notification = draft.build(notification_id, now);
            if let Some(key) = notification.dedup_key.clone() {
                store.dedup_index.insert(key, notification_id);
            }
            store.notifications.insert(notification_id, notification);
            store.queued.push_back(notification_id);

            let visible_changed = store.promote_pending(now);
            (Ok(notification_id), visible_changed)
        });

        result.map(|notification_id| NotificationStorePush {
            notification_id,
            update,
        })
    }

    /// Promote pending notifications into vacant slots without reordering an
    /// existing visible card. Returns whether the visible projection changed.
    fn promote_pending(&mut self, now: Instant) -> bool {
        let mut visible_changed = false;
        for slot in self.visible.iter_mut().filter(|slot| slot.is_none()) {
            let Some(notification_id) = self.queued.pop_front() else {
                break;
            };

            let notification = self
                .notifications
                .get_mut(&notification_id)
                .expect("queued notification must have canonical storage");
            notification.placement = NotificationPlacement::Visible {
                visible_since: now,
                expires_at: match notification.lifecycle.lifetime() {
                    NotificationLifetime::Auto(duration) => now.checked_add(duration),
                    NotificationLifetime::Sticky => None,
                },
            };
            *slot = Some(notification_id);
            visible_changed = true;
        }
        visible_changed
    }

    /// Execute one store-owned transition at a caller-supplied time.
    ///
    /// All future lifecycle mutations use this funnel rather than exposing the
    /// queue, visible slots, history, or dedup index to consumers. The closure
    /// reports a same-slot payload change; structural slot changes are detected
    /// automatically. The returned [`NotificationStoreUpdate`] is the only
    /// refresh signal the host needs.
    fn transition_at<R>(
        &mut self,
        now: Instant,
        transition: impl FnOnce(&mut Self, Instant) -> (R, bool),
    ) -> (R, NotificationStoreUpdate) {
        let visible_before = self.visible;
        let (result, payload_changed) = transition(self, now);
        self.assert_invariants();

        (
            result,
            NotificationStoreUpdate {
                visible_projection_changed: payload_changed || self.visible != visible_before,
            },
        )
    }

    /// Assert the representation invariants after every transition in debug
    /// builds. The release path pays no validation cost.
    fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            let mut placed = std::collections::HashSet::new();

            for id in &self.queued {
                assert!(placed.insert(*id), "notification id appears in multiple placements");
                assert!(
                    matches!(self.notifications.get(id).map(|n| n.placement), Some(NotificationPlacement::Queued)),
                    "queued id must resolve to a queued notification"
                );
            }
            for id in self.visible.iter().flatten() {
                assert!(placed.insert(*id), "notification id appears in multiple placements");
                assert!(
                    matches!(self.notifications.get(id).map(|n| n.placement), Some(NotificationPlacement::Visible { .. })),
                    "visible slot must resolve to a visible notification"
                );
            }
            for id in &self.history {
                assert!(placed.insert(*id), "notification id appears in multiple placements");
                assert!(
                    matches!(self.notifications.get(id).map(|n| n.placement), Some(NotificationPlacement::History)),
                    "history id must resolve to an archived notification"
                );
            }

            assert_eq!(
                placed.len(),
                self.notifications.len(),
                "every canonical notification must occupy exactly one placement"
            );
            for (key, id) in &self.dedup_index {
                assert_eq!(
                    self.notifications.get(id).and_then(|notification| notification.dedup_key.as_deref()),
                    Some(key.as_str()),
                    "dedup index must resolve to the notification that owns its key"
                );
            }
        }
    }
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

    /// Set the optional notification action.
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.action = Some(action);
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

    /// The store-assigned instant at which this entry became visible.
    pub fn visible_since(&self) -> Option<Instant> {
        match self.placement {
            NotificationPlacement::Visible { visible_since, .. } => Some(visible_since),
            _ => None,
        }
    }

    /// The computed expiry instant, if any (only `Visible` with a deadline).
    pub fn expires_at(&self) -> Option<Instant> {
        match self.placement {
            NotificationPlacement::Visible { expires_at, .. } => expires_at,
            _ => None,
        }
    }
}

/// Project a visible notification into the grid-ui presentation contract.
///
/// The host supplies opaque hint targets after registering the corresponding
/// `Intent`/dismiss operation in its `HintTargetRegistry`. Only presentation
/// data crosses the seam: id, title/body, mapped severity, action label,
/// dismissibility, and generic targets. The `Intent`, queue, dedup policy, and
/// timer state stay in the app domain. `None` means the notification is not
/// currently visible (`Queued` or `History`).
///
/// Same-id updates naturally produce a new `ToastSpec`; `ToastStack` reconciles
/// that payload in place according to its P058 contract.
pub fn project_visible_toast(
    notification: &AppNotification,
    action_target: Option<HintTargetId>,
    dismiss_target: Option<HintTargetId>,
) -> Option<ToastSpec> {
    if !notification.is_visible() {
        return None;
    }

    let mut spec = ToastSpec::new(notification.id.as_u64(), notification.title.clone())
        .severity(notification.severity.as_toast_severity())
        .dismissible(notification.lifecycle.is_dismissible());

    if let Some(target) = dismiss_target {
        spec = spec.dismiss_target(target);
    }
    if let Some(body) = &notification.body {
        spec = spec.body(body.clone());
    }
    if let Some(action) = &notification.action {
        spec = spec.action(action.label.clone());
        if let Some(target) = action_target {
            spec = spec.action_target(target);
        }
    }

    Some(spec)
}

/// Producer-facing builder for an [`AppNotification`].
///
/// The only required input is a title. Defaults are app-owned: source `App`,
/// severity `Info`, and the default dismissible five-second lifecycle. The
/// draft deliberately does **not** allocate a [`NotificationId`], read
/// `Instant::now()`, choose a delivery channel, or project to `ToastSpec`.
/// The router/store supplies the id and creation instant at acceptance time;
/// this keeps producer code deterministic and prevents a draft from claiming
/// visibility before the store promotes it.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationDraft {
    title: String,
    body: Option<String>,
    source: NotificationSource,
    severity: NotificationSeverity,
    lifecycle: NotificationLifecycle,
    dedup_key: Option<String>,
    action: Option<NotificationAction>,
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
            dedup_key: None,
            action: None,
        }
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
        self
    }

    /// Set only dismissibility while retaining the configured lifetime.
    pub fn dismissible(mut self, dismissible: bool) -> Self {
        self.lifecycle = self.lifecycle.dismissible(dismissible);
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

    /// Set an optional name-keyed notification action.
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Materialize the draft into a queued notification.
    ///
    /// The caller supplies both the monotonic id and creation instant; this
    /// method never calls `Instant::now()` and never marks the result visible.
    pub fn build(self, id: NotificationId, created_at: Instant) -> AppNotification {
        let mut notification = AppNotification::new(
            id,
            self.source,
            self.severity,
            self.title,
            created_at,
        )
        .lifecycle(self.lifecycle);
        notification.body = self.body;
        notification.dedup_key = self.dedup_key;
        notification.action = self.action;
        notification
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_store_starts_empty_with_four_stable_slots() {
        let store = NotificationStore::new(100);

        assert!(store.is_empty());
        assert_eq!(store.next_id, Some(1));
        assert_eq!(store.history_limit(), 100);
        assert!(store.queued.is_empty());
        assert_eq!(store.visible, [None; MAX_VISIBLE_NOTIFICATIONS]);
        assert!(store.history.is_empty());
        assert!(store.dedup_index.is_empty());
    }

    #[test]
    fn notification_store_allows_history_to_be_disabled() {
        assert_eq!(NotificationStore::new(0).history_limit(), 0);
    }

    #[test]
    fn push_allocates_an_id_and_promotes_into_a_vacant_slot() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);

        let push = store.push(NotificationDraft::new("Connected"), now).unwrap();
        let id = push.notification_id();
        assert_eq!(id.as_u64(), 1);
        assert!(push.update().visible_projection_changed());
        assert_eq!(store.visible[0], Some(id));
        assert_eq!(store.notifications[&id].expires_at(), now.checked_add(std::time::Duration::from_secs(5)));
        assert!(store.queued.is_empty());
    }

    #[test]
    fn push_queues_after_all_stable_slots_are_occupied_without_an_expiry() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        for number in 0..MAX_VISIBLE_NOTIFICATIONS {
            store.push(NotificationDraft::new(format!("Visible {number}")), now).unwrap();
        }

        let queued = store.push(NotificationDraft::new("Queued"), now).unwrap();
        let id = queued.notification_id();
        assert!(!queued.update().visible_projection_changed());
        assert_eq!(store.queued, VecDeque::from([id]));
        assert_eq!(store.notifications[&id].placement, NotificationPlacement::Queued);
        assert!(store.notifications[&id].expires_at().is_none());
    }

    #[test]
    fn push_reuses_known_dedup_id_without_creating_a_duplicate() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        let first = store.push(NotificationDraft::new("Connecting").dedup_key("network"), now).unwrap();
        let duplicate = store.push(NotificationDraft::new("Still connecting").dedup_key("network"), now).unwrap();

        assert_eq!(duplicate.notification_id(), first.notification_id());
        assert!(!duplicate.update().visible_projection_changed());
        assert_eq!(store.notifications.len(), 1);
    }

    #[test]
    fn promoting_a_pending_entry_fills_only_the_released_slot() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        let mut visible_ids = Vec::new();
        for number in 0..MAX_VISIBLE_NOTIFICATIONS {
            visible_ids.push(
                store.push(NotificationDraft::new(format!("Visible {number}")), now)
                    .unwrap()
                    .notification_id(),
            );
        }
        let queued_id = store.push(NotificationDraft::new("Queued"), now).unwrap().notification_id();

        let released_id = store.visible[1].take().expect("second slot is visible");
        store.notifications.get_mut(&released_id).unwrap().placement = NotificationPlacement::History;
        store.history.push_back(released_id);

        let (_, update) = store.transition_at(now, |store, now| ((), store.promote_pending(now)));
        assert!(update.visible_projection_changed());
        assert_eq!(store.visible[0], Some(visible_ids[0]));
        assert_eq!(store.visible[1], Some(queued_id));
        assert_eq!(store.visible[2], Some(visible_ids[2]));
        assert_eq!(store.visible[3], Some(visible_ids[3]));
        assert!(store.queued.is_empty());
    }

    #[test]
    fn expire_due_archives_due_visible_entries_and_promotes_pending() {
        let now = Instant::now();
        let later = now + std::time::Duration::from_secs(1);
        let mut store = NotificationStore::new(100);
        let mut visible_ids = Vec::new();
        for number in 0..MAX_VISIBLE_NOTIFICATIONS {
            visible_ids.push(
                store.push(
                    NotificationDraft::new(format!("Visible {number}"))
                        .lifecycle(NotificationLifecycle::auto(std::time::Duration::from_secs(1))),
                    now,
                )
                .unwrap()
                .notification_id(),
            );
        }
        let queued_id = store.push(NotificationDraft::new("Queued"), now).unwrap().notification_id();

        let update = store.expire_due(later);
        assert!(update.visible_projection_changed());
        assert_eq!(store.visible[0], Some(queued_id));
        assert!(visible_ids.iter().all(|id| store.notifications[id].is_history()));
        assert_eq!(store.history.len(), MAX_VISIBLE_NOTIFICATIONS);
        assert_eq!(store.notifications[&queued_id].visible_since(), Some(later));
    }

    #[test]
    fn sticky_notifications_do_not_expire() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        let id = store.push(
            NotificationDraft::new("Sticky").lifecycle(NotificationLifecycle::sticky()),
            now,
        ).unwrap().notification_id();

        assert!(!store.expire_due(now + std::time::Duration::from_secs(60)).visible_projection_changed());
        assert_eq!(store.visible[0], Some(id));
    }

    #[test]
    fn push_explicitly_reports_id_counter_exhaustion() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        store.next_id = Some(u64::MAX);

        let last = store.push(NotificationDraft::new("Last"), now).unwrap();
        assert_eq!(last.notification_id().as_u64(), u64::MAX);
        assert_eq!(store.push(NotificationDraft::new("Overflow"), now), Err(NotificationStoreError::IdExhausted));
    }

    #[test]
    fn store_transition_receives_caller_time_and_reports_projection_change() {
        let now = Instant::now();
        let mut store = NotificationStore::new(100);
        let (observed_now, unchanged) = store.transition_at(now, |_, observed_now| (observed_now, false));
        assert_eq!(observed_now, now);
        assert!(!unchanged.visible_projection_changed());

        let (_, changed) = store.transition_at(now, |_, _| ((), true));
        assert!(changed.visible_projection_changed());
    }

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

    // -- severity adapter (T190) ------------------------------------------

    #[test]
    fn notification_severity_maps_totally_to_toast_severity() {
        assert_eq!(NotificationSeverity::Info.as_toast_severity(), ToastSeverity::Info);
        assert_eq!(NotificationSeverity::Success.as_toast_severity(), ToastSeverity::Success);
        assert_eq!(NotificationSeverity::Warning.as_toast_severity(), ToastSeverity::Warning);
        assert_eq!(NotificationSeverity::Error.as_toast_severity(), ToastSeverity::Danger);
    }

    #[test]
    fn notification_severity_from_matches_adapter() {
        let severities = [
            NotificationSeverity::Info,
            NotificationSeverity::Success,
            NotificationSeverity::Warning,
            NotificationSeverity::Error,
        ];
        for severity in severities {
            assert_eq!(ToastSeverity::from(severity), severity.as_toast_severity());
        }
    }

    // -- notification actions (T189) --------------------------------------

    #[test]
    fn notification_action_defaults_to_non_dismissive_retry() {
        let action = NotificationAction::new("Retry", Intent::new("build.retry"));
        assert_eq!(action.label, "Retry");
        assert_eq!(action.intent.action, "build.retry");
        assert!(!action.dismiss_after);
    }

    #[test]
    fn notification_action_preserves_intent_arguments() {
        let intent = Intent::new("deploy.retry").arg("attempt", heca_view::PropValue::Int(2));
        let action = NotificationAction::new("Retry", intent).dismiss_after(true);
        assert!(action.dismiss_after);
        assert_eq!(action.intent.args.get("attempt"), Some(&heca_view::PropValue::Int(2)));
    }

    #[test]
    fn notification_action_serializes_as_data_without_callbacks() {
        let action = NotificationAction::dismissing("Open", Intent::new("open_log"));
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("open_log"));
        assert!(json.contains("dismiss_after"));
        let back: NotificationAction = serde_json::from_str(&json).unwrap();
        assert_eq!(back, action);
    }

    // -- ToastSpec projection (T193) --------------------------------------

    #[test]
    fn queued_and_history_notifications_do_not_project() {
        let queued = NotificationDraft::new("Queued")
            .build(NotificationId::from_raw(20), Instant::now());
        assert!(project_visible_toast(&queued, None, None).is_none());

        let mut history = queued;
        history.placement = NotificationPlacement::History;
        assert!(project_visible_toast(&history, None, None).is_none());
    }

    #[test]
    fn visible_notification_projects_presentation_data_and_opaque_targets() {
        let mut notification = NotificationDraft::new("Build failed")
            .source(NotificationSource::app("test"))
            .severity(NotificationSeverity::Error)
            .body("compiler exited with code 1")
            .action(NotificationAction::new("Retry", Intent::new("build.retry")))
            .build(NotificationId::from_raw(21), Instant::now());
        notification.placement = NotificationPlacement::Visible { visible_since: Instant::now(), expires_at: None };

        let spec = project_visible_toast(
            &notification,
            Some(HintTargetId::new(7)),
            Some(HintTargetId::new(8)),
        )
        .expect("visible notification projects");
        assert_eq!(spec.id, 21);
        assert_eq!(spec.title, "Build failed");
        assert_eq!(spec.body.as_deref(), Some("compiler exited with code 1"));
        assert_eq!(spec.severity, ToastSeverity::Danger);
        assert_eq!(spec.action.as_deref(), Some("Retry"));
        assert_eq!(spec.action_target, Some(HintTargetId::new(7)));
        assert_eq!(spec.dismiss_target, Some(HintTargetId::new(8)));
        // The Intent itself does not cross into ToastSpec.
    }

    #[test]
    fn same_id_payload_update_produces_a_different_spec() {
        let id = NotificationId::from_raw(22);
        let now = Instant::now();
        let mut first = NotificationDraft::new("Connecting")
            .build(id, now);
        first.placement = NotificationPlacement::Visible { visible_since: now, expires_at: None };
        let mut second = NotificationDraft::new("Connected")
            .severity(NotificationSeverity::Success)
            .build(id, now);
        second.placement = NotificationPlacement::Visible { visible_since: now, expires_at: None };

        let first_spec = project_visible_toast(&first, None, None).unwrap();
        let second_spec = project_visible_toast(&second, None, None).unwrap();
        assert_eq!(first_spec.id, second_spec.id);
        assert_ne!(first_spec, second_spec);
        assert_eq!(second_spec.severity, ToastSeverity::Success);
    }

    // -- draft builder (T192) ---------------------------------------------

    #[test]
    fn notification_draft_has_domain_defaults() {
        let draft = NotificationDraft::new("Hello");
        let now = Instant::now();
        let notification = draft.build(NotificationId::from_raw(10), now);
        assert_eq!(notification.title, "Hello");
        assert_eq!(notification.source.kind(), NotificationSourceKind::App);
        assert_eq!(notification.severity, NotificationSeverity::Info);
        assert_eq!(notification.lifecycle, NotificationLifecycle::default());
        assert_eq!(notification.placement, NotificationPlacement::Queued);
        assert_eq!(notification.created_at, now);
    }

    #[test]
    fn notification_draft_sets_all_optional_fields() {
        let now = Instant::now();
        let notification = NotificationDraft::new("Failed")
            .source(NotificationSource::new(NotificationSourceKind::Plugin, "git"))
            .severity(NotificationSeverity::Error)
            .lifecycle(NotificationLifecycle::sticky())
            .dismissible(false)
            .body("remote rejected")
            .dedup_key("git-push")
            .action(NotificationAction::new("Retry", Intent::new("git.retry")))
            .build(NotificationId::from_raw(11), now);
        assert_eq!(notification.source.id(), Some("git"));
        assert_eq!(notification.severity, NotificationSeverity::Error);
        assert!(!notification.lifecycle.is_dismissible());
        assert_eq!(notification.body.as_deref(), Some("remote rejected"));
        assert_eq!(notification.dedup_key.as_deref(), Some("git-push"));
        assert_eq!(notification.action.unwrap().intent.action, "git.retry");
    }

    #[test]
    fn notification_draft_build_does_not_make_notification_visible() {
        let n = NotificationDraft::new("Queued").build(NotificationId::from_raw(12), Instant::now());
        assert_eq!(n.placement, NotificationPlacement::Queued);
        assert!(!n.is_visible());
        assert!(n.expires_at().is_none());
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
        .action(NotificationAction::dismissing("Push", Intent::new("git.push")));
        assert_eq!(n.dedup_key.as_deref(), Some("git-push"));
        assert_eq!(n.body.as_deref(), Some("rejected by remote"));
        assert!(n.lifecycle.is_dismissible());
        assert_eq!(n.action.as_ref().unwrap().intent.action, "git.push");
        assert_eq!(n.action.as_ref().unwrap().label, "Push");
        assert!(n.action.as_ref().unwrap().dismiss_after);
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
        n.placement = NotificationPlacement::Visible { visible_since: deadline - std::time::Duration::from_secs(5), expires_at: Some(deadline) };
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