//! The window-free owner loop. Connection and PTY workers only wake it or send links.

#[cfg(unix)]
mod headless;
#[cfg(unix)]
pub(crate) use headless::serve;

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Instant;

use heca_config::settings::ConnectionConfig;

use super::{ServerWaker, run::ServerLoop, wire::ServerLink};

enum Event {
    Wake,
    Connect(Box<dyn ServerLink + Send>),
    Stop,
    Rpc { line: String, reply: mpsc::Sender<String> },
    Notification(crate::notification::NotificationDraft),
}

/// A handle to a server whose state never leaves its owner thread.
#[derive(Clone)]
pub(crate) struct RuntimeHandle {
    sender: SyncSender<Event>,
    outer_wake: Option<ServerWaker>,
    stopped: Arc<AtomicBool>,
}

pub(crate) struct RuntimeInbox(Receiver<Event>, Arc<AtomicBool>);

pub(crate) fn channel(config: ConnectionConfig) -> (RuntimeHandle, RuntimeInbox) {
    make_channel(config, None)
}

pub(crate) fn channel_waking(config: ConnectionConfig, wake: ServerWaker) -> (RuntimeHandle, RuntimeInbox) {
    make_channel(config, Some(wake))
}

fn make_channel(config: ConnectionConfig, outer_wake: Option<ServerWaker>) -> (RuntimeHandle, RuntimeInbox) {
    let (sender, receiver) = mpsc::sync_channel(config.queue_capacity.get());
    let stopped = Arc::new(AtomicBool::new(false));
    (RuntimeHandle { sender, outer_wake, stopped: stopped.clone() }, RuntimeInbox(receiver, stopped))
}

impl RuntimeHandle {
    pub(crate) fn waker(&self) -> ServerWaker {
        let handle = self.clone();
        ServerWaker::new(move || {
            if handle.stopped.load(Ordering::Acquire) { return; }
            // A full inbox already guarantees a loop turn; wakeups carry no state.
            let _ = handle.sender.try_send(Event::Wake);
            handle.notify_outer();
        })
    }

    pub(super) fn connect(&self, link: impl ServerLink + Send + 'static) -> io::Result<()> {
        self.send(Event::Connect(Box::new(link)))
    }

    pub(crate) fn stop(&self) -> io::Result<()> {
        self.stopped.store(true, Ordering::Release);
        let result = match self.sender.try_send(Event::Stop) {
            Ok(()) | Err(mpsc::TrySendError::Full(_)) => Ok(()),
            Err(mpsc::TrySendError::Disconnected(_)) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "server stopped")),
        };
        self.notify_outer();
        result
    }

    pub(super) fn rpc(&self, line: String, reply: mpsc::Sender<String>) -> io::Result<()> {
        self.send(Event::Rpc { line, reply })
    }

    pub(super) fn raise(&self, draft: crate::notification::NotificationDraft) -> io::Result<()> {
        self.send(Event::Notification(draft))
    }

    fn send(&self, event: Event) -> io::Result<()> {
        if self.stopped.load(Ordering::Acquire) { return Err(io::Error::new(io::ErrorKind::BrokenPipe, "server stopped")); }
        self.sender.try_send(event).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => io::Error::new(io::ErrorKind::WouldBlock, "server inbox full"),
            mpsc::TrySendError::Disconnected(_) => io::Error::new(io::ErrorKind::BrokenPipe, "server stopped"),
        })?;
        self.notify_outer();
        Ok(())
    }

    fn notify_outer(&self) {
        if let Some(wake) = &self.outer_wake { wake.wake(); }
    }
}

impl RuntimeInbox {
    /// Sleep until an event or the nearest server deadline, never on a fixed frame interval.
    pub(super) fn run(self, mut server: ServerLoop) {
        loop {
            if self.1.load(Ordering::Acquire) { break; }
            server.pump(Instant::now());
            // `heca --kill-server` asked the server to stop: end the loop so the process exits and
            // its socket, panes and terminals go with it. The pump above already dropped the clients
            // and backends.
            if server.wants_shutdown() { break; }
            let event = match server.next_wake() {
                Some(deadline) => match self.0.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(event) => Some(event),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match self.0.recv() {
                    Ok(event) => Some(event),
                    Err(_) => break,
                },
            };
            if let Some(event) = event && !Self::dispatch(event, &mut server) { break; }
        }
    }

    /// Standalone drains the same inbox from winit instead of sleeping in a second owner loop.
    pub(crate) fn pump(&self, server: &mut ServerLoop) {
        while let Ok(event) = self.0.try_recv() {
            if !Self::dispatch(event, server) { break; }
        }
        server.pump(Instant::now());
    }

    pub(crate) fn reject_pending(&self) {
        while let Ok(event) = self.0.try_recv() {
            if let Event::Rpc { reply, .. } = event { let _ = reply.send("error: server stopped".into()); }
        }
    }

    fn dispatch(event: Event, server: &mut ServerLoop) -> bool {
        match event {
            Event::Connect(link) => server.add_link(link),
            Event::Stop => return false,
            Event::Rpc { line, reply } => server.begin_rpc(line, reply),
            Event::Notification(draft) => server.raise(draft),
            Event::Wake => {},
        }
        true
    }
}

#[cfg(test)]
mod tests;
