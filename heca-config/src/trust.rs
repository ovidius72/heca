//! **Which project settings files the user has trusted.**
//!
//! A project's `.heca/config.toml` ships with the repository, so a cloned one could bind a key to a
//! command and have it apply the moment heca starts in that folder. It is therefore **ignored until
//! the user trusts it**. Trust is remembered **per folder together with a hash of the file's
//! content**: if the file changes the hash no longer matches, it is ignored again, and the user is
//! asked again.
//!
//! The list is machine-written state (`project-trust.json` in the data directory, written through
//! [`state_file`](crate::state_file)). A missing, corrupt or unknown-version file means **nothing is
//! trusted** — never a crash, never the other way round.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Version 2: SHA-256 hashes. Version 1 held FNV hashes, which are simply not trusted any more —
/// the safe direction.
const VERSION: u32 = 2;
const FILE: &str = "project-trust.json";

#[derive(Serialize, Deserialize, Default)]
struct Persisted {
    version: u32,
    /// Folder (as a string) → hash of the file content that was trusted.
    #[serde(default)]
    trusted: BTreeMap<String, String>,
}

/// The SHA-256 of a file's content, as hex. **Cryptographic on purpose**: whoever controls the
/// repository controls the file, so a hash that can be collided deliberately would let a malicious
/// version pass for the one the user trusted. Stable across toolchains.
pub fn content_hash(content: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(content.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read(path: &Path) -> Persisted {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Persisted>(&text).ok())
        .filter(|p| p.version == VERSION)
        .unwrap_or_default()
}

fn key(dir: &Path) -> String {
    dir.display().to_string()
}

/// Is the project file in `dir`, with content hash `hash`, trusted? `false` on any doubt.
pub fn is_trusted(dir: &Path, hash: &str) -> bool {
    crate::state_file::data_path(FILE).is_some_and(|p| is_trusted_in(&p, dir, hash))
}

/// Trust the project file in `dir` as it is now (`hash`). Returns whether it could be remembered.
pub fn trust(dir: &Path, hash: &str) -> bool {
    crate::state_file::data_path(FILE).is_some_and(|p| trust_in(&p, dir, hash))
}

/// Forget any trust for `dir`.
pub fn untrust(dir: &Path) -> bool {
    crate::state_file::data_path(FILE).is_some_and(|p| untrust_in(&p, dir))
}

/// [`is_trusted`] against a chosen state file — what a test uses.
pub fn is_trusted_in(state: &Path, dir: &Path, hash: &str) -> bool {
    read(state)
        .trusted
        .get(&key(dir))
        .is_some_and(|h| h == hash)
}

/// [`trust`] against a chosen state file.
pub fn trust_in(state: &Path, dir: &Path, hash: &str) -> bool {
    let mut file = read(state);
    file.version = VERSION;
    file.trusted.insert(key(dir), hash.to_string());
    crate::state_file::write_json(state, &file).is_ok()
}

/// [`untrust`] against a chosen state file.
pub fn untrust_in(state: &Path, dir: &Path) -> bool {
    let mut file = read(state);
    file.version = VERSION;
    file.trusted.remove(&key(dir));
    crate::state_file::write_json(state, &file).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn state(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("heca-trust-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(FILE)
    }

    /// **Untrusted by default; trusted once the user says so; ignored again when the file changes.**
    #[test]
    fn trust_is_per_folder_and_per_content() {
        let s = state("basic");
        let dir = Path::new("/work/project");
        let before = content_hash("[settings]\nmouse = false\n");
        assert!(
            !is_trusted_in(&s, dir, &before),
            "nothing is trusted to begin with"
        );
        assert!(trust_in(&s, dir, &before));
        assert!(is_trusted_in(&s, dir, &before), "trusted as it was");
        let edited = content_hash("[settings]\nmouse = true\n");
        assert_ne!(before, edited);
        assert!(
            !is_trusted_in(&s, dir, &edited),
            "an edited file is asked about again"
        );
        assert!(
            !is_trusted_in(&s, Path::new("/work/other"), &before),
            "trust is per folder"
        );
        assert!(untrust_in(&s, dir));
        assert!(!is_trusted_in(&s, dir, &before), "forgotten");
    }

    /// **A missing, corrupt or wrong-version state file trusts nothing and never fails.**
    #[test]
    fn the_hash_is_sha256() {
        assert_eq!(
            content_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn a_broken_state_file_trusts_nothing() {
        let s = state("broken");
        let dir = Path::new("/work/project");
        assert!(!is_trusted_in(&s, dir, "x"), "missing");
        std::fs::create_dir_all(s.parent().unwrap()).unwrap();
        std::fs::write(&s, "{ not json").unwrap();
        assert!(!is_trusted_in(&s, dir, "x"), "corrupt");
        std::fs::write(&s, r#"{"version":99,"trusted":{"/work/project":"x"}}"#).unwrap();
        assert!(!is_trusted_in(&s, dir, "x"), "unknown version");
        std::fs::write(&s, r#"{"version":1,"trusted":{"/work/project":"x"}}"#).unwrap();
        assert!(
            !is_trusted_in(&s, dir, "x"),
            "an old FNV-era entry is not trusted"
        );
    }
}
