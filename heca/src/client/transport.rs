//! Start or reconnect to the local server. No alternate process or terminal implementation.

use std::io;
use heca_config::settings::ConnectionConfig;
use crate::server::{ServerWaker, wire::ClientLink};

pub(super) fn connect(config: ConnectionConfig, wake: ServerWaker) -> io::Result<Box<dyn ClientLink>> {
    use std::{process::{Command, Stdio}, time::{Duration, Instant}};
    use crate::server::{socket::{local::detach, socket_path}, wire::socket::SocketClient};
    let path = socket_path().ok_or_else(|| io::Error::other("no runtime folder for the server"))?;
    match SocketClient::connect(&path, config, wake.clone()) {
        Ok(client) => return Ok(Box::new(client)),
        Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {},
        Err(error) => return Err(error),
    }
    let child = detach(Command::new(std::env::current_exe()?).arg("--server").arg(&path)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())).spawn()?;
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

fn reap(mut child: std::process::Child) -> std::sync::mpsc::Receiver<io::Result<std::process::ExitStatus>> {
    let (sender, receiver) = std::sync::mpsc::channel();
    // The daemon outlives the client connection. Waiting must not own or close that connection.
    std::thread::spawn(move || { let _ = sender.send(child.wait()); });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_is_reaped_and_its_exit_status_is_reported() {
        use std::{process::Command, time::Duration};
        let exit_7 = if cfg!(windows) { ("cmd", ["/C", "exit 7"]) } else { ("/bin/sh", ["-c", "exit 7"]) };
        let status = reap(Command::new(exit_7.0).args(exit_7.1).spawn().unwrap())
            .recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        assert_eq!(status.code(), Some(7));
    }
}
