use super::*;
use std::{collections::{HashMap, VecDeque}, time::Instant};
use heca_view::Intent;

mod transitions;

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

/// Outcome of a user-requested notification dismissal.
///
/// The store treats an unknown id and a non-dismissible lifecycle as stable
/// no-ops, allowing the future action handler to report either case without a
/// panic or a second lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationDismissResult {
    /// The entry moved to runtime history.
    Dismissed(NotificationStoreUpdate),
    /// No canonical notification has the requested id.
    Unknown,
    /// The lifecycle deliberately prevents user dismissal.
    NotDismissible,
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
/// placements directly; the transition helpers perform the matching
/// storage/index updates together.
///
/// # Invariants
///
/// - An id appears in exactly one placement: `queued`, one entry of `visible`,
///   or `history`.
/// - `visible` has exactly `max_visible` slots, and the occupied
///   ones are packed at the front: a card leaving closes its gap by sliding the ones after it up,
///   and a promoted card joins at the end. The column therefore reads top to bottom in the order
///   the notifications were raised, with no blank between them.
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
    /// Presentation slots, packed from the front — see [`close_gaps`](Self::close_gaps).
    /// Presentation slots — as many as `max_visible`, fixed for the store's life. Length is the
    /// capacity, and the occupied ones are packed at the front: `None`s are the free tail, never a
    /// hole in the middle. See [`close_gaps`](Self::close_gaps).
    visible: Vec<Option<NotificationId>>,
    /// FIFO archival order; the oldest entry is evicted first.
    history: VecDeque<NotificationId>,
    /// Maximum retained history entries; zero disables retention.
    history_limit: usize,
}

impl NotificationStore {
    /// Create an empty store with a bounded runtime history and the default number of slots.
    ///
    /// `history_limit` is a runtime retention policy, not persisted state.
    pub fn new(history_limit: usize) -> Self {
        Self::with_capacity(history_limit, heca_config::settings::NotificationSystemConfig::default().max_visible)
    }

    /// The same, with an explicit number of simultaneous cards
    /// (`[settings.notification_system] max_visible`).
    ///
    /// **Clamped to at least one.** A zero-slot store would queue every notification for ever and
    /// show none, which is what `mode = "none"` is for — a capacity of nothing is a config typo,
    /// not a way to silence the app.
    pub fn with_capacity(history_limit: usize, max_visible: usize) -> Self {
        Self {
            next_id: Some(1),
            notifications: HashMap::new(),
            dedup_index: HashMap::new(),
            queued: VecDeque::new(),
            visible: vec![None; max_visible.max(1)],
            history: VecDeque::new(),
            history_limit,
        }
    }

    /// How many cards this store shows at once.
    pub fn max_visible(&self) -> usize {
        self.visible.len()
    }

    /// The configured maximum number of retained archived notifications.
    pub fn history_limit(&self) -> usize {
        self.history_limit
    }

    /// Change the bounded archived-history retention policy.
    pub fn set_history_limit(&mut self, history_limit: usize) {
        self.history_limit = history_limit;
        while self.history.len() > self.history_limit {
            let notification_id = self
                .history
                .pop_front()
                .expect("history length was above its limit");
            self.remove_canonical(notification_id);
        }
    }

    /// **Change how many cards are on screen at once** — the `prefix+Shift+r` path for
    /// `[settings.notification_system] max_visible`.
    ///
    /// Clamped to at least one, like [`with_capacity`](Self::with_capacity): a zero would queue
    /// everything and show nothing, which is what `mode = "none"` is for.
    ///
    /// **Shrinking hands the surplus back to the queue, at the front**, in the order they were
    /// raised — they were on screen before anything still waiting, so they keep their turn rather
    /// than going to the back of it. Nothing is archived: a card the user never saw dismissed must
    /// not be silently swallowed by a config edit. Growing simply opens slots; the next
    /// [`promote_pending`](Self::promote_pending) fills them.
    ///
    /// Returns whether the visible set changed, so the caller can refresh its projection.
    pub fn set_max_visible(&mut self, max_visible: usize, now: Instant) -> bool {
        let want = max_visible.max(1);
        if want == self.visible.len() {
            return false;
        }
        let (_, update) = self.transition_at(now, |store, now| {
            if want < store.visible.len() {
                // Surplus goes back to the head of the queue, newest-last, so the FIFO still reads
                // in the order the notifications were raised.
                let surplus: Vec<NotificationId> = store
                    .visible
                    .split_off(want)
                    .into_iter()
                    .flatten()
                    .collect();
                for id in surplus.into_iter().rev() {
                    if let Some(n) = store.notifications.get_mut(&id) {
                        n.placement = NotificationPlacement::Queued;
                    }
                    store.queued.push_front(id);
                }
            } else {
                store.visible.resize(want, None);
            }
            ((), store.promote_pending(now))
        });
        update.visible_projection_changed()
    }

    /// Iterate retained archived notifications from oldest to newest.
    ///
    /// The store keeps ownership of all lifecycle buckets; callers receive no
    /// mutable access and cannot reorder or reinsert history entries.
    pub fn history(&self) -> impl Iterator<Item = &AppNotification> {
        self.history
            .iter()
            .filter_map(|id| self.notifications.get(id))
    }

