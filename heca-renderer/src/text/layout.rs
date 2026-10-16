//! **The cached layouts**: where a shaped label's or terminal run's glyphs go, kept across frames
//! so static text is not shaped again.

use cosmic_text::CacheKey;

/// Identity of a shaped label: same text + size + weight + family produces the same
/// glyph layout, so the layout is cached and reused across frames. Color is **not**
/// part of the key — glyph masks are tinted per-draw in the shader (vertex color).
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct LabelKey {
    pub(super) text: String,
    /// Scaled font size in `f32::to_bits()` form (exact, hashable).
    pub(super) size_bits: u32,
    pub(super) bold: bool,
    pub(super) icon: bool,
    pub(super) italic: bool,
    pub(super) font_family: Option<String>,
    /// Whether `calt`/`liga`/`clig` are applied (terminal runs can disable them).
    /// Always `true` for the generic UI/chrome path (default features on).
    pub(super) ligatures: bool,
}

/// One placed glyph within a label: its quad (relative to the label's ink-box
/// top-left, physical px) and the atlas region to sample. The glyph bitmaps live in
/// the shared [`Atlas`](crate::atlas::Atlas), so caching a label is just caching
/// where its glyphs go — no per-label texture.
#[derive(Clone, Copy)]
pub(super) struct PlacedGlyph {
    pub(super) rel_x: f32,
    pub(super) rel_y: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) uv: [f32; 4],
    /// Identity of the rasterized glyph, kept so a halo can ask the atlas for a
    /// **blurred** variant of this same glyph at emission time. The layout itself is
    /// cached independently of any glow, so the blur cannot be resolved earlier.
    pub(super) cache_key: CacheKey,
}

/// A shaped label's cached layout: its glyph quads + the metrics needed to place
/// the whole label. A cache hit re-emits the quads (cheap); only a **miss** shapes
/// the text and rasterizes any not-yet-seen glyphs into the atlas.
pub(super) struct LabelLayout {
    pub(super) glyphs: Vec<PlacedGlyph>,
    /// Ink width in physical px (`0` = empty/whitespace), for horizontal alignment.
    pub(super) content_w: u32,
    /// Stable line-box metrics (physical px) for baseline-anchored centering.
    pub(super) line_top: f32,
    pub(super) line_height: f32,
    /// Top of the ink box (physical px) for the ink offset within the line box.
    pub(super) min_y: i32,
    /// Last frame this label was used, for eviction of stale (dynamic) text.
    pub(super) last_used: u64,
}

/// One placed glyph within a shaped terminal run. Horizontal position is split
/// into a cell column (snapped to the grid at emission via `col * cell_w`) plus an
/// intra-cell offset in physical px (the glyph's natural offset from its cell's
/// pen origin, i.e. side bearing + any within-cell shaping). A ligature is a
/// single glyph whose `col` is its first cell; it then renders across the cells it
/// spans without disturbing the grid.
#[derive(Clone, Copy)]
pub(super) struct RunGlyph {
    /// Cell column offset (relative to the run start) this glyph snaps to.
    pub(super) col: u16,
    /// Intra-cell horizontal offset (physical px) added after the cell snap.
    pub(super) off_x: f32,
    /// Glyph top in physical px relative to the run's line top.
    pub(super) top: f32,
    pub(super) w: f32,
    pub(super) h: f32,
    pub(super) uv: [f32; 4],
}

/// A shaped terminal run's cached layout: per-glyph cell-snapped placement plus
/// the stable line metrics used to center within the cell box. Emission applies
/// `cell_w` and the run origin; only a cache miss re-shapes the run.
pub(super) struct RunLayout {
    pub(super) glyphs: Vec<RunGlyph>,
    /// Stable line-box metrics (physical px) for baseline-anchored centering.
    pub(super) line_top: f32,
    pub(super) line_height: f32,
    /// Last frame this run was used, for eviction of stale (dynamic) text.
    pub(super) last_used: u64,
}
