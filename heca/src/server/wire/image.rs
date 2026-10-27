//! Image resources are fetched by content handle, separately from terminal state.

use std::{io, time::Duration};
use heca_core::backend::TerminalImage;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ImageInfo {
    pub(crate) id: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: Vec<FrameInfo>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct FrameInfo { pub(crate) bytes: usize, pub(crate) delay: Duration }

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ImageChunk {
    pub(crate) id: u64,
    pub(crate) frame: usize,
    pub(crate) offset: usize,
    pub(crate) rgba: Vec<u8>,
}

impl ImageInfo {
    pub(crate) fn of(image: &TerminalImage) -> Self {
        Self { id: image.id, width: image.width, height: image.height,
            frames: image.frames.iter().map(|frame| FrameInfo { bytes: frame.rgba.len(), delay: frame.delay }).collect() }
    }

    pub(crate) fn validate(&self, limit: usize) -> io::Result<()> {
        let bytes = (self.width as usize).checked_mul(self.height as usize).and_then(|pixels| pixels.checked_mul(4));
        let valid = bytes.is_some_and(|bytes| bytes > 0 && !self.frames.is_empty()
            && self.frames.iter().all(|frame| frame.bytes == bytes)
            && bytes.checked_mul(self.frames.len()).is_some_and(|total| total <= limit));
        if valid { Ok(()) } else { Err(io::Error::new(io::ErrorKind::InvalidData, "invalid image resource dimensions or size")) }
    }
}

pub(crate) fn chunk(image: &TerminalImage, frame: usize, offset: usize, max_frame_bytes: usize) -> io::Result<ImageChunk> {
    let pixels = image.frames.get(frame).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "unknown image frame"))?;
    if offset >= pixels.rgba.len() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "image offset outside frame")); }
    let mut chunk = ImageChunk { id: image.id, frame, offset, rgba: Vec::new() };
    let overhead = serde_json::to_vec(&super::ServerMsg::ImageChunk(ImageChunk {
        id: chunk.id, frame, offset, rgba: Vec::new(),
    })).map_err(io::Error::other)?.len();
    // A JSON byte takes at most three digits and a comma; framing is outside this limit.
    let capacity = max_frame_bytes.saturating_sub(overhead) / 4;
    if capacity == 0 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "frame limit cannot carry an image chunk")); }
    let end = offset.saturating_add(capacity).min(pixels.rgba.len());
    chunk.rgba.extend_from_slice(&pixels.rgba[offset..end]);
    Ok(chunk)
}
