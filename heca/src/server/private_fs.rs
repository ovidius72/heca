//! **Owner-only folders and lock files** — the two things a server keeps private on disk.
//!
//! The socket's runtime folder and a server's identity folders are both made here, so the rule
//! (owner only, never a symlink planted in their place) is written once.

use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::path::Path;

/// `folder` and any missing parents, readable by the owner alone (unix) — also when it was already
/// there and had been loosened. Refuses a `folder` that is a symlink or not a folder.
pub(crate) fn private_folder(folder: &Path) -> io::Result<()> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)?;
    // `symlink_metadata`: a link to a folder must not pass for the folder.
    if !std::fs::symlink_metadata(folder)?.file_type().is_dir() {
        return Err(io::Error::other(format!("{} is not a plain folder (a symlink or a file)", folder.display())));
    }
    #[cfg(unix)]
    std::fs::set_permissions(folder, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    Ok(())
}

/// Lock `path` exclusively, or `None` when another process holds it. The lock lives as long as the
/// returned file. The file is never deleted: removing it would let a second process lock a new file
/// of the same name while the first still holds the old one.
pub(crate) fn try_lock(path: &Path) -> io::Result<Option<File>> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let lock = options.open(path)?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(Some(lock)),
        Err(error) if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_symlink_at_the_folder_is_refused() {
        let scratch = tempfile::tempdir().unwrap();
        let real = scratch.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = scratch.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(private_folder(&link).is_err());
    }

    #[test]
    fn a_file_at_the_folder_is_refused() {
        let scratch = tempfile::tempdir().unwrap();
        let file = scratch.path().join("file");
        std::fs::write(&file, b"").unwrap();
        assert!(private_folder(&file).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_loosened_folder_is_tightened_back() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = tempfile::tempdir().unwrap();
        let folder = scratch.path().join("a").join("b");
        private_folder(&folder).unwrap();
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        private_folder(&folder).unwrap();
        assert_eq!(std::fs::metadata(&folder).unwrap().permissions().mode() & 0o777, 0o700);
    }

    #[test]
    fn a_second_lock_on_a_held_file_is_none() {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("x.lock");
        let first = try_lock(&path).unwrap();
        assert!(first.is_some());
        assert!(try_lock(&path).unwrap().is_none());
        drop(first);
        assert!(try_lock(&path).unwrap().is_some());
    }
}
