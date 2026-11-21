//! **The saved-state store** — every extension's entries, global and per session, kept by the
//! server, which is the only writer. See [`crate::entry::service::saved`] for what an extension sees.
//!
//! The global entries are one file in the server's own folder (`heca.saved-state.json`), written at
//! once on every change, atomically, by `heca_config::state_file`; a session's entries travel with
//! the session's own save (`SavedSession::saved_state`) and are restored with it.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::entry::service::{Scope, SavedError, SessionKey, StateEvent};

/// Entries by full key (`<extension>.<key>`), then by id.
pub(crate) type Entries = BTreeMap<String, BTreeMap<String, serde_json::Value>>;

/// The version of the global file. Another version is read as no entries (a first run).
const FILE_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
struct File {
    version: u32,
    entries: Entries,
}

/// Every saved entry the server holds, and the changes not yet announced.
#[derive(Default)]
pub(crate) struct SavedStore {
    global: Entries,
    sessions: HashMap<SessionKey, Entries>,
    /// Where the global entries are written; `None` writes nothing (a server with no folder, a test).
    file: Option<PathBuf>,
    /// The largest value one entry may hold, in bytes of JSON.
    limit: usize,
    changes: Vec<StateEvent>,
    /// The extensions whose entries changed since the last call — their windows are shown again.
    touched: std::collections::BTreeSet<String>,
}

/// One extension's view of the store for one call: its name (the namespace) and the session the
/// call is in, if any.
pub(crate) struct Access<'a> {
    pub(crate) store: &'a std::cell::RefCell<SavedStore>,
    pub(crate) extension: &'a str,
    pub(crate) session: Option<SessionKey>,
}

impl SavedStore {
    /// The store, with the global entries read from `file` — missing, corrupt or another version
    /// is no entries.
    pub(crate) fn open(file: Option<PathBuf>, limit: usize) -> Self {
        let global = file
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<File>(&bytes).ok())
            .filter(|file| file.version == FILE_VERSION)
            .map(|file| file.entries)
            .unwrap_or_default();
        Self { global, sessions: HashMap::new(), file, limit, changes: Vec::new(), touched: Default::default() }
    }

    fn entries(&self, session: Option<SessionKey>) -> Option<&Entries> {
        match session {
            None => Some(&self.global),
            Some(key) => self.sessions.get(&key),
        }
    }

    fn entries_mut(&mut self, session: Option<SessionKey>) -> &mut Entries {
        match session {
            None => &mut self.global,
            Some(key) => self.sessions.entry(key).or_default(),
        }
    }

    /// A session's entries, for its save.
    pub(crate) fn session_entries(&self, session: SessionKey) -> Entries {
        self.sessions.get(&session).cloned().unwrap_or_default()
    }

    /// A restored session brings its entries back.
    pub(crate) fn adopt_session(&mut self, session: SessionKey, entries: Entries) {
        if entries.is_empty() {
            self.sessions.remove(&session);
        } else {
            self.sessions.insert(session, entries);
        }
    }

    /// A deleted session's entries go with it.
    pub(crate) fn forget_session(&mut self, session: SessionKey) {
        self.sessions.remove(&session);
    }

    /// The changes made since the last call, to announce as `state.changed`.
    pub(crate) fn take_changes(&mut self) -> Vec<StateEvent> {
        std::mem::take(&mut self.changes)
    }

    /// The extensions whose entries changed since the last call.
    pub(crate) fn take_touched(&mut self) -> std::collections::BTreeSet<String> {
        std::mem::take(&mut self.touched)
    }

    fn write_global(&self) {
        if let Some(path) = &self.file
            && let Err(error) = heca_config::state_file::write_json(path, &File { version: FILE_VERSION, entries: self.global.clone() })
        {
            eprintln!("[heca] saved state not written ({}): {error}", path.display());
        }
    }
}

