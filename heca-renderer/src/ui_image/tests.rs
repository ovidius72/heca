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
    let (_, dst, uv) = placement(&base, 2.0).unwrap();
    assert_eq!(dst, (20.0, 90.0, 200.0, 100.0)); assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    let (_, dst, uv) = placement(&ImageAt { fit: ImageFit::Cover, ..base.clone() }, 1.0).unwrap();
    assert_eq!(dst, (10.0, 20.0, 100.0, 100.0)); assert_eq!(uv, [0.25, 0.0, 0.75, 1.0]);
    let (_, dst, uv) = placement(&ImageAt { fit: ImageFit::Stretch, ..base.clone() }, 1.0).unwrap();
    assert_eq!(dst, (10.0, 20.0, 100.0, 100.0)); assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
    let clipped = ImageAt { fit: ImageFit::Stretch, clip: Some(Rectangle::new(Point::new(60.0, 20.0), Size::new(20.0, 100.0))), ..base };
    let (_, dst, uv) = placement(&clipped, 1.0).unwrap();
    assert_eq!(dst, (60.0, 20.0, 20.0, 100.0)); assert_eq!(uv, [0.5, 0.0, 0.7, 1.0]);
}
