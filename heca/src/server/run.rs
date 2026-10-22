//! **The server runtime** (F012, P099/T544 C4) — the shared state, the session, and the clients
//! attached to it, driven one [`pump`](ServerLoop::pump) at a time.
//!
//! A pump is one turn of the loop: take what each client sent, do it, send back what changed. It
//! needs no winit and no threads, so it runs and is tested headless. The headless `heca --server`
//! wraps it in a thread that pumps when its mpsc wakes (a client message, a terminal's output, a
//! timer); standalone pumps it from the window's loop. One loop body, both ways.

// Not yet called from the running app — the entry wiring (the next C4/C5 step) consumes it.
#![allow(dead_code)]

use std::time::Instant;

use heca_core::layout::Session;

use super::wire::{ClientMsg, InProcessClient, InProcessServer, ServerMsg, in_process};
use super::{Asker, Change, ServerState};

/// Everything a session is and the clients watching it.
pub(crate) struct ServerLoop {
    state: ServerState,
    session: Session,
    /// The server end of each attached client. A detached client is dropped from here.
    clients: Vec<InProcessServer>,
}

impl ServerLoop {
    pub(crate) fn new(state: ServerState, session: Session) -> Self {
        Self {
            state,
            session,
            clients: Vec::new(),
        }
    }

    /// Attach an in-process client and hand back its end. The socket transport adds its clients the
    /// same way, so the loop never knows which kind it is serving.
    pub(crate) fn connect(&mut self) -> InProcessClient {
        let (client, server) = in_process();
        self.clients.push(server);
        client
    }

    /// One turn of the loop: drain every client, do what it asked, and send back what changed;
    /// then let time pass (notification deadlines). Detached clients are dropped.
    pub(crate) fn pump(&mut self, now: Instant) {
        // Drain first (borrows the client ends), then act (borrows the state + session), so the two
        // borrows never overlap.
        let mut pending: Vec<(usize, ClientMsg)> = Vec::new();
        for (i, client) in self.clients.iter().enumerate() {
            while let Some(msg) = client.poll() {
                pending.push((i, msg));
            }
        }

        let mut detached: Vec<usize> = Vec::new();
        for (i, msg) in pending {
            match msg {
                ClientMsg::Attach => {
                    let content = self.session.content();
                    self.clients[i].send(ServerMsg::Attached { content });
                }
                ClientMsg::Detach => detached.push(i),
                ClientMsg::Act { action, asker } => {
                    let changes = self.act(action, asker);
                    self.broadcast(&changes);
                }
            }
        }

        let ticks = self.state.tick(now);
        self.broadcast(&ticks);

        // Drop detached clients, highest index first so the earlier ones keep their positions.
        detached.sort_unstable();
        detached.dedup();
        for i in detached.into_iter().rev() {
            self.clients.remove(i);
        }
    }

    /// Run one server action on the shared state for the window that asked, and say what changed.
    fn act(&mut self, action: crate::input::WmAction, asker: Asker) -> Vec<Change> {
        self.state.run(&mut self.session, asker, &action)
    }

    /// Tell every attached client what changed — each reacts to it for itself.
    fn broadcast(&self, changes: &[Change]) {
        for change in changes {
            for client in &self.clients {
                client.send(ServerMsg::Changed(change.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::wire::ClientLink;
    use heca_core::layout::{LayoutOptions, SessionId};

    fn a_loop() -> ServerLoop {
        let session = Session::new(SessionId(1), LayoutOptions::default());
        ServerLoop::new(ServerState::for_tests(Default::default()), session)
    }

    /// A client that attaches is sent the whole content to draw from — the server's current session,
    /// exactly as [`Session::content`] projects it.
    #[test]
    fn attaching_sends_the_content() {
        let mut server = a_loop();
        let client = server.connect();
        let expected = server_content(&server);

        client.send(ClientMsg::Attach);
        server.pump(Instant::now());

        match client.poll() {
            Some(ServerMsg::Attached { content }) => assert_eq!(content, expected),
            other => panic!("attach did not return the content: {other:?}"),
        }
    }

    /// A detached client stops being sent to: after it detaches, a later change reaches no one and
    /// the loop does not panic reaching for a client that is gone.
    #[test]
    fn a_detached_client_is_dropped() {
        let mut server = a_loop();
        let client = server.connect();
        client.send(ClientMsg::Detach);
        server.pump(Instant::now());
        assert!(server.clients.is_empty(), "the detached client was not dropped");
    }

    fn server_content(server: &ServerLoop) -> heca_core::layout::SessionContent {
        server.session.content()
    }
}