    /// Iterate visible notifications in stable ToastStack slot order.
    pub fn visible(&self) -> impl Iterator<Item = &AppNotification> {
        self.visible
            .iter()
            .filter_map(|id| id.and_then(|id| self.notifications.get(&id)))
    }

    /// Return the first visible card eligible for the default dismissal action.
    ///
    /// The stable visible-slot order chooses the target. Sticky entries are
    /// deliberately excluded, even when manually dismissible: they require
    /// explicit acknowledgement through their own close affordance.
    pub fn first_auto_dismissible_visible_id(&self) -> Option<NotificationId> {
        self.visible()
            .find(|notification| {
                notification.lifecycle.is_dismissible()
                    && matches!(
                        notification.lifecycle.lifetime(),
                        NotificationLifetime::Auto(_)
                    )
            })
            .map(|notification| notification.id)
    }

    /// Return the earliest auto-dismiss deadline among visible slots.
    ///
    /// Pending and sticky entries have no active timer and are excluded.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.visible().filter_map(AppNotification::expires_at).min()
    }

    /// **Give every visible card `by` more time.**
    ///
    /// This is what "the pointer is resting on the stack" costs: a card the user is reading — or
    /// reaching across to click — must not retire under the cursor, and when they leave it should
    /// resume with the time it actually had left rather than the time it started with.
    ///
    /// Shifting the deadlines is what makes that true. The alternative, simply not sweeping while
    /// hovered, looks identical until the pointer leaves: every card whose deadline passed during
    /// the hover is then instantly due, and the stack empties in the frame after the user moves
    /// away — the same disappearing-under-the-click this exists to stop, moved a moment later.
    ///
    /// Sticky entries have no deadline and are untouched. Returns whether anything moved, so a
    /// caller can skip re-arming a timer for a stack that is all sticky.
    pub fn extend_visible_deadlines(&mut self, by: std::time::Duration) -> bool {
        if by.is_zero() {
            return false;
        }
        let ids: Vec<NotificationId> = self.visible.iter().flatten().copied().collect();
        let mut moved = false;
        for id in ids {
            let Some(notification) = self.notifications.get_mut(&id) else {
                continue;
            };
            if let NotificationPlacement::Visible {
                expires_at: Some(deadline),
                ..
            } = &mut notification.placement
                && let Some(later) = deadline.checked_add(by)
            {
                *deadline = later;
                moved = true;
            }
        }
        moved
    }

    /// Look up one action of a currently visible notification, by the `key`
    /// `ToastStack::on_action(id, key)` reports back — matched against that
    /// action's `intent.action` name, which doubles as its identity.
    ///
    /// Queued and archived entries deliberately have no activatable target.
    pub fn action_for_visible(
        &self,
        notification_id: NotificationId,
        key: &str,
    ) -> Option<&NotificationAction> {
        self.visible()
            .find(|notification| notification.id == notification_id)
            .and_then(|notification| {
                notification
                    .actions
                    .iter()
                    .find(|action| action.intent.action == key)
            })
    }

    /// Clone the serializable action intent immediately before dispatch.
    pub fn action_intent_for_visible(
        &self,
        notification_id: NotificationId,
        key: &str,
    ) -> Option<Intent> {
        self.action_for_visible(notification_id, key)
            .map(|action| action.intent.clone())
    }

    /// Resolve an explicit deduplication key to its canonical ID.
    pub fn notification_id_for_dedup_key(&self, key: &str) -> Option<NotificationId> {
        self.dedup_index.get(key).copied()
    }

    /// Project the stable visible slots to serializable [`NotificationView`]s, in visible-slot
    /// order — what the server sends a window, which builds its own `ToastSpec`s from them.
    ///
    /// The store remains the only owner of queueing and lifecycle state; this is a pure read.
    pub fn visible_views(&self) -> Vec<NotificationView> {
        self.visible().filter_map(notification_view).collect()
    }

    /// Whether the store holds no queued, visible, or retained notification.
    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }

    /// Dismiss an entry through the user-facing lifecycle path.
    ///
    /// A visible entry leaves its slot and the cards after it slide up to close the gap; a pending
    /// entry is removed from the FIFO. Both are archived, then a pending entry joins at the end.
    /// Dedup keys remain indexed while in history so the same ID can be reactivated.
    pub fn dismiss(
        &mut self,
        notification_id: NotificationId,
        now: Instant,
    ) -> NotificationDismissResult {
        self.archive(notification_id, now, true)
    }

    /// Resolve an entry from a producer/runtime path, regardless of its user
    /// dismissibility. This is intentionally crate-private: only Heca-owned
    /// runtime integrations may clear a persistent status notification.
    pub(crate) fn resolve(
        &mut self,
        notification_id: NotificationId,
        now: Instant,
    ) -> NotificationDismissResult {
        self.archive(notification_id, now, false)
    }

}

#[cfg(test)]
mod tests_1;

#[cfg(test)]
mod tests_2;
