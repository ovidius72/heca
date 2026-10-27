//! Start or reconnect to the local server. No alternate process or terminal implementation.

use std::io;
use heca_config::settings::ConnectionConfig;
use crate::server::{ServerWaker, wire::ClientLink};

#[cfg(unix)]
pub(super) fn connect(config: ConnectionConfig, wake: ServerWaker) -> io::Result<Box<dyn ClientLink>> {
    use std::{process::{Command, Stdio}, os::unix::process::CommandExt, time::{Duration, Instant}};
    use crate::server::{socket::socket_path, wire::socket::SocketClient};
    let path = socket_path().ok_or_else(|| io::Error::other("no runtime folder for the server"))?;
    match SocketClient::connect(&path, config, wake.clone()) {
        Ok(client) => return Ok(Box::new(client)),
        Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {},
        Err(error) => return Err(error),
    }
    let child = Command::new(std::env::current_exe()?).arg("--server").arg(&path)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn()?;
    let exited = reap(child);
    let deadline = Instant::now() + Duration::from_millis(config.request_timeout_ms.get());
    loop {
        match SocketClient::connect(&path, config, wake.clone()) {
            Ok(client) => return Ok(Box::new(client)),
            Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {},
            Err(error) => return Err(error),
        }
        match exited.try_recv() {
            Ok(status) => return Err(io::Error::other(format!("server exited: {}", status?))),
            Err(std::sync::mpsc::TryRecvError::Empty) => {},
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return Err(io::Error::other("server watcher disconnected")),
        }
        if Instant::now() >= deadline { return Err(io::Error::new(io::ErrorKind::TimedOut, "server did not start")); }
        std::thread::sleep(Duration::from_millis(config.connect_retry_ms.get()));
    }
}

#[cfg(unix)]
fn reap(mut child: std::process::Child) -> std::sync::mpsc::Receiver<io::Result<std::process::ExitStatus>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    // The daemon outlives the client connection. Waiting must not own or close that connection.
    std::thread::spawn(move || { let _ = sender.send(child.wait()); });
    receiver
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn daemon_is_reaped_and_its_exit_status_is_reported() {
        use std::{process::Command, time::Duration};
        let status = reap(Command::new("/bin/sh").args(["-c", "exit 7"]).spawn().unwrap())
            .recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(status.code(), Some(7));
    }
}

#[cfg(not(unix))]
pub(super) fn connect(_: ConnectionConfig, _: ServerWaker) -> io::Result<Box<dyn ClientLink>> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "socket attachment is not available on this platform"))
}
