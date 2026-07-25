//! Glyph atlas: one shared R8 alpha texture holding rasterized glyph bitmaps.
//!
//! The text renderer used to rasterize each **label** into its own GPU texture +
//! bind group, keyed by the whole string. That made *new* text expensive (a texture
//! per label) and can't scale to an editor, where every line is a unique string and
//! every keystroke/scroll produces new labels. The atlas caches individual
//! **glyphs** (by font-system cache key) in one texture, so labels are composed from
//! already-rasterized glyphs — new text only rasterizes glyphs it hasn't seen.

use cosmic_text::{CacheKey, FontSystem, SwashCache};
use std::collections::HashMap;

/// Atlas texture side length (px). R8 → `SIZE²` bytes; 2048 ≈ 4 MB, thousands of
/// glyphs (a UI/editor uses a small, bounded glyph set per size).
const SIZE: u32 = 2048;
/// 1px gutter between glyphs so linear sampling never bleeds a neighbor.
const PAD: u32 = 1;

/// A glyph's region in the atlas plus its bitmap placement (offset from the pen).
#[derive(Clone, Copy)]
pub struct AtlasGlyph {
    /// `[u0, v0, u1, v1]` in `[0, 1]` atlas coordinates.
    pub uv: [f32; 4],
    /// Bitmap left/top offset from the pen origin (physical px).
    pub left: i32,
    pub top: i32,
    /// Bitmap size (physical px).
    pub width: u32,
    pub height: u32,
}

/// Shared glyph atlas with a simple shelf packer.
pub struct Atlas {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Shelf packer cursor.
    pen_x: u32,
    pen_y: u32,
    shelf_h: u32,
    /// Cached glyphs; `None` = the glyph has no bitmap (whitespace).
    glyphs: HashMap<CacheKey, Option<AtlasGlyph>>,
    /// Cached **blurred** glyph variants for halos, keyed by `(glyph, sigma bucket)`.
    blurred: HashMap<(CacheKey, u32), Option<AtlasGlyph>>,
    full: bool,
}

impl Atlas {
    pub fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            pen_x: PAD,
            pen_y: PAD,
            shelf_h: 0,
            glyphs: HashMap::new(),
            blurred: HashMap::new(),
            full: false,
        }
    }

    /// The atlas texture view, bound once for all text draws.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Drop every cached glyph and rewind the shelf packer (the texture itself is
    /// left as-is — stale pixels are never referenced once the UV map is cleared).
    /// Used when the render scale changes (zoom / HiDPI): old-scale glyphs would
    /// never be reused and, left to accumulate, would fill the atlas and make text
    /// vanish. The caller must also drop any caches holding [`AtlasGlyph`] UVs.
    pub fn clear(&mut self) {
        self.blurred.clear();
        self.pen_x = PAD;
        self.pen_y = PAD;
        self.shelf_h = 0;
        self.glyphs.clear();
        self.full = false;
    }

    /// The atlas entry for `key`, rasterizing + inserting it on a miss. `None` for a
    /// glyph with no bitmap (whitespace) or if the atlas is full.
    pub fn glyph(
        &mut self,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
    ) -> Option<AtlasGlyph> {
        if let Some(g) = self.glyphs.get(&key) {
            return *g;
        }
        let entry = self.rasterize(queue, font_system, swash, key);
        self.glyphs.insert(key, entry);
        entry
    }

    fn rasterize(
        &mut self,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
    ) -> Option<AtlasGlyph> {
        let img = swash.get_image(font_system, key).as_ref()?.clone();
        self.upload(
            queue,
            &img.data,
            img.placement.width,
            img.placement.height,
            img.placement.left,
            img.placement.top,
        )
    }

    /// A **blurred** copy of `key`'s coverage mask, for a glyph halo.
    ///
    /// This is what makes a glyph glow look like every other glow in the UI. The
    /// alternative — drawing the glyph many times around a ring — produces discrete
    /// shells you can see as spokes and arcs, and it gets worse as the radius grows
    /// because a fixed tap count spreads thinner. A blurred mask is a *continuous*
    /// falloff by construction, costs **one** quad, and scales with `sigma` for free.
    ///
    /// The result is its own atlas entry keyed by `(glyph, sigma bucket)`, so the blur
    /// is computed once per distinct halo and then reused like any cached glyph.
    /// `sigma` is bucketed to a quarter pixel to bound how many variants a sliding
    /// `glow_size` can create.
    pub fn glyph_blurred(
        &mut self,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
        sigma: f32,
    ) -> Option<AtlasGlyph> {
        let bucket = (sigma.max(0.0) * SIGMA_BUCKETS_PER_PX).round() as u32;
        if bucket == 0 {
            return None;
        }
        if let Some(g) = self.blurred.get(&(key, bucket)) {
            return *g;
        }
        let entry = self.rasterize_blurred(queue, font_system, swash, key, sigma);
        self.blurred.insert((key, bucket), entry);
        entry
    }

    fn rasterize_blurred(
        &mut self,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash: &mut SwashCache,
        key: CacheKey,
        sigma: f32,
    ) -> Option<AtlasGlyph> {
        let img = swash.get_image(font_system, key).as_ref()?.clone();
        let (gw, gh) = (img.placement.width, img.placement.height);
        if gw == 0 || gh == 0 {
            return None;
        }
        let (buf, w, h, pad) = blur_mask(&img.data, gw, gh, sigma);
        self.upload(
            queue,
            &buf,
            w,
            h,
            img.placement.left - pad as i32,
            img.placement.top + pad as i32,
        )
    }

    /// Shelf-allocate a `w×h` slot and upload `data` (one coverage byte per texel).
    fn upload(
        &mut self,
        queue: &wgpu::Queue,
        data: &[u8],
        w: u32,
        h: u32,
        left: i32,
        top: i32,
    ) -> Option<AtlasGlyph> {
        if w == 0 || h == 0 || self.full || w + 2 * PAD > SIZE {
            return None;
        }

        // Shelf-allocate a `w×h` slot (with the gutter).
        if self.pen_x + w + PAD > SIZE {
            self.pen_x = PAD;
            self.pen_y += self.shelf_h + PAD;
            self.shelf_h = 0;
        }
        if self.pen_y + h + PAD > SIZE {
            // Atlas full. A bounded glyph set won't reach this; growth/eviction is a
            // follow-up. Stop allocating so we never panic — affected glyphs just
            // don't draw until then.
            self.full = true;
            return None;
        }
        let (x, y) = (self.pen_x, self.pen_y);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.pen_x += w + PAD;
        self.shelf_h = self.shelf_h.max(h);

        let s = SIZE as f32;
        Some(AtlasGlyph {
            uv: [
                x as f32 / s,
                y as f32 / s,
                (x + w) as f32 / s,
                (y + h) as f32 / s,
            ],
            left,
            top,
            width: w,
            height: h,
        })
    }
}

