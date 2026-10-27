//! Keep a live server's socket intact and reclaim only a stale socket.

use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{Incoming, UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub(super) struct ControlListener {
    socket: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
    // Keep the lock file inode stable: deleting it would let a second binder lock a different file.
    _lock: File,
}

impl ControlListener {
    pub(super) fn bind(path: &Path) -> io::Result<Self> {
        let lock = OpenOptions::new().read(true).write(true).create(true)
            .truncate(false).mode(0o600).open(path.with_extension("lock"))?;
        fs2::FileExt::try_lock_exclusive(&lock)?;
        let socket = match UnixListener::bind(path) {
            Ok(socket) => socket,
            Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
                let metadata = std::fs::symlink_metadata(path)?;
                if !metadata.file_type().is_socket() { return Err(error); }
                match UnixStream::connect(path) {
                    Err(probe) if probe.kind() == io::ErrorKind::ConnectionRefused => {
                        std::fs::remove_file(path)?;
                        UnixListener::bind(path)?
                    }
                    // Reachable, inaccessible, or otherwise uncertain: never unlink it.
                    _ => return Err(error),
                }
            }
            Err(error) => return Err(error),
        };
        let metadata = std::fs::symlink_metadata(path)?;
        let listener = Self { socket, path: path.to_owned(), identity: (metadata.dev(), metadata.ino()), _lock: lock };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    pub(super) fn incoming(&self) -> Incoming<'_> { self.socket.incoming() }
}

impl Drop for ControlListener {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
