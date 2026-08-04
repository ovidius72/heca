//! Persisting what the user has searched for and chosen — **the first runtime state this app keeps
//! across restarts**.
//!
//! Everything else read from disk here is *configuration*: `heca-config` and `heca-theme` load
//! `~/.config/heca/…` and never write. Machine-written state has different rules, and they are the
//! reason this does not live in `heca-config`:
//!
//! - it is rewritten without asking, so it must never surprise a user who edited it;
//! - it may be deleted at any time and lose nothing but convenience;
//! - a corrupt one must never stop the app — a search history is not worth a failed launch.
//!
//! It is also not in `heca-grid-ui`. A UI library has no business knowing where a heca install keeps
//! its state — a plugin, or another app embedding those widgets, would answer differently — and if
//! it opened files, every widget test would need a temp directory to run. So the library owns the
//! data and the rules ([`heca_grid_ui::search`]), and this owns the path, the format and the failure
//! behaviour.
//!
//! # Several heca instances share one file
//!
//! Running more than one heca is normal, so a save **merges with what is on disk** rather than
//! overwriting it — the shape neovim's shada file uses. Two windows no longer discard each other's
//! history the moment the second one saves.
//!
//! That needs an ordering both processes agree on, and the library's is deliberately process-local:
//! recency there is a distance in a **use-sequence**, not a timestamp, so a UI library needs no clock
//! and its tests can assert exact numbers. Instance A's "seq 40" and instance B's "seq 12" mean
//! nothing to each other.
//!
//! So the clock lives *here*, where a file already implies a filesystem:
//!
//! - the file stores **wall-clock seconds** per entry — `last_used_at`, and `at` per query;
//! - on load the sequence is **rebuilt** from those timestamps by sort order, so the library gets its
//!   clean monotonic sequence back and never learns a clock was involved;
//! - on save, counts merge by **delta** — what this instance added since it last agreed with the
//!   file, applied on top of whatever is there now. Two instances that each ran a command twice from
//!   a count of 5 end at 9. A plain `max` would quietly drop one of them.
//!
//! What is **not** solved: two saves landing at the same instant can still lose one increment, since
//! this is a read-modify-write with no lock. Neovim has the same race and accepts it; closing it
//! would mean advisory file locking — a dependency and a new failure mode — in exchange for one
//! increment.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use heca_grid_ui::search::{Caps, Frecency, History, Scope, SearchStore};
use serde::{Deserialize, Serialize};

/// Schema version. Written from the first line ever saved, so a later layout can be introduced by
/// **deciding** what to do with an older file rather than discovering the mismatch as a parse error.
///
/// v2 replaced the process-local `seq` with wall-clock timestamps, which is what makes a merge
/// between two instances possible at all.
const VERSION: u32 = 2;

// The caps come from `[settings] search_history_size` / `search_usage_size` and are applied on write
// as well as in memory. The library already bounds both, but a file that a future bug lets grow is a
// file that stays grown.

