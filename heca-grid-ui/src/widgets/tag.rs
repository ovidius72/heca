//! [`Tag`] — a small **chip**: one or more segments (each an optional leading
//! [`Icon`](super::Icon) + a label) in a rounded, subtly-bordered pill, with a
//! thin divider between segments.
//!
//! Where [`Badge`](super::Badge) is a solid status token (`RUN`, `M`, `OFFLINE`),
//! `Tag` is the quieter metadata chip that can **carry an icon** and **multiple
//! sections** — e.g. a status-bar segment `⎇ main │ 5 • +152 -12`. It is a
//! display widget: it lays out its segments and paints the pill chrome (radius,
//! border width and colors all from the [`Theme`](crate::theme::Theme) — nothing
//! hardcoded); the slots draw themselves, so it stays composable and neutral.

use crate::builders::{LayoutExt, Parent};
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::reactive::{Signal, SignalGet};
use crate::scene::Border;
use crate::style::{Align, Direction, Justify};
use crate::widgets::{Flex, Label};
use heca_core::layout::{Point, Rectangle, Size};

/// Horizontal inner padding — generous, so the chip has breathing room.
const PAD_X: f32 = 12.0;
/// Vertical inner padding — tight, to keep the pill slim.
const PAD_Y: f32 = 4.0;
/// Gap between segments (holds the divider).
const SEG_GAP: f32 = 12.0;
/// Gap between a leading slot and its label, within a segment.
const SLOT_GAP: f32 = 6.0;
/// Chip text size relative to the base font.
const FONT_SCALE: f32 = 0.8;
/// Vertical inset of a segment divider as a fraction of the chip height.
const DIVIDER_INSET_FRAC: f64 = 0.18;
/// `Theme.radius` multiplier for the chip corners, clamped to a capsule — so the
/// corner radius follows the theme, never hardcoded. A modest, rounded-rect look
/// (not a full capsule).
const RADIUS_MUL: f32 = 1.0;

/// A small labeled chip of one or more segments.
pub struct Tag {
    base: Base,
    /// Pill hue for the fill, border and dividers (default: theme muted).
    color: Option<Color>,
    label: Signal<String>,
}

/// Build an empty segment container (a centered row of slots).
fn segment() -> Flex {
    Flex::row().align(Align::Center).gap(SLOT_GAP)
}

impl Tag {
    /// A new chip whose first segment shows `label`. Add a leading icon with
    /// [`leading`](Tag::leading), or further segments with [`segment`](Tag::segment).
    pub fn new(label: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Row;
        base.style.align = Align::Center;
        base.style.justify = Justify::Center;
        base.style.padding_x = Some(PAD_X);
        base.style.padding_y = Some(PAD_Y);
        base.style.gap = SEG_GAP;
        let label = Label::new(label).font_scale(FONT_SCALE);
        let label_signal = label.text_signal();
        base.children.push(Box::new(segment().child(label)));
        Self {
            base,
            color: None,
            label: label_signal,
        }
    }

    /// Set the leading slot of the **first** segment — typically an
    /// [`Icon`](super::Icon).
    pub fn leading(mut self, c: impl Component + 'static) -> Self {
        self.base.children[0]
            .base_mut()
            .children
            .insert(0, Box::new(c));
        self
    }

    /// Append a segment (its own composed content, e.g. a row of `Icon`/`Label`).
    /// A thin divider is drawn before it.
    pub fn segment(mut self, c: impl Component + 'static) -> Self {
        self.base.children.push(Box::new(c));
        self
    }

    /// Append a component **inside** the segment at `idx` (no new segment, no divider), so it sits
    /// right next to that segment's existing content — e.g. a small dimmed suffix after a label.
    /// Out-of-range `idx` is a no-op.
    pub fn append_to_segment(mut self, idx: usize, c: impl Component + 'static) -> Self {
        if let Some(seg) = self.base.children.get_mut(idx) {
            seg.base_mut().children.push(Box::new(c));
        }
        self
    }

    /// Append a `label` segment with an optional leading icon — a convenience over
    /// [`segment`](Tag::segment) for the common icon-plus-text section.
    pub fn segment_text(
        self,
        label: impl Into<String>,
        leading: Option<Box<dyn Component>>,
    ) -> Self {
        let mut seg = segment().child(Label::new(label).font_scale(FONT_SCALE));
        if let Some(icon) = leading {
            seg.base_mut().children.insert(0, icon);
        }
        self.segment(seg)
    }

    /// Pill hue for the fill, border and dividers (default: the theme muted token).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// The reactive signal for the first segment's text label.
    pub fn label_signal(&self) -> Signal<String> {
        self.label
    }
}

impl Component for Tag {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (muted, radius_tok, border_w, ia) = {
            let t = cx.theme();
            (t.colors.muted, t.colors.border_radius, t.colors.border_width, t.colors.interaction)
        };
        let c = self.color.unwrap_or(muted);
        let pill = self.base.bounds;
        // Radius from the theme (rounder than a box), clamped to a capsule.
        let radius = (radius_tok * RADIUS_MUL).min((pill.size.h / 2.0) as f32);
        cx.rect(
            pill,
            c.with_alpha(ia.tag_fill),
            Some(Border {
                color: c.with_alpha(ia.tag_border),
                width: border_w,
            }),
            radius,
            None,
        );

        // Thin dividers in the gap between consecutive segments.
        let divider = c.with_alpha(ia.tag_border);
        let inset = pill.size.h * DIVIDER_INSET_FRAC;
        for i in 1..self.base.children.len() {
            let prev = self.base.children[i - 1].base().bounds;
            let cur = self.base.children[i].base().bounds;
            let x = (prev.loc.x + prev.size.w + cur.loc.x) / 2.0;
            cx.rect(
                Rectangle::new(
                    Point::new(x, pill.loc.y + inset),
                    Size::new(f64::from(border_w), pill.size.h - 2.0 * inset),
                ),
                divider,
                None,
                0.0,
                None,
            );
        }

        // Segments draw themselves.
        for child in &self.base.children {
            child.paint(cx);
        }
    }
}

impl LayoutExt for Tag {}
