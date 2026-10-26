use super::*;
#[test]
fn hovering_hands_back_the_time_it_cost_rather_than_skipping_the_deadline() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let id = store
        .push(
            NotificationDraft::new("Read me").lifecycle(NotificationLifecycle::auto(
                std::time::Duration::from_secs(4),
            )),
            now,
        )
        .unwrap()
        .notification_id();
    assert_eq!(
        store.notifications[&id].expires_at(),
        now.checked_add(std::time::Duration::from_secs(4))
    );

    // The pointer rested for three seconds — a card read, then reached for.
    assert!(store.extend_visible_deadlines(std::time::Duration::from_secs(3)));
    assert_eq!(
        store.notifications[&id].expires_at(),
        now.checked_add(std::time::Duration::from_secs(7)),
        "it resumes with the second it had left, not with a fresh four",
    );

    // Three seconds in, it is not due: without the shift it would already have gone.
    let update = store.expire_due(now + std::time::Duration::from_secs(5));
    assert!(!update.visible_projection_changed(), "still on screen");
    // And it still goes, at the moment it now has.
    let update = store.expire_due(now + std::time::Duration::from_secs(8));
    assert!(
        update.visible_projection_changed(),
        "the hold was a pause, not a reprieve"
    );
}

#[test]
fn dismiss_visible_closes_its_gap_and_appends_the_next_queued() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let visible_ids: Vec<_> = (0..store.max_visible())
        .map(|number| {
            store
                .push(NotificationDraft::new(format!("Visible {number}")), now)
                .unwrap()
                .notification_id()
        })
        .collect();
    let queued_id = store
        .push(NotificationDraft::new("Queued"), now)
        .unwrap()
        .notification_id();

    let result = store.dismiss(visible_ids[1], now);
    assert!(
        matches!(result, NotificationDismissResult::Dismissed(update) if update.visible_projection_changed())
    );
    assert!(store.notifications[&visible_ids[1]].is_history());
    assert_eq!(store.history, VecDeque::from([visible_ids[1]]));

    // **The gap closes and the newcomer joins at the end.** Closing the second card slides the
    // rest up, so there is never a blank slot in the column, and the promoted card appears
    // where a new one belongs — after the ones already being read, not inserted among them.
    let mut expected: Vec<Option<NotificationId>> = visible_ids
        .iter()
        .copied()
        .filter(|id| *id != visible_ids[1])
        .map(Some)
        .collect();
    expected.push(Some(queued_id));
    assert_eq!(store.visible, expected);
}

#[test]
fn dismiss_pending_archives_without_changing_visible_slots() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let visible_ids: Vec<_> = (0..store.max_visible())
        .map(|number| {
            store
                .push(NotificationDraft::new(format!("Visible {number}")), now)
                .unwrap()
                .notification_id()
        })
        .collect();
    let queued_id = store
        .push(NotificationDraft::new("Queued"), now)
        .unwrap()
        .notification_id();

    let result = store.dismiss(queued_id, now);
    assert!(
        matches!(result, NotificationDismissResult::Dismissed(update) if !update.visible_projection_changed())
    );
    assert!(store.notifications[&queued_id].is_history());
    assert!(store.queued.is_empty());
    let expected: Vec<Option<NotificationId>> = visible_ids.iter().copied().map(Some).collect();
    assert_eq!(store.visible, expected);
}

#[test]
fn first_auto_dismissible_visible_id_skips_sticky_and_persistent_cards() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let sticky = store
        .push(
            NotificationDraft::new("Sticky").lifecycle(NotificationLifecycle::sticky()),
            now,
        )
        .unwrap()
        .notification_id();
    let persistent = store
        .push(
            NotificationDraft::new("Persistent")
                .lifecycle(NotificationLifecycle::sticky_persistent()),
            now,
        )
        .unwrap()
        .notification_id();
    assert_eq!(store.first_auto_dismissible_visible_id(), None);
    let eligible = store
        .push(
            NotificationDraft::new("Auto")
                .action(NotificationAction::new("Retry", Intent::new("retry"))),
            now,
        )
        .unwrap()
        .notification_id();

    assert_ne!(sticky, eligible);
    assert_ne!(persistent, eligible);
    assert_eq!(store.first_auto_dismissible_visible_id(), Some(eligible));
}

