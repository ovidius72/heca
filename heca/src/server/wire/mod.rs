//! **The client ↔ server protocol** (F012, P099/T544 C2).
//!
//! A client sends [`ClientMsg`]; the server sends [`ServerMsg`]. Both ends are written against this
//! one vocabulary whatever carries it: in standalone an in-process channel pair ([`in_process`])
//! moves the values directly, with no serialization; over the socket the same values are
//! framed bytes. The server and client code never know which is under them — that is what keeps
//! standalone and an attached window one code path (decision d108b88c).
//!
//! Terminal snapshots, the per-pane input and the event subscription are added with the attach
//! protocol (T544 C5 / T546); this is the layout/action spine both ends share.

// Not yet called from the running app — the entry wiring (the next C4/C5 step) consumes it.
#![allow(dead_code)]

#[cfg(test)]
use std::sync::mpsc::channel;
use std::io;

mod frame;
mod queue;
pub(crate) mod terminal;
pub(crate) mod image;
#[cfg(unix)]
pub(crate) mod socket;

use heca_core::layout::SessionContent;

use super::{Asker, Change, Refusal, ServerWaker};
use crate::input::WmAction;

/// What a client asks the server to do.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ClientMsg {
    /// Attach to the session; the server replies with [`ServerMsg::Attached`].
    Attach,
    /// Detach — the window is going away; the session and its programs keep running.
    Detach,
    /// Run a server action, carrying this window's own view facts (the server holds no per-client
    /// view, decision 2c249a29, so each request brings its own [`Asker`]).
    Act { action: WmAction, asker: Asker, #[serde(default)] request_id: Option<u64>,
        #[serde(default)] terminal_owner: Option<crate::app::backend_store::TerminalOwner> },
    EnsureTerminal {
        owner: crate::app::backend_store::TerminalOwner,
        spec: crate::app::backend_store::TerminalSpec,
        cell: (f32, f32),
    },
    Terminal {
        owner: crate::app::backend_store::TerminalOwner,
        input: terminal::ProcessInput,
    },
    Viewport { owner: crate::app::backend_store::TerminalOwner, offset: usize },
    RpcReply { request_id: u64, answer: String },
    /// Raise/dismiss/hover a notification — the server owns the queue, timers and dedup and sends
    /// back a [`ServerMsg::Notifications`] projection.
    Notify(crate::server::NotificationCommand),
    /// Config changed on disk (`prefix+Shift+r`): the server re-reads it and re-applies its own
    /// terminal and notification settings. The window never hands settings over — it keeps only
    /// its own view-side config.
    Reload,
    Invoke { intent: crate::chrome::Intent },
    ImageInfo { id: u64 },
    ImageChunk { id: u64, frame: usize, offset: usize },
    Read { request_id: u64, owner: crate::app::backend_store::TerminalOwner,
        start: isize, end: isize, cols: usize, query: Option<String> },
}

/// What the server tells a client.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum ServerMsg {
    /// The whole shared content, on attach — the client rebuilds a `Session` from it to draw
    /// ([`SessionContent::rebuild`]).
    Attached { content: SessionContent },
    /// Something changed in the shared content; the client reacts to it for itself.
    Changed(Change),
    /// The server refused an action, and why.
    Refused(Refusal),
    /// Process output changed, or a process emitted a client-side effect.
    BackendEffects(super::BackendPoll),
    Terminal(Box<terminal::TerminalFrame>),
    Content { content: SessionContent },
    /// The notifications on show, as plain serializable data — the window builds its own toasts
    /// from them. Sent on attach and whenever the visible set changes; a newer one replaces a
    /// still-queued older one (it is a whole snapshot).
    Notifications(Vec<crate::notification::NotificationView>),
    Reply { request_id: u64, changes: Vec<Change> },
    Rpc { request_id: u64, line: String },
    Dispatch { intents: Vec<crate::chrome::Intent> },
    ImageInfo(image::ImageInfo),
    ImageChunk(image::ImageChunk),
    ResourceUnavailable { id: u64, reason: String },
    ProcessExited { pane: heca_core::layout::PaneId, code: Option<i32> },
    Read { request_id: u64, start: isize, data: terminal::ReadData },
    TerminalClosed { owner: crate::app::backend_store::TerminalOwner },
}

/// The client's end of the link. The client is written against this, so the socket transport (later)
/// drops in behind the same two calls.
pub(crate) trait ClientLink: Send {
    /// Send a message to the server.
    fn send(&self, msg: ClientMsg) -> io::Result<()>;
    /// Take the next message from the server, if one is waiting. Never blocks.
    fn poll(&self) -> io::Result<Option<ServerMsg>>;
}

/// The server's end, independent of the transport used by the client.
pub(crate) trait ServerLink {
    fn poll(&self) -> io::Result<Option<ClientMsg>>;
    fn send(&self, msg: ServerMsg) -> io::Result<()>;
}

/// The client half of an in-process pair.
pub(crate) struct InProcessClient {
    to_server: WakeSender<ClientMsg>,
    from_server: queue::Receiver<ServerMsg>,
}

