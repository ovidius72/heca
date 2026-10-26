use super::*;

#[test]
fn notification_store_starts_empty_with_four_stable_slots() {
    let store = NotificationStore::new(100);

    assert!(store.is_empty());
    assert_eq!(store.next_id, Some(1));
    assert_eq!(store.history_limit(), 100);
    assert!(store.queued.is_empty());
    assert!(store.visible.iter().all(Option::is_none));
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

    let push = store
        .push(NotificationDraft::new("Connected"), now)
        .unwrap();
    let id = push.notification_id();
    assert_eq!(id.as_u64(), 1);
    assert!(push.update().visible_projection_changed());
    assert_eq!(store.visible[0], Some(id));
    assert_eq!(
        store.notifications[&id].expires_at(),
        now.checked_add(std::time::Duration::from_millis(
            heca_config::settings::NotificationSystemConfig::default().auto_dismiss_ms,
        ))
    );
    assert!(store.queued.is_empty());
}

#[test]
fn push_queues_after_all_stable_slots_are_occupied_without_an_expiry() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let capacity = store.max_visible();
    for number in 0..capacity {
        store
            .push(NotificationDraft::new(format!("Visible {number}")), now)
            .unwrap();
    }

    let queued = store.push(NotificationDraft::new("Queued"), now).unwrap();
    let id = queued.notification_id();
    assert!(!queued.update().visible_projection_changed());
    assert_eq!(store.queued, VecDeque::from([id]));
    assert_eq!(
        store.notifications[&id].placement,
        NotificationPlacement::Queued
    );
    assert!(store.notifications[&id].expires_at().is_none());
}

#[test]
fn push_reuses_known_dedup_id_without_creating_a_duplicate() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let first = store
        .push(
            NotificationDraft::new("Connecting").dedup_key("network"),
            now,
        )
        .unwrap();
    let duplicate = store
        .push(
            NotificationDraft::new("Still connecting").dedup_key("network"),
            now,
        )
        .unwrap();

    assert_eq!(duplicate.notification_id(), first.notification_id());
    assert!(duplicate.update().visible_projection_changed());
    assert_eq!(store.notifications.len(), 1);
    assert_eq!(
        store.notifications[&first.notification_id()].title,
        "Still connecting"
    );
}

#[test]
fn promoting_a_pending_entry_fills_only_the_released_slot() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let mut visible_ids = Vec::new();
    let capacity = store.max_visible();
    for number in 0..capacity {
        visible_ids.push(
            store
                .push(NotificationDraft::new(format!("Visible {number}")), now)
                .unwrap()
                .notification_id(),
        );
    }
    let queued_id = store
        .push(NotificationDraft::new("Queued"), now)
        .unwrap()
        .notification_id();

    let released_id = store.visible[1].take().expect("second slot is visible");
    store.notifications.get_mut(&released_id).unwrap().placement =
        NotificationPlacement::History {
            cause: NotificationArchiveCause::Dismissed,
            archived_at: now,
        };
    store.history.push_back(released_id);

    let (_, update) = store.transition_at(now, |store, now| ((), store.promote_pending(now)));
    assert!(update.visible_projection_changed());
    // The hole closes and the promoted card lands at the end, so the column reads in the order
    // the notifications were raised with no blank in it.
    let mut expected: Vec<Option<NotificationId>> = visible_ids
        .iter()
        .copied()
        .filter(|id| *id != released_id)
        .map(Some)
        .collect();
    expected.push(Some(queued_id));
    assert_eq!(store.visible, expected);
    assert!(store.queued.is_empty());
}

#[test]
fn expire_due_archives_due_visible_entries_and_promotes_pending() {
    let now = Instant::now();
    let later = now + std::time::Duration::from_secs(1);
    let mut store = NotificationStore::new(100);
    let mut visible_ids = Vec::new();
    let capacity = store.max_visible();
    for number in 0..capacity {
        visible_ids.push(
            store
                .push(
                    NotificationDraft::new(format!("Visible {number}")).lifecycle(
                        NotificationLifecycle::auto(std::time::Duration::from_secs(1)),
                    ),
                    now,
                )
                .unwrap()
                .notification_id(),
        );
    }
    let queued_id = store
        .push(NotificationDraft::new("Queued"), now)
        .unwrap()
        .notification_id();

    let update = store.expire_due(later);
    assert!(update.visible_projection_changed());
    assert_eq!(store.visible[0], Some(queued_id));
    assert!(
        visible_ids
            .iter()
            .all(|id| store.notifications[id].is_history())
    );
    assert_eq!(store.history.len(), capacity);
    assert_eq!(store.notifications[&queued_id].visible_since(), Some(later));
}

