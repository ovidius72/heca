//! Keep a live server's socket intact and reclaim only a stale socket.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use super::local::{self, FileIdentity, UnixListener, UnixStream};

pub(super) struct ControlListener {
    socket: UnixListener,
    path: PathBuf,
    identity: FileIdentity,
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
                if !local::is_socket(path)? { return Err(error); }
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
        let listener = Self { socket, path: path.to_owned(), identity: local::identity(path)?, _lock: lock };
        local::restrict_to_owner(path)?;
        Ok(listener)
    }

    pub(super) fn incoming(&self) -> impl Iterator<Item = io::Result<UnixStream>> + '_ { self.socket.incoming() }
}

impl Drop for ControlListener {
    fn drop(&mut self) {
        if local::identity(&self.path).is_ok_and(|identity| identity == self.identity) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[path = "listener_tests.rs"]
mod tests;
