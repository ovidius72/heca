//! **What changed between two scenes** — the part of the window a frame has to repaint.
//!
//! A scene is the whole picture as a list of commands, each with the box it draws in. Two
//! frames' scenes are compared command by command: where they differ, the old command's reach and
//! the new one's are both repainted — the old to erase it, the new to draw it. This is what lets a
//! host keep its picture between frames and redraw only a part, without any widget having to
//! report what it changed: whatever a widget paints differently shows up here.
//!
//! A command added or removed (a hover pill appearing, a caret blinking) shifts every command
//! after it, so the lists are not compared index by index: the run they **start** with in common
//! and the run they **end** with in common are left alone, and only what lies between — in either
//! scene — is repainted. A command in the common runs draws exactly what it drew before, and what
//! changed around it is in the repainted box, where it is drawn again in order.

use heca_core::layout::{Point, Rectangle, Size};

use super::{DrawCommand, HostDraw, Scene};

/// What a frame has to repaint, from comparing its scene with the last one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneDamage {
    /// Nothing the scene draws changed.
    Nothing,
    /// Only this box (logical px) changed.
    Area(Rectangle),
    /// Repaint the whole window.
    Everything,
}

impl SceneDamage {
    /// This damage, grown to include `rect`.
    pub fn with(self, rect: Rectangle) -> Self {
        match self {
            Self::Nothing => Self::Area(rect),
            Self::Area(a) => Self::Area(a.union(rect)),
            Self::Everything => Self::Everything,
        }
    }
}

impl Scene {
    /// **What changed since `before`**: the union of the reach of every command that differs.
    /// See the module docs.
    pub fn damage_since(&self, before: &Scene) -> SceneDamage {
        let mut damage = SceneDamage::Nothing;
        for (now, then) in [
            (&self.commands, &before.commands),
            (&self.outline, &before.outline),
        ] {
            damage = changed_between(now, then, damage);
        }
        // The overlays are flushed in segments, by depth: when the segments moved, every overlay
        // is repainted (they are small); otherwise they compare like the rest.
        if self.overlay_segs == before.overlay_segs {
            damage = changed_between(&self.overlay, &before.overlay, damage);
        } else {
            for c in self.overlay.iter().chain(&before.overlay) {
                damage = damage.with(reach(c));
            }
        }
        damage
    }

    /// Every box surface `id` is drawn in (a terminal may be drawn in more than one place).
    pub fn surface_rects(&self, id: u64) -> impl Iterator<Item = Rectangle> + '_ {
        self.all().filter_map(move |c| match c {
            DrawCommand::Host(h) if h.draw == (HostDraw::Surface { id }) => Some(h.rect),
            _ => None,
        })
    }

    /// **`damage`, grown for what blurs it.** A backdrop blurs what is drawn behind it, so a change
    /// under one, or within its blur's reach of it, changes the whole frosted box: that box, and
    /// the reach of its blur around it, is repainted too.
    pub fn grow_for_backdrops(&self, damage: SceneDamage) -> SceneDamage {
        let SceneDamage::Area(area) = damage else {
            return damage;
        };
        let mut grown = area;
        for c in self.all() {
            if let DrawCommand::Host(h) = c
                && let HostDraw::Backdrop { radius, .. } = h.draw
            {
                let reach = outset(h.rect, f64::from(radius) * 2.0);
                if reach.intersection(area).is_some() {
                    grown = grown.union(reach);
                }
            }
        }
        SceneDamage::Area(grown)
    }

    fn all(&self) -> impl Iterator<Item = &DrawCommand> {
        self.commands
            .iter()
            .chain(&self.outline)
            .chain(&self.overlay)
    }
}

/// `damage`, grown by the reach of every command between the run `now` and `then` start with in
/// common and the run they end with in common.
fn changed_between(
    now: &[DrawCommand],
    then: &[DrawCommand],
    mut damage: SceneDamage,
) -> SceneDamage {
    let head = now.iter().zip(then).take_while(|(a, b)| a == b).count();
    let tail = now[head..]
        .iter()
        .rev()
        .zip(then[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    for c in now[head..now.len() - tail]
        .iter()
        .chain(&then[head..then.len() - tail])
    {
        damage = damage.with(reach(c));
    }
    damage
}

/// **How far a command draws**: its box, and every halo around it — glow, shadow, border, a
/// glyph's overhang. Generous on purpose: repainting a few pixels too many costs nothing visible,
/// one too few leaves a stale pixel.
fn reach(c: &DrawCommand) -> Rectangle {
    let glow = |g: Option<super::Glow>| g.map_or(0.0, |g| f64::from(g.radius) * 2.0);
    match c {
        DrawCommand::Rect(r) => {
            let shadow = r.shadow.map_or(0.0, |s| {
                f64::from(s.radius) * 2.0 + f64::from(s.dx.abs().max(s.dy.abs()))
            });
            let border = r.border.map_or(0.0, |b| f64::from(b.width));
            outset(r.rect, glow(r.glow).max(shadow).max(border) + 2.0)
        }
        DrawCommand::Brackets(b) => outset(b.rect, glow(b.glow) + f64::from(b.thickness) + 2.0),
        DrawCommand::Text(t) => outset(t.rect, glow(t.glow) + f64::from(t.size) + 2.0),
        DrawCommand::Scanline(s) => outset(s.rect, 2.0),
        DrawCommand::Line(l) => {
            let x0 = l.start.x.min(l.end.x);
            let y0 = l.start.y.min(l.end.y);
            let size = Size::new((l.start.x - l.end.x).abs(), (l.start.y - l.end.y).abs());
            let pad = f64::from(l.width.max(l.arrow_size)) + 2.0;
            outset(Rectangle::new(Point::new(x0, y0), size), pad)
        }
        DrawCommand::PushClip(r) => *r,
        DrawCommand::Host(h) => match h.draw {
            HostDraw::Backdrop { radius, .. } => outset(h.rect, f64::from(radius) * 2.0),
            _ => outset(h.rect, 2.0),
        },
        // A pop draws nothing; a pop that moved changed the commands around it, which say so.
        DrawCommand::PopClip => Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)),
    }
}

