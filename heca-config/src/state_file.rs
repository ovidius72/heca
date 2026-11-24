//! **Machine-written state that lives beside the config, not in it.**
//!
//! Everything read from `~/.config/heca/` is configuration a person wrote and heca only reads.
//! What heca writes about itself — the search history, what it has already told the user — goes to
//! the platform's *data* directory, under these rules (the ones `search_state` started with):
//!
//! - it is rewritten without asking, so it must never surprise someone who edited it;
//! - it may be deleted at any time and lose nothing but convenience;
//! - a missing, corrupt or unknown-version file is a first run, never a failed launch;
//! - a write is atomic — a temp file, then a rename — so an interrupted one leaves the last good
//!   file rather than half of a new one.
//!
//! This is the one place those rules are written; each kind of state owns only its own shape.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// **The environment variable that moves heca's data folder** — set, heca writes its state there
/// instead of the platform's data folder, and HOME stays what it is (a test run keeps the user's
/// real environment for the programs it starts). Not passed on to programs in panes.
pub const DATA_DIR_ENV: &str = "HECA_DATA_DIR";

/// The same for the config folder (`crate::loader::config_dir`).
pub const CONFIG_DIR_ENV: &str = "HECA_CONFIG_DIR";

/// **The variables that belong to this heca and to no program it starts** — removed from every
/// pane's environment, so a heca started inside a pane uses the normal folders.
pub const HOST_ONLY_ENV: &[&str] = &[CONFIG_DIR_ENV, DATA_DIR_ENV];

/// A folder named by environment variable `name`, when it is set and not empty.
pub fn folder_from_env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// **heca's data folder** — `HECA_DATA_DIR` when set, else `~/.local/share/heca` on Linux and
/// `~/Library/Application Support/heca` on macOS. `None` when the platform has no answer.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_given(folder_from_env(DATA_DIR_ENV))
}

/// [`data_dir`] with the variable's value given, so it can be tested without touching the
/// process's environment.
fn data_dir_given(from_env: Option<PathBuf>) -> Option<PathBuf> {
    from_env.or_else(|| dirs::data_dir().map(|d| d.join("heca")))
}

/// Where heca keeps state called `file`, in [`data_dir`]. `None` when there is no data folder, in
/// which case the state lives in memory only and nothing panics.
pub fn data_path(file: &str) -> Option<PathBuf> {
    data_dir().map(|d| d.join(file))
}

/// Write `value` as JSON at `path`, atomically.
pub fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    write_bytes(path, text.as_bytes())
}

/// Replace a file atomically, preserving its permissions. New files are private to their owner.
///
/// A unique temporary name lets independent state writers use this same operation safely.
pub fn write_bytes(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let (tmp, mut file) = loop {
        let tmp = path.with_extension(format!(
            "{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match options.open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        if let Ok(metadata) = fs::metadata(path) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn atomic_replacement_keeps_existing_permissions_and_new_files_are_private() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("owned.json");
        write_bytes(&path, b"first").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_bytes(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[test]
    fn independent_writers_cannot_rename_each_others_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shared.json");
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let writers: Vec<_> = b"ab"
                .iter()
                .copied()
                .map(|byte| {
                    let path = &path;
                    let barrier = &barrier;
                    scope.spawn(move || {
                        barrier.wait();
                        write_bytes(path, &vec![byte; 131072])
                    })
                })
                .collect();
            for writer in writers {
                writer.join().unwrap().unwrap();
            }
        });
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes == vec![b'a'; 131072] || bytes == vec![b'b'; 131072]);
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;

    /// **`HECA_DATA_DIR` moves the data folder, and only when it says something** — set, every
    /// state file goes there; unset (or empty), the platform's data folder stays.
    #[test]
    fn the_data_folder_follows_its_variable() {
        let moved = PathBuf::from("/tmp/heca-test-data");
        assert_eq!(data_dir_given(Some(moved.clone())), Some(moved));
        assert_eq!(
            data_dir_given(None),
            dirs::data_dir().map(|d| d.join("heca")),
            "unset: the platform's folder"
        );
        assert_eq!(
            std::env::var_os("HECA_T588_NEVER_SET").map(PathBuf::from),
            None
        );
        assert_eq!(
            folder_from_env("HECA_T588_NEVER_SET"),
            None,
            "an unset variable names nothing"
        );
    }

    /// The variables that move the folders are the ones kept from programs in panes.
    #[test]
    fn the_folder_variables_are_kept_from_panes() {
        assert!(HOST_ONLY_ENV.contains(&DATA_DIR_ENV));
        assert!(HOST_ONLY_ENV.contains(&CONFIG_DIR_ENV));
    }
}