/// Blur buckets per pixel of sigma — how finely distinct halo radii are cached.
const SIGMA_BUCKETS_PER_PX: f32 = 4.0;

/// Pad a `gw x gh` coverage mask by the blur's reach, blur it, and **renormalize so
/// the result peaks at full coverage** again.
///
/// Returns `(bitmap, width, height, pad)`.
///
/// The renormalization is the load-bearing step. Blurring conserves total energy but
/// not peak: spreading a thin glyph stroke over a few pixels drops its peak coverage
/// to a fraction, so a halo built from the raw blurred mask comes out invisible even
/// at a strength that was clearly visible before (observed: the halo vanished
/// entirely on switching from stacked copies to a blur). Rescaling the peak back to
/// full coverage makes the caller's `intensity` mean *peak alpha* — the same thing it
/// means for a rect's glow, which is what lets the two be calibrated against each
/// other instead of by eye.
fn blur_mask(mask: &[u8], gw: u32, gh: u32, sigma: f32) -> (Vec<u8>, u32, u32, u32) {
    // Beyond ~3 sigma a Gaussian is indistinguishable from zero, so that is the
    // whole visible extent the halo needs room for.
    let pad = (sigma * 3.0).ceil().max(1.0) as u32;
    let (w, h) = (gw + 2 * pad, gh + 2 * pad);

    // Centre the mask in the padded buffer, then blur in place.
    let mut buf = vec![0u8; (w * h) as usize];
    for row in 0..gh {
        let src = (row * gw) as usize;
        let dst = ((row + pad) * w + pad) as usize;
        buf[dst..dst + gw as usize].copy_from_slice(&mask[src..src + gw as usize]);
    }
    box_blur(&mut buf, w, h, sigma);

    if let Some(peak) = buf.iter().copied().max().filter(|&p| p > 0) {
        for v in &mut buf {
            *v = ((u32::from(*v) * 255) / u32::from(peak)) as u8;
        }
    }
    (buf, w, h, pad)
}

/// Approximate a Gaussian blur of `sigma` with three successive box passes.
///
/// Three boxes is the standard cheap Gaussian: the error is under a couple of
/// percent, which is far below what is visible in a halo, and each pass is a running
/// sum — linear in the number of texels rather than in the radius.
fn box_blur(buf: &mut [u8], w: u32, h: u32, sigma: f32) {
    // Radius that makes three boxes match the target sigma's second moment.
    let radius = ((sigma * 3.0 * (std::f32::consts::TAU / 4.0).sqrt() / 4.0) + 0.5) as i32;
    if radius < 1 {
        return;
    }
    let mut scratch = vec![0u8; buf.len()];
    for _ in 0..3 {
        box_pass_horizontal(buf, &mut scratch, w, h, radius);
        box_pass_vertical(&scratch, buf, w, h, radius);
    }
}