/// The server half of an in-process pair: what the server loop drains and answers for this client.
pub(crate) struct InProcessServer {
    from_client: queue::Receiver<ClientMsg>,
    to_client: WakeSender<ServerMsg>,
}

/// Sending and closing a channel both wake its reader, including an idle headless loop.
struct WakeSender<T> {
    sender: Option<queue::Sender<T>>,
    wake: ServerWaker,
}

impl<T: queue::Message> WakeSender<T> {
    fn send(&self, message: T) -> io::Result<()> {
        let Some(sender) = &self.sender else {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "channel closed"));
        };
        sender.send(message)?;
        self.wake.wake();
        Ok(())
    }
}

impl<T> Drop for WakeSender<T> {
    fn drop(&mut self) {
        // Disconnect before waking so a reader cannot observe an empty but still-live channel.
        self.sender.take();
        self.wake.wake();
    }
}

impl ClientLink for InProcessClient {
    fn send(&self, msg: ClientMsg) -> io::Result<()> {
        self.to_server.send(msg)
    }
    fn poll(&self) -> io::Result<Option<ServerMsg>> {
        self.from_server.poll()
    }
}

impl ServerLink for InProcessServer {
    /// Take the next message from this client, if one is waiting. Never blocks.
    fn poll(&self) -> io::Result<Option<ClientMsg>> {
        self.from_client.poll()
    }
    /// Send a message to this client.
    fn send(&self, msg: ServerMsg) -> io::Result<()> {
        self.to_client.send(msg)
    }
}

/// A client and the server in one process, joined by two channels that carry the messages **as they
/// are** — no serialization. The standalone path.
pub(crate) fn in_process() -> (InProcessClient, InProcessServer) {
    in_process_waking(ServerWaker::new(|| {}), ServerWaker::new(|| {}))
}

pub(crate) fn in_process_waking(
    server_wake: ServerWaker,
    client_wake: ServerWaker,
) -> (InProcessClient, InProcessServer) {
    in_process_with_limits(server_wake, client_wake, Default::default())
}

pub(crate) fn in_process_with_limits(
    server_wake: ServerWaker,
    client_wake: ServerWaker,
    limits: heca_config::settings::ConnectionConfig,
) -> (InProcessClient, InProcessServer) {
    let (to_server, from_client) = queue::channel(limits.queue_capacity);
    let (to_client, from_server) = queue::channel(limits.queue_capacity);
    (
        InProcessClient { to_server: WakeSender { sender: Some(to_server), wake: server_wake }, from_server },
        InProcessServer { from_client, to_client: WakeSender { sender: Some(to_client), wake: client_wake } },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::{PaneId, Rectangle, Size};

    fn asker() -> Asker {
        Asker {
            focused_pane: Some(PaneId(1)),
            workspace: 0,
            column: 0,
            area: Rectangle::from_size(Size::new(800.0, 600.0)),
            cell: (8.0, 14.0),
        }
    }

    /// An action goes client→server and a change comes back server→client, over the in-process
    /// link, with nothing serialized — the standalone spine.
    #[test]
    fn a_message_crosses_each_way() {
        let (client, server) = in_process();

        client.send(ClientMsg::Act {
            action: WmAction::SplitHorizontal,
            asker: asker(),
            request_id: None,
            terminal_owner: None,
        }).expect("connected");
        match server.poll().expect("connected") {
            Some(ClientMsg::Act { action, .. }) => assert_eq!(action, WmAction::SplitHorizontal),
            other => panic!("server did not receive the action: {other:?}"),
        }

        server.send(ServerMsg::Changed(Change::LayoutChanged)).expect("connected");
        match client.poll().expect("connected") {
            Some(ServerMsg::Changed(change)) => assert_eq!(change, Change::LayoutChanged),
            other => panic!("client did not receive the change: {other:?}"),
        }
    }

    #[test]
    fn a_dropped_peer_is_not_an_empty_queue() {
        let (client, server) = in_process();
        assert!(client.poll().expect("connected").is_none());
        drop(server);
        assert_eq!(client.poll().unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert!(client.send(ClientMsg::Attach).is_err());
    }

    #[test]
    fn in_process_messages_and_disconnects_wake_the_reader() {
        let (server_tx, server_rx) = channel();
        let (client_tx, client_rx) = channel();
        let (client, server) = in_process_waking(
            ServerWaker::new(move || { let _ = server_tx.send(()); }),
            ServerWaker::new(move || { let _ = client_tx.send(()); }),
        );
        client.send(ClientMsg::Attach).unwrap();
        server_rx.try_recv().expect("message wakes server");
        assert!(matches!(server.poll().unwrap(), Some(ClientMsg::Attach)));
        server.send(ServerMsg::Changed(Change::LayoutChanged)).unwrap();
        client_rx.try_recv().expect("reply wakes client");
        assert!(client.poll().unwrap().is_some());
        drop(client);
        server_rx.try_recv().expect("disconnect wakes server");
        assert_eq!(server.poll().unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }
}
