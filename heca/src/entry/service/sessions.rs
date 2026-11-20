//! **The sessions a server holds, as a service reads them** — and the event that says they changed.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::SessionKey;

/// Whether a session's processes are running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    /// Running.
    Active,
    /// Stopped, kept for an explicit restart.
    Terminated,
}

/// **One session**, as a service sees it: the key that outlives the run, its name, its project
/// folder and whether it is running.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionEntry {
    key: SessionKey,
    name: String,
    folder: Option<PathBuf>,
    state: SessionState,
}

impl SessionEntry {
    pub(crate) fn new(key: SessionKey, name: String, folder: Option<PathBuf>, state: SessionState) -> Self {
        Self { key, name, folder, state }
    }

    /// The session's key — the same for its whole life. See [`SessionKey`].
    pub fn key(&self) -> SessionKey {
        self.key
    }

    /// The session's name, as the user sees it. It changes on a rename; the key does not.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// **The session's project folder**, or `None` for a session made without one.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Whether the session is running or stopped.
    pub fn state(&self) -> SessionState {
        self.state
    }
}

/// **Every session this server holds**, terminated ones included — the caller decides what to
/// show. Cheap to clone and to keep, and it can be sent to a service's worker threads.
///
/// ```ignore
/// // The other sessions open on the same project as the one this action ran in.
/// let mine = cx.session_key();
/// let siblings: Vec<_> = cx.sessions().same_project(mine).filter(|s| s.key() != mine).collect();
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sessions(Arc<[SessionEntry]>);

impl Sessions {
    pub(crate) fn new(entries: Vec<SessionEntry>) -> Self {
        Self(entries.into())
    }

    /// Every session, in the order the server holds them.
    pub fn iter(&self) -> impl Iterator<Item = &SessionEntry> {
        self.0.iter()
    }

    /// The session with this key, if it still exists.
    pub fn get(&self, key: SessionKey) -> Option<&SessionEntry> {
        self.0.iter().find(|entry| entry.key == key)
    }

    /// **The project folder of the session with this key** — `None` when the session is gone or
    /// was made without a folder.
    pub fn folder_of(&self, key: SessionKey) -> Option<&Path> {
        self.get(key).and_then(SessionEntry::folder)
    }

    /// **Every session open on `folder`.** Folders are compared as the file system resolves them,
    /// so a project reached through a symbolic link, or written with a trailing slash, is the same
    /// project.
    pub fn of_project<'a>(&'a self, folder: &Path) -> impl Iterator<Item = &'a SessionEntry> + 'a {
        let wanted = resolved(folder);
        self.0.iter().filter(move |entry| entry.folder().is_some_and(|f| resolved(f) == wanted))
    }

    /// **Every session on the same project as the session with this key**, that session included.
    /// Empty when the session is gone or has no folder.
    pub fn same_project(&self, key: SessionKey) -> impl Iterator<Item = &SessionEntry> + '_ {
        let folder = self.folder_of(key).map(Path::to_path_buf);
        self.0.iter().filter(move |entry| match (&folder, entry.folder()) {
            (Some(wanted), Some(theirs)) => resolved(theirs) == resolved(wanted),
            _ => false,
        })
    }
}

/// A folder as the file system resolves it, or as written when it cannot be resolved (gone, or no
/// permission) — two such folders then match only when written the same.
fn resolved(folder: &Path) -> PathBuf {
    folder.canonicalize().unwrap_or_else(|_| folder.components().collect())
}

/// What happened to a session, in a [`SessionEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionChange {
    /// A new session was made.
    Created,
    /// The session's name changed; its key did not.
    Renamed,
    /// Its processes were stopped; the session is kept for a restart.
    Terminated,
    /// A terminated session was started again.
    Restarted,
    /// The session is gone; the event carries it as it was just before.
    Deleted,
}

/// **`session.changed`** — a session was created, renamed, terminated, restarted or deleted.
///
/// It carries what happened, the session it happened to (as it was just before, for a deleted
/// one) and every session as they are now, so a listener does not read anything back.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionEvent {
    pub(crate) kind: SessionChange,
    pub(crate) session: SessionEntry,
    pub(crate) sessions: Sessions,
}

impl SessionEvent {
    /// What happened.
    pub fn kind(&self) -> SessionChange {
        self.kind
    }

    /// The session it happened to.
    pub fn session(&self) -> &SessionEntry {
        &self.session
    }

    /// Every session, after the change.
    pub fn sessions(&self) -> &Sessions {
        &self.sessions
    }
}

/// **An event on the server**, delivered to every service that listens for its name with
/// [`ServiceRegistration::on`](super::ServiceRegistration::on) — THE EVENTS: the same
/// `on("name", …)` heca's own components, extensions and plugins use.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ServerEvent {
    /// `session.changed`.
    SessionChanged(SessionEvent),
}

impl ServerEvent {
    /// Every name a listener can ask for, besides `"*"`.
    pub const NAMES: &'static [&'static str] = &["session.changed"];

    /// The event's name, as a listener asks for it.
    pub fn name(&self) -> &'static str {
        match self {
            ServerEvent::SessionChanged(_) => "session.changed",
        }
    }

    /// The session event, when this is `session.changed`.
    pub fn as_session(&self) -> Option<&SessionEvent> {
        match self {
            ServerEvent::SessionChanged(event) => Some(event),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, folder: Option<&Path>, state: SessionState) -> SessionEntry {
        SessionEntry::new(SessionKey::generate(), name.into(), folder.map(Path::to_path_buf), state)
    }

    #[test]
    fn a_session_reads_its_own_folder_and_none_without_one() {
        let dir = tempfile::tempdir().unwrap();
        let a = entry("a", Some(dir.path()), SessionState::Active);
        let b = entry("b", None, SessionState::Active);
        let sessions = Sessions::new(vec![a.clone(), b.clone()]);
        assert_eq!(sessions.folder_of(a.key()), Some(dir.path()));
        assert_eq!(sessions.folder_of(b.key()), None);
        assert_eq!(sessions.same_project(b.key()).count(), 0, "no folder, no project");
    }

    #[test]
    fn two_sessions_on_one_project_group_together_through_a_link_and_a_trailing_slash() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let other = dir.path().join("other");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&project, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&project, &link).unwrap();
        let mut slashed = project.clone().into_os_string();
        slashed.push("/");

        let a = entry("a", Some(&project), SessionState::Active);
        let b = entry("b", Some(&link), SessionState::Active);
        let c = entry("c", Some(Path::new(&slashed)), SessionState::Terminated);
        let d = entry("d", Some(&other), SessionState::Active);
        let sessions = Sessions::new(vec![a.clone(), b.clone(), c.clone(), d]);

        let names: Vec<_> = sessions.same_project(a.key()).map(SessionEntry::name).collect();
        assert_eq!(names, ["a", "b", "c"]);
        assert_eq!(sessions.of_project(&link).count(), 3);
        assert_eq!(sessions.get(c.key()).map(SessionEntry::state), Some(SessionState::Terminated),
            "a terminated session is listed, with its state");
    }
}
