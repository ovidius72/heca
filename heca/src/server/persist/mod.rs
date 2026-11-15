//! **Keeping a server's sessions on disk**, so they come back when it starts again.
//!
//! The file is the server's own — `heca.sessions.json` in its identity folder
//! ([`ServerIdentity::own_file`]) — so two servers never write the same one. It is machine-written
//! state, with the rules every such file here follows (`heca_config::state_file`): it is rewritten
//! without asking, it may be deleted and lose only the convenience, a write is atomic, and a
//! missing, unreadable or newer file is never a failed start — the server starts with no sessions
//! and says why, once.
//!
//! This module owns the file and *when* it is written; [`file`] owns its shape; the server loop owns
//! what goes in it and what is done with what comes out.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use heca_core::layout::SessionId;

use super::identity::ServerIdentity;

mod file;
#[cfg(test)]
mod tests;

pub(crate) use file::{Focus, Saved, SavedSession};

/// What reading the file gave.
pub(crate) struct Loaded {
    pub saved: Saved,
    /// One line for each thing in the file that could not be used.
    pub problems: Vec<String>,
}

/// The server's saved-sessions file, and when it is next due to be written.
pub(crate) struct SessionStore {
    path: PathBuf,
    delay: Duration,
    /// The text the file holds now, as this server last read or wrote it.
    on_disk: Option<String>,
    /// When the next write is due, if anything may have changed since the last one.
    due: Option<Instant>,
    /// Sessions already reported as having no JSON form, so a bad one is said once, not each write.
    left_out: HashSet<SessionId>,
    write_failed: bool,
}

impl SessionStore {
    /// The store for the server `identity`, or `None` when sessions are not kept: the setting is
    /// off (a file left from before is removed, so it can never bring back what was deleted since),
    /// or the server has no folder of its own to keep one in.
    pub(crate) fn open(identity: &ServerIdentity, settings: &heca_config::settings::SettingsConfig) -> Option<Self> {
        let path = match identity.own_file("sessions.json") {
            Ok(path) => path,
            Err(why) => {
                if settings.save_sessions {
                    eprintln!("[heca] this server's sessions will not be saved: {why}");
                }
                return None;
            }
        };
        if !settings.save_sessions {
            if let Err(error) = std::fs::remove_file(&path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                eprintln!("[heca] could not remove {}: {error}", path.display());
            }
            return None;
        }
        Some(Self {
            path,
            delay: Duration::from_millis(settings.session_save_delay_ms),
            on_disk: None,
            due: None,
            left_out: HashSet::new(),
            write_failed: false,
        })
    }

    /// **Read the file.** Total: a file that is missing is no sessions; one that cannot be read at
    /// all — not JSON, or a version this build does not know — is no sessions and one problem, and
    /// is moved aside so the next write cannot destroy what a newer heca left there.
    pub(crate) fn load(&mut self) -> Loaded {
        let empty = |problems| Loaded { saved: Saved { next_session: 1, ..Saved::default() }, problems };
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return empty(Vec::new()),
            Err(error) => return empty(vec![format!("{} cannot be read ({error})", self.path.display())]),
        };
        match Saved::decode(&text) {
            Ok((saved, problems)) => {
                self.on_disk = Some(text);
                Loaded { saved, problems }
            }
            Err(why) => {
                let aside = aside(&self.path);
                let moved = match std::fs::rename(&self.path, &aside) {
                    Ok(()) => format!("; it was kept as {}", aside.display()),
                    Err(error) => format!("; it could not be moved aside ({error})"),
                };
                empty(vec![format!("{}: {why}{moved}", self.path.display())])
            }
        }
    }

    /// Say that something may have changed. The write is due `delay` from the first such word, and a
    /// word said while one is due changes nothing: a busy server writes once per delay, not per turn.
    pub(crate) fn arm(&mut self, now: Instant) {
        self.due.get_or_insert(now + self.delay);
    }

    /// Whether a write that was due is due now. Taking it disarms: nothing is due again until
    /// something is said to have changed.
    pub(crate) fn take_due(&mut self, now: Instant) -> bool {
        self.due.take_if(|due| now >= *due).is_some()
    }

    /// When the loop must wake to write, if a write is waiting.
    pub(crate) fn next_wake(&self) -> Option<Instant> {
        self.due
    }

    /// **Write `saved`**, atomically, unless the file already says exactly that.
    pub(crate) fn save(&mut self, saved: &Saved) {
        self.due = None;
        let (text, left_out) = saved.encode();
        for (id, why) in left_out {
            if self.left_out.insert(id) {
                eprintln!("[heca] session {} is not saved: {why}", id.0);
            }
        }
        // A file deleted while the server runs is written again, whatever this server last wrote.
        if self.on_disk.as_deref() == Some(&text) && self.path.exists() {
            return;
        }
        match heca_config::state_file::write_bytes(&self.path, text.as_bytes()) {
            Ok(()) => {
                self.on_disk = Some(text);
                self.write_failed = false;
            }
            Err(error) => {
                if !std::mem::replace(&mut self.write_failed, true) {
                    eprintln!("[heca] could not save sessions to {}: {error}", self.path.display());
                }
            }
        }
    }
}

/// Where a file nobody can read is kept: beside it, with `.unreadable` added.
fn aside(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(std::ffi::OsStr::to_owned).unwrap_or_default();
    name.push(".unreadable");
    path.with_file_name(name)
}
