//! A socket file nobody listens on answers "not found" or "refused" — the two answers on which the
//! client starts the background server (heca/src/server/socket/connect.rs).
//!
//! Its own test binary on purpose: inside the library's tests, another test spawning a child process
//! can inherit a listening socket in the moment between `bind` and the close-on-exec flag, keeping
//! it open, so a connect to a "dead" socket would sometimes succeed.

use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(windows)]
use uds_windows::{UnixListener, UnixStream};

#[test]
fn a_socket_file_nobody_listens_on_is_refused() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("stale.sock");
    drop(UnixListener::bind(&path).unwrap());
    let kind = UnixStream::connect(&path).unwrap_err().kind();
    assert!(matches!(kind, ErrorKind::NotFound | ErrorKind::ConnectionRefused), "{kind:?}");
}