/// Where the file lives. `data_dir`, **not** `config_dir`: this is not something a user writes.
///
/// `~/.local/share/heca/` on Linux, `~/Library/Application Support/heca/` on macOS. `None` when the
/// platform has no answer, in which case the session simply remembers nothing — in memory only,
/// never a panic.
pub fn default_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("heca").join("search-history.json"))
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Persisted {
    version: u32,
    #[serde(default)]
    scopes: BTreeMap<String, PersistedScope>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct PersistedScope {
    /// Past queries, oldest first, each with when it was last searched.
    #[serde(default)]
    history: Vec<PersistedQuery>,
    #[serde(default)]
    uses: Vec<PersistedUse>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PersistedQuery {
    query: String,
    at: u64,
}

#[derive(Serialize, Deserialize, Clone)]
struct PersistedUse {
    id: String,
    count: u32,
    last_used_at: u64,
}

/// What this instance last agreed the file said — the point a delta is measured from.
///
/// Without it a merge cannot tell "the disk says 5 because of us" from "the disk says 5 because of
/// another window", and every save would either double-count or discard.
#[derive(Default, Clone)]
pub struct Baseline(Persisted);

/// Read the store from `path`, and the baseline to measure the next save against. **Total** — it
/// cannot fail.
///
/// A missing file, an unreadable one, malformed JSON and a version this build does not know all give
/// the same answer: an empty store, which behaves exactly like a first run. The alternative —
/// reporting it — would interrupt a launch over something the user can fix by deleting a file they
/// did not know existed.
pub fn load(path: &Path, caps: Caps) -> (SearchStore, Baseline) {
    let file = read_file(path);
    (store_from(&file, caps), Baseline(file))
}

/// Parse the file at `path`, or an empty one — the single place a bad file becomes a fresh start.
fn read_file(path: &Path) -> Persisted {
    let empty = || Persisted { version: VERSION, ..Default::default() };
    let Ok(text) = std::fs::read_to_string(path) else {
        return empty();
    };
    let parsed: Persisted = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[heca] search history at {} is unreadable ({e}); starting empty",
                path.display(),
            );
            return empty();
        }
    };
    if parsed.version != VERSION {
        eprintln!(
            "[heca] search history at {} is version {}, not {VERSION}; starting empty",
            path.display(),
            parsed.version,
        );
        return empty();
    }
    parsed
}

/// Rebuild a live store from a parsed file.
///
/// **The use-sequence is reconstructed here** from the timestamps: entries sorted oldest-first take
/// sequence positions `1..n`. The library then sees exactly what it would have built itself.
fn store_from(file: &Persisted, caps: Caps) -> SearchStore {
    let mut store = SearchStore::with_caps(caps);
    for (name, scope) in &file.scopes {
        let mut uses: Vec<&PersistedUse> = scope.uses.iter().collect();
        uses.sort_by_key(|u| u.last_used_at);
        let seq = uses.len() as u64;
        let entries = uses.iter().enumerate().map(|(i, u)| (u.id.clone(), u.count, i as u64 + 1));
        store.insert(
            name.clone(),
            Scope {
                history: History::restore(
                    caps.history,
                    scope.history.iter().map(|q| q.query.clone()),
                ),
                frecency: Frecency::restore(seq, caps.usage, entries),
            },
        );
    }
    store
}

/// Merge `store` into whatever is on disk **now** and write the result, atomically.
///
/// Returns the new baseline — what this instance now agrees the file says.
pub fn save(path: &Path, store: &SearchStore, baseline: &Baseline) -> std::io::Result<Baseline> {
    let caps = store.caps();
    let stamp = now();
    // Re-read: another instance may have written since we loaded, and its work is not ours to
    // discard. This re-read is the whole difference between merging and overwriting.
    let disk = read_file(path);
    let mut merged = Persisted { version: VERSION, scopes: BTreeMap::new() };

    let names: std::collections::BTreeSet<String> = store
        .scopes()
        .map(|(n, _)| n.to_string())
        .chain(disk.scopes.keys().cloned())
        .collect();

    for name in names {
        let ours = store.scope(&name);
        let base = baseline.0.scopes.get(&name);
        let on_disk = disk.scopes.get(&name);

        // ── usage: the disk's counts, plus what we added since the baseline ──
        let mut counts: BTreeMap<String, PersistedUse> = BTreeMap::new();
        for u in on_disk.iter().flat_map(|s| &s.uses) {
            counts.insert(u.id.clone(), u.clone());
        }
        for (id, count, _) in ours.iter().flat_map(|s| s.frecency.entries()) {
            let before = base.and_then(|b| b.uses.iter().find(|u| u.id == id));
            let delta = count.saturating_sub(before.map_or(0, |u| u.count));
            let entry = counts.entry(id.to_string()).or_insert(PersistedUse {
                id: id.to_string(),
                count: 0,
                last_used_at: 0,
            });
            if delta > 0 {
                entry.count = entry.count.saturating_add(delta);
                entry.last_used_at = stamp;
            } else if entry.count == 0 {
                // Ours, untouched this session, and the disk has never heard of it — carry it over
                // rather than dropping what we loaded.
                entry.count = count;
                entry.last_used_at = before.map_or(stamp, |u| u.last_used_at);
            }
        }
        let mut uses: Vec<PersistedUse> = counts.into_values().filter(|u| u.count > 0).collect();
        // Most recent first, so a truncation keeps what matters.
        uses.sort_by_key(|u| std::cmp::Reverse(u.last_used_at));
        uses.truncate(caps.usage);

        // ── queries: the union, the newer timestamp winning ──
        let mut when: BTreeMap<String, u64> = BTreeMap::new();
        for q in on_disk.iter().flat_map(|s| &s.history) {
            when.insert(q.query.clone(), q.at);
        }
        for q in ours.iter().flat_map(|s| s.history.entries()) {
            // Searched this session ⇒ stamp it now; otherwise keep the time it already had.
            let at = base
                .and_then(|b| b.history.iter().find(|p| &p.query == q))
                .map_or(stamp, |p| p.at);
            let slot = when.entry(q.clone()).or_insert(0);
            *slot = (*slot).max(at);
        }
        let mut history: Vec<PersistedQuery> =
            when.into_iter().map(|(query, at)| PersistedQuery { query, at }).collect();
        history.sort_by_key(|q| q.at);
        let overflow = history.len().saturating_sub(caps.history);
        history.drain(..overflow);

        if !history.is_empty() || !uses.is_empty() {
            merged.scopes.insert(name, PersistedScope { history, uses });
        }
    }

    write_file(path, &merged)?;
    Ok(Baseline(merged))
}

