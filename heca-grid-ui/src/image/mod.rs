//! Shared local image assets. Loading and decoding run off the UI thread; live consumers share
//! weakly cached results. The host only uploads ready pixels and composites the scene.

mod loader;
mod decode;

use std::{collections::HashMap, path::{Path, PathBuf}, sync::{Arc, Mutex, Weak}};
use crate::{Color, FrameWake};

/// Where an image comes from. Inline data may contain SVG or a supported bitmap format.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ImageSource {
    File(PathBuf),
    Bytes(Arc<[u8]>),
}
impl ImageSource {
    pub fn file(path: impl Into<PathBuf>) -> Self { Self::File(path.into()) }
    pub fn bytes(data: impl Into<Arc<[u8]>>) -> Self { Self::Bytes(data.into()) }
    /// Resolve files against an explicit asset root, including a plugin package's directory.
    /// This performs no I/O. Absolute paths stay absolute.
    pub fn relative_to(&self, root: &Path) -> Self {
        match self { Self::File(path) => Self::File(root.join(path)), other => other.clone() }
    }
    /// Request this source. Consumers in different windows each retain their own redraw callback.
    pub fn load(&self, wake: FrameWake) -> ImageHandle { loader::load(self.clone(), wake) }
}

/// How the picture fits the box. Independent of the box's layout aspect ratio.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, heca_grid_ui_macros::PropName)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    /// Preserve proportions and show the whole picture, centred in the box.
    #[default]
    Contain,
    /// Preserve proportions and fill the box, cropping overflow.
    Cover,
    /// Fill both dimensions, allowing distortion.
    Stretch,
}

/// A recoverable load/decode error, exposed to widget users without logging or panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError(pub String);
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for ImageError {}

pub(super) struct Pending<T> {
    result: Mutex<Option<Result<Arc<T>, ImageError>>>,
    wakes: Mutex<Vec<FrameWake>>,
}
impl<T> Pending<T> {
    fn new() -> Self { Self { result: Mutex::new(None), wakes: Mutex::new(Vec::new()) } }
    fn result(&self) -> Option<Result<Arc<T>, ImageError>> { self.result.lock().unwrap_or_else(|e| e.into_inner()).clone() }
    fn subscribe(&self, wake: FrameWake) {
        let mut wakes = self.wakes.lock().unwrap_or_else(|e| e.into_inner());
        if !wakes.iter().any(|existing| existing.same(&wake)) { wakes.push(wake); }
    }
    fn complete(&self, result: Result<T, ImageError>) {
        *self.result.lock().unwrap_or_else(|e| e.into_inner()) = Some(result.map(Arc::new));
        let wakes = self.wakes.lock().unwrap_or_else(|e| e.into_inner()).clone();
        for wake in wakes { wake.request(); }
    }
}

/// A shared asynchronous source request. `None` means loading, never a reason to poll continuously.
#[derive(Clone)]
pub struct ImageHandle(Arc<Pending<ImageData>>);
impl ImageHandle {
    pub fn result(&self) -> Option<Result<Arc<ImageData>, ImageError>> { self.0.result() }
}

pub(super) enum Content { Bitmap(Arc<ImagePixels>), Vector(Arc<[u8]>) }

/// A decoded asset with intrinsic dimensions. SVG drawing stays vector until a resolution is asked for.
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    content: Content,
    rasters: Mutex<HashMap<RasterSpec, Weak<Pending<ImagePixels>>>>,
}
impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData").field("width", &self.width).field("height", &self.height).finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct RasterSpec { width: u32, height: u32, color: [u8; 4] }

/// Ready, straight-alpha RGBA8 pixels, shared by terminal/UI texture upload consumers.
#[derive(Debug)]
pub struct ImagePixels { pub width: u32, pub height: u32, pub rgba: Arc<[u8]> }

/// A background vector rasterisation. Bitmaps return their original pixels immediately.
#[derive(Clone)]
pub struct RasterHandle(Arc<Pending<ImagePixels>>);
impl RasterHandle {
    pub fn result(&self) -> Option<Result<Arc<ImagePixels>, ImageError>> { self.0.result() }
}
impl ImageData {
    pub fn raster(self: &Arc<Self>, width: u32, height: u32, color: Color, wake: FrameWake) -> RasterHandle {
        if let Content::Bitmap(pixels) = &self.content {
            return RasterHandle(Arc::new(Pending { result: Mutex::new(Some(Ok(pixels.clone()))), wakes: Mutex::new(Vec::new()) }));
        }
        let spec = RasterSpec { width, height, color: [color.r, color.g, color.b, color.a] };
        let mut rasters = self.rasters.lock().unwrap_or_else(|e| e.into_inner());
        rasters.retain(|_, entry| entry.strong_count() > 0);
        if let Some(pending) = rasters.get(&spec).and_then(Weak::upgrade) {
            pending.subscribe(wake); return RasterHandle(pending);
        }
        let pending = Arc::new(Pending::new()); pending.subscribe(wake);
        rasters.insert(spec, Arc::downgrade(&pending));
        loader::raster(self.clone(), spec, pending.clone());
        RasterHandle(pending)
    }
}

impl PartialEq for ImageData { fn eq(&self, other: &Self) -> bool { std::ptr::eq(self, other) } }
impl Eq for ImageData {}
impl ImageData { pub fn is_vector(&self) -> bool { matches!(self.content, Content::Vector(_)) } }
