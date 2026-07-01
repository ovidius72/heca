use super::TerminalLine;

/// Cursor state for a terminal snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Default,
    Block,
    Underline,
    Bar,
}

/// Cursor state for a terminal snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursor {
    pub col: usize,
    pub row: usize,
    pub visible: bool,
    pub shape: TerminalCursorShape,
}

/// Inclusive-exclusive row range describing invalidated visible terminal rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRowRange {
    pub start: usize,
    pub end: usize,
}

impl TerminalRowRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Damage metadata for terminal redraw scheduling.
///
/// `Full` means the entire visible viewport should be redrawn. `Rows` scopes
/// the redraw to the provided visible row ranges. `None` means there is no
/// pending visual change to acknowledge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalDamage {
    None,
    Full,
    Rows(Vec<TerminalRowRange>),
}

impl TerminalDamage {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// A run of consecutive cells on one visible row that share an OSC 8 hyperlink.
///
/// Capture-only extension point: the snapshot carries the link spans so a later
/// phase can underline them and open them on click. An empty `hyperlinks` list
/// means the renderer has nothing to do (links are opt-in; the renderer ignores
/// this until the hyperlink-open feature lands).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HyperlinkSpan {
    /// Visible row (`0` = top of the viewport).
    pub row: usize,
    /// First cell column of the run (inclusive).
    pub start_col: usize,
    /// One past the last cell column (exclusive).
    pub end_col: usize,
    /// Target URI from the OSC 8 sequence.
    pub uri: String,
}

/// Placement of an inline image/graphic in the terminal grid.
///
/// One placement is a single rectangular block of cells covered by one image
/// (one protocol-level placement). The backend coalesces the per-cell image
/// attachments wezterm produces (Sixel / iTerm2 `OSC 1337` / Kitty graphics all
/// funnel through the same per-cell path) into these blocks; the renderer blits
/// one textured quad per placement, sampling the source sub-rect described by
/// `src_top_left`/`src_bottom_right`.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphicsPlacement {
    /// Top-left cell of the placement (visible-row coordinates).
    pub row: usize,
    pub col: usize,
    /// Size of the placement in cells.
    pub cols: usize,
    pub rows: usize,
    /// Stable handle derived from the source image content hash. Matches a
    /// [`TerminalImage::id`] in the snapshot's `images` registry and is the
    /// renderer's texture-cache key.
    pub image_id: u64,
    /// Source-image texture coordinates (`0.0..=1.0`, top-left origin) at the
    /// block's top-left and bottom-right corners. The renderer samples this
    /// slice so a partially scrolled-off image still shows the correct region
    /// (wezterm clips whole rows/cols, so corner texcoords describe the visible
    /// slice exactly).
    pub src_top_left: [f32; 2],
    pub src_bottom_right: [f32; 2],
    /// Stacking order relative to text: `z < 0` draws under the glyphs, `z >= 0`
    /// over them.
    pub z_index: i32,
}

/// One frame of a decoded terminal image: its pixels plus how long it displays.
///
/// A static image is a single frame with a zero delay; an animated GIF / APNG /
/// `AnimRgba8` sequence has one entry per frame. All frames of an image share the
/// parent [`TerminalImage`]'s `width`/`height` (the decoders composite each frame
/// to the full canvas), so the renderer can reuse one same-sized GPU texture.
#[derive(Clone)]
pub struct TerminalImageFrame {
    /// Tightly packed RGBA8 pixels (`width * height * 4` bytes).
    pub rgba: std::sync::Arc<[u8]>,
    /// How long this frame is shown before the next one.
    pub delay: std::time::Duration,
}

/// Decoded RGBA image referenced by one or more [`GraphicsPlacement`]s.
///
/// The backend decodes each unique source image once (keyed by content hash)
/// and shares the frame buffers via `Arc`, so cloning a snapshot stays cheap. The
/// renderer uploads each `id` to a GPU texture once and reuses it across frames;
/// for an animated image it re-uploads the current frame as the clock advances.
#[derive(Clone)]
pub struct TerminalImage {
    /// Stable handle derived from the source content hash; matches
    /// [`GraphicsPlacement::image_id`].
    pub id: u64,
    pub width: u32,
    pub height: u32,
    /// One or more frames (always non-empty). A single frame = a static image.
    pub frames: std::sync::Arc<[TerminalImageFrame]>,
}

impl TerminalImage {
    /// Whether this image has more than one frame (an animation).
    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    /// Total loop duration (sum of every frame's delay).
    pub fn total_duration(&self) -> std::time::Duration {
        self.frames.iter().map(|f| f.delay).sum()
    }

    /// Index of the frame shown at `elapsed` since the animation started, looping.
    /// Returns `0` for a static image or a zero-duration loop.
    pub fn frame_index_at(&self, elapsed: std::time::Duration) -> usize {
        if self.frames.len() <= 1 {
            return 0;
        }
        let total = self.total_duration();
        if total.is_zero() {
            return 0;
        }
        let t = elapsed.as_nanos() % total.as_nanos();
        let mut acc: u128 = 0;
        for (i, frame) in self.frames.iter().enumerate() {
            acc += frame.delay.as_nanos();
            if t < acc {
                return i;
            }
        }
        self.frames.len() - 1
    }
}

impl std::fmt::Debug for TerminalImage {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_struct("TerminalImage")
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("frames", &self.frames.len())
            .finish()
    }
}

