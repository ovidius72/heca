use super::*;

impl NotificationStore {
    pub(super) fn archive(
        &mut self,
        notification_id: NotificationId,
        now: Instant,
        require_dismissible: bool,
    ) -> NotificationDismissResult {
        let Some(notification) = self.notifications.get(&notification_id) else {
            return NotificationDismissResult::Unknown;
        };
        if require_dismissible && !notification.lifecycle.is_dismissible() {
            return NotificationDismissResult::NotDismissible;
        }
        if notification.is_history() {
            return NotificationDismissResult::Unknown;
        }

        let cause = if require_dismissible {
            NotificationArchiveCause::Dismissed
        } else {
            NotificationArchiveCause::Resolved
        };
        let (_, update) = self.transition_at(now, |store, now| {
            store.queued.retain(|id| *id != notification_id);
            for slot in &mut store.visible {
                if *slot == Some(notification_id) {
                    *slot = None;
                }
            }
            store
                .notifications
                .get_mut(&notification_id)
                .expect("known notification must have canonical storage")
                .placement = NotificationPlacement::History {
                cause,
                archived_at: now,
            };
            store.retain_history(notification_id);
            ((), store.promote_pending(now))
        });
        NotificationDismissResult::Dismissed(update)
    }

    /// Archive every visible auto-dismiss notification due at `now`.
    ///
    /// A due card leaves, the cards after it slide up to close the gap, and pending entries join
    /// at the end FIFO. Sticky notifications have no deadline and are never selected.
    pub fn expire_due(&mut self, now: Instant) -> NotificationStoreUpdate {
        let (_, update) = self.transition_at(now, |store, now| {
            let mut due_ids = Vec::new();
            for slot in &mut store.visible {
                let Some(notification_id) = *slot else {
                    continue;
                };
                let due = store.notifications[&notification_id]
                    .expires_at()
                    .is_some_and(|expires_at| expires_at <= now);
                if due {
                    *slot = None;
                    due_ids.push(notification_id);
                }
            }
            for notification_id in due_ids {
                store
                    .notifications
                    .get_mut(&notification_id)
                    .expect("visible notification must have canonical storage")
                    .placement = NotificationPlacement::History {
                    cause: NotificationArchiveCause::Expired,
                    archived_at: now,
                };
                store.retain_history(notification_id);
            }
            ((), store.promote_pending(now))
        });
        update
    }

