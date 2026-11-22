//! Length-prefixed JSON shared by both socket directions, and how each direction's messages are
//! written into it ([`Carried`]).

use std::io::{self, Read, Write};
use std::num::NonZeroUsize;

use serde::{Serialize, de::DeserializeOwned};

pub(super) fn write<T: Serialize>(
    stream: &mut impl Write,
    message: &T,
    limit: NonZeroUsize,
) -> io::Result<()> {
    let payload = serde_json::to_vec(message).map_err(io::Error::other)?;
    let length = u32::try_from(payload.len())
        .ok()
        .filter(|_| payload.len() <= limit.get())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "message exceeds frame limit"))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(&payload)
}

pub(super) fn read<T: DeserializeOwned>(
    stream: &mut impl Read,
    limit: NonZeroUsize,
) -> io::Result<Option<T>> {
    let mut header = [0; size_of::<u32>()];
    // Reading the first byte separately distinguishes a clean EOF from a truncated frame.
    loop {
        match stream.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    stream.read_exact(&mut header[1..])?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > limit.get() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame length",
        ));
    }
    let mut payload = Vec::new();
    payload
        .try_reserve_exact(length)
        .map_err(io::Error::other)?;
    payload.resize(length, 0);
    stream.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// **How one direction's messages cross the socket.** The writer and the reader of one stream each
/// keep a [`Memory`](Self::Memory), so a message may travel as what changed since the previous one
/// and come out whole on the other side.
pub(crate) trait Carried: Sized {
    type Memory: Default + Send + 'static;
    fn write_to(
        self,
        memory: &mut Self::Memory,
        stream: &mut impl Write,
        limit: NonZeroUsize,
    ) -> io::Result<()>;
    fn read_from(
        memory: &mut Self::Memory,
        stream: &mut impl Read,
        limit: NonZeroUsize,
    ) -> io::Result<Option<Self>>;
}

/// What a window sends is small: each message is written as it is.
impl Carried for super::ClientMsg {
    type Memory = ();
    fn write_to(self, _: &mut (), stream: &mut impl Write, limit: NonZeroUsize) -> io::Result<()> {
        write(stream, &self, limit)
    }
    fn read_from(
        _: &mut (),
        stream: &mut impl Read,
        limit: NonZeroUsize,
    ) -> io::Result<Option<Self>> {
        read(stream, limit)
    }
}

/// What the server sends: a terminal travels as the rows that changed ([`super::rows`]).
impl Carried for super::ServerMsg {
    type Memory = super::rows::Screens;
    fn write_to(
        self,
        screens: &mut Self::Memory,
        stream: &mut impl Write,
        limit: NonZeroUsize,
    ) -> io::Result<()> {
        use super::rows::Outgoing;
        match self {
            super::ServerMsg::Terminal(frame) => {
                write(stream, &Outgoing::Terminal(&screens.pack(frame)), limit)
            }
            message => {
                if let super::ServerMsg::TerminalClosed { owner } = &message {
                    screens.forget(owner);
                }
                write(stream, &Outgoing::Message(&message), limit)
            }
        }
    }
    fn read_from(
        screens: &mut Self::Memory,
        stream: &mut impl Read,
        limit: NonZeroUsize,
    ) -> io::Result<Option<Self>> {
        use super::rows::Incoming;
        Ok(match read(stream, limit)? {
            None => None,
            Some(Incoming::Terminal(packed)) => {
                Some(super::ServerMsg::Terminal(screens.unpack(*packed)?))
            }
            Some(Incoming::Message(message)) => {
                if let super::ServerMsg::TerminalClosed { owner } = &message {
                    screens.forget(owner);
                }
                Some(message)
            }
        })
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