#[test]
fn dismiss_unknown_or_persistent_notification_is_a_noop() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let persistent_id = store
        .push(
            NotificationDraft::new("Persistent")
                .lifecycle(NotificationLifecycle::sticky_persistent()),
            now,
        )
        .unwrap()
        .notification_id();

    assert_eq!(
        store.dismiss(NotificationId::from_raw(99), now),
        NotificationDismissResult::Unknown
    );
    assert_eq!(
        store.dismiss(persistent_id, now),
        NotificationDismissResult::NotDismissible
    );
    assert_eq!(store.visible[0], Some(persistent_id));
    assert!(store.history.is_empty());
}

#[test]
fn dedup_pending_moves_entry_to_fifo_front_without_displacing_visible() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    for index in 0..store.max_visible() {
        store
            .push(NotificationDraft::new(format!("Visible {index}")), now)
            .unwrap();
    }
    let pending_id = store
        .push(NotificationDraft::new("Old").dedup_key("pending"), now)
        .unwrap()
        .notification_id();
    let later_id = store
        .push(NotificationDraft::new("Later"), now)
        .unwrap()
        .notification_id();

    let duplicate = store
        .push(NotificationDraft::new("Updated").dedup_key("pending"), now)
        .unwrap();
    assert_eq!(duplicate.notification_id(), pending_id);
    assert!(!duplicate.update().visible_projection_changed());
    assert_eq!(store.queued, VecDeque::from([pending_id, later_id]));
    assert_eq!(store.notifications[&pending_id].title, "Updated");
}

#[test]
fn dedup_history_reactivates_the_same_id() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let id = store
        .push(NotificationDraft::new("Old").dedup_key("history"), now)
        .unwrap()
        .notification_id();
    assert!(matches!(
        store.dismiss(id, now),
        NotificationDismissResult::Dismissed(_)
    ));

    let duplicate = store
        .push(NotificationDraft::new("New").dedup_key("history"), now)
        .unwrap();
    assert_eq!(duplicate.notification_id(), id);
    assert!(duplicate.update().visible_projection_changed());
    assert_eq!(store.visible[0], Some(id));
    assert!(store.history.is_empty());
}

#[test]
fn history_limit_evicts_oldest_entry_and_its_dedup_key() {
    let now = Instant::now();
    let mut store = NotificationStore::new(1);
    let first = store
        .push(NotificationDraft::new("First").dedup_key("first"), now)
        .unwrap()
        .notification_id();
    let second = store
        .push(NotificationDraft::new("Second").dedup_key("second"), now)
        .unwrap()
        .notification_id();

    assert!(matches!(
        store.dismiss(first, now),
        NotificationDismissResult::Dismissed(_)
    ));
    assert!(matches!(
        store.dismiss(second, now),
        NotificationDismissResult::Dismissed(_)
    ));
    assert_eq!(store.history, VecDeque::from([second]));
    assert!(!store.notifications.contains_key(&first));
    assert!(!store.dedup_index.contains_key("first"));
    assert_eq!(store.dedup_index.get("second"), Some(&second));
}

#[test]
fn resolve_allows_runtime_to_clear_a_persistent_notification() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let id = store
        .push(
            NotificationDraft::new("Resolved error")
                .lifecycle(NotificationLifecycle::sticky_persistent()),
            now,
        )
        .unwrap()
        .notification_id();

    assert!(matches!(
        store.resolve(id, now),
        NotificationDismissResult::Dismissed(_)
    ));
    assert!(store.notifications[&id].is_history());
    assert!(store.visible.iter().all(Option::is_none));
}

#[test]
fn push_explicitly_reports_id_counter_exhaustion() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    store.next_id = Some(u64::MAX);

    let last = store.push(NotificationDraft::new("Last"), now).unwrap();
    assert_eq!(last.notification_id().as_u64(), u64::MAX);
    assert_eq!(
        store.push(NotificationDraft::new("Overflow"), now),
        Err(NotificationStoreError::IdExhausted)
    );
}

#[test]
fn store_transition_receives_caller_time_and_reports_projection_change() {
    let now = Instant::now();
    let mut store = NotificationStore::new(100);
    let (observed_now, unchanged) =
        store.transition_at(now, |_, observed_now| (observed_now, false));
    assert_eq!(observed_now, now);
    assert!(!unchanged.visible_projection_changed());

    let (_, changed) = store.transition_at(now, |_, _| ((), true));
    assert!(changed.visible_projection_changed());
}
