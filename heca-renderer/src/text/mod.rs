//! **Text on the GPU**: one renderer that shapes text with cosmic-text, caches what it shaped and
//! placed, and draws glyph quads from a shared atlas.
//!
//! | File | What it owns |
//! |---|---|
//! | `style` | what a caller says about text: style, box, halo |
//! | `queue` | asking for text — one queued command per label or terminal run |
//! | `layout` | the cached layouts of shaped labels and runs |
//! | `emit` | turning queued text into glyph quads, and the cache of finished quads |
//! | `pipeline` | the GPU setup and drawing a frame |

mod emit;
mod layout;
mod pipeline;
mod queue;
mod shape;
mod style;
#[cfg(test)]
mod tests;

pub use style::{TextBox, TextGlow, TextStyle};

/// A monospace cell's width as a share of its font size, used only when the font does not say its
/// own — about what common coding fonts measure.
const FALLBACK_CELL_EM: f32 = 0.6;

use cosmic_text::{FontSystem, SwashCache, Weight};

use emit::{EmitKey, Emitted};
use layout::{LabelKey, LabelLayout, RunLayout};
use queue::TextCommand;

pub struct TextRenderer {
    /// Glyph-halo compositing mode for the current scene (see
    /// [`set_glow_alpha_scale`](TextRenderer::set_glow_alpha_scale)).
    glow_alpha_scale: f32,
    font_system: FontSystem,
    swash_cache: SwashCache,
    pipeline: wgpu::RenderPipeline,
    /// Stencil-test variant: same as `pipeline` but tests `Equal` against the
    /// rounded content-clip mask. Used when `render` is given a stencil view.
    stencil_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    scale_factor: f64,
    /// Physical framebuffer size `[w, h]` for clamping scissor rects.
    target_size: [u32; 2],
    /// Clip rect (logical px) applied to subsequently queued text; `None` = unclipped.
    current_clip: Option<[f32; 4]>,
    /// Frame-level damage region (logical px); each label is also scissored to it
    /// for damage-region redraw. `None` = full frame.
    damage: Option<[f32; 4]>,
    /// Vertices/indices already written to the persistent buffers this frame. Each
    /// pass appends at its own offset (via `queue.write_buffer`) rather than
    /// allocating a staging buffer per frame. Reset by `begin_frame`.
    frame_vtx: u32,
    /// Render passes begun since [`begin_frame`](Self::begin_frame): what the frame-time log reports.
    passes: u32,
    frame_idx: u32,
    commands: Vec<TextCommand>,
    font_family: String,
    icon_family: String,
    /// Shared glyph atlas — one texture holding every rasterized glyph.
    atlas: crate::atlas::Atlas,
    /// The single bind group (uniform + atlas + sampler) bound for all text draws.
    atlas_bind_group: wgpu::BindGroup,
    /// Shaped-label layout cache, keyed by [`LabelKey`]. Lets static text skip
    /// per-frame shaping; glyph bitmaps are cached separately in the atlas.
    label_cache: std::collections::HashMap<LabelKey, LabelLayout>,
    /// Shaped run-layout cache for terminal row-runs, keyed by [`LabelKey`]. Each
    /// entry stores per-glyph cell-column + intra-cell offsets so the emission can
    /// snap glyphs to the grid. Separate from `label_cache` because the placement
    /// model differs (cell-snapped, not ink-box-relative).
    run_layout_cache: std::collections::HashMap<LabelKey, RunLayout>,
    /// Retained emitted-geometry cache, keyed by [`EmitKey`]. Lets an *unchanged*
    /// label skip per-frame glyph placement — its quads are re-appended verbatim.
    emit_cache: std::collections::HashMap<EmitKey, Emitted>,
    /// Monotonic frame counter (advanced by [`begin_frame`](Self::begin_frame), so it
    /// counts visual frames, not render passes) driving cache eviction.
    frame: u64,
}

impl TextRenderer {
    /// Select how glyph halos composite for this scene, mirroring the rect
    /// renderer's `glow_alpha_scale`: `0.0` = additive (dark backgrounds), non-zero
    /// = translucent tinted halo (light backgrounds, where *adding* light to a
    /// near-white surface does nothing).
    ///
    /// Baked into the cached geometry, so a change invalidates the emission cache.
    pub fn set_glow_alpha_scale(&mut self, scale: f32) {
        self.glow_alpha_scale = scale;
    }

