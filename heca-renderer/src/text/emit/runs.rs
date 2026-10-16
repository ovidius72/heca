//! **One terminal row-run to glyph quads**: shape the whole run so ligatures can form, then snap
//! every glyph back to its cell.

use super::{Emitted, Ink, glyph_quad};
use crate::text::TextBox;
use crate::text::shape;
use crate::text::TextRenderer;
use crate::text::layout::{LabelKey, RunGlyph, RunLayout};
use crate::text::pipeline::TextVertex;
use crate::text::queue::TextCommand;

impl TextRenderer {
    /// Emit one terminal row-run: shape the whole run text together (so OpenType
    /// ligatures/contextual alternates can form across cells), then snap every
    /// glyph back to its originating cell column so the monospace grid is exact.
    /// Shaping is reused from `run_layout_cache` on a hit.
    pub(super) fn build_run_emission(
        &mut self,
        queue: &wgpu::Queue,
        cmd: &TextCommand,
        scaled_size: f32,
        scale: f32,
    ) -> Emitted {
        let run = cmd
            .run
            .as_ref()
            .expect("build_run_emission requires run info");
        let key = LabelKey {
            text: cmd.text.clone(),
            size_bits: scaled_size.to_bits(),
            bold: cmd.bold,
            icon: false,
            italic: cmd.italic,
            font_family: cmd.font_family.clone(),
            ligatures: run.ligatures,
        };

        if !self.run_layout_cache.contains_key(&key) {
            // The run's whole text is shaped at once, so ligatures can form across cells.
            let face = shape::Face {
                family: cmd.font_family.as_deref().unwrap_or(&self.font_family),
                bold: cmd.bold,
                italic: cmd.italic,
                ligatures: run.ligatures,
            };
            let buffer = shape::shape(&mut self.font_system, &cmd.text, scaled_size, &face);

            // First pass: collect raw placements (cell column, pen x, bearing, top).
            // `glyph.start` is a byte offset into the run text; `byte_cols` maps it to
            // the cell column the glyph anchors to (a ligature anchors to its first
            // cell). We keep cosmic's natural pen x only to recover the glyph's offset
            // *within* its cell — the absolute pen is discarded for the grid snap.
            let last_col = run.byte_cols.last().copied().unwrap_or(0);
            let mut line_top = 0.0f32;
            let mut line_height = 0.0f32;
            // (col, pen_x, left, top, atlas glyph)
            let mut raw: Vec<(u16, i32, i32, i32, crate::atlas::AtlasGlyph)> = Vec::new();
            for layout_run in buffer.layout_runs() {
                line_top = layout_run.line_top;
                line_height = layout_run.line_height;
                for glyph in layout_run.glyphs {
                    let physical = glyph.physical((0.0, layout_run.line_y), 1.0);
                    let Some(ag) = self.atlas.glyph(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        physical.cache_key,
                    ) else {
                        continue; // whitespace / no bitmap
                    };
                    let col = run.byte_cols.get(glyph.start).copied().unwrap_or(last_col);
                    let top = physical.y - ag.top;
                    raw.push((col, physical.x, ag.left, top, ag));
                }
            }

            // Per-cell anchor pen: the smallest pen x among glyphs in that cell, so a
            // glyph's intra-cell offset is `pen_x - anchor + bearing`. For the common
            // one-glyph-per-cell case this is just the side bearing.
            let mut anchor: std::collections::HashMap<u16, i32> = std::collections::HashMap::new();
            for &(col, pen_x, _, _, _) in &raw {
                anchor
                    .entry(col)
                    .and_modify(|a| *a = (*a).min(pen_x))
                    .or_insert(pen_x);
            }

            let glyphs = raw
                .into_iter()
                .map(|(col, pen_x, left, top, ag)| {
                    let off_x = (pen_x - anchor.get(&col).copied().unwrap_or(pen_x) + left) as f32;
                    RunGlyph {
                        col,
                        off_x,
                        top: top as f32,
                        w: ag.width as f32,
                        h: ag.height as f32,
                        uv: ag.uv,
                    }
                })
                .collect();

            self.run_layout_cache.insert(
                key.clone(),
                RunLayout {
                    glyphs,
                    line_top,
                    line_height,
                    last_used: self.frame,
                },
            );
        }

        let layout = self.run_layout_cache.get_mut(&key).expect("just inserted");
        layout.last_used = self.frame;
        if layout.glyphs.is_empty() {
            return Emitted {
                verts: Vec::new(),
                bbox: [0.0; 4],
                last_used: self.frame,
            };
        }

        // Vertical: center the stable line box within the cell height, then place each
        // glyph by its top relative to the line top (matches the generic line-box path,
        // so baselines don't jump with ascenders/descenders).
        let line_box_top = cmd.y + (cmd.h - layout.line_height / scale) * 0.5;
        let cell_w = run.cell_w;
        let slant = shape::faux_italic_slant(layout.line_height, scale, cmd.faux_italic);

        let mut verts: Vec<TextVertex> = Vec::with_capacity(layout.glyphs.len() * 4);
        let mut ink = Ink::new();
        for g in &layout.glyphs {
            // Grid snap: cell column → exact x, plus the glyph's intra-cell offset.
            let rect = TextBox {
                x: cmd.x + g.col as f32 * cell_w + g.off_x / scale,
                y: line_box_top + (g.top - layout.line_top) / scale,
                w: g.w / scale,
                h: g.h / scale,
            };
            // Color each glyph by its own cell, so a ligature crossing a color
            // boundary keeps each half tinted as its source cell.
            let color = run
                .col_colors
                .get(g.col as usize)
                .copied()
                .unwrap_or(cmd.color);
            verts.extend(glyph_quad(rect, slant, g.uv, color, 0.0));
            ink.add(rect, slant);
        }
        Emitted {
            verts,
            bbox: ink.bbox(0.0),
            last_used: self.frame,
        }
    }
}
