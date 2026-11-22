//! One listener accepts the versioned attach protocol and the existing line RPC protocol.

use super::{listener::ControlListener, local::UnixStream};
use crate::server::{
    ServerWaker,
    runtime::RuntimeHandle,
    wire::socket::{SocketClient, SocketServer},
};
use heca_config::settings::ConnectionConfig;
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

// Protocol identity, not a user preference. It consumes exactly one line before framed messages,
// and the server answers it with one line: accepted, or refused because the versions differ. The
// number moves whenever what travels changes shape (2: terminals travel as the rows that changed).
const ATTACH_WORD: &[u8] = b"@heca-attach ";
const ATTACH_PREFACE: &[u8] = b"@heca-attach 2\n";
const ATTACH_ACCEPTED: &[u8] = b"@heca-attach ok\n";
const ATTACH_REFUSED: &[u8] = b"@heca-attach refused 2\n";
// `heca rpc --events`: the connection then stays open and receives one event line at a time.
pub(super) const EVENTS_PREFACE: &[u8] = b"@heca-events 1\n";

pub(crate) struct RuntimeListener {
    stop: Arc<AtomicBool>,
    path: PathBuf,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeListener {
    pub(crate) fn bind(
        path: &Path,
        handle: RuntimeHandle,
        config: ConnectionConfig,
    ) -> io::Result<Self> {
        let listener = ControlListener::bind(path)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::Builder::new()
            .name("heca-listener".into())
            .spawn(move || {
                let mut workers: Vec<JoinHandle<()>> = Vec::new();
                for stream in listener.incoming() {
                    if stopped.load(Ordering::Acquire) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    // Join finished handshakes while keeping accept independent of slow peers.
                    let mut i = 0;
                    while i < workers.len() {
                        if workers[i].is_finished() {
                            let _ = workers.swap_remove(i).join();
                        } else {
                            i += 1;
                        }
                    }
                    let handle = handle.clone();
                    if let Ok(worker) =
                        thread::Builder::new()
                            .name("heca-handshake".into())
                            .spawn(move || {
                                if let Err(error) = serve(stream, handle, config) {
                                    eprintln!("[heca] connection: {error}");
                                }
                            })
                    {
                        workers.push(worker);
                    }
                }
                for worker in workers {
                    let _ = worker.join();
                }
            })?;
        Ok(Self {
            stop,
            path: path.to_owned(),
            worker: Some(worker),
        })
    }
}

impl Drop for RuntimeListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake blocking accept; the sentinel connection is never handed to a protocol worker.
        let _ = UnixStream::connect(&self.path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn serve(
    mut stream: UnixStream,
    handle: RuntimeHandle,
    config: ConnectionConfig,
) -> io::Result<()> {
    let timeout = Duration::from_millis(config.request_timeout_ms.get());
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let line = read_line(&mut stream, config.max_frame_bytes.get())?;
    if line == ATTACH_PREFACE {
        stream.write_all(ATTACH_ACCEPTED)?;
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        let link = SocketServer::new(stream, config, handle.waker())?;
        return handle.connect(link);
    }
    if line.starts_with(ATTACH_WORD) {
        stream.write_all(ATTACH_REFUSED)?;
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            format!(
                "refused a window of another heca version ({})",
                String::from_utf8_lossy(line.trim_ascii_end())
            ),
        ));
    }
    if line == EVENTS_PREFACE {
        // A read-only stream: the server writes one event line at a time, for as long as the script
        // keeps the socket open. No read timeout — the connection is long-lived.
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        // Bounded like every other connection queue: a script that falls behind is dropped by the
        // server, never buffered without end.
        let (sender, lines) = mpsc::sync_channel::<String>(config.queue_capacity.get());
        handle.subscribe(sender)?;
        // Each event line goes to the socket. A failed write means the script left; the receiver is
        // then dropped and the server forgets this subscriber on its next event. The iterator also
        // ends when the server stops or drops this subscriber for falling behind (sender dropped).
        for line in lines {
            if writeln!(stream, "{line}").is_err() {
                break;
            }
        }
        return Ok(());
    }
    let line = String::from_utf8(line)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    // The terminator ends the request; it is not part of the line, so a line break left inside is refused.
    let line = crate::rpc::line::without_terminator(&line).to_string();
    let (reply, answer) = mpsc::channel();
    handle.rpc(line, reply)?;
    let answer = answer
        .recv_timeout(timeout)
        .map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error))?;
    writeln!(stream, "{answer}")
}

fn read_line(stream: &mut UnixStream, limit: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        if line.len() == limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request line exceeds connection limit",
            ));
        }
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        line.push(byte[0]);
        if byte[0] == b'\n' {
            return Ok(line);
        }
    }
}

impl SocketClient {
    pub(crate) fn connect(
        path: &Path,
        config: ConnectionConfig,
        wake: ServerWaker,
    ) -> io::Result<Self> {
        let mut stream = UnixStream::connect(path)?;
        let timeout = Some(Duration::from_millis(config.request_timeout_ms.get()));
        stream.set_write_timeout(timeout)?;
        stream.set_read_timeout(timeout)?;
        stream.write_all(ATTACH_PREFACE)?;
        let answer = read_line(&mut stream, config.max_frame_bytes.get())?;
        if answer != ATTACH_ACCEPTED {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "The heca server already running is another version of heca, so this window cannot \
                 talk to it and has changed nothing.\n\nTo use this version, stop the server with \
                 `heca --kill-server` in a terminal (closing the windows does not stop it) and open \
                 heca again. \
                 Your sessions come back with their workspaces, columns and panes; the programs \
                 that ran in them start again where `resume_programs` is on (the default), and \
                 everything else gets a fresh shell.\n\nTo try this version alongside the running \
                 one instead: `heca --standalone`.",
            ));
        }
        stream.set_write_timeout(None)?;
        stream.set_read_timeout(None)?;
        Self::new(stream, config, wake)
    }
}
