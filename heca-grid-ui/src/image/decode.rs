//! Content detection and bounded decoding. SVGs cannot fetch external files or network resources.
use std::io::Cursor;
use resvg::{tiny_skia, usvg};
use super::*;

pub(super) fn source(source: &ImageSource) -> Result<ImageData, ImageError> {
    let bytes = match source {
        ImageSource::File(path) => std::fs::read(path).map(Arc::<[u8]>::from).map_err(|e| ImageError(e.to_string()))?,
        ImageSource::Bytes(bytes) => bytes.clone(),
    };
    let (content, width, height) = if image::guess_format(&bytes).is_ok() {
        let mut reader = image::ImageReader::new(Cursor::new(&bytes)).with_guessed_format().map_err(|e| ImageError(e.to_string()))?;
        reader.limits(image::Limits::default());
        let pixels = reader.decode().map_err(|e| ImageError(e.to_string()))?.to_rgba8();
        let (width, height) = pixels.dimensions();
        (Content::Bitmap(Arc::new(ImagePixels { width, height, rgba: pixels.into_raw().into() })), width, height)
    } else {
        let tree = usvg::Tree::from_data(&bytes, &options()).map_err(|e| ImageError(e.to_string()))?;
        (Content::Vector(bytes), tree.size().width().ceil() as u32, tree.size().height().ceil() as u32)
    };
    Ok(ImageData { width, height, content, rasters: Mutex::new(HashMap::new()) })
}
fn options() -> usvg::Options<'static> {
    usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None), resolve_data: Box::new(|_, _, _| None),
        }, ..Default::default()
    }
}
pub(super) fn raster(data: &ImageData, spec: RasterSpec) -> Result<ImagePixels, ImageError> {
    let Content::Vector(source) = &data.content else { return Err(ImageError("expected vector data".into())) };
    let count = u64::from(spec.width).checked_mul(u64::from(spec.height)).and_then(|n| n.checked_mul(4));
    if count.is_none() || spec.width == 0 || spec.height == 0
        || count.zip(image::Limits::default().max_alloc).is_some_and(|(bytes, limit)| bytes > limit) {
        return Err(ImageError("image exceeds decoder allocation limits".into()));
    }
    // currentColor is resolved by the SVG parser. Reparse only when a consumer asks for a new
    // themed raster, in this background queue; the parsed source is shared across frames.
    let mut options = options();
    options.style_sheet = Some(format!("svg {{ color: rgba({},{},{},{}) }}", spec.color[0], spec.color[1], spec.color[2], f32::from(spec.color[3]) / 255.0));
    let tree = usvg::Tree::from_data(source, &options).map_err(|e| ImageError(e.to_string()))?;
    let mut pixmap = tiny_skia::Pixmap::new(spec.width, spec.height).ok_or_else(|| ImageError("cannot allocate image".into()))?;
    let transform = tiny_skia::Transform::from_scale(spec.width as f32 / tree.size().width(), spec.height as f32 / tree.size().height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.take();
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 { continue }
        for channel in &mut pixel[..3] { *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8; }
    }
    Ok(ImagePixels { width: spec.width, height: spec.height, rgba: pixels.into() })
}