fn box_pass_horizontal(src: &[u8], dst: &mut [u8], w: u32, h: u32, radius: i32) {
    let (w_i, span) = (w as i32, (radius * 2 + 1) as u32);
    for y in 0..h {
        let row = (y * w) as usize;
        let mut sum: u32 = 0;
        for x in -radius..=radius {
            sum += u32::from(src[row + x.clamp(0, w_i - 1) as usize]);
        }
        for x in 0..w_i {
            dst[row + x as usize] = (sum / span) as u8;
            let out = (x - radius).clamp(0, w_i - 1) as usize;
            let inc = (x + radius + 1).clamp(0, w_i - 1) as usize;
            sum = sum + u32::from(src[row + inc]) - u32::from(src[row + out]);
        }
    }
}

fn box_pass_vertical(src: &[u8], dst: &mut [u8], w: u32, h: u32, radius: i32) {
    let (h_i, span) = (h as i32, (radius * 2 + 1) as u32);
    for x in 0..w {
        let col = x as usize;
        let mut sum: u32 = 0;
        for y in -radius..=radius {
            sum += u32::from(src[col + (y.clamp(0, h_i - 1) as u32 * w) as usize]);
        }
        for y in 0..h_i {
            dst[col + (y as u32 * w) as usize] = (sum / span) as u8;
            let out = (y - radius).clamp(0, h_i - 1) as u32;
            let inc = (y + radius + 1).clamp(0, h_i - 1) as u32;
            sum = sum + u32::from(src[col + (inc * w) as usize])
                - u32::from(src[col + (out * w) as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::blur_mask;

    /// A 1px dot in a 3x3 mask — the harshest case for peak attenuation.
    fn dot() -> (Vec<u8>, u32, u32) {
        let mut m = vec![0u8; 9];
        m[4] = 255;
        (m, 3, 3)
    }

    /// **Regression guard.** Blurring conserves energy, not peak: without
    /// renormalization a blurred stroke's peak collapses and the halo built from it
    /// is invisible on screen, at a strength that read clearly before. This is
    /// exactly the bug that made the glyph halo disappear when it moved from stacked
    /// copies to a real blur.
    #[test]
    fn a_blurred_mask_is_renormalized_back_to_full_peak() {
        let (m, w, h) = dot();
        let (out, _, _, _) = blur_mask(&m, w, h, 2.0);
        assert_eq!(
            out.iter().copied().max(),
            Some(255),
            "peak coverage must survive the blur"
        );
    }

    /// The halo has to reach *outside* the glyph, which is the whole point of the
    /// padding — a blur confined to the glyph box would light nothing around it.
    #[test]
    fn a_blurred_mask_is_padded_for_the_blur_to_spread_into() {
        let (m, w, h) = dot();
        let (_, _, _, pad) = blur_mask(&m, w, h, 2.0);
        assert!(pad > 0, "no room was added for the blur to spread into");
    }

    /// The returned dimensions must describe the padded bitmap, or the caller places
    /// the halo quad at the wrong size and it detaches from its glyph.
    #[test]
    fn a_blurred_mask_reports_its_padded_dimensions() {
        let (m, w, h) = dot();
        let (_, bw, bh, pad) = blur_mask(&m, w, h, 2.0);
        assert_eq!((bw, bh), (w + 2 * pad, h + 2 * pad));
    }

    /// Coverage must fall off with distance from the source, not sit flat — a flat
    /// field would read as a box, which is precisely what the discarded ring-of-copies
    /// approach looked like.
    #[test]
    fn a_blurred_mask_falls_off_with_distance() {
        let (m, w, h) = dot();
        let (out, bw, bh, _) = blur_mask(&m, w, h, 2.0);
        let centre = out[((bh / 2) * bw + bw / 2) as usize];
        let edge = out[((bh / 2) * bw) as usize];
        assert!(
            centre > edge,
            "centre {centre} should exceed the edge {edge}"
        );
    }

    /// A wider sigma must reach further — this is what makes `glow_size` scale the
    /// halo instead of only brightening it.
    #[test]
    fn a_wider_sigma_pads_further() {
        let (m, w, h) = dot();
        let (_, _, _, narrow) = blur_mask(&m, w, h, 1.0);
        let (_, _, _, wide) = blur_mask(&m, w, h, 4.0);
        assert!(wide > narrow, "sigma 4 padded {wide}, sigma 1 padded {narrow}");
    }
}
