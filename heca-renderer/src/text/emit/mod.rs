//! **From queued text to glyph quads**: the per-frame loop, the cache of finished quads and how it
//! is keyed, and how long cached work is kept.

mod labels;
mod runs;

use heca_grid_ui::scene::TextAlign;

use super::{TextBox, TextGlow};
use super::TextRenderer;
use super::pipeline::{DrawRange, TextVertex};
use crate::clip::{combine_clip, intersect};

/// **One glyph's quad**: its four corners over `rect` (logical px), leaning by `slant` — the top
/// moves right by half of it and the bottom left by half — sampling `uv` and tinted `color`.
/// `additive` is [`TextVertex::additive`]: `1.0` for a halo that adds light.
pub(super) fn glyph_quad(
    rect: TextBox,
    slant: f32,
    uv: [f32; 4],
    color: [f32; 4],
    additive: f32,
) -> [TextVertex; 4] {
    let [u0, v0, u1, v1] = uv;
    let top_x = rect.x + slant * 0.5;
    let bottom_x = rect.x - slant * 0.5;
    let corner = |position, texcoord| TextVertex {
        position,
        texcoord,
        color,
        additive,
    };
    [
        corner([top_x, rect.y], [u0, v0]),
        corner([top_x + rect.w, rect.y], [u1, v0]),
        corner([bottom_x + rect.w, rect.y + rect.h], [u1, v1]),
        corner([bottom_x, rect.y + rect.h], [u0, v1]),
    ]
}

/// **The box a set of glyph quads covers**, grown as each is added — what damage and clip culling
/// test an emitted label against.
pub(super) struct Ink {
    min: [f32; 2],
    max: [f32; 2],
}

impl Ink {
    pub(super) fn new() -> Self {
        Self {
            min: [f32::MAX; 2],
            max: [f32::MIN; 2],
        }
    }

    /// Take in one glyph over `rect`, leaning by `slant` as [`glyph_quad`] draws it.
    pub(super) fn add(&mut self, rect: TextBox, slant: f32) {
        self.min[0] = self.min[0].min(rect.x - slant * 0.5);
        self.min[1] = self.min[1].min(rect.y);
        self.max[0] = self.max[0].max(rect.x + slant * 0.5 + rect.w);
        self.max[1] = self.max[1].max(rect.y + rect.h);
    }

    /// The covered box `[x, y, w, h]`, grown by `reach` on every side (a halo's).
    pub(super) fn bbox(&self, reach: f32) -> [f32; 4] {
        [
            self.min[0] - reach,
            self.min[1] - reach,
            (self.max[0] - self.min[0]) + reach * 2.0,
            (self.max[1] - self.min[1]) + reach * 2.0,
        ]
    }
}

/// Identity of an **emitted** label: the full set of command inputs that determine
/// its final screen-space geometry — content + box + color + scale. Same key ⇒ byte-
/// identical quads, so the placed vertices are cached across frames and a static
/// label is re-appended (a `memcpy`) instead of re-placed glyph-by-glyph. This is the
/// retained-geometry layer on top of [`LabelLayout`](crate::text::layout::LabelLayout) (which
/// caches only *shaping*): only labels whose inputs actually changed (moved/recolored/new text) get
/// rebuilt.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(super) struct EmitKey {
    pub(super) text: String,
    /// Scaled font size in `f32::to_bits()` form (exact, hashable).
    pub(super) size_bits: u32,
    pub(super) bold: bool,
    pub(super) icon: bool,
    pub(super) italic: bool,
    pub(super) faux_italic: bool,
    /// Baked vertex color (`f32::to_bits()` per channel).
    pub(super) color_bits: [u32; 4],
    /// Command box `(x, y, w, h)` in `f32::to_bits()` form.
    pub(super) box_bits: [u32; 4],
    /// `TextAlign` discriminant (0/1/2).
    pub(super) align: u8,
    pub(super) centered: bool,
    pub(super) line_box: bool,
    pub(super) font_family: Option<String>,
    /// Scale factor in `f32::to_bits()` form.
    pub(super) scale_bits: u32,
    /// True for terminal row-runs (cell-snapped emission path).
    pub(super) run: bool,
    /// Cell width in `f32::to_bits()` form for terminal runs (`0` otherwise);
    /// it drives per-glyph cell snapping, so it must be part of the identity.
    pub(super) cell_w_bits: u32,
    /// Hash of a terminal run's per-column colors (`0` otherwise). The colors are
    /// baked into the vertices, so a recolor (e.g. syntax highlighting changing)
    /// must miss the retained-geometry cache and rebuild.
    pub(super) run_colors_hash: u64,
    /// Whether ligatures are applied (terminal runs only; `true` otherwise).
    /// Toggling it changes the shaped glyphs, so it must be part of the identity.
    pub(super) ligatures: bool,
    /// Halo composite mode (`glow_alpha_scale`) in `f32::to_bits()` form, or `0`
    /// when the run has no visible halo.
    pub(super) glow_mode_bits: u32,
    /// Halo colour/radius/intensity in `f32::to_bits()` form (`None` → all zero).
    /// The halo quads are baked into the cached vertices, so changing the glow must
    /// miss the retained-geometry cache and rebuild.
    pub(super) glow_bits: [u32; 6],
}

