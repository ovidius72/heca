//! One listener accepts the versioned attach protocol and the existing line RPC protocol.

use std::{io::{self, Read, Write}, os::unix::net::UnixStream, path::{Path, PathBuf},
    sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc}, thread::{self, JoinHandle}, time::Duration};
use heca_config::settings::ConnectionConfig;
use crate::server::{runtime::RuntimeHandle, wire::socket::{SocketClient, SocketServer}, ServerWaker};
use super::listener::ControlListener;

// Protocol identity, not a user preference. It consumes exactly one line before framed messages.
const ATTACH_PREFACE: &[u8] = b"@heca-attach 1\n";

pub(crate) struct RuntimeListener {
    stop: Arc<AtomicBool>,
    path: PathBuf,
    worker: Option<JoinHandle<()>>,
}

impl RuntimeListener {
    pub(crate) fn bind(path: &Path, handle: RuntimeHandle, config: ConnectionConfig) -> io::Result<Self> {
        let listener = ControlListener::bind(path)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let worker = thread::Builder::new().name("heca-listener".into()).spawn(move || {
            let mut workers: Vec<JoinHandle<()>> = Vec::new();
            for stream in listener.incoming() {
                if stopped.load(Ordering::Acquire) { break; }
                let Ok(stream) = stream else { continue };
                // Join finished handshakes while keeping accept independent of slow peers.
                let mut i = 0;
                while i < workers.len() {
                    if workers[i].is_finished() { let _ = workers.swap_remove(i).join(); }
                    else { i += 1; }
                }
                let handle = handle.clone();
                if let Ok(worker) = thread::Builder::new().name("heca-handshake".into()).spawn(move || {
                    if let Err(error) = serve(stream, handle, config) {
                        eprintln!("[heca] connection: {error}");
                    }
                }) { workers.push(worker); }
            }
            for worker in workers { let _ = worker.join(); }
        })?;
        Ok(Self { stop, path: path.to_owned(), worker: Some(worker) })
    }
}

impl Drop for RuntimeListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Wake blocking accept; the sentinel connection is never handed to a protocol worker.
        let _ = UnixStream::connect(&self.path);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

fn serve(mut stream: UnixStream, handle: RuntimeHandle, config: ConnectionConfig) -> io::Result<()> {
    let timeout = Duration::from_millis(config.request_timeout_ms.get());
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    let line = read_line(&mut stream, config.max_frame_bytes.get())?;
    if line == ATTACH_PREFACE {
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        let link = SocketServer::new(stream, config, handle.waker())?;
        return handle.connect(link);
    }
    let line = String::from_utf8(line).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let (reply, answer) = mpsc::channel();
    handle.rpc(line, reply)?;
    let answer = answer.recv_timeout(timeout).map_err(|error| io::Error::new(io::ErrorKind::TimedOut, error))?;
    writeln!(stream, "{answer}")
}

fn read_line(stream: &mut UnixStream, limit: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        if line.len() == limit { return Err(io::Error::new(io::ErrorKind::InvalidData, "request line exceeds connection limit")); }
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        line.push(byte[0]);
        if byte[0] == b'\n' { return Ok(line); }
    }
}

impl SocketClient {
    pub(crate) fn connect(path: &Path, config: ConnectionConfig, wake: ServerWaker) -> io::Result<Self> {
        let mut stream = UnixStream::connect(path)?;
        stream.set_write_timeout(Some(Duration::from_millis(config.request_timeout_ms.get())))?;
        stream.write_all(ATTACH_PREFACE)?;
        stream.set_write_timeout(None)?;
        Self::new(stream, config, wake)
    }
}
