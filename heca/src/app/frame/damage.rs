//! **What a frame repaints** — the whole window, or only what changed since the last one.
//!
//! The scene texture is kept between frames, so a frame that changed one terminal's text redraws
//! that terminal's box and leaves the rest of the picture as it was. What changed comes from two
//! places, neither of which anything has to remember to report:
//!
//! - **the scene** — this frame's is compared with the last one ([`Scene::damage_since`]): every
//!   widget that paints differently shows up there;
//! - **the terminals** — their text is drawn by the host, not the scene, so a terminal whose
//!   program wrote something is repainted where the scene places it.
//!
//! The whole window is repainted whenever anything else may have changed: something marked the
//! window for a redraw (`needs_redraw` — a resize, a theme, a key), the session or a chrome rebuild
//! moved things, or there is no last scene to compare with. A see-through window repaints in part
//! too: the damaged part is cleared first (`Compositor::clear_region`), so nothing old shows
//! through what is drawn over it. `HECA_FULL_REPAINT=1` repaints it every frame, to
//! compare by eye.

use std::collections::HashSet;

use heca_grid_ui::{Rectangle, Scene, SceneDamage};

use crate::chrome::panes::terminal::TerminalId;

/// What the next frame has to repaint, gathered between frames.
pub(crate) struct Repaint {
    /// Repaint the whole window.
    whole: bool,
    /// Terminals on screen whose text changed since the last frame.
    terminals: HashSet<TerminalId>,
    /// The scene the last frame drew, to compare the next one with.
    last_scene: Option<Scene>,
    /// Why the last frame repainted the whole window, for `HECA_FRAME_REASONS`.
    why_whole: &'static str,
}

impl Default for Repaint {
    fn default() -> Self {
        Self {
            whole: true,
            terminals: HashSet::new(),
            last_scene: None,
            why_whole: "",
        }
    }
}

impl Repaint {
    /// Repaint the whole window next frame.
    pub(crate) fn whole(&mut self) {
        self.whole = true;
    }

    /// Terminal `id`'s text changed: repaint it where it is drawn.
    pub(crate) fn terminal(&mut self, id: TerminalId) {
        self.terminals.insert(id);
    }

    /// **The box this frame repaints** (logical px), `None` for the whole window — and start
    /// gathering for the next one.
    pub(crate) fn take(&mut self, scene: &Scene) -> Option<Rectangle> {
        let marked = std::mem::take(&mut self.whole);
        let terminals = std::mem::take(&mut self.terminals);
        self.why_whole = if self.last_scene.is_none() {
            "no last scene"
        } else if marked {
            "marked"
        } else if full_repaint_forced() {
            "HECA_FULL_REPAINT"
        } else {
            ""
        };
        if !self.why_whole.is_empty() {
            return None;
        }
        let last = self.last_scene.as_ref()?;
        let mut damage = scene.damage_since(last);
        for id in terminals {
            for rect in scene.surface_rects(id.0) {
                damage = damage.with(rect);
            }
        }
        match scene.grow_for_backdrops(damage) {
            SceneDamage::Nothing => Some(Rectangle::from_size(Default::default())),
            SceneDamage::Area(area) => Some(area),
            SceneDamage::Everything => {
                self.why_whole = "the scene";
                None
            }
        }
    }

    /// Why the last [`take`](Self::take) repainted the whole window.
    pub(crate) fn why_whole(&self) -> &'static str {
        self.why_whole
    }

    /// Keep the scene this frame drew, for the next frame to compare with.
    pub(crate) fn remember(&mut self, scene: Scene) {
        self.last_scene = Some(scene);
    }
}

/// `HECA_FULL_REPAINT=1`: repaint the whole window every frame. Read once.
fn full_repaint_forced() -> bool {
    static FORCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FORCED.get_or_init(|| std::env::var_os("HECA_FULL_REPAINT").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::{DrawCommand, Point, Size};

    fn surface(id: u64, x: f64) -> Scene {
        let mut s = Scene::default();
        s.push(DrawCommand::Host(heca_grid_ui::scene::HostCmd {
            draw: heca_grid_ui::scene::HostDraw::Surface { id },
            rect: Rectangle::new(Point::new(x, 0.0), Size::new(100.0, 50.0)),
            alpha: 1.0,
            echo: false,
            rounded: None,
        }));
        s
    }

    /// The first frame, and any frame after something marked the window, repaints all of it.
    #[test]
    fn with_nothing_to_compare_or_a_mark_the_whole_window_is_repainted() {
        let mut repaint = Repaint::default();
        let scene = surface(1, 0.0);
        assert_eq!(repaint.take(&scene), None, "no last scene");
        repaint.remember(scene.clone());
        repaint.whole();
        assert_eq!(repaint.take(&scene), None, "marked");
    }

    /// **A terminal that printed is repainted where it is drawn, and nothing else is.**
    #[test]
    fn a_terminal_that_printed_is_repainted_in_its_box() {
        let mut repaint = Repaint::default();
        let scene = surface(7, 40.0);
        repaint.remember(scene.clone());
        let _ = repaint.take(&scene);
        assert_eq!(
            repaint.take(&scene),
            Some(Rectangle::from_size(Size::default())),
            "the same scene and no output: nothing to repaint"
        );
        repaint.terminal(TerminalId(7));
        assert_eq!(
            repaint.take(&scene),
            Some(Rectangle::new(
                Point::new(40.0, 0.0),
                Size::new(100.0, 50.0)
            ))
        );
    }
}
