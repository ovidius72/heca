//! **One label to glyph quads**: shape it on a cache miss, place it in its box, and add its halo.

use cosmic_text::CacheKey;
use heca_grid_ui::scene::TextAlign;

use super::{Emitted, Ink, glyph_quad};
use crate::text::TextBox;
use crate::text::shape;
use crate::text::layout::{LabelKey, LabelLayout, PlacedGlyph};
use crate::text::pipeline::TextVertex;
use crate::text::queue::TextCommand;
use crate::text::TextRenderer;
use crate::text::style::{HALO_DARK_GAIN, HALO_LIGHT_GAIN, HALO_SIGMA_FRAC};

impl TextRenderer {
    /// Place one label's glyphs into final screen-space quads (logical px, color
    /// baked). Shaping is reused from `label_cache` on a hit; a miss shapes the text
    /// and rasterizes any not-yet-seen glyphs into the atlas. Returns the [`Emitted`]
    /// quads + their ink bbox for the retained-geometry cache.
    pub(super) fn build_emission(
        &mut self,
        queue: &wgpu::Queue,
        cmd: &TextCommand,
        scaled_size: f32,
        scale: f32,
    ) -> Emitted {
        if cmd.run.is_some() {
            return self.build_run_emission(queue, cmd, scaled_size, scale);
        }
        let key = LabelKey {
            text: cmd.text.clone(),
            size_bits: scaled_size.to_bits(),
            bold: cmd.bold,
            icon: cmd.icon,
            italic: cmd.italic,
            font_family: cmd.font_family.clone(),
            ligatures: true,
        };

        // Shape on a `label_cache` miss; rasterize each glyph into the atlas on its
        // first sighting.
        if !self.label_cache.contains_key(&key) {
            let family = if cmd.icon {
                self.icon_family.as_str()
            } else {
                cmd.font_family.as_deref().unwrap_or(&self.font_family)
            };
            let face = shape::Face {
                family,
                bold: cmd.bold,
                italic: cmd.italic,
                ligatures: true,
            };
            let buffer = shape::shape(&mut self.font_system, &cmd.text, scaled_size, &face);

            // Stable, glyph-independent line metrics for vertical centering so the
            // baseline doesn't jump with ascenders/descenders.
            let mut min_x = i32::MAX;
            let mut min_y = i32::MAX;
            let mut max_x = i32::MIN;
            let mut line_top = 0.0f32;
            let mut line_height = 0.0f32;
            // (glyph left, glyph top in physical px, atlas entry).
            let mut placed: Vec<(i32, i32, crate::atlas::AtlasGlyph, CacheKey)> = Vec::new();
            for run in buffer.layout_runs() {
                line_top = run.line_top;
                line_height = run.line_height;
                for glyph in run.glyphs {
                    let physical = glyph.physical((0.0, run.line_y), 1.0);
                    let Some(ag) = self.atlas.glyph(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        physical.cache_key,
                    ) else {
                        continue; // whitespace / no bitmap
                    };
                    let left = physical.x + ag.left;
                    let top = physical.y - ag.top;
                    min_x = min_x.min(left);
                    min_y = min_y.min(top);
                    max_x = max_x.max(left + ag.width as i32);
                    placed.push((left, top, ag, physical.cache_key));
                }
            }

            let layout = if placed.is_empty() {
                // Empty/whitespace — cache an empty layout so we don't re-shape it.
                LabelLayout {
                    glyphs: Vec::new(),
                    content_w: 0,
                    line_top,
                    line_height,
                    min_y: 0,
                    last_used: self.frame,
                }
            } else {
                let glyphs = placed
                    .into_iter()
                    .map(|(l, t, ag, cache_key)| PlacedGlyph {
                        rel_x: (l - min_x) as f32,
                        rel_y: (t - min_y) as f32,
                        w: ag.width as f32,
                        h: ag.height as f32,
                        uv: ag.uv,
                        cache_key,
                    })
                    .collect();
                LabelLayout {
                    glyphs,
                    content_w: (max_x - min_x) as u32,
                    line_top,
                    line_height,
                    min_y,
                    last_used: self.frame,
                }
            };
            self.label_cache.insert(key.clone(), layout);
        }

        let layout = self.label_cache.get_mut(&key).expect("just inserted");
        layout.last_used = self.frame;
        if layout.glyphs.is_empty() {
            return Emitted {
                verts: Vec::new(),
                bbox: [0.0; 4],
                last_used: self.frame,
            };
        }

        // Centering: place the label ink box's top-left — horizontally per `align` on
        // the ink width, vertically on the stable line box — then offset each glyph by
        // its (cached) position within the ink box.
        let screen_w = layout.content_w as f32 / scale;
        let (screen_x, screen_y) = if cmd.centered || cmd.line_box {
            let x = cmd.x
                + match cmd.align {
                    TextAlign::Start => 0.0,
                    TextAlign::Center => (cmd.w - screen_w) * 0.5,
                    TextAlign::End => cmd.w - screen_w,
                };
            let line_box_top = cmd.y + (cmd.h - layout.line_height / scale) * 0.5;
            let ink_offset = (layout.min_y as f32 - layout.line_top) / scale;
            (x, line_box_top + ink_offset)
        } else {
            (cmd.x, cmd.y)
        };

        let slant = shape::faux_italic_slant(layout.line_height, scale, cmd.faux_italic);
        // Where a glyph's quad goes on screen, from its place in the ink box (physical px). Takes
        // explicit geometry because the sharp glyph and its halo sample different atlas entries
        // at different sizes.
        let on_screen = |rel_x: f32, rel_y: f32, w: f32, h: f32| TextBox {
            x: screen_x + rel_x / scale,
            y: screen_y + rel_y / scale,
            w: w / scale,
            h: h / scale,
        };

        let halo = cmd.glow.filter(|g| g.is_visible());
        let mut verts: Vec<TextVertex> =
            Vec::with_capacity(layout.glyphs.len() * 4 * (1 + usize::from(halo.is_some())));

        // Halo first, so the sharp glyphs land on top of it. ONE quad per glyph,
        // sampling a pre-blurred copy of that glyph's coverage mask — a continuous
        // falloff, rather than a stack of offset copies whose shells read as spokes.
        let mut halo_reach = 0.0f32;
        if let Some(g) = halo {
            // Light backgrounds take a translucent tinted halo; dark ones take added
            // light. Adding light to a near-white surface does nothing, which is why
            // the rect renderer makes the same switch (`grid.wgsl`'s alpha path).
            let tinted = self.glow_alpha_scale > 0.0;
            let additive = if tinted { 0.0 } else { 1.0 };
            let base = g.color[3] * g.intensity;
            // Because the blurred mask is renormalized to full peak, `base` IS the
            // halo's peak alpha — the same quantity the rect shader's glow peaks at,
            // so the two are directly comparable rather than eyeballed.
            let alpha = if tinted {
                // Mirror the rect shader's light-background curve (`grid.wgsl`):
                // the square root lifts a faint intensity into a visible tint, which
                // a linear scale cannot do against a near-white surface.
                base.sqrt() * self.glow_alpha_scale * HALO_LIGHT_GAIN
            } else {
                base * HALO_DARK_GAIN
            }
            .clamp(0.0, 1.0);
            let color = [g.color[0], g.color[1], g.color[2], alpha];
            // Sigma is in physical px, like everything the atlas rasterizes.
            let sigma = g.radius * HALO_SIGMA_FRAC * scale;
            halo_reach = g.radius;

            // Collected first: the atlas needs `&mut self` while `push_quad` borrows
            // the frame's placement values.
            let blurred: Vec<(usize, crate::atlas::AtlasGlyph)> = layout
                .glyphs
                .iter()
                .enumerate()
                .filter_map(|(i, glyph)| {
                    let entry = self.atlas.glyph_blurred(
                        queue,
                        &mut self.font_system,
                        &mut self.swash_cache,
                        glyph.cache_key,
                        sigma,
                    )?;
                    Some((i, entry))
                })
                .collect();

            for (i, entry) in blurred {
                let glyph = &layout.glyphs[i];
                // The blurred bitmap is the glyph's, grown by the blur's padding on
                // every side; recover that padding to place it concentrically.
                let pad = (entry.width as f32 - glyph.w) * 0.5;
                let rect = on_screen(
                    glyph.rel_x - pad,
                    glyph.rel_y - pad,
                    entry.width as f32,
                    entry.height as f32,
                );
                verts.extend(glyph_quad(rect, slant, entry.uv, color, additive));
            }
        }

        let mut ink = Ink::new();
        for g in &layout.glyphs {
            let rect = on_screen(g.rel_x, g.rel_y, g.w, g.h);
            verts.extend(glyph_quad(rect, slant, g.uv, cmd.color, 0.0));
            ink.add(rect, slant);
        }

        // The ink box must contain the halo too, or damage/clip culling would
        // scissor away the very light the halo adds outside the glyph box.
        Emitted {
            verts,
            bbox: ink.bbox(halo_reach),
            last_used: self.frame,
        }
    }
}
