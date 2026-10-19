use super::*;
use crate::component::WidgetIntent;
use std::cell::RefCell;
use std::rc::Rc;

/// The three modes, on the same query and the same text.
#[test]
fn the_case_mode_is_the_users_choice() {
    let items = [((), "Git Push")];
    let hit = |case: MatchCase, q: &str| {
        !SearchModel::detached("s")
            .case(case)
            .rank(&items, q, |i| (None, i.1))
            .is_empty()
    };
    // Smart: lowercase finds anything; reaching for Shift is how you say you meant it.
    assert!(hit(MatchCase::Smart, "git"));
    assert!(!hit(MatchCase::Smart, "GIT"));
    // Sensitive: the query is taken literally, always.
    assert!(!hit(MatchCase::Sensitive, "git"));
    assert!(hit(MatchCase::Sensitive, "Git"));
    // Insensitive: case never matters, even when the query shouts.
    assert!(hit(MatchCase::Insensitive, "git"));
    assert!(hit(MatchCase::Insensitive, "GIT"));
}

/// Chosen more often outranks chosen once; used recently outranks used long ago.
#[test]
fn frecency_counts_uses_and_how_recent_they_were() {
    let mut f = Frecency::new();
    for _ in 0..5 {
        f.record("often");
    }
    f.record("once");
    assert!(
        f.boost("often") > f.boost("once"),
        "{} vs {}",
        f.boost("often"),
        f.boost("once"),
    );
    assert_eq!(f.boost("never"), 0, "an id never chosen adds nothing");

    // Same count, different age.
    let mut g = Frecency::new();
    g.record("old");
    for i in 0..20 {
        g.record(&format!("filler{i}"));
    }
    g.record("new");
    assert!(
        g.boost("new") > g.boost("old"),
        "recency separates equal counts"
    );
}

/// **A second use outranks a single more-recent one.** The boundary a user actually notices, and
/// the one the first calibration got wrong: integer `log2` stepped only at 1, 3, 7, so running
/// something twice scored exactly the same as running it once and recency decided everything.
#[test]
fn using_something_twice_beats_using_something_else_once_just_now() {
    let mut f = Frecency::new();
    f.record("twice");
    f.record("twice");
    f.record("once_just_now");
    assert!(
        f.boost("twice") > f.boost("once_just_now"),
        "twice={} once={}",
        f.boost("twice"),
        f.boost("once_just_now"),
    );
    // …and recency still separates entries of equal count.
    let mut g = Frecency::new();
    g.record("stale");
    for i in 0..5 {
        g.record(&format!("f{i}"));
    }
    g.record("fresh");
    assert!(g.boost("fresh") > g.boost("stale"));
}

/// **The cap is the point.** A much-used entry must not overtake a clearly better textual match,
/// or typing stops meaning anything.
#[test]
fn frecency_decides_ties_but_never_beats_a_better_match() {
    let store = Rc::new(RefCell::new(SearchStore::new()));
    let mut model = SearchModel::new("command", store);
    for _ in 0..50 {
        model.record_run("", Some("much_used"));
    }
    let items = [
        ("much_used", "Zoom Out Everything"),
        ("close", "Close Pane"),
    ];
    let ranked = model.rank(&items, "close", |i| (Some(i.0), i.1));
    assert_eq!(
        items[ranked[0].index].0, "close",
        "the query spells `Close Pane`, so it leads however often the other was chosen",
    );

    // With nothing typed, the boost is all there is — and now it leads.
    let empty = model.rank(&items, "", |i| (Some(i.0), i.1));
    assert_eq!(items[empty[0].index].0, "much_used");
}

/// An empty store must reproduce the matcher's own order exactly — first-run behaviour cannot
/// differ from no-frecency behaviour.
#[test]
fn an_empty_store_changes_no_ordering() {
    let items = [("a", "Close Pane"), ("b", "Close Window")];
    let with_ids = SearchModel::detached("command").rank(&items, "close", |i| (Some(i.0), i.1));
    let without = SearchModel::detached("command").rank(&items, "close", |i| (None, i.1));
    assert_eq!(with_ids, without);
}

