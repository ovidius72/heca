//! Keep a live server's socket intact and reclaim only a stale socket.

use std::fs::File;
use std::io;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::{Incoming, UnixListener, UnixStream};
use std::path::{Path, PathBuf};

pub(super) struct ControlListener {
    socket: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
    // Held, never read: the lock goes when this file is closed.
    _lock: File,
}

impl ControlListener {
    pub(super) fn bind(path: &Path) -> io::Result<Self> {
        let lock = crate::server::private_fs::try_lock(&path.with_extension("lock"))?
            .ok_or_else(|| io::Error::new(io::ErrorKind::AddrInUse, "another server holds the socket"))?;
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
