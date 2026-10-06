//! **The box a terminal is cut to.**

pub(crate) fn pane_scissor_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f64,
    physical_size: winit::dpi::PhysicalSize<u32>,
) -> Option<(u32, u32, u32, u32)> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let scale = scale_factor as f32;
    let left = (x.max(0.0) * scale).floor() as u32;
    let top = (y.max(0.0) * scale).floor() as u32;
    let right = ((x + w).max(0.0) * scale).ceil() as u32;
    let bottom = ((y + h).max(0.0) * scale).ceil() as u32;

    let clipped_left = left.min(physical_size.width);
    let clipped_top = top.min(physical_size.height);
    let clipped_right = right.min(physical_size.width);
    let clipped_bottom = bottom.min(physical_size.height);
    let clipped_width = clipped_right.saturating_sub(clipped_left);
    let clipped_height = clipped_bottom.saturating_sub(clipped_top);

    if clipped_width == 0 || clipped_height == 0 {
        return None;
    }

    Some((clipped_left, clipped_top, clipped_width, clipped_height))
}