/// Write `file` at `path`, atomically: a sibling temp file then a rename, so an interrupted write
/// leaves the previous file intact rather than a half-parsed one.
fn write_file(path: &Path, file: &Persisted) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(file)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Which half of a scope's memory to forget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Forget {
    /// The past queries — what the history keys walk.
    Queries,
    /// The usage counts — what orders a list.
    Ranking,
}

/// Forget part of the search memory, **in this process and on disk**.
///
/// The file has to be rewritten here rather than left to the next
/// [`persist_if_changed`]: a save *merges* with what is on disk, so an emptied store would simply
/// have the old entries merged straight back in. Clearing is the one operation that must remove
/// rather than combine.
///
/// `scope` of `None` forgets every scope. Another instance that still holds the old entries in
/// memory will merge them back on its next save — the same trade-off the merge makes everywhere,
/// and better than one window silently discarding another's work.
pub fn forget(state: &mut crate::app_state::AppState, scope: Option<&str>, what: Forget) -> bool {
    let cleared = {
        let mut store = state.search_store.borrow_mut();
        match what {
            Forget::Queries => store.clear_history(scope),
            Forget::Ranking => store.clear_ranking(scope),
        }
    };
    if !cleared {
        return false;
    }
    // Keep the revision in step so the next ordinary save does not think it has work to do.
    state.search_saved_revision = state.search_store.borrow().revision();
    let Some(path) = default_path() else {
        return true;
    };
    match forget_on_disk(&path, scope, what) {
        Ok(baseline) => state.search_baseline = baseline,
        Err(e) => eprintln!("[heca] could not rewrite {}: {e}", path.display()),
    }
    true
}

/// Strip one half of the memory out of the file and write it back — the part of [`forget`] that
/// needs nothing but a path, so it can be tested without an `AppState` (which needs a window).
fn forget_on_disk(path: &Path, scope: Option<&str>, what: Forget) -> std::io::Result<Baseline> {
    let mut file = read_file(path);
    let strip = |s: &mut PersistedScope| match what {
        Forget::Queries => s.history.clear(),
        Forget::Ranking => s.uses.clear(),
    };
    match scope {
        Some(name) => {
            if let Some(s) = file.scopes.get_mut(name) {
                strip(s);
            }
        }
        None => file.scopes.values_mut().for_each(strip),
    }
    file.scopes.retain(|_, s| !s.history.is_empty() || !s.uses.is_empty());
    write_file(path, &file)?;
    Ok(Baseline(file))
}

