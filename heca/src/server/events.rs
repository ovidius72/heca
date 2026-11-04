//! Fan server-side events — the same [`Change`](super::Change)s a window reacts to — out to
//! `heca rpc --events` subscribers (T546).
//!
//! A subscriber is a script connected over the control socket. The server holds the sending end of
//! an mpsc channel per subscriber; the socket's handshake thread holds the receiving end and writes
//! each line to the socket. One shared fanout serves every session, so a subscriber sees the whole
//! server's events. Nothing is invented per event: the line is a serialized existing `Change`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use heca_core::layout::{PaneId, SessionId};

use super::Change;

/// One line of the `heca rpc --events` stream. It is a tagged union of the shared content
/// [`Change`]s a window reacts to and the per-window events a script also wants — where each window
/// is looking (focus, workspace) and which session it is on. Every per-window event carries the
/// window's id (its connection id), so a script can tell the windows apart.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum StreamEvent {
    /// A shared change to the session content (layout): a window reacts to the same thing.
    Change(Change),
    /// A window's focused pane changed (per-window view state).
    Focus { window: u64, workspace: usize, pane: Option<PaneId> },
    /// A window's shown workspace changed (per-window view state).
    Workspace { window: u64, workspace: usize },
    /// A window attached to or switched to a session.
    Session { window: u64, session: SessionId, name: String },
}

/// The subscribers' sending ends, shared by the owner loop and its sessions. Cheap to clone (an
/// `Rc`), and it never leaves the server's own thread — only the `Sender` crosses to the socket
/// thread, which mpsc already supports — so `Rc`/`RefCell` are enough.
#[derive(Clone, Default)]
pub(crate) struct EventFanout(Rc<RefCell<Vec<Sender<String>>>>);

impl EventFanout {
    /// Register a new subscriber's sending end.
    pub(crate) fn subscribe(&self, sender: Sender<String>) {
        self.0.borrow_mut().push(sender);
    }

    /// Whether anyone is listening — so the caller skips the work when no one subscribed.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    /// Serialize `event` to one line and send it to every subscriber, dropping the ones whose
    /// socket side has gone (the send fails once the receiver is dropped). A no-op with no
    /// subscribers, so the caller need not guard.
    pub(crate) fn emit(&self, event: &StreamEvent) {
        if self.0.borrow().is_empty() {
            return;
        }
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        self.0
            .borrow_mut()
            .retain(|sender| sender.send(line.clone()).is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn recv(rx: &mpsc::Receiver<String>) -> StreamEvent {
        serde_json::from_str(&rx.recv().unwrap()).unwrap()
    }

    #[test]
    fn an_event_reaches_every_live_subscriber_and_a_gone_one_is_forgotten() {
        let fanout = EventFanout::default();
        assert!(fanout.is_empty());
        // An emit with no subscribers is a no-op, not a panic.
        fanout.emit(&StreamEvent::Change(Change::LayoutChanged));

        let (a_tx, a_rx) = mpsc::channel();
        let (b_tx, b_rx) = mpsc::channel();
        fanout.subscribe(a_tx);
        fanout.subscribe(b_tx);
        assert!(!fanout.is_empty());

        let one = StreamEvent::Workspace { window: 1, workspace: 0 };
        fanout.emit(&one);
        assert_eq!(recv(&a_rx), one);
        assert_eq!(recv(&b_rx), one);

        // B's socket side goes: the next emit drops it, and A still receives.
        drop(b_rx);
        let two = StreamEvent::Focus { window: 1, workspace: 0, pane: Some(PaneId(5)) };
        fanout.emit(&two);
        assert_eq!(recv(&a_rx), two);
        assert!(!fanout.is_empty(), "A is still listening");

        drop(a_rx);
        fanout.emit(&two);
        assert!(fanout.is_empty(), "no subscribers left");
    }
}
