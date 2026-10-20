//! **Images in the terminal** — decoding what a program drew, and grouping it into placements.

use super::*;

/// Convert a wezterm source texture coordinate to a plain `[x, y]` pair.
pub(super) fn texcoord(c: wezterm_surface::TextureCoordinate) -> [f32; 2] {
    [c.x.into_inner(), c.y.into_inner()]
}

/// Derive the stable 64-bit image handle the snapshot/renderer use from
/// wezterm's 32-byte content hash. Truncating to 8 bytes of SHA-256 keeps
/// collisions astronomically unlikely while giving a cheap cache key.
pub(super) fn image_id_from_hash(hash: [u8; 32]) -> u64 {
    u64::from_le_bytes(hash[..8].try_into().expect("hash is 32 bytes"))
}

/// GIF/APNG frame delays at or below this are treated as "as fast as possible"
/// and clamped to [`DEFAULT_FRAME_DELAY`], matching how browsers render such GIFs
/// (a 0ms delay would otherwise spin the animation at the frame rate).
pub(super) const MIN_HONORED_FRAME_DELAY: Duration = Duration::from_millis(10);
pub(super) const DEFAULT_FRAME_DELAY: Duration = Duration::from_millis(100);

/// Decode one source image to one or more tightly packed RGBA8 frames. `Rgba8`
/// is a single static frame; `AnimRgba8` and encoded GIF/APNG payloads decode
/// every frame with its delay (`terminal-task-24`). Returns `None` when the
/// payload cannot be decoded.
pub(super) fn decode_terminal_image(
    hash: [u8; 32],
    data: &wezterm_term::image::ImageDataType,
) -> Option<TerminalImage> {
    use wezterm_term::image::ImageDataType;
    let (width, height, frames) = match data {
        ImageDataType::Rgba8 {
            data,
            width,
            height,
            ..
        } => (*width, *height, vec![(data.clone(), Duration::ZERO)]),
        ImageDataType::AnimRgba8 {
            width,
            height,
            frames,
            durations,
            ..
        } => {
            let collected: Vec<(Vec<u8>, Duration)> = frames
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let delay = durations.get(i).copied().unwrap_or(DEFAULT_FRAME_DELAY);
                    (f.clone(), clamp_frame_delay(delay))
                })
                .collect();
            (*width, *height, collected)
        }
        ImageDataType::EncodedFile(bytes) => decode_encoded_frames(bytes)?,
        ImageDataType::EncodedLease(lease) => {
            let bytes = lease.get_data().ok()?;
            decode_encoded_frames(&bytes)?
        }
    };

    if width == 0 || height == 0 || frames.is_empty() {
        return None;
    }
    let stride = (width as usize) * (height as usize) * 4;
    let frames: Vec<TerminalImageFrame> = frames
        .into_iter()
        .filter(|(rgba, _)| rgba.len() == stride)
        .map(|(rgba, delay)| TerminalImageFrame {
            rgba: Arc::from(rgba.into_boxed_slice()),
            delay,
        })
        .collect();
    if frames.is_empty() {
        return None;
    }
    // Optional decode diagnostic (off by default): set `HECA_DEBUG_IMAGES` to log
    // the source variant and how many frames each inline image decoded to. Handy
    // for telling apart tools that send a raw animation from ones that pre-flatten
    // it to a single frame (e.g. some file-manager previews).
    if std::env::var_os("HECA_DEBUG_IMAGES").is_some() {
        let variant = match data {
            ImageDataType::Rgba8 { .. } => "Rgba8",
            ImageDataType::AnimRgba8 { .. } => "AnimRgba8",
            ImageDataType::EncodedFile(_) => "EncodedFile",
            ImageDataType::EncodedLease(_) => "EncodedLease",
        };
        eprintln!(
            "[heca-img] decoded {variant} {width}x{height} -> {} frame(s)",
            frames.len()
        );
    }
    Some(TerminalImage {
        id: image_id_from_hash(hash),
        width,
        height,
        frames: Arc::from(frames.into_boxed_slice()),
    })
}

/// Clamp near-zero animation frame delays up to a sane default (browser behavior).
pub(super) fn clamp_frame_delay(delay: Duration) -> Duration {
    if delay <= MIN_HONORED_FRAME_DELAY {
        DEFAULT_FRAME_DELAY
    } else {
        delay
    }
}

/// Decoded RGBA frames: canvas `(width, height)` and one `(pixels, delay)` per
/// frame (a single entry for a static image).
pub(super) type DecodedFrames = (u32, u32, Vec<(Vec<u8>, Duration)>);

