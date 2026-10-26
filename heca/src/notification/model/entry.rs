use super::*;
use serde::{Deserialize, Serialize};
use std::{sync::atomic::{AtomicU64, Ordering}, time::Instant};

/// A session-monotonic, never-reused notification identity.
///
/// Allocated by the store's own counter, so ids strictly increase and are never recycled within a session —
/// even after a notification is dismissed and archived. This is what the store reconciles on and what
/// `on_dismiss`/`on_action` report back.
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

    /// Reconstruct an id supplied by a transport boundary (RPC, a widget event,
    /// or a serialized Intent). Allocation remains store-owned; this
    /// constructor never advances the counter.
    pub const fn from_raw(v: u64) -> Self {
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
///   lazily by the store at promotion time.
/// - `History` — dismissed/expired/archived; kept up to
///   `notification_history_limit` then dropped (FIFO).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationArchiveCause {
    /// Removed through the user-facing dismiss path.
    Dismissed,
    /// Auto-dismiss deadline elapsed while visible.
    Expired,
    /// Cleared by a Heca-owned producer/runtime integration.
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationPlacement {
    Queued,
    Visible {
        /// The instant at which the store promoted this entry into a slot.
        visible_since: Instant,
        /// Auto-dismiss deadline; absent for sticky notifications.
        expires_at: Option<Instant>,
    },
    History {
        cause: NotificationArchiveCause,
        archived_at: Instant,
    },
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
/// Each entry of [`actions`](Self::actions) carries a [`heca_view::Intent`] (a
/// routing name plus args), never a closure. A card may show **several** actions (the widget supports it);
/// `on_action(id, key)` resolves back to one of them by matching `key` against
/// that action's `intent.action` name, which doubles as its identity — no
/// separate key field, so two actions on one notification must name distinct
/// intents.
///
/// `Clone` is cheap-ish (title/body/actions are `Arc`-shared on clone in a
/// follow-up; for now they are owned `String`s — clone only for projection, not
/// on every frame). Targeted projection to [`ToastSpec`](heca_grid_ui::widgets::ToastSpec) reads the
/// fields it needs without cloning the whole store.
#[derive(Debug, Clone, PartialEq)]
pub struct AppNotification {
    /// Session-monotonic, never-reused identity.
    pub id: NotificationId,
    /// Optional dedup key. When set, the store treats two notifications with the
    /// same key as the same logical event (updates in place rather than
    /// stacking). Constructed from the source + a caller-supplied suffix.
    pub dedup_key: Option<String>,
    /// Descriptive provenance (log/history/dedup).
    pub source: NotificationSource,
    pub severity: NotificationSeverity,
    pub title: String,
    pub body: Option<String>,
    /// Lifecycle policy (lifetime + dismissibility).
    pub lifecycle: NotificationLifecycle,
    /// Inline actions, each dispatched as a name-keyed [`Intent`](heca_view::Intent) with a label
    /// and post-dispatch dismiss policy. Empty means no action affordance.
    pub actions: Vec<NotificationAction>,
    /// When the notification was created. Injected by the caller (not `now()`)
    /// so domain tests are deterministic — instants stay out of the builder
    /// when tests need them.
    pub created_at: Instant,
    /// Current placement bucket. Mutated by the store; not by the builder.
    pub(in crate::notification) placement: NotificationPlacement,
}

impl AppNotification {
    /// Build a notification with the given identity + title. Prefer the
    /// [`NotificationDraft`] builder for ergonomics; this is the canonical
    /// constructor the builder reduces to.
    ///
    /// `created_at` is taken explicitly so tests stay deterministic.
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
            actions: Vec::new(),
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

    /// Append an inline notification action. Callable more than once — a card
    /// may show several actions; each must name a distinct
    /// `intent.action`, which `on_action(id, key)` uses to find it again.
    pub fn action(mut self, action: NotificationAction) -> Self {
        self.actions.push(action);
        self
    }

    /// Whether the notification is currently visible in the toast stack.
    pub fn is_visible(&self) -> bool {
        matches!(self.placement, NotificationPlacement::Visible { .. })
    }

    /// Whether the notification has been archived to history.
    pub fn is_history(&self) -> bool {
        matches!(self.placement, NotificationPlacement::History { .. })
    }

    /// The store-owned archive cause and instant, if this entry is retained.
    pub fn archived_at(&self) -> Option<(NotificationArchiveCause, Instant)> {
        match self.placement {
            NotificationPlacement::History { cause, archived_at } => Some((cause, archived_at)),
            _ => None,
        }
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