/// Renderer-agnostic visible terminal state.
///
/// This is intentionally a transitional contract: it gives the renderer enough
/// information to migrate away from the old full-frame `BackendRenderData`
/// transport while keeping terminal semantics out of GPU code.
///
/// Invariants:
/// - `lines.len() == rows`
/// - each line must describe at most `cols` visible cells
/// - `at_bottom` is true iff `viewport_offset == 0`
/// - `scrollback_rows` is the total number of retained content rows
///   (history + visible), so the maximum valid `viewport_offset` is
///   `scrollback_rows.saturating_sub(rows)`
/// - `viewport_top_stable_row` is the wezterm `StableRowIndex` of visible
///   row 0; converting a stable row `s` to a visible row is
///   `s - viewport_top_stable_row` (valid when in `[0, rows)`)
#[derive(Debug, Clone)]
pub struct TerminalSnapshot {
    pub cols: usize,
    pub rows: usize,
    pub cell_w: f32,
    pub cell_h: f32,
    pub default_fg: [f32; 4],
    pub default_bg: [f32; 4],
    pub cursor_color: [f32; 4],
    pub cursor: TerminalCursor,
    pub lines: Vec<TerminalLine>,
    /// Host viewport offset in rows above the live bottom (`0` = pinned to the
    /// live bottom; `N` = `N` rows of history visible below the cursor row).
    /// The backend is the rendering source of truth for this (see
    /// `terminal-01a` Q1); the app mirrors it into the chrome store for
    /// observability.
    pub viewport_offset: usize,
    /// Whether the viewport is pinned to the live bottom (`viewport_offset == 0`).
    pub at_bottom: bool,
    /// Total retained contents rows (history + visible). Scrollbar thumb sizing
    /// and the max viewport offset derive from this.
    pub scrollback_rows: usize,
    /// The wezterm `StableRowIndex` of visible row 0. Host-grid selections are
    /// stored in stable-row coordinates (anchored to content, survive viewport
    /// scroll); rendering and extraction convert via `visible = stable -
    /// viewport_top_stable_row`.
    pub viewport_top_stable_row: isize,
    /// OSC 8 hyperlink spans over the visible grid (empty = none). Capture-only
    /// extension point; the renderer ignores it until hyperlink opening lands.
    pub hyperlinks: Vec<HyperlinkSpan>,
    /// Inline image/graphic placements over the visible grid (empty = none).
    /// Each entry references a decoded image in `images` by `image_id`.
    pub graphics: Vec<GraphicsPlacement>,
    /// Decoded images referenced by `graphics`, deduplicated by content hash.
    /// Empty when the viewport has no inline images. Shares pixel buffers via
    /// `Arc` so snapshot clones stay cheap.
    pub images: Vec<TerminalImage>,
}

impl TerminalSnapshot {
    pub fn debug_assert_valid(&self) {
        debug_assert_eq!(
            self.lines.len(),
            self.rows,
            "terminal snapshot must contain one line per visible row"
        );
        debug_assert!(
            self.lines.iter().all(|line| line.cells.len() <= self.cols),
            "terminal snapshot lines must not exceed visible column count"
        );
        debug_assert_eq!(
            self.at_bottom,
            self.viewport_offset == 0,
            "terminal snapshot `at_bottom` must agree with `viewport_offset`"
        );
        debug_assert!(
            self.viewport_offset <= self.scrollback_rows.saturating_sub(self.rows),
            "terminal snapshot `viewport_offset` must stay within `[0, scrollback_rows - rows]`"
        );
    }
}

#[cfg(test)]
mod image_tests {
    use super::{TerminalImage, TerminalImageFrame};
    use std::sync::Arc;
    use std::time::Duration;

    fn img(delays_ms: &[u64]) -> TerminalImage {
        let frames: Vec<TerminalImageFrame> = delays_ms
            .iter()
            .map(|&ms| TerminalImageFrame {
                rgba: Arc::from(vec![0u8; 4].into_boxed_slice()),
                delay: Duration::from_millis(ms),
            })
            .collect();
        TerminalImage {
            id: 1,
            width: 1,
            height: 1,
            frames: Arc::from(frames.into_boxed_slice()),
        }
    }

    #[test]
    fn static_image_is_not_animated_and_stays_on_frame_zero() {
        let image = img(&[0]);
        assert!(!image.is_animated());
        assert_eq!(image.frame_index_at(Duration::from_secs(5)), 0);
    }

    #[test]
    fn animated_frame_index_advances_and_loops() {
        // Three frames of 100ms each; total loop = 300ms.
        let image = img(&[100, 100, 100]);
        assert!(image.is_animated());
        assert_eq!(image.total_duration(), Duration::from_millis(300));
        assert_eq!(image.frame_index_at(Duration::from_millis(0)), 0);
        assert_eq!(image.frame_index_at(Duration::from_millis(99)), 0);
        assert_eq!(image.frame_index_at(Duration::from_millis(100)), 1);
        assert_eq!(image.frame_index_at(Duration::from_millis(250)), 2);
        // Loops back to frame 0 after the total duration.
        assert_eq!(image.frame_index_at(Duration::from_millis(300)), 0);
        assert_eq!(image.frame_index_at(Duration::from_millis(450)), 1);
    }

    #[test]
    fn zero_duration_loop_stays_on_frame_zero() {
        // Degenerate: multiple frames but all zero delay — avoid divide-by-zero.
        let image = img(&[0, 0]);
        assert_eq!(image.frame_index_at(Duration::from_millis(10)), 0);
    }
}
