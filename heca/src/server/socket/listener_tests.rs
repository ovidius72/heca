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
fn a_stale_socket_is_reclaimed() {
    let path = SocketPath::new();
    drop(UnixListener::bind(&path.0).unwrap());
    let listener = ControlListener::bind(&path.0).unwrap();
    assert!(UnixStream::connect(&path.0).is_ok());
    #[cfg(unix)]
    assert_eq!(std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&path.0).unwrap().permissions()) & 0o777, 0o600);
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

#[test]
fn connecting_to_nobody_is_not_found_or_refused() {
    // The client starts the background server on exactly these two answers (server/socket/connect.rs).
    let path = SocketPath::new();
    let kind = UnixStream::connect(&path.0).unwrap_err().kind();
    assert!(matches!(kind, io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused), "{kind:?}");
    drop(UnixListener::bind(&path.0).unwrap());
    let kind = UnixStream::connect(&path.0).unwrap_err().kind();
    assert!(matches!(kind, io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused), "{kind:?}");
}

#[test]
fn bytes_cross_in_both_directions_and_the_end_is_seen() {
    use std::io::{Read, Write};
    let path = SocketPath::new();
    let listener = ControlListener::bind(&path.0).unwrap();
    let mut client = UnixStream::connect(&path.0).unwrap();
    let mut server = listener.incoming().next().unwrap().unwrap();
    client.write_all(b"ping").unwrap();
    let mut got = [0; 4];
    server.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"ping");
    server.write_all(b"pong").unwrap();
    client.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"pong");
    drop(server);
    assert_eq!(client.read(&mut got).unwrap(), 0);
}

/// The reader thread of a wire link sits in `read` for as long as the connection lives; the link's
/// drop must be able to wake it by shutting the connection down from another thread.
#[test]
fn a_blocked_reader_wakes_when_the_connection_is_shut_down() {
    use std::io::Read;
    let (reader, _peer) = UnixStream::pair().unwrap();
    let shutter = reader.try_clone().unwrap();
    let (done, woke) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        let mut reader = reader;
        let mut byte = [0];
        let _ = done.send(reader.read(&mut byte));
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    shutter.shutdown(std::net::Shutdown::Both).unwrap();
    let read = woke.recv_timeout(std::time::Duration::from_secs(5)).expect("the blocked read did not return");
    assert!(matches!(read, Ok(0) | Err(_)));
    thread.join().unwrap();
}

#[test]
fn a_read_timeout_ends_a_read_that_gets_nothing() {
    use std::io::Read;
    let (mut quiet, _peer) = UnixStream::pair().unwrap();
    quiet.set_read_timeout(Some(std::time::Duration::from_millis(50))).unwrap();
    let kind = quiet.read(&mut [0]).unwrap_err().kind();
    assert!(matches!(kind, io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock), "{kind:?}");
}

#[test]
fn a_reader_and_a_writer_on_one_connection_do_not_hold_each_other_up() {
    use std::io::{Read, Write};
    let (a, mut b) = UnixStream::pair().unwrap();
    let mut writer = a.try_clone().unwrap();
    let mut reader = a;
    let blocked = std::thread::spawn(move || { let mut byte = [0]; reader.read_exact(&mut byte).map(|()| byte[0]) });
    std::thread::sleep(std::time::Duration::from_millis(100));
    writer.write_all(b"x").unwrap();
    let mut got = [0];
    b.read_exact(&mut got).unwrap();
    b.write_all(b"y").unwrap();
    assert_eq!(blocked.join().unwrap().unwrap(), b'y');
}
