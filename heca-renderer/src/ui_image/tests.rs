use super::*;
use heca_grid_ui::{Color, image::ImageSource};
use std::{sync::mpsc, time::Duration};
fn image() -> Arc<ImageData> {
    let (tx, rx) = mpsc::channel();
    heca_grid_ui::install_frame_request(move || { let _ = tx.send(()); });
    let source = ImageSource::bytes(include_bytes!("../../assets/showcase/image.svg").as_slice());
    let handle = source.load(heca_grid_ui::frame_waker());
    loop {
        heca_grid_ui::frame_served();
        if let Some(result) = handle.result() { return result.unwrap() }
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
}
#[test]
fn fitting_and_clipping_keep_destination_and_source_in_step() {
    let data = image();
    let base = ImageAt { image: data, fit: ImageFit::Contain, color: Color::rgb(255, 255, 255),
        rect: Rectangle::new(Point::new(10.0, 20.0), Size::new(100.0, 100.0)),
        alpha: 1.0, clip: None, rounded: None };
    let (_, dst, uv) = placement(&base, 2.0, None).unwrap();
    assert_eq!(dst, (20.0, 90.0, 200.0, 100.0)); assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    let (_, dst, uv) = placement(&ImageAt { fit: ImageFit::Cover, ..base.clone() }, 1.0, None).unwrap();
    assert_eq!(dst, (10.0, 20.0, 100.0, 100.0)); assert_eq!(uv, [0.25, 0.0, 0.75, 1.0]);
    let (_, dst, uv) = placement(&ImageAt { fit: ImageFit::Stretch, ..base.clone() }, 1.0, None).unwrap();
    assert_eq!(dst, (10.0, 20.0, 100.0, 100.0)); assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    let clipped = ImageAt { fit: ImageFit::Stretch, clip: Some(Rectangle::new(Point::new(60.0, 20.0), Size::new(20.0, 100.0))), ..base };
    let (_, dst, uv) = placement(&clipped, 1.0, None).unwrap();
    assert_eq!(dst, (60.0, 20.0, 20.0, 100.0)); assert_eq!(uv, [0.5, 0.0, 0.7, 1.0]);
}

#[test]
fn partial_and_idle_redraws_do_not_accumulate_image_alpha() {
    fn pixels(device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::Texture) -> Vec<u8> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("image regression readback"), size: 64 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: target, mip_level: 0,
                origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout {
                offset: 0, bytes_per_row: Some(256), rows_per_image: Some(64),
            } },
            wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| { tx.send(result).unwrap(); });
        device.poll(wgpu::PollType::Wait).unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
        let result = buffer.slice(..).get_mapped_range().to_vec();
        buffer.unmap();
        result
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("persistent image regression"),
        size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let at = ImageAt { image: image(), fit: ImageFit::Stretch, color: Color::rgb(255, 255, 255),
        rect: Rectangle::new(Point::new(0.0, 0.0), Size::new(64.0, 64.0)),
        alpha: 0.5, clip: None, rounded: None };
    let mut renderer = UiImages::new(&device, format);
    let render = |renderer: &mut UiImages, damage, clear| {
        let ready = renderer.begin_frame();
        let mut encoder = device.create_command_encoder(&Default::default());
        if clear {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store },
                })], ..Default::default()
            });
        }
        renderer.draw(ImageFrame { device: &device, queue: &queue, encoder: &mut encoder,
            target: &view, stencil: None, viewport: (64.0, 64.0), scale: 1.0, damage }, &at);
        queue.submit([encoder.finish()]);
        ready
    };
    render(&mut renderer, None, true);
    let pending = renderer.cache.values().next().unwrap().view.is_none();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while renderer.cache.values().next().unwrap().raster.result().is_none() {
        assert!(std::time::Instant::now() < deadline, "SVG raster did not finish");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(render(&mut renderer, None, true), pending,
        "a newly ready raster requests a full repaint");
    let baseline = pixels(&device, &queue, &target);
    assert!(baseline.chunks_exact(4).any(|p| p[..3] != [0, 0, 0]), "the image must actually render");
    for _ in 0..4 {
        render(&mut renderer, Some([0.0, 0.0, 0.0, 0.0]), false);
        assert!(pixels(&device, &queue, &target) == baseline,
            "idle frames must preserve the existing image pixels");
        assert!(renderer.cache.values().next().unwrap().view.is_some(),
            "undamaged images retain their uploaded texture");
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0,
            origin: wgpu::Origin3d { x: 16, y: 16, z: 0 }, aspect: wgpu::TextureAspect::All },
        &[0, 0, 0, 255].repeat(16 * 16),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(64), rows_per_image: Some(16) },
        wgpu::Extent3d { width: 16, height: 16, depth_or_array_layers: 1 },
    );
    render(&mut renderer, Some([16.0, 16.0, 16.0, 16.0]), false);
    assert!(pixels(&device, &queue, &target) == baseline,
        "a partial repaint restores only cleared pixels, retaining the source UVs");
    render(&mut renderer, None, true);
    assert!(pixels(&device, &queue, &target) == baseline,
        "a mouse-triggered full redraw must not change image brightness");
}
