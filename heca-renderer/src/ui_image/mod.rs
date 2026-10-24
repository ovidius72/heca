//! Ready image textures. CPU work is in grid-ui's shared background asset service.
use std::{collections::HashMap, sync::Arc};
use heca_grid_ui::{ImageAt, image::{ImageData, RasterHandle, ImageFit}};
use heca_core::layout::{Rectangle, Point, Size};
use crate::{backdrop::Backdrop, image::texture};

pub struct ImageFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: &'a wgpu::TextureView,
    pub stencil: Option<&'a wgpu::TextureView>,
    pub viewport: (f32, f32),
    pub scale: f32,
}

#[derive(PartialEq, Eq, Hash)]
struct Key { identity: usize, size: (u32, u32), color: [u8; 4] }
struct Cached {
    _asset: Arc<ImageData>,
    raster: RasterHandle,
    view: Option<wgpu::TextureView>,
    generation: u64,
}
/// One cached texture per asset/resolution/colour, discarded after leaving the scene.
/// Uses the same generic texture upload and compositor as terminal images and surfaces.
pub struct UiImages { sampler: Backdrop, mask: crate::grid::GridRenderer, cache: HashMap<Key, Cached>, generation: u64 }
impl UiImages {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        Self { sampler: Backdrop::new(device, format), mask: crate::grid::GridRenderer::new(device, format), cache: HashMap::new(), generation: 0 }
    }
    pub fn begin_frame(&mut self) {
        self.mask.begin_frame();
        self.cache.retain(|_, entry| entry.generation == self.generation);
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn draw(&mut self, frame: ImageFrame<'_>, at: &ImageAt) {
        let Some((picture, dst, uv)) = placement(at, frame.scale) else { return };
        if at.alpha <= 0.0 { return }
        let size = if at.image.is_vector() {
            ((picture.size.w as f32 * frame.scale).ceil() as u32, (picture.size.h as f32 * frame.scale).ceil() as u32)
        } else { (at.image.width, at.image.height) };
        if size.0 == 0 || size.1 == 0 || size.0 > frame.device.limits().max_texture_dimension_2d
            || size.1 > frame.device.limits().max_texture_dimension_2d { return }
        let color = if at.image.is_vector() { [at.color.r, at.color.g, at.color.b, at.color.a] } else { [0; 4] };
        let key = Key { identity: Arc::as_ptr(&at.image) as usize, size, color };
        let cached = self.cache.entry(key).or_insert_with(|| Cached {
            _asset: at.image.clone(), raster: at.image.raster(size.0, size.1, at.color, heca_grid_ui::frame_waker()),
            view: None, generation: self.generation,
        });
        cached.generation = self.generation;
        if cached.view.is_none() && let Some(Ok(pixels)) = cached.raster.result() {
            cached.view = Some(texture::upload(frame.device, frame.queue, pixels.width, pixels.height, &pixels.rgba)
                .create_view(&wgpu::TextureViewDescriptor::default()));
        }
        let Some(view) = &cached.view else { return };
        if let (Some(rounded), Some(stencil)) = (at.rounded, frame.stencil) {
            self.mask.set_scale_factor(f64::from(frame.scale));
            self.mask.set_target_size(frame.viewport.0 as u32, frame.viewport.1 as u32);
            self.mask.set_screen_size(frame.queue, frame.viewport.0 / frame.scale, frame.viewport.1 / frame.scale);
            self.mask.draw(crate::grid::GlowRect {
                x: rounded.rect.loc.x as f32, y: rounded.rect.loc.y as f32,
                w: rounded.rect.size.w as f32, h: rounded.rect.size.h as f32,
                fill: [0.0; 4], border: [0.0; 4], border_width: 0.0, radius: rounded.radius,
                glow: [0.0; 4], glow_radius: 0.0, glow_intensity: 0.0, glow_alpha_scale: 0.0,
                shadow: [0.0; 4], shadow_radius: 0.0, shadow_offset: [0.0; 2],
            });
            self.mask.render_stencil(frame.queue, stencil, frame.encoder);
        }
        self.sampler.draw(frame.device, frame.queue, frame.encoder, frame.target, view,
            frame.viewport, dst, Some(uv), at.alpha, 0.0, frame.stencil);
    }
}

/// Fit first, then clip while retaining source UVs. Cover cropping never stretches the picture.
fn placement(at: &ImageAt, scale: f32) -> Option<(Rectangle, crate::backdrop::RectPx, [f32; 4])> {
    let size = at.rect.size;
    if !scale.is_finite() || scale <= 0.0 || !size.w.is_finite() || !size.h.is_finite()
        || size.w <= 0.0 || size.h <= 0.0 { return None }
    let (w, h) = (f64::from(at.image.width), f64::from(at.image.height));
    let (pw, ph) = if at.fit == ImageFit::Stretch { (size.w, size.h) } else {
        let k = if at.fit == ImageFit::Cover { (size.w / w).max(size.h / h) } else { (size.w / w).min(size.h / h) };
        (w * k, h * k)
    };
    let picture = Rectangle::new(Point::new(at.rect.loc.x + (size.w - pw) / 2.0, at.rect.loc.y + (size.h - ph) / 2.0), Size::new(pw, ph));
    let visible = picture.intersection(at.rect)?;
    let visible = match at.clip { Some(clip) => visible.intersection(clip)?, None => visible };
    if visible.size.w <= 0.0 || visible.size.h <= 0.0 { return None }
    let uv = [((visible.loc.x - picture.loc.x) / pw) as f32, ((visible.loc.y - picture.loc.y) / ph) as f32,
        ((visible.loc.x + visible.size.w - picture.loc.x) / pw) as f32, ((visible.loc.y + visible.size.h - picture.loc.y) / ph) as f32];
    Some((picture, (visible.loc.x as f32 * scale, visible.loc.y as f32 * scale,
        visible.size.w as f32 * scale, visible.size.h as f32 * scale), uv))
}

#[cfg(test)]
mod tests;
