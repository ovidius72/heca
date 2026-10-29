//! A local image in any layout. File I/O and decoding belong to the shared asset loader.

use std::{path::PathBuf, sync::Arc};
use crate::{Base, Color, Component, PaintCx};
use crate::builders::{LayoutExt, Parent};
use crate::style::Length;
use crate::widgets::{Visibility, Flex};
use crate::reactive::create_effect;
use crate::image::{ImageData, ImageHandle, ImageSource};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
pub use crate::image::ImageFit;

/// Observable loading state. Errors remain available even when a fallback is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageState { Loading, Ready { width: u32, height: u32 }, Failed(String) }

/// Shared image widget for inline SVG or local SVG/PNG/JPEG/GIF/WebP/BMP files.
/// Animated files show their first frame. Layout builders size the box; `fit` sizes its picture.
pub struct Image {
    base: Base,
    source: Signal<ImageSource>,
    root: PathBuf,
    requested: Option<ImageSource>,
    handle: Option<ImageHandle>,
    image: Signal<Option<Arc<ImageData>>>,
    state: Signal<ImageState>,
    fit: Signal<ImageFit>,
    color: Signal<Option<Color>>,
    alt: String,
}

#[heca_grid_ui_macros::props]
impl Image {
    /// Inline SVG. `Svg::new` is this same constructor, exposed as a convenience alias.
    pub fn new(svg: impl AsRef<str>) -> Self { Self::from_bytes(svg.as_ref().as_bytes()) }

    /// Load a local file asynchronously. Relative paths start at the creation directory unless
    /// `asset_root` is set. Format detection reads the contents, not the filename extension.
    pub fn load(path: impl Into<PathBuf>) -> Self { Self::from_source(ImageSource::file(path)) }

    /// Decode embedded SVG or bitmap bytes asynchronously, through the same cache as files.
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Self { Self::from_source(ImageSource::bytes(bytes)) }

    pub fn from_source(source: ImageSource) -> Self {
        let image = signal(None);
        let fit = signal(ImageFit::default());
        let color = signal(None);
        let mut base = Base::container();
        base.style.layout.direction = crate::style::Direction::Column;
        base.children.push(crate::WidgetModel::new(Picture::new(image, fit, color)));
        Self {
            base, source: signal(source), root: std::env::current_dir().unwrap_or_default(),
            requested: None, handle: None, image, state: signal(ImageState::Loading),
            fit, color, alt: String::new(),
        }
    }

    /// Replace the source with a local filename.
    #[heca_grid_ui_macros::prop]
    pub fn source(self, path: impl Into<String>) -> Self { self.source.set(ImageSource::file(path.into())); self }

    /// Replace the source with inline SVG text.
    #[heca_grid_ui_macros::prop]
    pub fn svg(self, svg: impl Into<String>) -> Self { self.source.set(ImageSource::bytes(svg.into().into_bytes())); self }

    /// Resolve relative filenames against this explicit directory, such as a plugin package root.
    #[heca_grid_ui_macros::prop]
    pub fn asset_root(mut self, root: impl Into<String>) -> Self { self.root = self.root.join(root.into()); self }

    /// Contain (default), cover or stretch the picture inside its laid-out box.
    #[heca_grid_ui_macros::prop]
    pub fn fit(self, fit: ImageFit) -> Self { self.fit.set(fit); self }

    /// CSS `currentColor` for SVGs; unset inherits the enclosing control or theme foreground.
    /// Bitmap pixels and explicit SVG brand colours are preserved.
    #[heca_grid_ui_macros::prop]
    pub fn color(self, color: Color) -> Self { self.color.set(Some(color)); self }

    /// Accessible description, also used as the image's identity in the widget tree.
    #[heca_grid_ui_macros::prop]
    pub fn alt(mut self, alt: impl Into<String>) -> Self { self.alt = alt.into(); self }

