//! One recoverable batch for native settings, owned files and the installation receipt.

use std::{collections::BTreeMap, fs, io, path::{Path, PathBuf}};

use serde::{Deserialize, Serialize};

use heca_config::state_file::{write_bytes, write_json};

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Edit {
    path: PathBuf,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
}

#[derive(Default)]
pub(super) struct Files {
    entries: BTreeMap<PathBuf, Edit>,
}

pub(super) fn read(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            Err(io::Error::other(format!("not a regular owned file: {}", path.display())))
        }
        Ok(_) => fs::read(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl Files {
    pub fn get(&mut self, path: &Path) -> io::Result<Option<&[u8]>> {
        if !self.entries.contains_key(path) {
            let before = read(path)?;
            self.entries.insert(path.to_owned(), Edit { path: path.to_owned(), after: before.clone(), before });
        }
        Ok(self.entries[path].after.as_deref())
    }

    pub fn set(&mut self, path: &Path, after: Option<Vec<u8>>) -> io::Result<()> {
        self.get(path)?;
        if let Some(entry) = self.entries.get_mut(path) { entry.after = after; }
        Ok(())
    }

    pub fn commit(self, journal: &Path) -> io::Result<()> {
        let edits: Vec<_> = self.entries.into_values().filter(|edit| edit.before != edit.after).collect();
        if edits.is_empty() { return Ok(()) }
        for edit in &edits {
            if read(&edit.path)? != edit.before {
                return Err(io::Error::other(format!("file changed during installation: {}", edit.path.display())));
            }
        }
        write_json(journal, &edits)?;
        for edit in &edits {
            if read(&edit.path)? != edit.before {
                recover(journal)?;
                return Err(io::Error::other(format!("file changed during installation: {}", edit.path.display())));
            }
            if let Err(error) = replace(&edit.path, edit.after.as_deref()) {
                // Recovery also checks that nothing else has edited a file since this batch.
                recover(journal)?;
                return Err(error);
            }
        }
        fs::remove_file(journal)
    }
}

fn replace(path: &Path, content: Option<&[u8]>) -> io::Result<()> {
    match content {
        Some(bytes) => write_bytes(path, bytes),
        None => match fs::remove_file(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        },
    }
}

pub(super) fn recover(journal: &Path) -> io::Result<()> {
    let Some(bytes) = read(journal)? else { return Ok(()) };
    let edits: Vec<Edit> = serde_json::from_slice(&bytes)?;
    for edit in &edits {
        let current = read(&edit.path)?;
        if current != edit.before && current != edit.after {
            return Err(io::Error::other(format!("cannot recover a file edited after installation: {}", edit.path.display())));
        }
    }
    for edit in edits.iter().rev() {
        let current = read(&edit.path)?;
        if current == edit.before { continue }
        if current != edit.after {
            return Err(io::Error::other(format!("file changed during recovery: {}", edit.path.display())));
        }
        replace(&edit.path, edit.before.as_deref())?;
    }
    fs::remove_file(journal)
}
