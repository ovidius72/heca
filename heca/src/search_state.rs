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
//! **Last writer wins.** Two heca processes sharing a home directory will overwrite each other's
//! history; the atomic rename keeps the file valid, and nothing merges. That is written down here so
//! nobody later assumes otherwise and builds on a guarantee that was never made.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use heca_grid_ui::search::{Caps, Frecency, History, Scope, SearchStore};
use serde::{Deserialize, Serialize};

/// Schema version. Written from the first line ever saved, so a later scope layout (the `>` / `@`
/// / `:` modes) can be introduced by **deciding** what to do with an older file rather than
/// discovering the mismatch as a parse error.
const VERSION: u32 = 1;

// The caps come from `[settings] search_history_size` / `search_usage_size` and are applied on
// write as well as in memory. The library already bounds both, but a file that a future bug lets
// grow is a file that stays grown.

/// Where the file lives. `data_dir`, **not** `config_dir`: this is not something a user writes.
///
/// `~/.local/share/heca/` on Linux, `~/Library/Application Support/heca/` on macOS. `None` when the
/// platform has no answer, in which case the session simply remembers nothing — in memory only,
/// never a panic.
pub fn default_path() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("heca").join("search-history.json"))
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    version: u32,
    #[serde(default)]
    scopes: BTreeMap<String, PersistedScope>,
}

#[derive(Serialize, Deserialize, Default)]
struct PersistedScope {
    /// Past queries, oldest first.
    #[serde(default)]
    history: Vec<String>,
    /// The use-sequence. **Persisted deliberately**: recency is a distance in this sequence rather
    /// than a timestamp, so dropping it would make every entry equally old on the next launch.
    #[serde(default)]
    seq: u64,
    #[serde(default)]
    uses: Vec<PersistedUse>,
}

#[derive(Serialize, Deserialize)]
struct PersistedUse {
    id: String,
    count: u32,
    last_seq: u64,
}

/// Read the store from `path`. **Total** — it cannot fail.
///
/// A missing file, an unreadable one, malformed JSON and a version this build does not know all
/// give the same answer: an empty store, which behaves exactly like a first run. The alternative —
/// reporting it — would interrupt a launch over something the user can fix by deleting a file they
/// did not know existed.
pub fn load(path: &Path, caps: Caps) -> SearchStore {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SearchStore::with_caps(caps);
    };
    let parsed: Persisted = match serde_json::from_str(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "[heca] search history at {} is unreadable ({e}); starting empty",
                path.display(),
            );
            return SearchStore::with_caps(caps);
        }
    };
    if parsed.version != VERSION {
        eprintln!(
            "[heca] search history at {} is version {}, not {VERSION}; starting empty",
            path.display(),
            parsed.version,
        );
        return SearchStore::with_caps(caps);
    }
    let mut store = SearchStore::with_caps(caps);
    for (name, scope) in parsed.scopes {
        store.insert(
            name,
            Scope {
                history: History::restore(caps.history, scope.history),
                frecency: Frecency::restore(
                    scope.seq,
                    caps.usage,
                    scope.uses.into_iter().map(|u| (u.id, u.count, u.last_seq)),
                ),
            },
        );
    }
    store
}

