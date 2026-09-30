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
pub(crate) fn data_path(file: &str) -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join("heca").join(file))
}

/// Write `value` as JSON at `path`, atomically.
pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}
