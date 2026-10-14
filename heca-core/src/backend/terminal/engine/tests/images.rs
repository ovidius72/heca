use super::*;

#[test]
fn captures_sixel_inline_image_placement() {
    let writer = SharedWriter::new(Box::new(SinkWriter));
    let mut engine = TerminalEngine::new(
        20,
        4,
        (8.0, 16.0),
        writer,
        None,
        crate::backend::TerminalBackendOptions::DEFAULT_SCROLLBACK_SIZE,
    )
    .expect("terminal engine should initialize");

    // Minimal Sixel: register color 0 = red, then paint a 16px-wide, 6px-tall
    // band (`!16~` = repeat the all-6-pixels-on column 16 times). Sixel decodes
    // to RGBA with no base64, so this exercises the full capture path.
    engine.advance_bytes(b"\x1bPq#0;2;100;0;0#0!16~\x1b\\");

    let snapshot = engine.snapshot((8.0, 16.0));
    assert_eq!(
        snapshot.graphics.len(),
        1,
        "one inline-image placement captured"
    );
    let placement = &snapshot.graphics[0];
    // 16px / 8px cells = 2 cells wide; 6px / 16px cells = 1 cell tall.
    assert_eq!(placement.cols, 2, "16px image spans 2 cells of 8px");
    assert_eq!(placement.rows, 1, "6px image fits in 1 cell of 16px");
    assert_eq!(placement.row, 0);
    assert_eq!(placement.col, 0);

    // The placement resolves to a decoded image in the registry.
    assert_eq!(snapshot.images.len(), 1, "one decoded image registered");
    let image = &snapshot.images[0];
    assert_eq!(
        image.id, placement.image_id,
        "placement references the image"
    );
    assert!(
        image.width >= 16 && image.height >= 6,
        "decoded at source size"
    );
    assert_eq!(image.frames.len(), 1, "static image has one frame");
    assert_eq!(
        image.frames[0].rgba.len(),
        (image.width as usize) * (image.height as usize) * 4,
        "RGBA buffer is tightly packed"
    );
}

#[test]
fn decode_terminal_image_handles_rgba_and_rejects_garbage() {
    use wezterm_term::image::ImageDataType;
    // A 2x1 opaque-red RGBA image is used as-is.
    let rgba = vec![255, 0, 0, 255, 255, 0, 0, 255];
    let data = ImageDataType::new_single_frame(2, 1, rgba.clone());
    let hash = [7u8; 32];
    let decoded = decode_terminal_image(hash, &data).expect("rgba decodes");
    assert_eq!((decoded.width, decoded.height), (2, 1));
    assert_eq!(decoded.frames.len(), 1);
    assert_eq!(&*decoded.frames[0].rgba, rgba.as_slice());
    assert_eq!(decoded.id, image_id_from_hash(hash));

    // Undecodable encoded bytes yield None rather than panicking.
    let junk = ImageDataType::EncodedFile(vec![0xde, 0xad, 0xbe, 0xef]);
    assert!(decode_terminal_image([1u8; 32], &junk).is_none());
}

#[test]
fn decode_terminal_image_decodes_animated_gif_frames() {
    use image::{Delay, Frame, RgbaImage};
    use wezterm_term::image::ImageDataType;

    // Encode a 2x2, two-frame GIF (red then blue, 120ms each) in memory.
    let red = RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
    let blue = RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255]));
    let mut bytes = Vec::new();
    {
        let mut enc = image::codecs::gif::GifEncoder::new(&mut bytes);
        enc.encode_frame(Frame::from_parts(
            red,
            0,
            0,
            Delay::from_numer_denom_ms(120, 1),
        ))
        .expect("encode frame 0");
        enc.encode_frame(Frame::from_parts(
            blue,
            0,
            0,
            Delay::from_numer_denom_ms(120, 1),
        ))
        .expect("encode frame 1");
    }

    let decoded = decode_terminal_image([3u8; 32], &ImageDataType::EncodedFile(bytes))
        .expect("animated gif decodes");
    assert_eq!(decoded.frames.len(), 2, "two animation frames captured");
    assert!(decoded.is_animated());
    assert_eq!(decoded.frames[0].delay, Duration::from_millis(120));
    assert_eq!(decoded.total_duration(), Duration::from_millis(240));
    // The two frames must decode to DIFFERENT pixels (red vs blue) — a
    // regression here would make an animation play but look frozen.
    assert_ne!(
        decoded.frames[0].rgba, decoded.frames[1].rgba,
        "animation frames must differ; frame 0 red, frame 1 blue"
    );
    assert_eq!(
        &decoded.frames[0].rgba[..4],
        &[255, 0, 0, 255],
        "frame 0 red"
    );
    assert_eq!(
        &decoded.frames[1].rgba[..4],
        &[0, 0, 255, 255],
        "frame 1 blue"
    );
}

#[test]
fn graphics_collector_coalesces_block_and_dedups_images() {
    let image = Arc::new(TerminalImage {
        id: 42,
        width: 4,
        height: 2,
        frames: Arc::from(
            vec![TerminalImageFrame {
                rgba: Arc::from(vec![0u8; 4 * 2 * 4].into_boxed_slice()),
                delay: Duration::ZERO,
            }]
            .into_boxed_slice(),
        ),
    });
    let hash = [9u8; 32];
    let mut collector = GraphicsCollector::default();
    // A 2x2 cell block, fed in row-major order. Corner texcoords come from the
    // first (top-left) and last (bottom-right) cells.
    collector.add_cell(hash, Some(1), 0, 0, 0, [0.0, 0.0], [0.5, 0.5], &image);
    collector.add_cell(hash, Some(1), 0, 0, 1, [0.5, 0.0], [1.0, 0.5], &image);
    collector.add_cell(hash, Some(1), 0, 1, 0, [0.0, 0.5], [0.5, 1.0], &image);
    collector.add_cell(hash, Some(1), 0, 1, 1, [0.5, 0.5], [1.0, 1.0], &image);

    let (placements, images) = collector.finish();
    assert_eq!(placements.len(), 1, "the 4 cells coalesce into one block");
    let p = &placements[0];
    assert_eq!((p.row, p.col, p.cols, p.rows), (0, 0, 2, 2));
    assert_eq!(p.image_id, 42);
    assert_eq!(p.src_top_left, [0.0, 0.0], "top-left from the first cell");
    assert_eq!(
        p.src_bottom_right,
        [1.0, 1.0],
        "bottom-right from the last cell"
    );
    assert_eq!(images.len(), 1, "the shared image is registered once");
}