/// `Option<TextGlow>` → stable bits for [`EmitKey`]. An inert glow
/// ([`TextGlow::is_visible`]) hashes identically to `None`, because both emit no
/// halo and must therefore share cached geometry rather than rebuild it.
pub(super) fn glow_bits(glow: Option<TextGlow>) -> [u32; 6] {
    match glow.filter(|g| g.is_visible()) {
        Some(g) => [
            g.color[0].to_bits(),
            g.color[1].to_bits(),
            g.color[2].to_bits(),
            g.color[3].to_bits(),
            g.radius.to_bits(),
            g.intensity.to_bits(),
        ],
        None => [0; 6],
    }
}

/// A label's fully-placed glyph quads in screen space, cached across frames. The
/// vertices are absolute (logical px) with color already baked in, so emitting a
/// cached label is just appending these (no per-glyph placement, no atlas lookups).
pub(super) struct Emitted {
    /// Quad vertices, 4 per glyph; indices are regenerated on append (6 per quad).
    pub(super) verts: Vec<TextVertex>,
    /// Screen-space ink bounds `[x, y, w, h]` (logical px) for damage/clip culling.
    pub(super) bbox: [f32; 4],
    /// Last frame this emission was used, for eviction of stale (dynamic) text.
    pub(super) last_used: u64,
}

/// `TextAlign` → a stable discriminant for hashing into [`EmitKey`].
pub(super) fn align_bits(align: TextAlign) -> u8 {
    match align {
        TextAlign::Start => 0,
        TextAlign::Center => 1,
        TextAlign::End => 2,
    }
}

