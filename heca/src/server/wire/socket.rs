//! Socket workers own blocking I/O; the server only polls bounded queues.

use std::io;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use serde::{Serialize, de::DeserializeOwned};

use super::{ClientLink, ClientMsg, ServerLink, ServerMsg, frame, queue};
use crate::server::ServerWaker;

pub(crate) use heca_config::settings::ConnectionConfig as TransportLimits;

pub(crate) type SocketClient = SocketPeer<ClientMsg, ServerMsg>;
pub(crate) type SocketServer = SocketPeer<ServerMsg, ClientMsg>;

type Failure = Arc<Mutex<Option<(io::ErrorKind, String)>>>;

enum WriteRequest<T> { Message(T), Flush(mpsc::SyncSender<()>) }

impl<T: queue::Message> queue::Message for WriteRequest<T> {
    fn is_boundary(&self) -> bool {
        match self { Self::Message(message) => message.is_boundary(), Self::Flush(_) => true }
    }
    fn replaces(&self, older: &Self) -> bool {
        match (self, older) {
            (Self::Message(new), Self::Message(old)) => new.replaces(old),
            _ => false,
        }
    }
}

/// A typed end of the same protocol carried by the in-process pair.
pub(crate) struct SocketPeer<Outgoing, Incoming> {
    outgoing: Option<queue::Sender<WriteRequest<Outgoing>>>,
    incoming: queue::Receiver<Incoming>,
    failure: Failure,
    stream: UnixStream,
    workers: Vec<JoinHandle<()>>,
    flush_timeout: std::time::Duration,
}

impl<Outgoing, Incoming> SocketPeer<Outgoing, Incoming>
where
    Outgoing: Serialize + queue::Message + Send + 'static,
    Incoming: DeserializeOwned + queue::Message + Send + 'static,
{
    pub(crate) fn new(stream: UnixStream, limits: TransportLimits, wake: ServerWaker) -> io::Result<Self> {
        let (outgoing, outbound) = queue::channel::<WriteRequest<Outgoing>>(limits.queue_capacity);
        let (inbound, incoming) = queue::channel(limits.queue_capacity);
        let failure: Failure = Arc::default();
        let mut reader = stream.try_clone()?;
        let mut writer = stream.try_clone()?;
        let read_failure = failure.clone();
        let read_wake = wake.clone();
        let read_worker = thread::Builder::new()
            .name("heca-connection-reader".into())
            .spawn(move || {
                let result = (|| {
                    while let Some(message) = frame::read(&mut reader, limits.max_frame_bytes)? {
                        inbound.send(message)?;
                        read_wake.wake();
                    }
                    Err(io::Error::new(io::ErrorKind::BrokenPipe, "peer disconnected"))
                })();
                finish(result, &reader, &read_failure);
                read_wake.wake();
            })?;
        let write_failure = failure.clone();
        let write_worker = thread::Builder::new()
            .name("heca-connection-writer".into())
            .spawn(move || {
                let result = (|| {
                    while let Some(message) = outbound.recv()? {
                        match message {
                            WriteRequest::Message(message) => frame::write(&mut writer, &message, limits.max_frame_bytes)?,
                            WriteRequest::Flush(done) => { let _ = done.send(()); }
                        }
                    }
                    Ok(())
                })();
                finish(result, &writer, &write_failure);
                wake.wake();
            });
        let write_worker = match write_worker {
            Ok(worker) => worker,
            Err(error) => {
                let _ = stream.shutdown(Shutdown::Both);
                let _ = read_worker.join();
                return Err(error);
            }
        };
        Ok(Self {
            outgoing: Some(outgoing),
            incoming,
            failure,
            stream,
            workers: vec![read_worker, write_worker],
            flush_timeout: std::time::Duration::from_millis(limits.request_timeout_ms.get()),
        })
    }

    fn send_message(&self, message: Outgoing) -> io::Result<()> {
        self.connection_status()?;
        self.outgoing.as_ref().expect("sender lives until drop")
            .send(WriteRequest::Message(message))
    }

    fn flush_messages(&self) -> io::Result<()> {
        self.connection_status()?;
        let (done, written) = mpsc::sync_channel(1);
        self.outgoing.as_ref().ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "connection closed"))?
            .send(WriteRequest::Flush(done))?;
        written.recv_timeout(self.flush_timeout).map_err(|error| io::Error::new(
            if matches!(error, mpsc::RecvTimeoutError::Timeout) { io::ErrorKind::TimedOut } else { io::ErrorKind::BrokenPipe }, error))
    }

    fn poll_message(&self) -> io::Result<Option<Incoming>> {
        match self.incoming.poll() {
            Ok(Some(message)) => Ok(Some(message)),
            Ok(None) => self.connection_status().map(|()| None),
            Err(error) => {
                self.connection_status()?;
                Err(error)
            }
        }
    }

    fn connection_status(&self) -> io::Result<()> {
        match &*self.failure.lock().expect("connection error lock") {
            Some((kind, message)) => Err(io::Error::new(*kind, message.clone())),
            None => Ok(()),
        }
    }
}

fn finish(result: io::Result<()>, stream: &UnixStream, failure: &Failure) {
    if let Err(error) = result {
        let mut first = failure.lock().expect("connection error lock");
        if first.is_none() {
            *first = Some((error.kind(), error.to_string()));
        }
    }
    let _ = stream.shutdown(Shutdown::Both);
}

impl<Outgoing, Incoming> Drop for SocketPeer<Outgoing, Incoming> {
    fn drop(&mut self) {
        self.outgoing.take();
        let _ = self.stream.shutdown(Shutdown::Both);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl ClientLink for SocketClient {
    fn send(&self, message: ClientMsg) -> io::Result<()> { self.send_message(message) }
    fn flush(&self) -> io::Result<()> { self.flush_messages() }
    fn poll(&self) -> io::Result<Option<ServerMsg>> { self.poll_message() }
}

impl ServerLink for SocketServer {
    fn send(&self, message: ServerMsg) -> io::Result<()> { self.send_message(message) }
    fn poll(&self) -> io::Result<Option<ClientMsg>> { self.poll_message() }
}

#[cfg(test)]
#[path = "socket_tests.rs"]
mod tests;
