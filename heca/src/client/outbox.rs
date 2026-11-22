//! Typed, nonblocking requests from terminal views to their connection owner.
//!
//! Unbounded on purpose: what goes in is what a person typed or did, made at human speed, and it is
//! never dropped. Backpressure belongs to the connection (`server/wire/queue.rs`), not here.

use crate::server::{
    ServerWaker,
    wire::{ClientMsg, terminal::ProcessInput},
};
use std::{
    io,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
};

#[derive(Clone)]
pub(super) struct Outbox {
    sender: Sender<ClientMsg>,
    wake: ServerWaker,
    failure: Arc<Mutex<Option<String>>>,
}

pub(super) fn channel(wake: ServerWaker) -> (Outbox, Receiver<ClientMsg>) {
    let (sender, receiver) = mpsc::channel();
    (
        Outbox {
            sender,
            wake,
            failure: Arc::default(),
        },
        receiver,
    )
}

/// Keep the final geometry of a consecutive resize batch. Never move it across input.
pub(super) fn drain(receiver: &Receiver<ClientMsg>) -> impl Iterator<Item = ClientMsg> + '_ {
    let mut messages = receiver.try_iter().peekable();
    std::iter::from_fn(move || {
        let mut message = messages.next()?;
        while messages.peek().is_some_and(|next| {
            matches!((&message, next), (
            ClientMsg::Terminal { owner: a, input: ProcessInput::Resize { .. } },
            ClientMsg::Terminal { owner: b, input: ProcessInput::Resize { .. } },
        ) if a == b)
        }) {
            message = messages.next()?;
        }
        Some(message)
    })
}

impl Outbox {
    pub(super) fn send(&self, message: ClientMsg) {
        // Fails only when the window's connection owner is gone.
        if let Err(error) = self.sender.send(message) {
            let mut failure = self
                .failure
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if failure.is_none() {
                *failure = Some(error.to_string());
            }
        }
        self.wake.wake();
    }

    pub(super) fn check(&self) -> io::Result<()> {
        match &*self
            .failure
            .lock()
            .map_err(|error| io::Error::other(error.to_string()))?
        {
            Some(error) => Err(io::Error::new(io::ErrorKind::BrokenPipe, error.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::backend_store::TerminalOwner;
    use heca_core::layout::PaneId;

    fn resize(pane: u64, cols: usize) -> ClientMsg {
        ClientMsg::Terminal {
            owner: TerminalOwner::Pane(PaneId(pane)),
            input: ProcessInput::Resize {
                cols,
                rows: 24,
                cell: (8.0, 16.0),
                scale: 1.0,
            },
        }
    }

    #[test]
    fn resize_batches_never_cross_input_actions_or_another_terminal() {
        let (outbox, requests) = channel(ServerWaker::new(|| {}));
        for message in [
            resize(1, 80),
            resize(1, 90),
            ClientMsg::Terminal {
                owner: TerminalOwner::Pane(PaneId(1)),
                input: ProcessInput::Bytes(b"x".to_vec()),
            },
            resize(1, 100),
            resize(2, 50),
            resize(1, 110),
            ClientMsg::Reload,
            resize(1, 120),
        ] {
            outbox.send(message);
        }
        let messages: Vec<_> = drain(&requests).collect();
        assert_eq!(messages.len(), 7);
        assert!(matches!(
            messages[0],
            ClientMsg::Terminal {
                input: ProcessInput::Resize { cols: 90, .. },
                ..
            }
        ));
        assert!(matches!(
            messages[1],
            ClientMsg::Terminal {
                input: ProcessInput::Bytes(_),
                ..
            }
        ));
        assert!(matches!(
            messages[2],
            ClientMsg::Terminal {
                input: ProcessInput::Resize { cols: 100, .. },
                ..
            }
        ));
        assert!(matches!(
            messages[3],
            ClientMsg::Terminal {
                owner: TerminalOwner::Pane(PaneId(2)),
                ..
            }
        ));
        assert!(matches!(
            messages[4],
            ClientMsg::Terminal {
                input: ProcessInput::Resize { cols: 110, .. },
                ..
            }
        ));
        assert!(matches!(messages[5], ClientMsg::Reload));
        assert!(matches!(
            messages[6],
            ClientMsg::Terminal {
                input: ProcessInput::Resize { cols: 120, .. },
                ..
            }
        ));
    }
}
