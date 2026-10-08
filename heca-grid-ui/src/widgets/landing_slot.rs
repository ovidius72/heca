//! [`LandingSlot`] — **a place something can land**, outlined, with a letter in its middle when a
//! key picks it.
//!
//! One widget serves both ways of choosing a place: the places a carried pane could land (empty
//! placeholders that exist only while a drag is in flight), and the places a keyboard pick offers
//! (existing things outlined, plus empty placeholders between them), each with its letter. It fills
//! the box it is given and works out no geometry of its own.

use crate::builders::{ComponentExt, LayoutExt};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{SignalGet, SignalUpdate};
use crate::scene::{Border, TextAlign, TextStyle};
use crate::style::{Length, Space, Spacing};
use heca_core::layout::{Point, Rectangle, Size};

/// How much bigger than the base font the letter is drawn.
const LETTER_SCALE: f32 = 1.4;
/// Alpha of the wash over an empty placeholder.
const WASH: u8 = 28;

/// **Where an edge's line runs inside its box**, as a share across it — so the box can be bigger
/// than the line: the area a drop counts in, with the line at one side of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EdgeLine {
    /// Standing up, at this share of the box's width from its left.
    Upright(f32),
    /// Lying down, at this share of the box's height from its top.
    Flat(f32),
}

/// A place something can land.
pub struct LandingSlot {
    base: Base,
    label: Option<String>,
    filled: bool,
    /// A line along the box instead of a filled place — see [`edge`](Self::edge).
    edge: bool,
    /// How far an edge reacts past the line it draws, on each side.
    reach: Space,
    /// Where the line runs in the box; along the box's longer side, in its middle, when unset.
    line: Option<EdgeLine>,
}

#[heca_grid_ui_macros::props]
impl LandingSlot {
    /// An empty placeholder: a faint wash with an accent outline.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Percent(1.0);
        Self {
            base,
            label: None,
            filled: false,
            edge: false,
            reach: Spacing::Xl.into(),
            line: None,
        }
    }

    /// **Outline something that is already there** instead of marking an empty place: no wash, the
    /// thing underneath stays readable.
    #[heca_grid_ui_macros::prop]
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// **A line instead of a place**: the box is drawn as one thin line along its middle — faint
    /// at rest, bright and thicker while a drag is over it — in the theme's `drag_edge_color`
    /// (accent when unset) and `drag_edge_width`. The box is the reacting zone, wider than the
    /// line, and takes no room of its own: it is laid over the border it marks.
    #[heca_grid_ui_macros::prop]
    pub fn edge(mut self, edge: bool) -> Self {
        self.edge = edge;
        self
    }

    /// **How far an edge reacts past its line**, on each side — a step of the theme's spacing (or
    /// pixels), resolved at layout time like the splitter's grab, so a border of no thickness is
    /// still a reachable target at any zoom.
    #[heca_grid_ui_macros::host_only("a spacing step or pixels, not a scalar a description carries")]
    pub fn reach(mut self, reach: impl Into<Space>) -> Self {
        self.reach = reach.into();
        self
    }

    /// **Where the line runs inside the box** — for an edge whose box is the whole area a drop
    /// counts in, with the line at one side of it. Unset, the line runs along the box's longer side,
    /// in its middle, and the box is the line itself.
    #[heca_grid_ui_macros::host_only("a position worked out by the host's layout each frame")]
    pub fn line(mut self, line: EdgeLine) -> Self {
        self.line = Some(line);
        self
    }

    /// The letter drawn in the middle — the key that picks this place.
    #[heca_grid_ui_macros::prop]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// **Take drops of `kind`**, for a place that is only there for the length of a drag — the
    /// owner seats it when places open, so it is shown from the start.
    #[heca_grid_ui_macros::prop]
    pub fn accepting(mut self, kind: impl Into<String>) -> Self {
        self.base.drop_target = true;
        self.base.accepts = vec![kind.into()];
        self
    }

    /// **Exist only while a drag of `kind` is in flight.** It accepts that kind, is hidden until the
    /// drag begins and hidden again when it ends — told by the framework, so nothing hands it a
    /// flag.
    #[heca_grid_ui_macros::prop]
    pub fn while_dragging(mut self, kind: impl Into<String>) -> Self {
        self.base.drop_target = true;
        self.base.accepts = vec![kind.into()];
        self.base.visible.set(false);
        let visible = self.base.visible;
        self.on_drag_in_flight(move |_| visible.set(true))
            .on_drag_settled(move |_| visible.set(false))
    }
}