/// `r` grown by `by` on every side.
fn outset(r: Rectangle, by: f64) -> Rectangle {
    Rectangle::new(
        Point::new(r.loc.x - by, r.loc.y - by),
        Size::new(r.size.w + 2.0 * by, r.size.h + 2.0 * by),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;

    fn rect_at(x: f64) -> DrawCommand {
        DrawCommand::Rect(super::super::RectCmd {
            rect: Rectangle::new(Point::new(x, 10.0), Size::new(20.0, 20.0)),
            fill: Color::rgb(1, 2, 3),
            border: None,
            radius: 0.0,
            glow: None,
            shadow: None,
        })
    }

    fn scene(commands: Vec<DrawCommand>) -> Scene {
        let mut s = Scene::default();
        for c in commands {
            s.push(c);
        }
        s
    }

    /// The same picture twice is nothing to repaint; one moved box is its old place and its new.
    #[test]
    fn a_moved_box_damages_where_it_was_and_where_it_is() {
        let before = scene(vec![rect_at(0.0), rect_at(100.0)]);
        assert_eq!(before.damage_since(&before), SceneDamage::Nothing);
        let after = scene(vec![rect_at(0.0), rect_at(150.0)]);
        let SceneDamage::Area(a) = after.damage_since(&before) else {
            panic!("one box moved: an area")
        };
        assert!(a.loc.x <= 100.0 && a.loc.x + a.size.w >= 170.0, "{a:?}");
        assert!(
            a.loc.x > 20.0,
            "the box that did not move is left alone: {a:?}"
        );
    }

    /// **A change under a frosted box repaints the whole box**: its blur reads what is behind it,
    /// so the part outside the change shows it too. A change far from it leaves it alone.
    #[test]
    fn a_change_under_a_backdrop_repaints_the_whole_frosted_box() {
        let mut s = Scene::default();
        s.push(DrawCommand::Host(super::super::HostCmd {
            draw: HostDraw::Backdrop {
                radius: 8.0,
                corner: 0.0,
            },
            rect: Rectangle::new(Point::new(100.0, 100.0), Size::new(200.0, 100.0)),
            alpha: 1.0,
            echo: false,
            rounded: None,
        }));
        let under = SceneDamage::Area(Rectangle::new(
            Point::new(110.0, 110.0),
            Size::new(5.0, 5.0),
        ));
        let SceneDamage::Area(grown) = s.grow_for_backdrops(under) else {
            panic!("still an area")
        };
        assert!(grown.size.w >= 200.0 && grown.size.h >= 100.0, "{grown:?}");
        let far = SceneDamage::Area(Rectangle::new(
            Point::new(600.0, 600.0),
            Size::new(5.0, 5.0),
        ));
        assert_eq!(s.grow_for_backdrops(far), far, "far from it: unchanged");
    }

    /// **A command added in the middle repaints only itself** — the commands after it moved in the
    /// list but draw exactly what they drew.
    #[test]
    fn a_command_added_repaints_only_where_it_draws() {
        let before = scene(vec![rect_at(0.0), rect_at(300.0)]);
        let after = scene(vec![rect_at(0.0), rect_at(150.0), rect_at(300.0)]);
        let SceneDamage::Area(a) = after.damage_since(&before) else {
            panic!("one box added: an area")
        };
        assert!(a.loc.x <= 150.0 && a.loc.x + a.size.w >= 170.0, "{a:?}");
        assert!(
            a.loc.x > 20.0 && a.loc.x + a.size.w < 300.0,
            "the others are left alone: {a:?}"
        );
        let SceneDamage::Area(gone) = before.damage_since(&after) else {
            panic!("one box removed: an area")
        };
        assert!(
            gone.loc.x <= 150.0 && gone.loc.x + gone.size.w >= 170.0,
            "its old place: {gone:?}"
        );
    }
}