    /// A composable loading view. Hidden after the request settles.
    #[heca_grid_ui_macros::host_only("described images use a child with slot=loading")]
    pub fn placeholder(mut self, view: impl crate::builders::IntoComponent) -> Self { self.base.children.push(slot(view, false, self.state)); self }

    /// A composable error view. It does not hide the error from `state_signal`.
    #[heca_grid_ui_macros::host_only("described images use a child with slot=error")]
    pub fn fallback(mut self, view: impl crate::builders::IntoComponent) -> Self { self.base.children.push(slot(view, true, self.state)); self }

    /// Observe state on the UI thread, for example to show a configured error message.
    pub fn state_signal(&self) -> Signal<ImageState> { self.state }
    /// Update the source in place; decoding and relayout remain the widget's responsibility.
    pub fn source_signal(&self) -> Signal<ImageSource> { self.source }

    fn refresh(&mut self) {
        let source = self.source.get_untracked().relative_to(&self.root);
        if self.requested.as_ref() != Some(&source) {
            self.requested = Some(source.clone());
            self.handle = Some(source.load(crate::frame_waker()));
            self.image.set(None);
            self.state.set(ImageState::Loading);
            self.base.mark_needs_layout();
        }
        let next = self.handle.as_ref().and_then(ImageHandle::result);
        let state = match next {
            None => ImageState::Loading,
            Some(Ok(image)) => {
                let state = ImageState::Ready { width: image.width, height: image.height };
                if self.image.get_untracked().as_ref().is_none_or(|previous| !Arc::ptr_eq(previous, &image)) {
                    self.image.set(Some(image));
                }
                state
            }
            Some(Err(error)) => ImageState::Failed(error.to_string()),
        };
        if self.state.get_untracked() != state {
            self.state.set(state.clone()); self.base.mark_needs_layout();
        }

    }
}

impl Component for Image {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn remeasure(&mut self) { self.refresh(); }
    fn text_summary(&self) -> Option<String> { (!self.alt.is_empty()).then(|| self.alt.clone()) }
    fn tick(&mut self, _dt: f32) -> bool { self.refresh(); false }
}
impl LayoutExt for Image {}

// Absolute slots do not change the picture's box; they remain real layout/event children.
fn slot(view: impl crate::builders::IntoComponent, error: bool, state: Signal<ImageState>) -> crate::WidgetModel {
    let slot = Visibility::new(Flex::column().at_rect(0, 0, Length::FULL, Length::FULL).child(view.into_component()), !error);
    let visible = slot.visible_signal();
    create_effect(move |_| {
        let show = matches!((error, state.get()), (false, ImageState::Loading) | (true, ImageState::Failed(_)));
        if visible.get_untracked() != show { visible.set(show); }
    });
    crate::WidgetModel::new(slot)
}

/// The replaced picture is a real leaf; loading/error views are siblings above it. This keeps
/// intrinsic measurement reachable even when Image has composed placeholders.
struct Picture {
    base: Base,
    image: Signal<Option<Arc<ImageData>>>, fit: Signal<ImageFit>, color: Signal<Option<Color>>,
}
impl Picture {
    fn new(image: Signal<Option<Arc<ImageData>>>, fit: Signal<ImageFit>, color: Signal<Option<Color>>) -> Self {
        let mut base = Base::new();
        base.style.layout.flex_grow = 1.0;
        base.style.layout.min_width = Some(Length::Px(0.0));
        base.style.layout.min_height = Some(Length::Px(0.0));
        Self { base, image, fit, color }
    }
}
impl Component for Picture {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn intrinsic_size(&self) -> Option<heca_core::layout::Size> {
        self.image.get_untracked().map(|image| heca_core::layout::Size::new(f64::from(image.width), f64::from(image.height)))
    }
    fn paint(&self, cx: &mut PaintCx) {
        if let Some(image) = self.image.get_untracked() {
            let color = self.color.get_untracked().or_else(|| cx.content_color()).unwrap_or(cx.theme().colors.foreground);
            cx.image(self.base.bounds, image, self.fit.get_untracked(), color);
        }
    }
}