    /// Accept a producer draft into the canonical store.
    ///
    /// A new id is allocated only when no known deduplication key resolves to
    /// an existing notification. The full update policy for that existing
    /// entry is planned; until then this method preserves its state and returns its canonical id without
    /// creating a duplicate.
    ///
    /// New entries first join the pending FIFO and are then promoted into every
    /// vacant stable slot, oldest first. Promotion starts an auto-dismiss timer
    /// from `now`; queued entries have no expiry.
    ///
    /// **Not public API** (`pub(crate)`): the deterministic `(draft, now)`
    /// primitive lifecycle tests drive, never a door a producer or plugin author reaches.
    /// The only public raise path is [`Notification::send`], which funnels through
    /// [`raise`].
    pub(crate) fn push(
        &mut self,
        draft: NotificationDraft,
        now: Instant,
    ) -> Result<NotificationStorePush, NotificationStoreError> {
        if let Some(key) = draft.dedup_key.as_deref()
            && let Some(&notification_id) = self.dedup_index.get(key)
        {
            return Ok(NotificationStorePush {
                notification_id,
                update: self.update_duplicate(notification_id, draft, now),
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

    /// Retain an archived entry, evicting the oldest records and their dedup
    /// keys once the configured runtime history bound is exceeded.
    pub(super) fn retain_history(&mut self, notification_id: NotificationId) {
        if self.history_limit == 0 {
            self.remove_canonical(notification_id);
            return;
        }
        self.history.push_back(notification_id);
        while self.history.len() > self.history_limit {
            let evicted_id = self
                .history
                .pop_front()
                .expect("history length was above its limit");
            self.remove_canonical(evicted_id);
        }
    }

    pub(super) fn remove_canonical(&mut self, notification_id: NotificationId) {
        let Some(notification) = self.notifications.remove(&notification_id) else {
            return;
        };
        if let Some(key) = notification.dedup_key
            && self.dedup_index.get(&key) == Some(&notification_id)
        {
            self.dedup_index.remove(&key);
        }
    }

    /// Replace a known deduplicated entry while retaining its canonical id.
    pub(super) fn update_duplicate(
        &mut self,
        notification_id: NotificationId,
        draft: NotificationDraft,
        now: Instant,
    ) -> NotificationStoreUpdate {
        let (_, update) = self.transition_at(now, |store, now| {
            let previous = store.notifications[&notification_id].placement;
            let mut replacement = draft.build(notification_id, now);
            let projection_changed = match previous {
                NotificationPlacement::Visible { .. } => {
                    replacement.placement = NotificationPlacement::Visible {
                        visible_since: now,
                        expires_at: match replacement.lifecycle.lifetime() {
                            NotificationLifetime::Auto(duration) => now.checked_add(duration),
                            NotificationLifetime::Sticky => None,
                        },
                    };
                    true
                }
                NotificationPlacement::Queued => {
                    replacement.placement = NotificationPlacement::Queued;
                    store.queued.retain(|id| *id != notification_id);
                    store.queued.push_front(notification_id);
                    false
                }
                NotificationPlacement::History { .. } => {
                    store.history.retain(|id| *id != notification_id);
                    replacement.placement = NotificationPlacement::Queued;
                    store.queued.push_front(notification_id);
                    false
                }
            };
            store.notifications.insert(notification_id, replacement);
            let promoted = store.promote_pending(now);
            ((), projection_changed || promoted)
        });
        update
    }

    /// Promote pending notifications into vacant slots without reordering an
    /// existing visible card. Returns whether the visible projection changed.
    pub(super) fn promote_pending(&mut self, now: Instant) -> bool {
        let mut visible_changed = self.close_gaps();
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

    /// **Slide the remaining cards up so the stack has no hole in it**, and report whether any
    /// moved.
    ///
    /// A card that goes leaves a gap wherever it was. The stack closes it: the cards below move up
    /// one, and whatever is promoted joins at the **end**, so the column always reads top to bottom
    /// in the order the notifications were raised, with no blank between them.
    ///
    /// The alternative — leave the hole and drop the next queued card straight into it — keeps
    /// every card perfectly still, which is why it was built that way. It also inserts a brand new
    /// card in the middle of ones that have been read, and shows a blank slot in the meantime.
    pub(super) fn close_gaps(&mut self) -> bool {
        let before = self.visible.clone();
        let mut occupied: Vec<Option<NotificationId>> = self
            .visible
            .iter()
            .filter(|slot| slot.is_some())
            .copied()
            .collect();
        occupied.resize(self.visible.len(), None);
        self.visible = occupied;
        self.visible != before
    }

    /// Execute one store-owned transition at a caller-supplied time.
    ///
    /// All future lifecycle mutations use this funnel rather than exposing the
    /// queue, visible slots, history, or dedup index to consumers. The closure
    /// reports a same-slot payload change; structural slot changes are detected
    /// automatically. The returned [`NotificationStoreUpdate`] is the only
    /// refresh signal the host needs.
    pub(super) fn transition_at<R>(
        &mut self,
        now: Instant,
        transition: impl FnOnce(&mut Self, Instant) -> (R, bool),
    ) -> (R, NotificationStoreUpdate) {
        let visible_before = self.visible.clone();
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
    pub(super) fn assert_invariants(&self) {
        #[cfg(debug_assertions)]
        {
            let mut placed = std::collections::HashSet::new();

            for id in &self.queued {
                assert!(
                    placed.insert(*id),
                    "notification id appears in multiple placements"
                );
                assert!(
                    matches!(
                        self.notifications.get(id).map(|n| n.placement),
                        Some(NotificationPlacement::Queued)
                    ),
                    "queued id must resolve to a queued notification"
                );
            }
            for id in self.visible.iter().flatten() {
                assert!(
                    placed.insert(*id),
                    "notification id appears in multiple placements"
                );
                assert!(
                    matches!(
                        self.notifications.get(id).map(|n| n.placement),
                        Some(NotificationPlacement::Visible { .. })
                    ),
                    "visible slot must resolve to a visible notification"
                );
            }
            for id in &self.history {
                assert!(
                    placed.insert(*id),
                    "notification id appears in multiple placements"
                );
                assert!(
                    matches!(
                        self.notifications.get(id).map(|n| n.placement),
                        Some(NotificationPlacement::History { .. })
                    ),
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
                    self.notifications
                        .get(id)
                        .and_then(|notification| notification.dedup_key.as_deref()),
                    Some(key.as_str()),
                    "dedup index must resolve to the notification that owns its key"
                );
            }
        }
    }
}
