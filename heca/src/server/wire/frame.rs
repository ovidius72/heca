//! Length-prefixed JSON shared by both socket directions.

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
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid frame length"));
    }
    let mut payload = Vec::new();
    payload.try_reserve_exact(length).map_err(io::Error::other)?;
    payload.resize(length, 0);
    stream.read_exact(&mut payload)?;
    serde_json::from_slice(&payload)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
