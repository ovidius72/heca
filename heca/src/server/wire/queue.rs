//! Bounded transport queues. Replace pending state, never discard ordered input or effects.
//!
//! What happens when a queue is full is decided here, once, by the message type
//! ([`Message::WHEN_FULL`]) and by who sends: a socket reader [waits](Sender::send_wait) for room, so
//! a slow loop slows the peer down through the socket instead of losing what it sent.

use std::{
    collections::VecDeque,
    io,
    num::NonZeroUsize,
    sync::{Arc, Condvar, Mutex},
};

/// What [`Sender::send`] does with a message when the queue is full and no queued one can take its
/// place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WhenFull {
    /// Queue it anyway. For what a person did — keys, pastes, actions — which is small, made at
    /// human speed and never to be lost: the capacity only says when the peer is behind.
    Keep,
    /// Report `WouldBlock`. For what the server tells a window: a window that stops reading is
    /// dropped and attaches again with a fresh copy, instead of the server holding a growing backlog.
    Refuse,
}

pub(crate) trait Message {
    /// What a full queue does with this kind of message.
    const WHEN_FULL: WhenFull = WhenFull::Refuse;
    fn replaces(&self, _older: &Self) -> bool {
        false
    }
    fn is_boundary(&self) -> bool {
        false
    }
}

struct State<T> {
    messages: VecDeque<T>,
    closed: bool,
}
struct Shared<T> {
    state: Mutex<State<T>>,
    ready: Condvar,
    room: Condvar,
    capacity: usize,
}
pub(super) struct Sender<T>(Arc<Shared<T>>);
pub(super) struct Receiver<T>(Arc<Shared<T>>);

pub(super) fn channel<T>(capacity: NonZeroUsize) -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            messages: VecDeque::new(),
            closed: false,
        }),
        ready: Condvar::new(),
        room: Condvar::new(),
        capacity: capacity.get(),
    });
    (Sender(shared.clone()), Receiver(shared))
}

impl<T: Message> Sender<T> {
    /// Queue a message without waiting. A full queue keeps it or refuses it as its type says
    /// ([`Message::WHEN_FULL`]); a queued message it replaces takes its place either way.
    pub(super) fn send(&self, message: T) -> io::Result<()> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        if state.closed {
            return Err(closed());
        }
        match self.place(&mut state, message) {
            Ok(()) => Ok(()),
            Err(message) if T::WHEN_FULL == WhenFull::Keep => {
                state.messages.push_back(message);
                self.0.ready.notify_one();
                Ok(())
            }
            Err(_) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "connection queue full",
            )),
        }
    }

    /// Queue a message, waiting for room when the queue is full. For a thread whose only job is
    /// to move messages — a socket reader — so a reader that waits stops reading the socket and
    /// the peer is slowed down instead of losing what it sent.
    pub(super) fn send_wait(&self, message: T) -> io::Result<()> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        let mut message = message;
        loop {
            if state.closed {
                return Err(closed());
            }
            match self.place(&mut state, message) {
                Ok(()) => return Ok(()),
                Err(back) => message = back,
            }
            state = self.0.room.wait(state).map_err(poisoned)?;
        }
    }

    /// Put `message` in the queue if it replaces a queued one or there is room; hand it back if not.
    fn place(&self, state: &mut State<T>, message: T) -> Result<(), T> {
        let start = state
            .messages
            .iter()
            .rposition(Message::is_boundary)
            .map_or(0, |index| index + 1);
        if let Some(index) = state
            .messages
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(index, older)| message.replaces(older).then_some(index))
        {
            state.messages[index] = message;
        } else if state.messages.len() >= self.0.capacity {
            return Err(message);
        } else {
            state.messages.push_back(message);
        }
        self.0.ready.notify_one();
        Ok(())
    }
}

impl<T> Receiver<T> {
    /// Close the queue from the receiving side: a sender waiting for room
    /// ([`send_wait`](Sender::send_wait)) stops waiting and is told the connection is closed.
    pub(super) fn close(&self) {
        self.0.close();
    }

    pub(super) fn poll(&self) -> io::Result<Option<T>> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        if let Some(message) = self.take(&mut state) {
            return Ok(Some(message));
        }
        if state.closed {
            return Err(closed());
        }
        Ok(None)
    }

    pub(super) fn recv(&self) -> io::Result<Option<T>> {
        let mut state = self.0.state.lock().map_err(poisoned)?;
        loop {
            if let Some(message) = self.take(&mut state) {
                return Ok(Some(message));
            }
            if state.closed {
                return Ok(None);
            }
            state = self.0.ready.wait(state).map_err(poisoned)?;
        }
    }

    fn take(&self, state: &mut State<T>) -> Option<T> {
        let message = state.messages.pop_front()?;
        self.0.room.notify_one();
        Some(message)
    }
}

impl<T> Shared<T> {
    fn close(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.closed = true;
        self.ready.notify_all();
        self.room.notify_all();
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.0.close();
    }
}
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.0.close();
    }
}
fn closed() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "connection closed")
}
fn poisoned<T>(error: std::sync::PoisonError<T>) -> io::Error {
    io::Error::other(error.to_string())
}