impl Access<'_> {
    /// The session `scope` names, `None` for global — or `NoSession`.
    fn session(&self, scope: Scope) -> Result<Option<SessionKey>, SavedError> {
        match scope {
            Scope::Global => Ok(None),
            Scope::Session => self.session.map(Some).ok_or(SavedError::NoSession),
            Scope::SessionOf(key) => Ok(Some(key)),
        }
    }

    fn full(&self, key: &str) -> String {
        format!("{}.{key}", self.extension)
    }

    /// The entry, as `T` — `None` when it is not there, or no longer fits `T`.
    pub(crate) fn get<T: serde::de::DeserializeOwned>(&self, scope: Scope, key: &str, id: &str) -> Option<T> {
        let session = self.session(scope).ok()?;
        let store = self.store.borrow();
        let value = store.entries(session)?.get(&self.full(key))?.get(id)?.clone();
        serde_json::from_value(value).ok()
    }

    /// Save the entry, written at once (global) or with the session's next save.
    pub(crate) fn set<T: serde::Serialize>(&self, scope: Scope, key: &str, id: &str, value: &T) -> Result<(), SavedError> {
        let session = self.session(scope)?;
        let value = serde_json::to_value(value).map_err(|error| SavedError::Encode(error.to_string()))?;
        let bytes = serde_json::to_vec(&value).map_or(0, |bytes| bytes.len());
        let mut store = self.store.borrow_mut();
        if bytes > store.limit {
            return Err(SavedError::TooLarge { bytes, limit: store.limit });
        }
        let full = self.full(key);
        store.entries_mut(session).entry(full.clone()).or_default().insert(id.to_owned(), value);
        store.changes.push(StateEvent { session, key: full, id: id.to_owned() });
        store.touched.insert(self.extension.to_owned());
        if session.is_none() {
            store.write_global();
        }
        Ok(())
    }

    /// Forget the entry. Forgetting one that is not there changes nothing and announces nothing.
    pub(crate) fn remove(&self, scope: Scope, key: &str, id: &str) -> Result<(), SavedError> {
        let session = self.session(scope)?;
        let full = self.full(key);
        let mut store = self.store.borrow_mut();
        let entries = store.entries_mut(session);
        let removed = entries.get_mut(&full).and_then(|ids| ids.remove(id)).is_some();
        if entries.get(&full).is_some_and(BTreeMap::is_empty) {
            entries.remove(&full);
        }
        if removed {
            store.changes.push(StateEvent { session, key: full, id: id.to_owned() });
        store.touched.insert(self.extension.to_owned());
            if session.is_none() {
                store.write_global();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn access<'a>(store: &'a RefCell<SavedStore>, extension: &'a str, session: Option<SessionKey>) -> Access<'a> {
        Access { store, extension, session }
    }

    #[test]
    fn an_entry_is_read_back_by_its_owner_only_and_announced() {
        let store = RefCell::new(SavedStore::open(None, 1024));
        let session = SessionKey::generate();
        let rooms = access(&store, "heca-pro", Some(session));
        rooms.set(Scope::Session, "room-order", "", &vec!["b", "a"]).unwrap();
        assert_eq!(rooms.get::<Vec<String>>(Scope::Session, "room-order", ""), Some(vec!["b".into(), "a".into()]));
        assert_eq!(access(&store, "other", Some(session)).get::<Vec<String>>(Scope::Session, "room-order", ""), None,
            "another extension cannot read it");
        assert_eq!(access(&store, "heca-pro", Some(SessionKey::generate())).get::<Vec<String>>(Scope::Session, "room-order", ""),
            None, "another session has its own");
        assert_eq!(store.borrow_mut().take_changes(),
            vec![StateEvent { session: Some(session), key: "heca-pro.room-order".into(), id: String::new() }]);
        assert_eq!(rooms.get::<u32>(Scope::Session, "room-order", ""), None, "a value that no longer fits T reads as nothing");
    }

    #[test]
    fn a_call_in_no_session_names_one_and_a_large_value_is_refused() {
        let store = RefCell::new(SavedStore::open(None, 8));
        let service = access(&store, "x", None);
        assert_eq!(service.set(Scope::Session, "k", "", &1), Err(SavedError::NoSession));
        assert!(matches!(service.set(Scope::Global, "k", "", &"a long string"), Err(SavedError::TooLarge { .. })));
        service.set(Scope::SessionOf(SessionKey::generate()), "k", "", &1).unwrap();
    }

    #[test]
    fn global_entries_survive_a_restart_and_a_bad_file_is_a_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("heca.saved-state.json");
        let store = RefCell::new(SavedStore::open(Some(path.clone()), 1024));
        access(&store, "x", None).set(Scope::Global, "theme", "", &"dark").unwrap();
        let again = RefCell::new(SavedStore::open(Some(path.clone()), 1024));
        assert_eq!(access(&again, "x", None).get::<String>(Scope::Global, "theme", ""), Some("dark".into()));
        std::fs::write(&path, b"not json").unwrap();
        let broken = RefCell::new(SavedStore::open(Some(path), 1024));
        assert_eq!(access(&broken, "x", None).get::<String>(Scope::Global, "theme", ""), None);
    }
}
