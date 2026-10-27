use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct SocketPath(PathBuf);

impl SocketPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let directory = std::env::temp_dir().join(format!("heca-listener-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            match std::fs::create_dir(&directory) {
                Ok(()) => return Self(directory.join("test.sock")),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("test directory: {error}"),
            }
        }
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) { std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap(); }
}

#[test]
fn a_second_server_cannot_unlink_the_first() {
    let path = SocketPath::new();
    let _first = ControlListener::bind(&path.0).unwrap();
    assert!(ControlListener::bind(&path.0).is_err());
    assert!(UnixStream::connect(&path.0).is_ok());
}

#[test]
fn a_live_legacy_socket_without_a_lock_is_preserved() {
    let path = SocketPath::new();
    let _legacy = UnixListener::bind(&path.0).unwrap();
    assert!(ControlListener::bind(&path.0).is_err());
    assert!(UnixStream::connect(&path.0).is_ok());
}

#[test]
fn a_stale_socket_is_reclaimed_with_private_permissions() {
    let path = SocketPath::new();
    drop(UnixListener::bind(&path.0).unwrap());
    let listener = ControlListener::bind(&path.0).unwrap();
    assert!(UnixStream::connect(&path.0).is_ok());
    assert_eq!(std::fs::metadata(&path.0).unwrap().permissions().mode() & 0o777, 0o600);
    drop(listener);
    assert!(!path.0.exists());
}

#[test]
fn a_regular_file_is_not_treated_as_a_stale_socket() {
    let path = SocketPath::new();
    std::fs::write(&path.0, b"keep me").unwrap();
    assert!(ControlListener::bind(&path.0).is_err());
    assert_eq!(std::fs::read(&path.0).unwrap(), b"keep me");
}

#[test]
fn listener_drop_does_not_remove_a_replacement() {
    let path = SocketPath::new();
    let first = ControlListener::bind(&path.0).unwrap();
    std::fs::remove_file(&path.0).unwrap();
    let _replacement = UnixListener::bind(&path.0).unwrap();
    drop(first);
    assert!(UnixStream::connect(&path.0).is_ok());
}