impl Message for super::ClientMsg {
    const WHEN_FULL: WhenFull = WhenFull::Keep;
}
impl Message for super::ServerMsg {
    fn is_boundary(&self) -> bool {
        matches!(
            self,
            super::ServerMsg::Attached { .. }
                | super::ServerMsg::SessionTerminated { .. }
                | super::ServerMsg::SessionDeleted { .. }
                | super::ServerMsg::SessionDetached { .. }
                | super::ServerMsg::ProjectionReady { .. }
        )
    }
    fn replaces(&self, older: &Self) -> bool {
        use super::{Change, ServerMsg};
        match (self, older) {
            (ServerMsg::Content { .. }, ServerMsg::Content { .. }) => true,
            // A notification projection is a whole snapshot: the newest replaces a still-queued one.
            (ServerMsg::Notifications(_), ServerMsg::Notifications(_)) => true,
            // Likewise a whole snapshot: asking twice before the first is read gets one answer.
            (ServerMsg::NotificationHistory(_), ServerMsg::NotificationHistory(_)) => true,
            (
                ServerMsg::Feedback {
                    session_id: new, ..
                },
                ServerMsg::Feedback {
                    session_id: old, ..
                },
            ) => new == old,
            (ServerMsg::Terminal(new), ServerMsg::Terminal(old)) => {
                new.owner == old.owner && new.screen.viewport_offset == old.screen.viewport_offset
            }
            (
                ServerMsg::Changed(Change::LayoutChanged),
                ServerMsg::Changed(Change::LayoutChanged),
            )
            | (
                ServerMsg::Changed(Change::NamesChanged),
                ServerMsg::Changed(Change::NamesChanged),
            )
            | (
                ServerMsg::Changed(Change::NotificationsChanged),
                ServerMsg::Changed(Change::NotificationsChanged),
            ) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{
        Change,
        wire::{ClientMsg, ServerMsg},
    };

    #[test]
    fn state_never_coalesces_across_a_session_projection_boundary() {
        let (sender, receiver) = channel(NonZeroUsize::new(3).unwrap());
        sender
            .send(ServerMsg::Changed(Change::LayoutChanged))
            .unwrap();
        sender
            .send(ServerMsg::SessionDeleted {
                id: heca_core::layout::SessionId(1),
            })
            .unwrap();
        sender
            .send(ServerMsg::Changed(Change::LayoutChanged))
            .unwrap();
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::Changed(Change::LayoutChanged))
        ));
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::SessionDeleted { .. })
        ));
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::Changed(Change::LayoutChanged))
        ));
    }

    #[test]
    fn later_state_cannot_replace_a_snapshot_before_projection_ready() {
        let (sender, receiver) = channel(NonZeroUsize::new(3).unwrap());
        sender.send(ServerMsg::Notifications(Vec::new())).unwrap();
        sender
            .send(ServerMsg::ProjectionReady {
                session_id: heca_core::layout::SessionId(1),
            })
            .unwrap();
        sender.send(ServerMsg::Notifications(Vec::new())).unwrap();
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::Notifications(_))
        ));
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::ProjectionReady { .. })
        ));
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::Notifications(_))
        ));
    }

    #[test]
    fn state_coalesces_and_a_full_queue_refuses_what_the_server_sends() {
        let capacity = NonZeroUsize::new(1).unwrap();
        let (sender, receiver) = channel(capacity);
        sender
            .send(ServerMsg::Changed(Change::LayoutChanged))
            .unwrap();
        sender
            .send(ServerMsg::Changed(Change::LayoutChanged))
            .unwrap();
        assert!(matches!(
            receiver.poll().unwrap(),
            Some(ServerMsg::Changed(Change::LayoutChanged))
        ));
        assert!(receiver.poll().unwrap().is_none());
        sender.send(ServerMsg::Followed { active: true }).unwrap();
        let refused = sender
            .send(ServerMsg::Followed { active: false })
            .unwrap_err();
        assert_eq!(refused.kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn a_full_queue_keeps_every_input_in_order() {
        let (sender, receiver) = channel(NonZeroUsize::new(1).unwrap());
        for _ in 0..100 {
            sender.send(ClientMsg::Attach).unwrap();
        }
        sender.send(ClientMsg::Detach).unwrap();
        for _ in 0..100 {
            assert!(matches!(receiver.poll().unwrap(), Some(ClientMsg::Attach)));
        }
        assert!(matches!(receiver.poll().unwrap(), Some(ClientMsg::Detach)));
        assert!(receiver.poll().unwrap().is_none());
    }

    #[test]
    fn a_waiting_sender_resumes_when_there_is_room_and_stops_when_closed() {
        let (sender, receiver) = channel(NonZeroUsize::new(1).unwrap());
        sender
            .send_wait(ServerMsg::Followed { active: true })
            .unwrap();
        let waiting = std::thread::spawn(move || {
            sender
                .send_wait(ServerMsg::Followed { active: false })
                .unwrap();
            sender.send_wait(ServerMsg::Followed { active: true })
        });
        let mut seen = Vec::new();
        while seen.len() < 3 {
            match receiver.recv().unwrap() {
                Some(ServerMsg::Followed { active }) => seen.push(active),
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(seen, [true, false, true]);
        waiting.join().unwrap().unwrap();
        let (sender, receiver) = channel(NonZeroUsize::new(1).unwrap());
        sender
            .send_wait(ServerMsg::Followed { active: true })
            .unwrap();
        let blocked =
            std::thread::spawn(move || sender.send_wait(ServerMsg::Followed { active: false }));
        std::thread::sleep(std::time::Duration::from_millis(20));
        drop(receiver);
        assert_eq!(
            blocked.join().unwrap().unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
