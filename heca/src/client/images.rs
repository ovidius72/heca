//! One resource chunk in flight at a time; all allocation is checked against connection limits.

use std::{collections::{HashMap, HashSet}, io, sync::Arc};
use heca_core::backend::{TerminalImage, TerminalImageFrame};
use crate::server::wire::{ClientMsg, image::{ImageChunk, ImageInfo}};
use super::outbox::Outbox;

struct Partial {
    info: ImageInfo,
    frames: Vec<TerminalImageFrame>,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub(super) struct Images {
    requested: HashSet<u64>,
    partial: HashMap<u64, Partial>,
}

impl Images {
    pub(super) fn need(&mut self, id: u64, outbox: &Outbox) {
        if self.requested.insert(id) { outbox.send(ClientMsg::ImageInfo { id }); }
    }

    pub(super) fn begin(&mut self, info: ImageInfo, limit: usize, outbox: &Outbox) -> io::Result<()> {
        info.validate(limit)?;
        let id = info.id;
        self.partial.insert(id, Partial { info, frames: Vec::new(), bytes: Vec::new() });
        outbox.send(ClientMsg::ImageChunk { id, frame: 0, offset: 0 });
        Ok(())
    }

    pub(super) fn receive(&mut self, chunk: ImageChunk, outbox: &Outbox) -> io::Result<Option<TerminalImage>> {
        let partial = self.partial.get_mut(&chunk.id).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "image chunk has no descriptor"))?;
        let frame = partial.frames.len();
        let expected = &partial.info.frames[frame];
        if chunk.frame != frame || chunk.offset != partial.bytes.len() || chunk.rgba.is_empty()
            || chunk.rgba.len() > expected.bytes.saturating_sub(partial.bytes.len())
        { return Err(io::Error::new(io::ErrorKind::InvalidData, "image chunk is out of order or out of range")); }
        partial.bytes.try_reserve(chunk.rgba.len()).map_err(io::Error::other)?;
        partial.bytes.extend(chunk.rgba);
        if partial.bytes.len() == expected.bytes {
            partial.frames.push(TerminalImageFrame { rgba: Arc::from(std::mem::take(&mut partial.bytes)), delay: expected.delay });
        }
        if partial.frames.len() == partial.info.frames.len() {
            let partial = self.partial.remove(&chunk.id).ok_or_else(|| io::Error::other("image transfer disappeared"))?;
            return Ok(Some(TerminalImage { id: partial.info.id, width: partial.info.width,
                height: partial.info.height, frames: Arc::from(partial.frames) }));
        }
        outbox.send(ClientMsg::ImageChunk { id: chunk.id, frame: partial.frames.len(), offset: partial.bytes.len() });
        Ok(None)
    }

    pub(super) fn unavailable(&mut self, id: u64) { self.partial.remove(&id); }
}
