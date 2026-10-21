//! Stroked segments use the same antialiased rounded-box pipeline and scissor as surfaces.

use super::{GlowRect, GridRenderer};
use heca_grid_ui::scene::{ArrowHeads, LineCmd};

impl GridRenderer {
    /// Queue a line and its optional open arrowheads. Non-finite and degenerate input is ignored.
    pub fn draw_line(&mut self, line: &LineCmd) {
        for (start, end) in segments(line) {
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let length = dx.hypot(dy);
            if length <= 0.0 { continue; }
            let direction = [dx / length, dy / length];
            let center = [(start[0] + end[0]) / 2.0, (start[1] + end[1]) / 2.0];
            let first = self.vertices.len();
            self.draw(GlowRect {
                x: center[0] - length / 2.0, y: center[1] - line.width / 2.0,
                w: length, h: line.width, fill: line.color.to_f32x4(),
                border: [0.0; 4], border_width: 0.0, radius: line.width / 2.0,
                glow: [0.0; 4], glow_radius: 0.0, glow_intensity: 0.0, glow_alpha_scale: 0.0,
                shadow: [0.0; 4], shadow_radius: 0.0, shadow_offset: [0.0; 2],
            });
            for vertex in &mut self.vertices[first..] {
                let x = vertex.position[0] - center[0];
                let y = vertex.position[1] - center[1];
                vertex.position = [center[0] + x * direction[0] - y * direction[1],
                    center[1] + x * direction[1] + y * direction[0]];
                vertex.direction = direction;
            }
        }
    }
}

type Segment = ([f32; 2], [f32; 2]);

fn segments(line: &LineCmd) -> Vec<Segment> {
    let a = [line.start.x as f32, line.start.y as f32];
    let b = [line.end.x as f32, line.end.y as f32];
    if !a.into_iter().chain(b).chain([line.width, line.arrow_size]).all(f32::is_finite)
        || line.width <= 0.0 { return Vec::new(); }
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    if !length.is_finite() || length <= 0.0 { return Vec::new(); }
    let mut out = vec![(a, b)];
    let size = line.arrow_size.max(0.0).min(length / 2.0);
    let mut arrow = |tip: [f32; 2], other: [f32; 2]| {
        let u = [(other[0] - tip[0]) / length, (other[1] - tip[1]) / length];
        for side in [-1.0, 1.0] {
            out.push((tip, [tip[0] + size * (u[0] - side * u[1] / 2.0),
                tip[1] + size * (u[1] + side * u[0] / 2.0)]));
        }
    };
    if size > 0.0 {
        if matches!(line.arrows, ArrowHeads::Start | ArrowHeads::Both) { arrow(a, b); }
        if matches!(line.arrows, ArrowHeads::End | ArrowHeads::Both) { arrow(b, a); }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::{Color, Point};

    #[test]
    fn arrows_point_to_the_ends_and_shrink_on_short_segments() {
        let mut line = LineCmd { start: Point::new(2.0, 4.0), end: Point::new(6.0, 4.0),
            color: Color::rgb(255, 255, 255), width: 1.0, arrow_size: 100.0, arrows: ArrowHeads::Both };
        let strokes = segments(&line);
        assert_eq!(strokes.len(), 5);
        assert_eq!(strokes[1], ([2.0, 4.0], [4.0, 3.0]));
        assert_eq!(strokes[3], ([6.0, 4.0], [4.0, 5.0]));
        line.end = line.start;
        assert!(segments(&line).is_empty());
        line.end.x = f64::NAN;
        assert!(segments(&line).is_empty());
    }
}
