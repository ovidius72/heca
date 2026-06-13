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
        let (w, h) = (img.placement.width, img.placement.height);
        if w == 0 || h == 0 || self.full {
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
            &img.data,
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
            left: img.placement.left,
            top: img.placement.top,
            width: w,
            height: h,
        })
    }
}
