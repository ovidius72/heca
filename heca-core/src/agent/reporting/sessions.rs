//! **Which pane a native session reports to, for a hook that runs outside heca.**
//!
//! A program can run the work of a session in a process heca never started — Claude Code resumes a
//! background session in its own daemon, and only the front-end runs in the pane. That process's
//! hooks have no `HECA_AGENT_*` variables, so they used to report nothing: the pane showed the
//! front-end's first event forever. So when a hook that *does* have them reports a native session
//! id, the pane's reporting connection is remembered under that id; a hook without them looks its
//! session up and reports to the same pane. A pane that is gone has no file to write to, and its
//! entry is dropped on the next look-up.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::agent::AgentState;

/// One pane's reporting connection, as the hook that had heca's variables saw it.
#[derive(Serialize, Deserialize)]
pub(super) struct Connection {
    pub endpoint: PathBuf,
    pub events: BTreeMap<String, AgentState>,
}

/// The per-user folder the entries live in — owner-only, under the user's own temporary folder.
pub(super) fn folder() -> PathBuf {
    std::env::temp_dir().join("heca-agent-sessions")
}

/// The entry for `driver`'s session `id`, or `None` when the id is not a plain identifier — it is a
/// file name, so nothing in it may reach outside the folder.
fn entry(folder: &Path, driver: &str, id: &str) -> Option<PathBuf> {
    let plain = |text: &str| {
        !text.is_empty()
            && text.len() <= 128
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    (plain(driver) && plain(id)).then(|| folder.join(format!("{driver}.{id}.json")))
}

/// Remember that `driver`'s session `id` reports through `connection`.
pub(super) fn remember(
    folder: &Path,
    driver: &str,
    id: &str,
    connection: &Connection,
) -> io::Result<()> {
    let Some(path) = entry(folder, driver, id) else {
        return Ok(());
    };
    fs::create_dir_all(folder)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(folder, fs::Permissions::from_mode(0o700))?;
    }
    let text = serde_json::to_vec(connection)?;
    if fs::read(&path).ok().as_deref() == Some(text.as_slice()) {
        return Ok(());
    }
    let mut file = tempfile::NamedTempFile::new_in(folder)?;
    io::Write::write_all(&mut file, &text)?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}

/// Whether any session of `driver` was seen in a pane — a hook outside heca reads nothing otherwise.
pub(super) fn any_for(folder: &Path, driver: &str) -> bool {
    let prefix = format!("{driver}.");
    fs::read_dir(folder).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
    })
}

/// The connection `driver`'s session `id` reports through — `None` when no hook with heca's
/// variables named it, or its pane is gone (whose entry is then dropped).
pub(super) fn find(folder: &Path, driver: &str, id: &str) -> Option<Connection> {
    let path = entry(folder, driver, id)?;
    let connection: Connection = serde_json::from_slice(&fs::read(&path).ok()?).ok()?;
    if !connection.endpoint.exists() {
        let _ = fs::remove_file(&path);
        return None;
    }
    Some(connection)
}
