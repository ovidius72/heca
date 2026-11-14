//! Fan server-side events — the same [`Change`]s a window reacts to — out to
//! `heca rpc --events` subscribers (T546).
//!
//! A subscriber is a script connected over the control socket. The server holds the sending end of
//! a bounded channel per subscriber (`[settings.connection] queue_capacity` lines); the socket's
//! handshake thread holds the receiving end and writes each line to the socket. A subscriber whose
//! queue is full has fallen behind and is dropped, as a window that falls behind is. One shared
//! fanout serves every session, so a subscriber sees the whole server's events. Nothing is invented per event: the line is a serialized existing `Change`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::SyncSender;

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
/// `Rc`), and it never leaves the server's own thread — only the receiver crosses to the socket
/// thread, which mpsc already supports — so `Rc`/`RefCell` are enough.
#[derive(Clone, Default)]
pub(crate) struct EventFanout(Rc<RefCell<Vec<SyncSender<String>>>>);

impl EventFanout {
    /// Register a new subscriber's sending end. Its capacity is the subscriber's queue size.
    pub(crate) fn subscribe(&self, sender: SyncSender<String>) {
        self.0.borrow_mut().push(sender);
    }

    /// Whether anyone is listening — so the caller skips the work when no one subscribed.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }

    /// Serialize `event` to one line and send it to every subscriber, dropping the ones whose
    /// socket side has gone (receiver dropped) and the ones whose queue is full (they fell behind).
    /// Never blocks. A no-op with no subscribers, so the caller need not guard.
    pub(crate) fn emit(&self, event: &StreamEvent) {
        if self.0.borrow().is_empty() {
            return;
        }
        let Ok(line) = serde_json::to_string(event) else {
            return;
        };
        self.0
            .borrow_mut()
            .retain(|sender| sender.try_send(line.clone()).is_ok());
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

        let (a_tx, a_rx) = mpsc::sync_channel(8);
        let (b_tx, b_rx) = mpsc::sync_channel(8);
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

    #[test]
    fn a_subscriber_that_never_reads_is_dropped_once_its_queue_is_full() {
        let fanout = EventFanout::default();
        let (slow_tx, slow_rx) = mpsc::sync_channel(2);
        let (live_tx, live_rx) = mpsc::sync_channel(2);
        fanout.subscribe(slow_tx);
        fanout.subscribe(live_tx);

        let event = StreamEvent::Workspace { window: 1, workspace: 0 };
        // Emit more events than the queue holds; the live one is read each time. Emit returns
        // every time (it would hang here if it blocked on the full queue).
        for _ in 0..3 {
            fanout.emit(&event);
            assert_eq!(recv(&live_rx), event);
        }
        // The slow one got its two queued lines, then was dropped: its stream ends.
        assert_eq!(recv(&slow_rx), event);
        assert_eq!(recv(&slow_rx), event);
        assert_eq!(slow_rx.try_recv(), Err(mpsc::TryRecvError::Disconnected), "dropped subscriber's stream ends");
        // The live one is still subscribed and keeps receiving.
        fanout.emit(&event);
        assert_eq!(recv(&live_rx), event);
        assert!(!fanout.is_empty());
    }
}