impl LandingSlot {
    /// The line, and a chip with the letter on it when a pick gives it one.
    fn paint_edge(&self, cx: &mut PaintCx) {
        let near = self.base.pointer.is_drag_over();
        // Every place shows faintly in the edge colour; the one a drop would land on takes the
        // target colour, so it stands apart from the rest.
        let (width, color, target) = {
            let colors = &cx.theme().colors;
            let target = colors.drag_edge_target_color.unwrap_or(colors.warning);
            (colors.drag_edge_width, colors.drag_edge_color, target)
        };
        let color = if near { target } else { color.unwrap_or_else(|| cx.accent()) };
        let thickness = f64::from(if near { width } else { width / 2.0 });
        let line = self.line_rect(thickness);
        cx.with_opacity(if near { 1.0 } else { 0.5 }, |cx| {
            // Round ends, like the panes it runs between.
            cx.rect(line, color, None, (thickness / 2.0) as f32, None);
        });
        if let Some(letter) = &self.label {
            let side = f64::from(self.base.font) * 1.6;
            let (cx_, cy_) = (line.loc.x + line.size.w / 2.0, line.loc.y + line.size.h / 2.0);
            let chip = Rectangle::new(
                Point::new(cx_ - side / 2.0, cy_ - side / 2.0),
                Size::new(side, side),
            );
            let bg = cx.theme().colors.background;
            cx.with_overlay(|cx| {
                cx.rect(chip, color, None, cx.theme().colors.border_radius, None);
                cx.text(chip, letter, bg, self.base.font, TextAlign::Center, TextStyle::BOLD);
            });
        }
    }

    /// Where the line runs: as [`line`](Self::line) says, else along the box's longer side, in
    /// its middle.
    fn resolved_line(&self) -> EdgeLine {
        let b = self.base.bounds;
        self.line.unwrap_or(match b.size.h > b.size.w {
            true => EdgeLine::Upright(0.5),
            false => EdgeLine::Flat(0.5),
        })
    }

    /// Where the line is drawn, `thickness` across: at the share [`line`](Self::line) names, or
    /// along the longer side of the box in its middle — a border between two columns stands up,
    /// one between two stacked panes lies down.
    fn line_rect(&self, thickness: f64) -> Rectangle {
        let b = self.base.bounds;
        match self.resolved_line() {
            EdgeLine::Upright(share) => Rectangle::new(
                Point::new(b.loc.x + b.size.w * f64::from(share) - thickness / 2.0, b.loc.y),
                Size::new(thickness, b.size.h),
            ),
            EdgeLine::Flat(share) => Rectangle::new(
                Point::new(b.loc.x, b.loc.y + b.size.h * f64::from(share) - thickness / 2.0),
                Size::new(b.size.w, thickness),
            ),
        }
    }
}

impl Default for LandingSlot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for LandingSlot {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// An edge reacts over its box **and a little either side of its line**: the box may be the
    /// border itself, of no thickness at all, or the whole area a drop counts in.
    fn hit_bounds(&self) -> Option<Rectangle> {
        let b = self.base.bounds;
        if !self.edge {
            return Some(b);
        }
        let reach = f64::from(self.reach.resolve(self.base.font));
        let line = self.line_rect(0.0);
        let around = match self.resolved_line() {
            EdgeLine::Upright(_) => Rectangle::new(
                Point::new(line.loc.x - reach, line.loc.y),
                Size::new(2.0 * reach, line.size.h),
            ),
            EdgeLine::Flat(_) => Rectangle::new(
                Point::new(line.loc.x, line.loc.y - reach),
                Size::new(line.size.w, 2.0 * reach),
            ),
        };
        // The box, and the line with a little either side of it.
        let (x, y) = (b.loc.x.min(around.loc.x), b.loc.y.min(around.loc.y));
        let right = (b.loc.x + b.size.w).max(around.loc.x + around.size.w);
        let bottom = (b.loc.y + b.size.h).max(around.loc.y + around.size.h);
        Some(Rectangle::new(Point::new(x, y), Size::new(right - x, bottom - y)))
    }

    fn paints_own_drag_feedback(&self) -> bool {
        self.edge
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        if self.edge {
            self.paint_edge(cx);
            return;
        }
        let accent = cx.accent();
        let bounds = self.base.bounds;
        let wash = if self.filled {
            Color::TRANSPARENT
        } else {
            accent.with_alpha(WASH)
        };
        cx.rect(
            bounds,
            wash,
            Some(Border {
                color: accent,
                width: cx.theme().colors.border_width.max(1.5),
            }),
            cx.theme().colors.border_radius,
            None,
        );
        if let Some(letter) = &self.label {
            let size = self.base.font * LETTER_SCALE;
            let line = f64::from(size) * 1.4;
            let rect = Rectangle::new(
                Point::new(bounds.loc.x, bounds.loc.y + (bounds.size.h - line) / 2.0),
                Size::new(bounds.size.w, line),
            );
            cx.text(
                rect,
                letter,
                accent,
                size,
                TextAlign::Center,
                TextStyle::BOLD,
            );
        }
    }
}
impl LayoutExt for LandingSlot {}