    pub fn set_scale_factor(&mut self, scale: f64) {
        if (scale - self.scale_factor).abs() <= f64::EPSILON {
            return;
        }
        self.scale_factor = scale;
        // Cached glyphs/layouts were rasterized for the old scale and are now wrong;
        // clear them. Clearing the atlas also keeps it from filling up as you zoom
        // through many scales (each scale is a distinct glyph set) — which otherwise
        // latched the atlas "full" and made all new text disappear.
        self.atlas.clear();
        self.label_cache.clear();
        self.run_layout_cache.clear();
        self.emit_cache.clear();
    }

    /// Physical framebuffer size in pixels; scissor rects are clamped to it.
    pub fn set_target_size(&mut self, width: u32, height: u32) {
        self.target_size = [width.max(1), height.max(1)];
    }

    /// Set the frame-level damage region (logical px) every label is scissored to,
    /// or `None` for a full-frame render. Set once per frame before `render`.
    pub fn set_damage(&mut self, damage: Option<[f32; 4]>) {
        self.damage = damage;
    }

    /// How many render passes this renderer has begun since [`begin_frame`](Self::begin_frame).
    pub fn passes(&self) -> u32 {
        self.passes
    }

    /// Reset the per-frame buffer write offsets and advance the frame counter. Call
    /// once at the start of each frame, before any `render` passes — the counter
    /// drives cache eviction, so it must count visual frames, not the (multiple)
    /// render passes within a frame.
    pub fn begin_frame(&mut self) {
        self.passes = 0;
        self.frame_vtx = 0;
        self.frame_idx = 0;
        self.frame += 1;
    }

    /// Set the clip rect (logical px, `[x, y, w, h]`) applied to subsequently
    /// queued text, or `None` to clear it.
    pub fn set_clip(&mut self, clip: Option<[f32; 4]>) {
        self.current_clip = clip;
    }

    pub fn set_font_family(&mut self, family: &str) {
        if self.font_family == family {
            return;
        }
        self.font_family = family.to_string();
        // Caches key on text/size/weight, not family — a family swap invalidates both.
        self.label_cache.clear();
        self.run_layout_cache.clear();
        self.emit_cache.clear();
    }

    /// Measure one logical terminal cell for the given monospace family/size.
    ///
    /// This uses the same shaping stack as runtime terminal text instead of
    /// theme heuristics, so PTY grid sizing can track the real loaded font.
    pub fn measure_monospace_cell(
        &mut self,
        font_size: f32,
        font_family: &str,
    ) -> Option<(f32, f32)> {
        if !font_size.is_finite() || font_size <= 0.0 {
            return None;
        }

        let scale = self.scale_factor as f32;
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let scaled_size = font_size * scale;
        let metrics = shape::metrics(scaled_size);
        let face = shape::Face {
            family: font_family,
            bold: false,
            italic: false,
            ligatures: true,
        };
        let buffer = shape::shape(&mut self.font_system, "M", scaled_size, &face);

        let mut line_width = None;
        let mut line_height = metrics.line_height;
        let mut font_id = None;
        if let Some(run) = buffer.layout_runs().next() {
            line_width = Some(run.line_w.max(0.0));
            line_height = run.line_height.max(metrics.line_height);
            font_id = run.glyphs.first().map(|glyph| glyph.font_id);
        }

        if let Some(font_id) = font_id
            && let Some(font) = self.font_system.get_font(font_id, Weight::NORMAL)
        {
            if let Some(monospace_em_width) = font.monospace_em_width() {
                line_width = Some(monospace_em_width * scaled_size);
            }

            let font_metrics = font.as_swash().metrics(&[]).scale(scaled_size);
            let font_line_height =
                (font_metrics.ascent + font_metrics.descent + font_metrics.leading).max(0.0);
            if font_line_height > 0.0 {
                line_height = font_line_height;
            }
        }

        let width = line_width.unwrap_or(scaled_size * FALLBACK_CELL_EM) / scale;
        let height = line_height.max(scaled_size) / scale;
        Some((width.max(1.0), height.max(1.0)))
    }
}
