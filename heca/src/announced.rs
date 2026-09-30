//! **Things heca has already told the user, so it says each one once.**
//!
//! "`pro.show_notes` is available; add it to `title_actions` to show it" is worth saying the first
//! time it is true and never again after — repeating it on every start is noise, and saying it
//! nowhere leaves the thing undiscoverable. So each message has a key, and the key is remembered.
//!
//! Machine-written state, so it follows [`state_file`](crate::state_file): a missing or unreadable
//! file is a first run (everything is announced again, which is harmless), and it never stops a
//! launch.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Default)]
struct Persisted {
    version: u32,
    #[serde(default)]
    keys: BTreeSet<String>,
}

/// Whether `key` has never been announced — and remember that it now has. `true` means say it.
///
/// Reads and writes the file each time: it is called a handful of times at startup and reload, and
/// never per frame. A write that fails (read-only disk) still answers `true` — better to say it
/// twice than never.
pub(crate) fn first_time(key: &str) -> bool {
    match crate::state_file::data_path("announced.json") {
        Some(path) => first_time_in(&path, key),
        None => true,
    }
}

/// [`first_time`] against a chosen file — what a test uses.
pub(crate) fn first_time_in(path: &Path, key: &str) -> bool {
    let mut file = read(path);
    if !file.keys.insert(key.to_string()) {
        return false;
    }
    file.version = VERSION;
    let _ = crate::state_file::write_json(path, &file);
    true
}

/// The file at `path`, or an empty one — the single place a bad file becomes a fresh start.
fn read(path: &Path) -> Persisted {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Persisted>(&text).ok())
        .filter(|p| p.version == VERSION)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("heca-announced-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("announced.json")
    }

    /// **Said once, then never** — including by the next launch, which is a fresh read of the file.
    #[test]
    fn a_message_is_first_time_once_and_across_launches() {
        let path = temp("once");
        assert!(first_time_in(&path, "pane_button:pro.show_notes"));
        assert!(
            !first_time_in(&path, "pane_button:pro.show_notes"),
            "already said"
        );
        assert!(
            first_time_in(&path, "pane_button:pro.other"),
            "a different message"
        );
    }

    /// **A broken file is a first run, never a failure.**
    #[test]
    fn an_unreadable_file_announces_again_instead_of_failing() {
        let path = temp("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert!(first_time_in(&path, "k"));
        assert!(
            !first_time_in(&path, "k"),
            "and it is remembered from then on"
        );
    }
}