#[test]
fn extending_deadlines_leaves_a_sticky_card_alone() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let id = store
        .push(
            NotificationDraft::new("Acknowledge me").lifecycle(NotificationLifecycle::sticky()),
            now,
        )
        .unwrap()
        .notification_id();
    assert!(!store.extend_visible_deadlines(std::time::Duration::from_secs(3)));
    assert!(store.notifications[&id].expires_at().is_none());
}

#[test]
fn next_expiry_uses_only_the_earliest_visible_auto_deadline() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let first = now + std::time::Duration::from_secs(2);
    let second = now + std::time::Duration::from_secs(5);
    store
        .push(
            NotificationDraft::new("Later").lifecycle(NotificationLifecycle::auto(
                std::time::Duration::from_secs(5),
            )),
            now,
        )
        .unwrap();
    store
        .push(
            NotificationDraft::new("Earlier").lifecycle(NotificationLifecycle::auto(
                std::time::Duration::from_secs(2),
            )),
            now,
        )
        .unwrap();
    store
        .push(
            NotificationDraft::new("Sticky").lifecycle(NotificationLifecycle::sticky()),
            now,
        )
        .unwrap();
    assert_eq!(store.next_expiry(), Some(first));
    assert_ne!(store.next_expiry(), Some(second));

    let full = NotificationStore::new(100);
    assert_eq!(full.next_expiry(), None);
}

#[test]
fn sticky_notifications_do_not_expire() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let id = store
        .push(
            NotificationDraft::new("Sticky").lifecycle(NotificationLifecycle::sticky()),
            now,
        )
        .unwrap()
        .notification_id();

    assert!(
        !store
            .expire_due(now + std::time::Duration::from_secs(60))
            .visible_projection_changed()
    );
    assert_eq!(store.visible[0], Some(id));
}

#[test]
fn resizing_the_stack_returns_surplus_cards_to_the_front_of_the_queue() {
    let now = Instant::now();
    let mut store = NotificationStore::with_capacity(100, 4);
    let ids: Vec<_> = (0..5)
        .map(|n| {
            store
                .push(NotificationDraft::new(format!("N{n}")), now)
                .unwrap()
                .notification_id()
        })
        .collect();
    assert_eq!(store.visible.len(), 4);
    assert_eq!(store.queued.len(), 1);

    assert!(store.set_max_visible(2, now));
    assert_eq!(store.visible, vec![Some(ids[0]), Some(ids[1])]);
    assert_eq!(
        store.queued.iter().copied().collect::<Vec<_>>(),
        vec![ids[2], ids[3], ids[4]],
        "the two that came off keep their place ahead of the one already waiting",
    );
    assert!(
        ids.iter().all(|id| !store.notifications[id].is_history()),
        "a config edit must not dismiss anything",
    );

    // And growing lets the queue back in, in order.
    assert!(store.set_max_visible(4, now));
    assert_eq!(
        store.visible,
        vec![Some(ids[0]), Some(ids[1]), Some(ids[2]), Some(ids[3])]
    );
    assert_eq!(store.queued.len(), 1);

    assert!(!store.set_max_visible(4, now), "no change, no churn");
    assert!(store.set_max_visible(0, now), "zero clamps to one");
    assert_eq!(store.visible.len(), 1);
}

#[test]
fn the_number_of_slots_is_configured_and_never_zero() {
    assert_eq!(
        NotificationStore::new(100).max_visible(),
        heca_config::settings::NotificationSystemConfig::default().max_visible
    );
    assert_eq!(NotificationStore::with_capacity(100, 2).max_visible(), 2);
    assert_eq!(
        NotificationStore::with_capacity(100, 0).max_visible(),
        1,
        "a zero-slot stack would swallow everything silently",
    );
}

#[test]
fn overflow_waits_its_turn_and_joins_at_the_end() {
    let now = Instant::now();
    let mut store = NotificationStore::with_capacity(100, 3);
    let ids: Vec<_> = (0..6)
        .map(|n| {
            store
                .push(NotificationDraft::new(format!("N{n}")), now)
                .unwrap()
                .notification_id()
        })
        .collect();

    assert_eq!(
        store.visible,
        vec![Some(ids[0]), Some(ids[1]), Some(ids[2])]
    );
    assert_eq!(store.queued.len(), 3, "the rest are waiting, not lost");

    // Close the middle card: the one after it slides up, and the next queued appends.
    store.dismiss(ids[1], now);
    assert_eq!(
        store.visible,
        vec![Some(ids[0]), Some(ids[2]), Some(ids[3])],
        "the gap closes and the newcomer goes last",
    );

    // And again, at the front this time — the whole column shifts up by one.
    store.dismiss(ids[0], now);
    assert_eq!(
        store.visible,
        vec![Some(ids[2]), Some(ids[3]), Some(ids[4])]
    );
    assert_eq!(store.queued.len(), 1);
}
