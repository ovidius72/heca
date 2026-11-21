//! **Owner-only folders and lock files** — the two things a server keeps private on disk.
//!
//! The socket's runtime folder and a server's identity folders are both made here, so the rule
//! (owner only, never a symlink planted in their place) is written once. Owner-only is mode `0700` on
//! Unix and, on Windows, an access list with the current user's entry alone.

#[cfg(windows)]
mod acl;

use std::fs::{DirBuilder, File, OpenOptions};
use std::io;
use std::path::Path;

/// `folder` and any missing parents, readable by the owner alone — also when it was already
/// there and had been loosened. Refuses a `folder` that is a symlink or not a folder.
pub(crate) fn private_folder(folder: &Path) -> io::Result<()> {
    // The folders this call makes, innermost first: every one of them is the owner's.
    #[cfg(windows)]
    let made: Vec<&Path> = folder
        .ancestors()
        .take_while(|path| !path.as_os_str().is_empty() && !path.exists())
        .collect();
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)?;
    // `symlink_metadata`: a link to a folder must not pass for the folder.
    if !std::fs::symlink_metadata(folder)?.file_type().is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a plain folder (a symlink or a file)",
            folder.display()
        )));
    }
    #[cfg(unix)]
    std::fs::set_permissions(folder, std::os::unix::fs::PermissionsExt::from_mode(0o700))?;
    #[cfg(windows)]
    for path in made
        .iter()
        .rev()
        .chain((!made.contains(&folder)).then_some(&folder))
    {
        acl::restrict_to_current_user(path)?;
    }
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
        Err(error) if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() => {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Fail unless `path` is the owner's alone: mode `0700` on Unix; on Windows a protected access list
/// with the current user's entry and no other.
#[cfg(test)]
pub(crate) fn assert_owner_only(path: &Path) {
    #[cfg(unix)]
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(path).unwrap().permissions())
            & 0o777,
        0o700,
        "{path:?}"
    );
    #[cfg(windows)]
    assert_eq!(
        acl::entries(path).unwrap(),
        (true, vec![acl::current_user().unwrap()]),
        "{path:?}"
    );
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

    #[test]
    fn a_new_folder_is_the_owners_alone() {
        let scratch = tempfile::tempdir().unwrap();
        let folder = scratch.path().join("a").join("b");
        private_folder(&folder).unwrap();
        assert_owner_only(&folder);
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
        assert_owner_only(&folder);
    }

    /// A folder that was open to everyone gets the owner-only list when `private_folder` meets it.
    #[cfg(windows)]
    #[test]
    fn a_folder_open_to_others_is_tightened_back() {
        let scratch = tempfile::tempdir().unwrap();
        // The scratch folder inherits its list from the temp folder, which is not protected.
        assert_ne!(
            acl::entries(scratch.path()).unwrap(),
            (true, vec![acl::current_user().unwrap()])
        );
        private_folder(scratch.path()).unwrap();
        assert_owner_only(scratch.path());
    }

    /// The socket, its lock and a server's folders are made inside a private folder and take its list.
    #[cfg(windows)]
    #[test]
    fn what_is_made_inside_a_private_folder_is_the_owners_alone() {
        let scratch = tempfile::tempdir().unwrap();
        let folder = scratch.path().join("heca");
        private_folder(&folder).unwrap();
        let lock = try_lock(&folder.join("default.lock")).unwrap();
        assert!(lock.is_some());
        let inner = folder.join("inner");
        std::fs::create_dir(&inner).unwrap();
        assert_eq!(
            acl::entries(&inner).unwrap().1,
            vec![acl::current_user().unwrap()]
        );
        assert_eq!(
            acl::entries(&folder.join("default.lock")).unwrap().1,
            vec![acl::current_user().unwrap()]
        );
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