/// Persist the app's search memory **if anything has been recorded since the last save**.
///
/// The one call any consumer needs — and the host makes it, not them. A search surface records a run
/// and nothing else; this notices, because `SearchStore::revision` moved. Making every consumer
/// remember to save would mean the second one silently stops being remembered, with nothing to
/// report it.
///
/// Best-effort: a home directory that cannot be written is not a reason to interrupt someone running
/// a command, so a failure is reported once and the session carries on remembering in memory.
pub fn persist_if_changed(state: &mut crate::app_state::AppState) {
    if !state.search_history {
        return;
    }
    let revision = state.search_store.borrow().revision();
    if revision == state.search_saved_revision {
        return;
    }
    state.search_saved_revision = revision;
    let Some(path) = default_path() else {
        return;
    };
    let saved = {
        let store = state.search_store.borrow();
        save(&path, &store, &state.search_baseline)
    };
    match saved {
        Ok(baseline) => state.search_baseline = baseline,
        Err(e) => eprintln!("[heca] could not save the search history to {}: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("heca-search-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("search-history.json")
    }

    fn model(store: &Rc<RefCell<SearchStore>>) -> SearchModel {
        SearchModel::new("command", store.clone())
    }

    /// Record, save, load — and the ordering that comes back is the one that went in.
    #[test]
    fn a_saved_store_comes_back_the_same() {
        let path = temp("roundtrip");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        let mut m = model(&store);
        m.record_run("clo", Some("close"));
        m.record_run("clo", Some("close"));
        m.record_run("spl", Some("split"));
        save(&path, &store.borrow(), &Baseline::default()).expect("save");

        let restored = Rc::new(RefCell::new(load(&path, Caps::default()).0));
        let items = [("close", "Close Pane"), ("split", "Split Pane")];
        assert_eq!(
            model(&store).rank(&items, "", |i| (Some(i.0), i.1)),
            model(&restored).rank(&items, "", |i| (Some(i.0), i.1)),
            "the ranking survives a round trip",
        );

        match model(&restored).handle(heca_grid_ui::WidgetIntent::MenuHistoryUp, "") {
            heca_grid_ui::search::SearchAction::SetQuery(q) => assert_eq!(q, "spl"),
            other => panic!("expected the newest past query, got {other:?}"),
        }
    }

    /// **Nothing about this file may stop the app.**
    #[test]
    fn an_unusable_file_is_an_empty_store_not_an_error() {
        let path = temp("corrupt");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let empty = |p: &Path| load(p, Caps::default()).0.scope("command").is_none();

        assert!(empty(&path), "missing");
        std::fs::write(&path, "{{{ not json").expect("write");
        assert!(empty(&path), "not json");
        std::fs::write(&path, r#"{"version":2,"scopes":{"command":{"history":"#).expect("write");
        assert!(empty(&path), "truncated");
        std::fs::write(&path, r#"{"version":99,"scopes":{}}"#).expect("write");
        assert!(empty(&path), "a version this build does not know");
        // v1 — the process-local `seq` layout — is discarded, not read leniently.
        std::fs::write(&path, r#"{"version":1,"scopes":{"command":{"history":["x"]}}}"#)
            .expect("write");
        assert!(empty(&path), "the previous schema");
    }

    /// **Two instances do not discard each other.** Both load the same file, both run the same
    /// command twice, and the count ends at the sum — not at whatever the last writer held.
    #[test]
    fn two_instances_merge_instead_of_overwriting() {
        let path = temp("merge");
        // A shared starting point: `close` already run once.
        let seed = Rc::new(RefCell::new(SearchStore::new()));
        model(&seed).record_run("q", Some("close"));
        save(&path, &seed.borrow(), &Baseline::default()).expect("seed");

        let (a, a_base) = load(&path, Caps::default());
        let (b, b_base) = load(&path, Caps::default());
        let (a, b) = (Rc::new(RefCell::new(a)), Rc::new(RefCell::new(b)));

        for _ in 0..2 {
            model(&a).record_run("from_a", Some("close"));
            model(&b).record_run("from_b", Some("close"));
        }
        save(&path, &a.borrow(), &a_base).expect("a saves");
        save(&path, &b.borrow(), &b_base).expect("b saves");

        let file = read_file(&path);
        let scope = file.scopes.get("command").expect("the scope");
        let close = scope.uses.iter().find(|u| u.id == "close").expect("close");
        assert_eq!(close.count, 5, "1 seeded + 2 from each instance, not 3");

        let queries: Vec<&str> = scope.history.iter().map(|q| q.query.as_str()).collect();
        assert!(queries.contains(&"from_a"), "A's search survived B's save: {queries:?}");
        assert!(queries.contains(&"from_b"), "and B's is there too: {queries:?}");
    }

    /// A save that recorded nothing must not inflate what is already there.
    #[test]
    fn saving_twice_without_recording_changes_nothing() {
        let path = temp("idempotent");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        model(&store).record_run("q", Some("close"));
        let base = save(&path, &store.borrow(), &Baseline::default()).expect("save");
        save(&path, &store.borrow(), &base).expect("save again");

        let file = read_file(&path);
        let close = file.scopes["command"].uses.iter().find(|u| u.id == "close").expect("close");
        assert_eq!(close.count, 1, "an unchanged store must not add to itself");
    }

    /// The file stays bounded however much is recorded.
    #[test]
    fn the_caps_hold_on_write() {
        let path = temp("caps");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        for i in 0..200 {
            model(&store).record_run(&format!("query{i}"), Some(&format!("id{i}")));
        }
        save(&path, &store.borrow(), &Baseline::default()).expect("save");
        let file = read_file(&path);
        let scope = file.scopes.get("command").expect("the scope");
        assert!(scope.history.len() <= Caps::default().history, "history: {}", scope.history.len());
        assert!(scope.uses.len() <= Caps::default().usage, "uses: {}", scope.uses.len());
    }

    /// **Forgetting removes; it does not merge.** A cleared store saved normally would have the old
    /// entries merged straight back in off the disk, so clearing rewrites the file — and takes only
    /// the half it was asked for.
    #[test]
    fn forgetting_queries_leaves_the_ranking_alone() {
        let path = temp("forget");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        model(&store).record_run("a query", Some("close"));
        save(&path, &store.borrow(), &Baseline::default()).expect("save");

        forget_on_disk(&path, None, Forget::Queries).expect("forget");
        let file = read_file(&path);
        let scope = file.scopes.get("command").expect("the scope survives");
        assert!(scope.history.is_empty(), "the queries are gone");
        assert_eq!(scope.uses.len(), 1, "…and what you use is not");

        forget_on_disk(&path, None, Forget::Ranking).expect("forget");
        assert!(read_file(&path).scopes.is_empty(), "an empty scope is dropped entirely");
    }

    /// A scope names which surface to forget — the others keep theirs.
    #[test]
    fn forgetting_one_scope_leaves_the_others() {
        let path = temp("forget-scope");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        SearchModel::new("command", store.clone()).record_run("cmd", Some("close"));
        SearchModel::new("symbol", store.clone()).record_run("sym", Some("thing"));
        save(&path, &store.borrow(), &Baseline::default()).expect("save");

        forget_on_disk(&path, Some("command"), Forget::Queries).expect("forget");
        let file = read_file(&path);
        assert!(file.scopes["command"].history.is_empty(), "the named scope is forgotten");
        assert_eq!(file.scopes["symbol"].history.len(), 1, "the others are untouched");
    }

    /// The write is atomic: no temp file is left behind for the next load to trip over.
    #[test]
    fn saving_leaves_no_temp_file() {
        let path = temp("atomic");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        model(&store).record_run("q", Some("id"));
        save(&path, &store.borrow(), &Baseline::default()).expect("save");
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists(), "the temp file was renamed, not left");
    }
}
