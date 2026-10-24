//! Real decoding, layout and live state, through the public image API.
use std::{sync::{Arc, mpsc}, time::Duration};
use heca_grid_ui::{Component, Image, ImageFit, ImageState, Svg, LayoutEngine, Scene, PaintCx, Theme, DrawCommand, HostWork};
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::image::{ImageSource, ImageHandle, ImageData, RasterHandle, ImagePixels};
use heca_core::layout::Size;

const SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='80' height='40'><rect width='40' height='40' fill='currentColor'/><rect x='40' width='40' height='40' fill='#eab65d'/></svg>";
fn receiver() -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    heca_grid_ui::install_frame_request(move || { let _ = tx.send(()); });
    rx
}
fn ready(source: ImageSource) -> (ImageHandle, Arc<ImageData>) {
    let rx = receiver(); let handle = source.load(heca_grid_ui::frame_waker());
    loop {
        heca_grid_ui::frame_served();
        if let Some(result) = handle.result() { return (handle, result.expect("image decodes")); }
        rx.recv_timeout(Duration::from_secs(5)).expect("decoder wakes the UI");
    }
}
fn raster(data: &Arc<ImageData>, width: u32, height: u32, color: heca_grid_ui::Color) -> (RasterHandle, Arc<ImagePixels>) {
    let rx = receiver(); let handle = data.raster(width, height, color, heca_grid_ui::frame_waker());
    loop {
        heca_grid_ui::frame_served();
        if let Some(result) = handle.result() { return (handle, result.expect("raster succeeds")); }
        rx.recv_timeout(Duration::from_secs(5)).expect("rasterisation wakes the UI");
    }
}
#[test]
fn svg_png_and_jpeg_use_the_same_asset_api() {
    for bytes in [SVG.as_bytes(), include_bytes!("../../heca-renderer/assets/showcase/image.png").as_slice(), include_bytes!("../../heca-renderer/assets/showcase/image.jpg").as_slice()] {
        let source = ImageSource::bytes(bytes);
        let (_handle, data) = ready(source.clone());
        let other = source.load(heca_grid_ui::frame_waker()).result().unwrap().unwrap();
        assert!(Arc::ptr_eq(&data, &other), "live consumers reuse one decode");
        assert!(data.width > 0 && data.height > 0);
    }
}
#[test]
fn vector_raster_preserves_brand_fill_and_follows_current_color() {
    let (_handle, data) = ready(ImageSource::bytes(SVG.as_bytes()));
    let color = heca_grid_ui::Color::rgb(32, 96, 160);
    let (_raster, pixels) = raster(&data, 160, 80, color);
    assert_eq!((pixels.width, pixels.height), (160, 80));
    assert_eq!(&pixels.rgba[(40 * 160 + 20) * 4..][..4], &[32, 96, 160, 255]);
    assert_eq!(&pixels.rgba[(40 * 160 + 120) * 4..][..4], &[234, 182, 93, 255]);
}
#[test]
fn layout_uses_intrinsic_ratio_without_overwriting_explicit_dimensions() {
    let (_handle, _data) = ready(ImageSource::bytes(SVG.as_bytes()));
    for (mut image, expected) in [
        (Svg::new(SVG).width(200), Size::new(200.0, 100.0)),
        (Svg::new(SVG).height(60), Size::new(120.0, 60.0)),
        (Svg::new(SVG).width(200).height(70).aspect_ratio(1.0), Size::new(200.0, 70.0)),
        (Svg::new(SVG).width(200).aspect_ratio(4.0), Size::new(200.0, 50.0)),
        (Svg::new(SVG).width(200).max_width(100), Size::new(100.0, 50.0)),
        (Svg::new(SVG).width(200).aspect_ratio(4.0).max_width(100), Size::new(100.0, 25.0)),
        (Svg::new(SVG).width(200).placeholder(heca_grid_ui::Label::new("loading")), Size::new(200.0, 100.0)),
    ] {
        LayoutEngine::new().compute(&mut image, Size::new(500.0, 500.0));
        assert_eq!(image.base().bounds.size, expected);
        let mut scene = Scene::new(); image.paint(&mut PaintCx::new(&mut scene, &Theme::default()));
        assert!(scene.base_runs().iter().any(|run| matches!(run.then, Some(HostWork::Image(_)))));
    }
}
#[test]
fn aspect_ratio_is_a_shared_layout_property() {
    use heca_grid_ui::widgets::Flex;
    for ratio in [2.0, 4.0] {
        let mut root = Flex::column().child(Flex::row().width(200).aspect_ratio(ratio));
        LayoutEngine::new().compute(&mut root, Size::new(400.0, 400.0));
        assert_eq!(root.base().children[0].base().bounds.size.h, f64::from(200.0 / ratio));
    }
    let mut root = Flex::row().width(200).height(50).aspect_ratio(f32::NAN);
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 400.0));
    assert_eq!(root.base().bounds.size, Size::new(200.0, 50.0));
}

#[test]
fn aspect_ratio_constraints_resolve_font_relative_lengths() {
    use heca_grid_ui::style::{Layout, Length};
    for font in [12.0, 24.0] {
        for (width, expected) in [(2.0, 3.0), (10.0, 5.0)] {
            let style = Layout {
                width: Length::Em(width),
                min_width: Some(Length::Em(3.0)),
                max_width: Some(Length::Em(5.0)),
                aspect_ratio: Some(2.0),
                ..Layout::default()
            }.to_taffy(font);
            assert_eq!(style.size.width, taffy::style::Dimension::Length(expected * font));
            assert_eq!(style.aspect_ratio, Some(2.0));
        }
    }
}
#[test]
fn replacing_a_source_updates_metadata_and_reports_errors() {
    let (_handle, _data) = ready(ImageSource::bytes(SVG.as_bytes()));
    let mut image = Svg::new(SVG).width(200).height(100).fit(ImageFit::Cover)
        .placeholder(heca_grid_ui::Label::new("loading"))
        .fallback(heca_grid_ui::Label::new("unavailable"));
    let state = image.state_signal();
    let source = image.source_signal();
    use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
    LayoutEngine::new().compute(&mut image, Size::new(500.0, 500.0));
    assert!(matches!(state.get_untracked(), ImageState::Ready { width: 80, height: 40 }));
    let bad = ImageSource::bytes(b"broken image".as_slice());
    let rx = receiver(); let failed = bad.load(heca_grid_ui::frame_waker());
    loop {
        heca_grid_ui::frame_served(); if failed.result().is_some() { break }
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    source.set(bad);
    LayoutEngine::new().compute(&mut image, Size::new(500.0, 500.0));
    assert!(matches!(state.get_untracked(), ImageState::Failed(_)));
    let mut scene = Scene::new(); image.paint(&mut PaintCx::new(&mut scene, &Theme::default()));
    assert!(scene.iter().any(|draw| matches!(draw, DrawCommand::Text(text) if text.text == "unavailable")));
    assert!(!scene.iter().any(|draw| matches!(draw, DrawCommand::Text(text) if text.text == "loading")));
}
#[test]
fn a_file_resolves_from_its_declared_asset_root() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../heca-renderer/assets/showcase");
    let (_handle, _data) = ready(ImageSource::file(root.join("image.png")));
    let mut image = Image::load("image.png").asset_root(root.to_string_lossy().into_owned());
    LayoutEngine::new().compute(&mut image, Size::new(500.0, 500.0));
    use heca_grid_ui::reactive::SignalGet;
    assert!(matches!(image.state_signal().get_untracked(), ImageState::Ready { width: 240, height: 120 }));
}
