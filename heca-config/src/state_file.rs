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

/// Where heca keeps state called `file` — `~/.local/share/heca/<file>` on Linux,
/// `~/Library/Application Support/heca/<file>` on macOS. `None` when the platform has no answer, in
/// which case the state lives in memory only and nothing panics.
pub fn data_path(file: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("heca").join(file))
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
    use std::{fs::{self, OpenOptions}, io::Write, sync::atomic::{AtomicU64, Ordering}};
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
        let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        match options.open(&tmp) {
            Ok(file) => break (tmp, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        if let Ok(metadata) = fs::metadata(path) { file.set_permissions(metadata.permissions())?; }
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() { let _ = fs::remove_file(tmp); }
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
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_bytes(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o640);
    }

    #[test]
    fn independent_writers_cannot_rename_each_others_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("shared.json");
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let writers: Vec<_> = [b'a', b'b'].into_iter().map(|byte| {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || { barrier.wait(); write_bytes(path, &vec![byte; 131072]) })
            }).collect();
            for writer in writers { writer.join().unwrap().unwrap(); }
        });
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes == vec![b'a'; 131072] || bytes == vec![b'b'; 131072]);
    }
}