#[test]
fn history_dedupes_and_stops_at_the_oldest() {
    let mut h = History::new();
    h.push("");
    h.push("close");
    h.push("close");
    h.push("split");
    assert_eq!(
        h.entries(),
        ["close", "split"],
        "empty and repeats are not remembered"
    );

    assert_eq!(h.older(""), Some("split"));
    assert_eq!(h.older(""), Some("close"));
    assert_eq!(
        h.older(""),
        Some("close"),
        "the oldest is a wall, not a loop"
    );
}

/// Walking back down past the newest restores **what the user was typing**, not the last entry.
#[test]
fn history_gives_the_draft_back() {
    let mut h = History::new();
    h.push("close");
    h.push("split");
    assert_eq!(h.older("dra"), Some("split"));
    assert_eq!(h.older("dra"), Some("close"));
    assert_eq!(h.newer(), Some("split"));
    assert_eq!(h.newer(), Some("dra"), "the interrupted draft comes back");
    assert_eq!(h.newer(), None, "and the walk is over");
}

/// The full walk driven **by intents alone, with no widget in the test**. If this needed one,
/// the seam would be in the wrong place.
#[test]
fn the_intents_drive_the_whole_walk() {
    let mut model = SearchModel::detached("command");
    model.record_run("close", Some("close"));
    model.record_run("split", Some("split"));

    let up = |m: &mut SearchModel, q: &str| match m.handle(WidgetIntent::MenuHistoryUp, q) {
        SearchAction::SetQuery(t) => t,
        other => panic!("expected a query, got {other:?}"),
    };
    let down = |m: &mut SearchModel| match m.handle(WidgetIntent::MenuHistoryDown, "") {
        SearchAction::SetQuery(t) => t,
        other => panic!("expected a query, got {other:?}"),
    };
    assert_eq!(up(&mut model, "typed"), "split");
    assert_eq!(up(&mut model, "typed"), "close");
    assert_eq!(down(&mut model), "split");
    assert_eq!(down(&mut model), "typed", "back to the draft");

    // Anything else is not ours.
    assert_eq!(
        model.handle(WidgetIntent::MenuDown, ""),
        SearchAction::Ignored,
    );
}

/// **The two halves are forgotten separately.** Clearing what you typed is not clearing what
/// you use, and a user asking for one rarely means the other.
#[test]
fn queries_and_ranking_are_cleared_independently() {
    let store = Rc::new(RefCell::new(SearchStore::new()));
    let mut m = SearchModel::new("command", store.clone());
    m.record_run("a query", Some("close"));
    let items = [("close", "Close Pane")];

    assert!(store.borrow_mut().clear_history(Some("command")));
    assert_eq!(
        m.handle(WidgetIntent::MenuHistoryUp, "typed"),
        SearchAction::SetQuery("typed".to_string()),
        "there is nothing left to recall",
    );
    assert!(
        m.rank(&items, "", |i| (Some(i.0), i.1))[0].score > 0,
        "…but what you use still ranks",
    );

    assert!(store.borrow_mut().clear_ranking(Some("command")));
    assert_eq!(
        m.rank(&items, "", |i| (Some(i.0), i.1))[0].score,
        0,
        "and now it does not"
    );

    // Nothing left to forget is not an error, and reports honestly.
    assert!(!store.borrow_mut().clear_history(Some("command")));
    assert!(!store.borrow_mut().clear_ranking(Some("nonexistent")));
}

/// A scope is a separate memory — that is what makes `>` / `@` / `:` data rather than a reshape.
#[test]
fn scopes_do_not_share_a_history() {
    let store = Rc::new(RefCell::new(SearchStore::new()));
    let mut commands = SearchModel::new("command", store.clone());
    let mut symbols = SearchModel::new("symbol", store);
    commands.record_run("close", None);
    assert_eq!(
        symbols.handle(WidgetIntent::MenuHistoryUp, "q"),
        SearchAction::SetQuery("q".to_string()),
        "the symbol scope has no history of its own yet",
    );
}

/// An abandoned search records nothing, and a run without an id still records the query.
#[test]
fn only_a_run_is_remembered() {
    let mut model = SearchModel::detached("command");
    model.query_changed();
    assert_eq!(
        model.handle(WidgetIntent::MenuHistoryUp, "typed"),
        SearchAction::SetQuery("typed".to_string()),
        "nothing was run, so there is nothing to recall",
    );
    model.record_run("filter", None);
    assert_eq!(
        model.handle(WidgetIntent::MenuHistoryUp, "typed"),
        SearchAction::SetQuery("filter".to_string()),
    );
}