/// Decode an encoded image blob to RGBA8 frames. Animated GIF and APNG decode
/// every frame (each composited to the full canvas) with its delay; every other
/// format (JPEG/WebP/BMP/static PNG/GIF) yields a single zero-delay frame.
pub(super) fn decode_encoded_frames(bytes: &[u8]) -> Option<DecodedFrames> {
    use image::AnimationDecoder;
    use std::io::Cursor;

    let animated: Option<Vec<image::Frame>> = match image::guess_format(bytes).ok()? {
        image::ImageFormat::Gif => image::codecs::gif::GifDecoder::new(Cursor::new(bytes))
            .ok()
            .and_then(|d| d.into_frames().collect_frames().ok()),
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes)).ok()?;
            if decoder.is_apng().ok()? {
                decoder
                    .apng()
                    .ok()
                    .and_then(|a| a.into_frames().collect_frames().ok())
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(frames) = animated.filter(|f| f.len() > 1) {
        let mut out = Vec::with_capacity(frames.len());
        let (mut width, mut height) = (0u32, 0u32);
        for frame in frames {
            let delay = clamp_frame_delay(duration_from_delay(frame.delay()));
            let buffer = frame.into_buffer();
            width = buffer.width();
            height = buffer.height();
            out.push((buffer.into_vec(), delay));
        }
        return Some((width, height, out));
    }

    // Static image (or single-frame animation): decode the one frame.
    let decoded = image::load_from_memory(bytes)
        .map_err(|err| eprintln!("terminal inline image: decode failed: {err:#}"))
        .ok()?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    Some((width, height, vec![(decoded.into_vec(), Duration::ZERO)]))
}

/// Convert an `image` crate frame delay to a [`Duration`].
pub(super) fn duration_from_delay(delay: image::Delay) -> Duration {
    let (numer, denom) = delay.numer_denom_ms();
    if denom == 0 {
        Duration::from_millis(numer as u64)
    } else {
        Duration::from_micros((numer as u64 * 1000) / denom as u64)
    }
}

/// Accumulates per-cell wezterm `ImageCell` slices into rectangular placement blocks
/// while a snapshot is built row by row.
///
/// wezterm lays an image across a uniform grid of cells, each carrying the
/// source texcoord sub-rect for its slice, and clips whole rows/cols when the
/// image scrolls off-screen. So one block per `(hash, placement id, z-index)`
/// group, sampling from the first (top-left) cell's `top_left` to the last
/// (bottom-right) cell's `bottom_right`, reproduces the visible region exactly —
/// even partially scrolled. Rows arrive top-to-bottom and cells left-to-right,
/// so the first cell seen for a group is its top-left and the last is its
/// bottom-right.
#[derive(Default)]
pub(super) struct GraphicsCollector {
    groups: Vec<GraphicsGroup>,
    /// Index into `groups` keyed by `(image hash, placement id, z-index)`.
    index: HashMap<([u8; 32], Option<u32>, i32), usize>,
    /// Unique decoded images encountered, deduplicated by id.
    images: HashMap<u64, Arc<TerminalImage>>,
}

pub(super) struct GraphicsGroup {
    image_id: u64,
    z_index: i32,
    min_row: usize,
    min_col: usize,
    max_row: usize,
    max_col: usize,
    src_top_left: [f32; 2],
    src_bottom_right: [f32; 2],
}

impl GraphicsCollector {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add_cell(
        &mut self,
        hash: [u8; 32],
        placement_id: Option<u32>,
        z_index: i32,
        row: usize,
        col: usize,
        src_top_left: [f32; 2],
        src_bottom_right: [f32; 2],
        decoded: &Arc<TerminalImage>,
    ) {
        let image_id = decoded.id;
        self.images
            .entry(image_id)
            .or_insert_with(|| decoded.clone());

        match self.index.get(&(hash, placement_id, z_index)) {
            Some(&idx) => {
                let group = &mut self.groups[idx];
                group.min_row = group.min_row.min(row);
                group.min_col = group.min_col.min(col);
                group.max_row = group.max_row.max(row);
                group.max_col = group.max_col.max(col);
                // Cells arrive in row-major order, so the latest is the new
                // bottom-right corner.
                group.src_bottom_right = src_bottom_right;
            }
            None => {
                self.index
                    .insert((hash, placement_id, z_index), self.groups.len());
                self.groups.push(GraphicsGroup {
                    image_id,
                    z_index,
                    min_row: row,
                    min_col: col,
                    max_row: row,
                    max_col: col,
                    src_top_left,
                    src_bottom_right,
                });
            }
        }
    }

    pub(super) fn finish(self) -> (Vec<GraphicsPlacement>, Vec<TerminalImage>) {
        let placements = self
            .groups
            .into_iter()
            .map(|g| GraphicsPlacement {
                row: g.min_row,
                col: g.min_col,
                cols: g.max_col - g.min_col + 1,
                rows: g.max_row - g.min_row + 1,
                image_id: g.image_id,
                src_top_left: g.src_top_left,
                src_bottom_right: g.src_bottom_right,
                z_index: g.z_index,
            })
            .collect();
        let images = self.images.into_values().map(|a| (*a).clone()).collect();
        (placements, images)
    }
}