/// FNV-1a hash of a terminal run's per-column colors, folded into the [`EmitKey`]
/// so a recolored run (same text/position) misses the retained-geometry cache.
pub(super) fn hash_colors(colors: &[[f32; 4]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for c in colors {
        for ch in c {
            h ^= ch.to_bits() as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    // A genuinely empty run hashes to a non-zero sentinel so it never collides
    // with the "not a run" `0`.
    if colors.is_empty() { 1 } else { h }
}

/// Drop shaped layouts unused for this many frames (~a few seconds) so dynamic text
/// (counters, clocks) doesn't leak cache entries. Glyph bitmaps stay in the atlas.
const LABEL_EVICT_AFTER: u64 = 240;
/// Drop emitted geometry unused for this many frames. Position-keyed, so an animating
/// (moving) label churns entries every frame — evict promptly to keep the cache to
/// roughly the set of on-screen labels.
const EMIT_EVICT_AFTER: u64 = 4;

impl TextRenderer {
    /// Render all queued text. Returns (vertices, indices, labels) for drawing.
    /// Shape every queued label and emit per-glyph quads, pulling glyph bitmaps from
    /// the shared atlas. A label's layout (its placed glyphs) is cached so static
    /// text isn't re-shaped; the glyphs themselves are cached in the atlas, so *new*
    /// text only rasterizes glyphs it hasn't seen — no per-label texture.
    pub(super) fn build_labels(&mut self, queue: &wgpu::Queue) -> (Vec<TextVertex>, Vec<u32>, Vec<DrawRange>) {
        let commands = std::mem::take(&mut self.commands);
        if commands.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }

        let scale = self.scale_factor as f32;
        let mut vertices: Vec<TextVertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut draws: Vec<DrawRange> = Vec::new();
        let frame = self.frame;

        for cmd in &commands {
            let scaled_size = cmd.font_size * scale;
            let emit_key = EmitKey {
                glow_bits: glow_bits(cmd.glow),
                // The halo's colour AND its composite mode are baked into the
                // cached vertices, so switching theme brightness must rebuild.
                glow_mode_bits: cmd
                    .glow
                    .filter(|g| g.is_visible())
                    .map_or(0, |_| self.glow_alpha_scale.to_bits()),
                text: cmd.text.clone(),
                size_bits: scaled_size.to_bits(),
                bold: cmd.bold,
                icon: cmd.icon,
                italic: cmd.italic,
                faux_italic: cmd.faux_italic,
                color_bits: cmd.color.map(f32::to_bits),
                box_bits: [cmd.x, cmd.y, cmd.w, cmd.h].map(f32::to_bits),
                align: align_bits(cmd.align),
                centered: cmd.centered,
                line_box: cmd.line_box,
                font_family: cmd.font_family.clone(),
                scale_bits: scale.to_bits(),
                run: cmd.run.is_some(),
                cell_w_bits: cmd.run.as_ref().map_or(0, |r| r.cell_w.to_bits()),
                run_colors_hash: cmd.run.as_ref().map_or(0, |r| hash_colors(&r.col_colors)),
                ligatures: cmd.run.as_ref().is_none_or(|r| r.ligatures),
            };

            // Retained-geometry miss → place every glyph (shaping itself is cached in
            // `label_cache`, glyph bitmaps in the atlas) and stash the final quads. A
            // hit skips straight to the append below — no per-glyph work.
            if !self.emit_cache.contains_key(&emit_key) {
                let emitted = self.build_emission(queue, cmd, scaled_size, scale);
                self.emit_cache.insert(emit_key.clone(), emitted);
            }

            let emitted = self.emit_cache.get_mut(&emit_key).expect("just inserted");
            emitted.last_used = frame;
            if emitted.verts.is_empty() {
                continue; // nothing to draw (whitespace/empty)
            }

            // Cull: a label entirely outside this frame's damage∩clip region needs no
            // geometry at all (the per-draw scissor in `render` would clip it away
            // anyway, but emitting + uploading it is wasted work — the whole point on a
            // small-damage frame like a caret blink). Skip only when *provably* outside.
            if let Some(c) = combine_clip(self.damage, cmd.clip) {
                let i = intersect(c, emitted.bbox);
                if i[2] <= 0.0 || i[3] <= 0.0 {
                    continue;
                }
            }

            let first_index = indices.len() as u32;
            let mut base = vertices.len() as u32;
            let quads = emitted.verts.chunks_exact(4);
            debug_assert!(
                quads.remainder().is_empty(),
                "emitted terminal/text geometry must contain 4 vertices per glyph quad"
            );
            for quad in quads {
                vertices.extend_from_slice(quad);
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
                base += 4;
            }
            draws.push(DrawRange {
                clip: cmd.clip,
                first_index,
                index_count: indices.len() as u32 - first_index,
            });
        }

        // Evict cache entries unused for a while so dynamic/animating text doesn't grow
        // either cache unbounded. Atlas glyphs are kept (a bounded set).
        self.emit_cache
            .retain(|_, e| frame.saturating_sub(e.last_used) <= EMIT_EVICT_AFTER);
        self.label_cache
            .retain(|_, l| frame.saturating_sub(l.last_used) <= LABEL_EVICT_AFTER);
        self.run_layout_cache
            .retain(|_, l| frame.saturating_sub(l.last_used) <= LABEL_EVICT_AFTER);

        (vertices, indices, draws)
    }
}
