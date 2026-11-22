//! The notification history: what the store lists, in what order, and how many.

use super::*;
use std::time::{Duration, SystemTime};

const WALL: Duration = Duration::from_secs(1_000_000);

/// `store`'s history for window `client`, read at `now`, as `(title, standing word)` newest first.
fn listed(store: &NotificationStore, client: u64, now: Instant) -> Vec<(String, &'static str)> {
    store
        .history_records_for(client, now, SystemTime::UNIX_EPOCH + WALL)
        .into_iter()
        .map(|record| (record.view.title, record.standing.word()))
        .collect()
}

/// A store roomy enough that nothing queues, holding `titles` raised one millisecond apart.
fn raised(
    limit: usize,
    titles: &[&str],
    start: Instant,
) -> (NotificationStore, Vec<NotificationId>) {
    let mut store = NotificationStore::with_capacity(limit, 10);
    let ids = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let at = start + Duration::from_millis(i as u64);
            store
                .push(NotificationDraft::new(*title), at)
                .unwrap()
                .notification_id()
        })
        .collect();
    (store, ids)
}

#[test]
fn the_history_keeps_at_most_its_limit_and_lists_the_newest_first() {
    let start = Instant::now();
    let (mut store, ids) = raised(3, &["one", "two", "three", "four", "five"], start);
    let later = start + Duration::from_secs(1);
    for id in ids {
        assert!(matches!(
            store.dismiss(id, later),
            NotificationDismissResult::Dismissed(_)
        ));
    }

    assert_eq!(
        listed(&store, 1, later),
        [
            ("five".into(), "dismissed"),
            ("four".into(), "dismissed"),
            ("three".into(), "dismissed")
        ],
        "the two oldest were forgotten, the rest read newest first",
    );
}

#[test]
fn a_limit_of_zero_keeps_nothing_but_what_is_still_on_screen() {
    let start = Instant::now();
    let (mut store, ids) = raised(0, &["gone", "still here"], start);
    let later = start + Duration::from_secs(1);
    store.dismiss(ids[0], later);

    assert_eq!(listed(&store, 1, later), [("still here".into(), "visible")]);
}

#[test]
fn a_smaller_limit_forgets_the_oldest_at_once_and_a_bigger_one_keeps_more() {
    let start = Instant::now();
    let (mut store, ids) = raised(100, &["a", "b", "c"], start);
    let later = start + Duration::from_secs(1);
    for id in ids {
        store.dismiss(id, later);
    }
    assert_eq!(listed(&store, 1, later).len(), 3);

    store.set_history_limit(1);
    assert_eq!(listed(&store, 1, later), [("c".into(), "dismissed")]);

    store.set_history_limit(0);
    assert!(listed(&store, 1, later).is_empty(), "zero empties it");
}

#[test]
fn what_is_still_on_screen_is_listed_with_its_actions_and_what_is_gone_is_not() {
    let start = Instant::now();
    let mut store = NotificationStore::with_capacity(10, 10);
    let retry = || NotificationAction::new("Retry", heca_view::Intent::new("restore.retry"));
    let first = store
        .push(NotificationDraft::new("first").action(retry()), start)
        .unwrap()
        .notification_id();
    store
        .push(
            NotificationDraft::new("second").action(retry()),
            start + Duration::from_millis(1),
        )
        .unwrap();
    store.dismiss(first, start + Duration::from_secs(1));

    let records = store.history_records_for(
        1,
        start + Duration::from_secs(2),
        SystemTime::UNIX_EPOCH + WALL,
    );
    let [second, first] = records.as_slice() else {
        panic!("two records, got {records:?}")
    };
    assert_eq!(second.standing.word(), "visible");
    assert_eq!(second.view.actions.len(), 1, "it can still be acted on");
    assert_eq!(first.standing.word(), "dismissed");
    assert!(
        first.view.actions.is_empty(),
        "nothing left to act on once it is gone"
    );
}

#[test]
fn newest_means_raised_last_not_dismissed_last() {
    let start = Instant::now();
    let (mut store, ids) = raised(10, &["older", "newer"], start);
    let later = start + Duration::from_secs(1);
    // The newer one is dismissed first, so it is archived before the older one.
    store.dismiss(ids[1], later);
    store.dismiss(ids[0], later);

    assert_eq!(
        listed(&store, 1, later),
        [("newer".into(), "dismissed"), ("older".into(), "dismissed")],
    );
}

#[test]
fn a_queued_notification_was_never_shown_so_it_is_not_in_the_history() {
    let start = Instant::now();
    let mut store = NotificationStore::with_capacity(10, 1);
    store
        .push(NotificationDraft::new("on screen"), start)
        .unwrap();
    store
        .push(
            NotificationDraft::new("waiting"),
            start + Duration::from_millis(1),
        )
        .unwrap();

    assert_eq!(listed(&store, 1, start), [("on screen".into(), "visible")]);
}

#[test]
fn a_window_reads_only_the_history_meant_for_it() {
    let start = Instant::now();
    let mut store = NotificationStore::with_capacity(10, 10);
    store
        .push(NotificationDraft::new("everyone"), start)
        .unwrap();
    store
        .push(
            NotificationDraft::new("window two")
                .originator()
                .resolve_originator(2),
            start + Duration::from_millis(1),
        )
        .unwrap();

    let titles = |client| {
        listed(&store, client, start)
            .into_iter()
            .map(|(title, _)| title)
            .collect::<Vec<_>>()
    };
    assert_eq!(titles(1), ["everyone"]);
    assert_eq!(titles(2), ["window two", "everyone"]);
}

#[test]
fn the_raised_time_is_the_wall_clock_minus_the_age() {
    let start = Instant::now();
    let (store, _) = raised(10, &["x"], start);
    let now = start + Duration::from_secs(90);
    let [record] = store
        .history_records_for(1, now, SystemTime::UNIX_EPOCH + WALL)
        .try_into()
        .unwrap_or_else(|_| panic!("one record"));
    assert_eq!(record.raised_at, WALL.as_secs() - 90);
}

#[test]
fn the_same_dedup_key_raised_again_is_one_entry_that_counts_its_repeats() {
    let start = Instant::now();
    let mut store = NotificationStore::with_capacity(10, 10);
    for at in 0..2 {
        store
            .push(
                NotificationDraft::new("Config reloaded").dedup_key("reload"),
                start + Duration::from_millis(at),
            )
            .unwrap();
    }
    let records = store.history_records_for(
        1,
        start + Duration::from_secs(1),
        SystemTime::UNIX_EPOCH + WALL,
    );
    let [record] = records.as_slice() else {
        panic!("one entry, got {records:?}")
    };
    assert_eq!(record.repeats, 2);

    // Dismissed and raised again, it is the same entry, counted again.
    let id = store.notification_id_for_dedup_key("reload").unwrap();
    store.dismiss(id, start + Duration::from_secs(2));
    store
        .push(
            NotificationDraft::new("Config reloaded").dedup_key("reload"),
            start + Duration::from_secs(3),
        )
        .unwrap();
    let records = store.history_records_for(
        1,
        start + Duration::from_secs(4),
        SystemTime::UNIX_EPOCH + WALL,
    );
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].repeats, 3);

    // A different notification starts at one.
    store
        .push(
            NotificationDraft::new("Other"),
            start + Duration::from_secs(5),
        )
        .unwrap();
    let records = store.history_records_for(
        1,
        start + Duration::from_secs(6),
        SystemTime::UNIX_EPOCH + WALL,
    );
    assert_eq!(records[0].repeats, 1);
}
