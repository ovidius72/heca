//! **The shape of the saved-sessions file**, and the one place it is written to text and read back.
//!
//! One file holds every session, because what makes them a server — the next session number, which
//! one a window opens first — belongs to none of them and has to agree with all of them. Each
//! session is still read on its own: one that cannot be understood is reported and left out, and the
//! others come back.

use std::collections::HashSet;
use std::path::PathBuf;

use heca_core::layout::{PaneId, SessionContent, SessionId, ViewSnapshot};
use serde::{Deserialize, Serialize};

use crate::entry::service::SessionKey;
use crate::server::sessions::{RecordedTerminal, SessionStatus};

/// The layout of the file. A different number is a file this build does not understand.
pub(super) const VERSION: u32 = 1;

/// Where a window was looking when it last said so: the workspace it showed and the pane it had
/// focused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Focus {
    pub workspace: usize,
    pub pane: Option<PaneId>,
}

/// **One session, as plain data**: who it is, what it holds, and what ran in it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SavedSession {
    pub id: SessionId,
    pub key: SessionKey,
    pub name: String,
    pub folder: Option<PathBuf>,
    pub label: Option<String>,
    /// Whether it was running or stopped. A stopped one comes back stopped.
    pub status: SessionStatus,
    /// Workspaces, columns, panes, their shares and names — the content a window draws from.
    pub content: SessionContent,
    /// The last place a window said it was looking.
    #[serde(default)]
    pub focus: Option<Focus>,
    /// The last view a followed window reported.
    #[serde(default)]
    pub view: Option<ViewSnapshot>,
    /// What each terminal was started with, and the harness (driver and native session) in it.
    #[serde(default)]
    pub terminals: Vec<RecordedTerminal>,
}

/// **Every session a server has**, with what the server itself keeps.
#[derive(Clone, Debug, Default)]
pub(crate) struct Saved {
    /// The number the next session gets.
    pub next_session: u64,
    /// Which sessions were used, oldest first; a window with no name opens the last.
    pub recent: Vec<SessionId>,
    pub sessions: Vec<SavedSession>,
}

#[derive(Deserialize)]
struct Header {
    version: u32,
}

#[derive(Serialize)]
struct Written<'a> {
    version: u32,
    next_session: u64,
    recent: &'a [SessionId],
    sessions: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct Read {
    next_session: u64,
    #[serde(default)]
    recent: Vec<SessionId>,
    sessions: Vec<serde_json::Value>,
}

impl Saved {
    /// This as the text of the file, and what could not be written: a session whose data has no
    /// JSON form (a path that is not text) is left out of the file rather than stopping the rest.
    pub(super) fn encode(&self) -> (String, Vec<(SessionId, String)>) {
        let mut left_out = Vec::new();
        let sessions = self
            .sessions
            .iter()
            .filter_map(|session| match serde_json::to_value(session) {
                Ok(value) => Some(value),
                Err(error) => {
                    left_out.push((session.id, error.to_string()));
                    None
                }
            })
            .collect();
        let written = Written { version: VERSION, next_session: self.next_session, recent: &self.recent, sessions };
        let text = serde_json::to_string_pretty(&written).expect("a list of JSON values is JSON");
        (text, left_out)
    }

    /// The sessions in `text`, and a line for everything that was left out. `Err` is a file that
    /// cannot be read at all: not JSON, or a version this build does not know.
    pub(super) fn decode(text: &str) -> Result<(Self, Vec<String>), String> {
        let header: Header = serde_json::from_str(text).map_err(|error| format!("it is not readable ({error})"))?;
        if header.version != VERSION {
            return Err(format!("it is version {}, and this heca reads version {VERSION}", header.version));
        }
        let read: Read = serde_json::from_str(text).map_err(|error| format!("it is not readable ({error})"))?;
        let mut problems = Vec::new();
        let mut saved = Self { next_session: read.next_session, recent: read.recent, sessions: Vec::new() };
        let mut seen: Vec<(SessionId, String, SessionKey)> = Vec::new();
        for (index, value) in read.sessions.into_iter().enumerate() {
            let label = || format!("session {}", index + 1);
            let session = match serde_json::from_value::<SavedSession>(value) {
                Ok(session) => session,
                Err(error) => {
                    problems.push(format!("{} is not readable ({error})", label()));
                    continue;
                }
            };
            if let Err(why) = session.check() {
                problems.push(format!("session '{}' {why}", session.name));
                continue;
            }
            if seen.iter().any(|(id, name, key)| *id == session.id || *name == session.name || *key == session.key) {
                problems.push(format!("session '{}' repeats the number, name or key of another", session.name));
                continue;
            }
            seen.push((session.id, session.name.clone(), session.key));
            saved.sessions.push(session.cleaned());
        }
        Ok((saved, problems))
    }
}

impl SavedSession {
    /// Whether this is a session that can be rebuilt, or why not.
    fn check(&self) -> Result<(), String> {
        crate::server::sessions::validate_name(&self.name).map_err(|why| format!("has a bad name: {why}"))?;
        if self.folder.as_deref().is_some_and(|folder| !folder.is_absolute()) {
            return Err("has a folder that is not an absolute path".into());
        }
        if self.content.workspaces.is_empty() {
            return Err("has no workspace".into());
        }
        let mut panes = HashSet::new();
        for pane in self.panes() {
            if !panes.insert(pane) {
                return Err(format!("has pane {} twice", pane.0));
            }
        }
        Ok(())
    }

    /// Every pane the content holds, tiled and floating.
    pub(crate) fn panes(&self) -> impl Iterator<Item = PaneId> + '_ {
        self.content.workspaces.iter().flat_map(|workspace| {
            workspace
                .columns
                .iter()
                .flat_map(|column| column.panes.iter().map(|pane| pane.id))
                .chain(workspace.floating.iter().map(|floating| floating.pane.id))
        })
    }

    /// This without the terminals of panes the content no longer has — nothing would show them.
    fn cleaned(mut self) -> Self {
        let panes: HashSet<_> = self.panes().collect();
        self.terminals.retain(|terminal| match &terminal.owner {
            crate::app::backend_store::TerminalOwner::Pane(pane) => panes.contains(pane),
            crate::app::backend_store::TerminalOwner::Named(_) => true,
        });
        self
    }
}