/// Write the store to `path`, atomically.
///
/// A sibling temp file then a rename, so an interrupted write leaves the previous file intact
/// rather than a half-parsed one. (A truncated file would be survivable — [`load`] treats it as
/// empty — but there is no reason to manufacture the case.)
pub fn save(path: &Path, store: &SearchStore) -> std::io::Result<()> {
    let caps = store.caps();
    let mut scopes = BTreeMap::new();
    for (name, scope) in store.scopes() {
        let history = scope.history.entries();
        let start = history.len().saturating_sub(caps.history);
        let mut uses: Vec<PersistedUse> = scope
            .frecency
            .entries()
            .map(|(id, count, last_seq)| PersistedUse { id: id.to_string(), count, last_seq })
            .collect();
        // Most-recently-used first, so a truncation keeps what matters.
        uses.sort_by_key(|u| std::cmp::Reverse(u.last_seq));
        uses.truncate(caps.usage);
        scopes.insert(
            name.to_string(),
            PersistedScope {
                history: history[start..].to_vec(),
                seq: scope.frecency.seq(),
                uses,
            },
        );
    }
    let text = serde_json::to_string_pretty(&Persisted { version: VERSION, scopes })?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// Persist the app's search memory **if anything has been recorded since the last save**.
///
/// The one call any consumer needs — and it is called by the host, not by them. A search surface
/// records a run and nothing else; this notices, because `SearchStore::revision` moved. Making
/// every consumer remember to save would mean the second one silently stops being remembered, with
/// nothing to report it.
///
/// Best-effort: a home directory that cannot be written is not a reason to interrupt someone
/// running a command, so a failure is reported once and the session carries on remembering in
/// memory.
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
    if let Err(e) = save(&path, &state.search_store.borrow()) {
        eprintln!("[heca] could not save the search history to {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::search::{SearchModel, SearchStore};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("heca-search-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("search-history.json")
    }

    /// Record, save, load — and the ordering that comes back is the one that went in.
    #[test]
    fn a_saved_store_comes_back_the_same() {
        let path = temp("roundtrip");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        let mut model = SearchModel::new("command", store.clone());
        model.record_run("clo", Some("close"));
        model.record_run("clo", Some("close"));
        model.record_run("spl", Some("split"));
        save(&path, &store.borrow()).expect("save");

        let restored = Rc::new(RefCell::new(load(&path, Caps::default())));
        let items = [("close", "Close Pane"), ("split", "Split Pane")];
        let before = SearchModel::new("command", store).rank(&items, "", |i| (Some(i.0), i.1));
        let after = SearchModel::new("command", restored.clone()).rank(&items, "", |i| (Some(i.0), i.1));
        assert_eq!(before, after, "the ranking survives a round trip");

        // …and so does the query history.
        let mut m = SearchModel::new("command", restored);
        match m.handle(heca_grid_ui::WidgetIntent::MenuHistoryUp, "") {
            heca_grid_ui::search::SearchAction::SetQuery(q) => assert_eq!(q, "spl"),
            other => panic!("expected the newest past query, got {other:?}"),
        }
    }

    /// **Nothing about this file may stop the app.** Missing, corrupt, truncated and from-the-future
    /// all mean the same thing: start empty.
    #[test]
    fn an_unusable_file_is_an_empty_store_not_an_error() {
        let path = temp("corrupt");
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");

        // Missing.
        assert!(load(&path, Caps::default()).scope("command").is_none());
        // Not JSON at all.
        std::fs::write(&path, "{{{ this is not json").expect("write");
        assert!(load(&path, Caps::default()).scope("command").is_none());
        // Valid JSON, truncated structure.
        std::fs::write(&path, r#"{"version":1,"scopes":{"command":{"history":"#).expect("write");
        assert!(load(&path, Caps::default()).scope("command").is_none());
        // A version this build does not know: discarded deliberately, not parsed leniently.
        std::fs::write(&path, r#"{"version":99,"scopes":{"command":{"history":["x"]}}}"#).expect("write");
        assert!(load(&path, Caps::default()).scope("command").is_none());
    }

    /// The file stays bounded however much is recorded.
    #[test]
    fn the_caps_hold_on_write() {
        let path = temp("caps");
        let store = Rc::new(RefCell::new(SearchStore::new()));
        let mut model = SearchModel::new("command", store.clone());
        for i in 0..200 {
            model.record_run(&format!("query{i}"), Some(&format!("id{i}")));
        }
        save(&path, &store.borrow()).expect("save");
        let text = std::fs::read_to_string(&path).expect("read");
        let parsed: Persisted = serde_json::from_str(&text).expect("parse");
        let scope = parsed.scopes.get("command").expect("the scope");
        assert!(scope.history.len() <= Caps::default().history, "history: {}", scope.history.len());
        assert!(scope.uses.len() <= Caps::default().usage, "uses: {}", scope.uses.len());
        assert_eq!(
            scope.history.last().map(String::as_str),
            Some("query199"),
            "the newest queries are the ones kept",
        );
    }

    /// The write is atomic: no temp file is left behind for the next load to trip over.
    #[test]
    fn saving_leaves_no_temp_file() {
        let path = temp("atomic");
        let store = SearchStore::new();
        save(&path, &store).expect("save");
        assert!(path.exists());
        assert!(!path.with_extension("json.tmp").exists(), "the temp file was renamed, not left");
    }
}
