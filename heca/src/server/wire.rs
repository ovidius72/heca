//! **The client ↔ server protocol** (F012, P099/T544 C2).
//!
//! A client sends [`ClientMsg`]; the server sends [`ServerMsg`]. Both ends are written against this
//! one vocabulary whatever carries it: in standalone an in-process channel pair ([`in_process`])
//! moves the values directly, with no serialization; over the socket (later) the same values are
//! framed bytes. The server and client code never know which is under them — that is what keeps
//! standalone and an attached window one code path (decision d108b88c).
//!
//! Terminal snapshots, the per-pane input and the event subscription are added with the attach
//! protocol (T544 C5 / T546); this is the layout/action spine both ends share.

// Not yet called from the running app — the entry wiring (the next C4/C5 step) consumes it.
#![allow(dead_code)]

use std::sync::mpsc::{Receiver, Sender, channel};

use heca_core::layout::SessionContent;

use super::{Asker, Change, Refusal};
use crate::input::WmAction;

/// What a client asks the server to do.
#[derive(Debug)]
pub(crate) enum ClientMsg {
    /// Attach to the session; the server replies with [`ServerMsg::Attached`].
    Attach,
    /// Detach — the window is going away; the session and its programs keep running.
    Detach,
    /// Run a server action, carrying this window's own view facts (the server holds no per-client
    /// view, decision 2c249a29, so each request brings its own [`Asker`]).
    Act { action: WmAction, asker: Asker },
}

/// What the server tells a client.
#[derive(Debug)]
pub(crate) enum ServerMsg {
    /// The whole shared content, on attach — the client rebuilds a `Session` from it to draw
    /// ([`SessionContent::rebuild`]).
    Attached { content: SessionContent },
    /// Something changed in the shared content; the client reacts to it for itself.
    Changed(Change),
    /// The server refused an action, and why.
    Refused(Refusal),
}

/// The client's end of the link. The client is written against this, so the socket transport (later)
/// drops in behind the same two calls.
pub(crate) trait ClientLink {
    /// Send a message to the server.
    fn send(&self, msg: ClientMsg);
    /// Take the next message from the server, if one is waiting. Never blocks.
    fn poll(&self) -> Option<ServerMsg>;
}

/// The client half of an in-process pair.
pub(crate) struct InProcessClient {
    to_server: Sender<ClientMsg>,
    from_server: Receiver<ServerMsg>,
}

/// The server half of an in-process pair: what the server loop drains and answers for this client.
pub(crate) struct InProcessServer {
    from_client: Receiver<ClientMsg>,
    to_client: Sender<ServerMsg>,
}

impl ClientLink for InProcessClient {
    fn send(&self, msg: ClientMsg) {
        // The server is gone only when the process is ending; nothing to do about it here.
        let _ = self.to_server.send(msg);
    }
    fn poll(&self) -> Option<ServerMsg> {
        self.from_server.try_recv().ok()
    }
}

impl InProcessServer {
    /// Take the next message from this client, if one is waiting. Never blocks.
    pub(crate) fn poll(&self) -> Option<ClientMsg> {
        self.from_client.try_recv().ok()
    }
    /// Send a message to this client.
    pub(crate) fn send(&self, msg: ServerMsg) {
        let _ = self.to_client.send(msg);
    }
}

/// A client and the server in one process, joined by two channels that carry the messages **as they
/// are** — no serialization. The standalone path.
pub(crate) fn in_process() -> (InProcessClient, InProcessServer) {
    let (to_server, from_client) = channel();
    let (to_client, from_server) = channel();
    (
        InProcessClient { to_server, from_server },
        InProcessServer { from_client, to_client },
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
        });
        match server.poll() {
            Some(ClientMsg::Act { action, .. }) => assert_eq!(action, WmAction::SplitHorizontal),
            other => panic!("server did not receive the action: {other:?}"),
        }

        server.send(ServerMsg::Changed(Change::LayoutChanged));
        match client.poll() {
            Some(ServerMsg::Changed(change)) => assert_eq!(change, Change::LayoutChanged),
            other => panic!("client did not receive the change: {other:?}"),
        }
    }
}
