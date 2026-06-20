//! [`Pane`] — a generic container with configurable frame decoration.
//!
//! Supports three frame modes via [`PaneFrame`]:
//!
//! | Mode | Visual |
//! |------|--------|
//! | [`PaneFrame::None`] | Background fill only — no border, no brackets |
//! | [`PaneFrame::Bordered`] | A clean border from `style.border` |
//! | [`PaneFrame::Bracketed`] | `style.border` + accent corner brackets on top |
//!
//! Default mode is [`PaneFrame::Bordered`] — a fill-only container that
//! becomes a bordered panel once `.border(color, width)` is called.
//!
//! Use `.bracketed()` to opt into the decorative corner-accent look.
//!
//! ## Title
//!
//! A [`Pane`] can carry an optional **title** that straddles the **top border**,
//! left-aligned with a small inset — an `Icon + Label` pair. Opt in with
//! [`title`](Pane::title); it decorates the frame without participating in the
//! child layout, so it never shifts the pane's content. The look is chosen with
//! [`title_style`](Pane::title_style) ([`PaneTitleStyle`]):
//!
//! - [`Cut`](PaneTitleStyle::Cut) — floats in a gap cut into the frame line (no
//!   visible box; the footprint is overpainted to match its surroundings).
//! - [`Filled`](PaneTitleStyle::Filled) — a solid chip in the frame color, text
//!   flipped to the interior color for contrast.
//! - [`Boxed`](PaneTitleStyle::Boxed) — a small bordered box on the line.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::color::Color;
use crate::component::{paint_child, Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::SignalGet;
use crate::scene::TextAlign;
use crate::style::Direction;
use crate::widgets::icon::Glyph;
use heca_core::layout::{Point, Rectangle, Size};

/// Title font size relative to the pane's resolved font — a touch smaller than
/// body text so the chip reads as chrome, not content.
const TITLE_FONT_SCALE: f32 = 0.85;
/// Gap between the title icon and its label (logical px).
const TITLE_ICON_GAP: f64 = 4.0;
/// Horizontal padding inside the title chip, each side (logical px).
const TITLE_PAD_X: f64 = 6.0;
/// Left inset of the title chip from the pane's left edge (logical px) — clears
/// the rounded corner so the chip sits on the straight top edge.
const TITLE_INSET_X: f64 = 12.0;
/// Max distance the title rises *above* the top border (logical px). Bounding the
/// overhang (rather than centering at half the chip height) keeps the title inside
/// the small gap above the first pane so a top-row title never clips against the
/// content area / tab bar, independent of font size. It also biases the title a
/// touch downward (more of it sits inside the pane), matching the design intent.
const TITLE_OVERHANG_MAX: f64 = 6.0;

/// How a [`Pane`] title is painted onto the top border.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneTitleStyle {
    /// Float the `icon + label` in a **gap cut into the frame line** — no visible
    /// box; the title's footprint is overpainted to match its surroundings so only
    /// the border vanishes under it. Title color follows the frame.
    #[default]
    Cut,
    /// A solid **chip filled with the frame color** straddling the border; the
    /// `icon + label` switch to the interior color for contrast.
    Filled,
    /// A small **bordered box** (interior fill + frame-colored border) on the
    /// line; the `icon + label` keep the frame color.
    Boxed,
}

/// An optional [`Pane`] title: an icon + a text label drawn on the top border.
struct PaneTitle {
    glyph: Glyph,
    text: String,
}

/// Frame decoration mode for a [`Pane`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneFrame {
    /// Fill only — no border, no brackets. Use when you just want the
    /// background without any frame decoration.
    None,
    /// A clean border from `style.border` (set via `.border(color, width)`).
    /// No corner brackets.
    Bordered,
    /// `style.border` + accent corner brackets on top (the classic
    /// bracket-framed look).
    Bracketed,
}

/// A generic container with configurable frame decoration.
///
/// Default [`PaneFrame`] is [`PaneFrame::Bordered`] — just a fill until
/// `.border(color, width)` is supplied.
pub struct Pane {
    base: Base,
    frame: PaneFrame,
    /// Optional title chip on the top border (see module docs).
    title: Option<PaneTitle>,
    /// How the title is painted onto the border (see [`PaneTitleStyle`]).
    title_style: PaneTitleStyle,
    /// Title frame color (the `Cut`/`Boxed` text + the `Filled` chip). Default:
    /// the frame's border color, else the theme accent.
    title_color: Option<Color>,
    /// The title's **interior color**: the `Cut` below-edge half / the `Boxed`
    /// fill / the `Filled` text color. Default: the pane's own fill, then the
    /// theme background.
    title_background: Option<Color>,
}

impl Pane {
    /// A new vertical (column) pane with the default [`PaneFrame::Bordered`].
    /// Content is inset by 8.0 px by default — override with `.padding(x)`.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.direction = Direction::Column;
        base.style.padding = 8.0;
        Self {
            base,
            frame: PaneFrame::Bordered,
            title: None,
            title_style: PaneTitleStyle::default(),
            title_color: None,
            title_background: None,
        }
    }

    /// A horizontal (row) pane.
    pub fn row() -> Self {
        let mut pane = Self::new();
        pane.base.style.direction = Direction::Row;
        pane
    }

    /// Choose the frame decoration mode.
    pub fn frame(mut self, f: PaneFrame) -> Self {
        self.frame = f;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::Bordered`].
    pub fn bordered(mut self) -> Self {
        self.frame = PaneFrame::Bordered;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::Bracketed`].
    pub fn bracketed(mut self) -> Self {
        self.frame = PaneFrame::Bracketed;
        self
    }

    /// Shorthand: set frame to [`PaneFrame::None`].
    pub fn frameless(mut self) -> Self {
        self.frame = PaneFrame::None;
        self
    }

    /// Add a title chip (`icon + label`) straddling the top border (see module
    /// docs). The title is pure decoration — it does not affect child layout.
    pub fn title(mut self, glyph: Glyph, text: impl Into<String>) -> Self {
        self.title = Some(PaneTitle {
            glyph,
            text: text.into(),
        });
        self
    }

    /// How the title is painted onto the border (default [`PaneTitleStyle::Cut`]).
    pub fn title_style(mut self, style: PaneTitleStyle) -> Self {
        self.title_style = style;
        self
    }

    /// Title **frame color** — the `Cut`/`Boxed` text + the `Filled` chip fill
    /// (default: the frame's border color, else the theme accent).
    pub fn title_color(mut self, color: Color) -> Self {
        self.title_color = Some(color);
        self
    }

    /// Title **interior color** — the `Cut` below-edge half, the `Boxed` fill, and
    /// the `Filled` text color (default: the pane's own fill, then the theme
    /// background). Set this to the pane's actual interior — e.g. a terminal's
    /// resolved background — so the title reads against the inside.
    pub fn title_background(mut self, color: Color) -> Self {
        self.title_background = Some(color);
        self
    }

    /// Paint the title's backing for the active [`PaneTitleStyle`] and return the
    /// resulting icon/text (`fg`) color. Owns the frame/interior color resolution
    /// so [`paint_title`](Self::paint_title) stays a short straight-through render.
    fn paint_title_backing(
        &self,
        cx: &mut PaintCx,
        chip: Rectangle,
        edge_y: f64,
        overhang: f64,
    ) -> Color {
        // Frame color (the line/accent the title belongs to), the interior color
        // (inside the pane), and `outside` (what sits behind the pane).
        let outside = cx.theme().background;
        let frame = self
            .title_color
            .or_else(|| self.base.style.border.map(|border| border.color))
            .unwrap_or_else(|| cx.theme().accent);
        let inside = self
            .title_background
            .or(self.base.style.fill)
            .unwrap_or(outside);
        let radius = cx.theme().control_radius().min((chip.size.h / 2.0) as f32);
        let (x, w) = (chip.loc.x, chip.size.w);

        match self.title_style {
            // Erase only the border line: overpaint the footprint in two halves that
            // each match their surroundings — the (thin) exterior strip above the
            // edge, the interior below — so only the frame line vanishes in the gap.
            // The halves meet exactly at `edge_y`; their heights are unequal because
            // the overhang is bounded. Title text in the frame color.
            PaneTitleStyle::Cut => {
                cx.rect(
                    Rectangle::new(chip.loc, Size::new(w, overhang)),
                    outside,
                    None,
                    0.0,
                    None,
                );
                cx.rect(
                    Rectangle::new(Point::new(x, edge_y), Size::new(w, chip.size.h - overhang)),
                    inside,
                    None,
                    0.0,
                    None,
                );
                frame
            }
            // Solid chip in the frame color; text flips to the interior for contrast.
            PaneTitleStyle::Filled => {
                cx.rect(chip, frame, None, radius, None);
                inside
            }
            // Bordered box: interior fill + frame-colored border; text in the frame.
            PaneTitleStyle::Boxed => {
                cx.rect(chip, inside, cx.border(frame), radius, None);
                frame
            }
        }
    }

    /// Paint the title chip on the top border: a background mask that interrupts
    /// the frame line, then a vertically-centered `icon + label` over it.
    fn paint_title(&self, cx: &mut PaintCx, title: &PaneTitle) {
        let b = self.base.bounds;
        let font = self.base.font * TITLE_FONT_SCALE;
        let advance = font * MONO_ADVANCE_RATIO;
        let icon_size = f64::from(font);
        let chip_h = f64::from(font * MONO_LINE_RATIO);

        // Cap the title so it never reaches the opposite rounded corner; truncate
        // the label with an ellipsis when it would overflow the available span.
        let max_chip_w = (b.size.w - 2.0 * TITLE_INSET_X).max(0.0);
        let fixed_w = icon_size + TITLE_ICON_GAP + 2.0 * TITLE_PAD_X;
        let max_label_w = (max_chip_w - fixed_w).max(0.0);
        let label = truncate_to_width(&title.text, advance, max_label_w);
        let label_w = label.chars().count() as f64 * f64::from(advance);
        let chip_w = icon_size + TITLE_ICON_GAP + label_w + 2.0 * TITLE_PAD_X;

        // Straddle the top border line, inset past the rounded corner. The rise
        // above the border is bounded (not half the chip) so a top-row title can't
        // clip against the content clip / tab bar (see `TITLE_OVERHANG_MAX`).
        let chip_x = b.loc.x + TITLE_INSET_X;
        let edge_y = b.loc.y;
        let overhang = (chip_h / 2.0).min(TITLE_OVERHANG_MAX);
        let chip_top = edge_y - overhang;
        let chip = Rectangle::new(Point::new(chip_x, chip_top), Size::new(chip_w, chip_h));

        // Paint the backing per style; `fg` is the resulting icon/text color.
        let fg = self.paint_title_backing(cx, chip, edge_y, overhang);

        // Duotone icon (secondary wash under the primary), matching every other
        // icon in the app.
        let secondary = {
            let a = (cx.theme().icon_secondary_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            fg.with_alpha(a)
        };
        let content_x = chip_x + TITLE_PAD_X;
        let icon_rect =
            Rectangle::new(Point::new(content_x, chip_top), Size::new(icon_size, chip_h));
        if let Some(ch) = title.glyph.secondary_char() {
            cx.icon(icon_rect, &ch.to_string(), secondary, font);
        }
        if let Some(ch) = title.glyph.primary_char() {
            cx.icon(icon_rect, &ch.to_string(), fg, font);
        }
        cx.text(
            Rectangle::new(
                Point::new(content_x + icon_size + TITLE_ICON_GAP, chip_top),
                Size::new(label_w, chip_h),
            ),
            &label,
            fg,
            font,
            TextAlign::Start,
            false,
        );
    }
}

impl Component for Pane {
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
        let b = self.base.bounds;
        let fill = self.base.style.fill;

        // Radius: per-widget override (> 0), else theme fallback.
        let radius = if self.base.style.radius > 0.0 {
            self.base.style.radius
        } else {
            cx.theme().radius
        };

        match self.frame {
            PaneFrame::None => {
                // Fill only — no border, no brackets.
                if let Some(f) = fill {
                    cx.rect(b, f, None, radius, self.base.style.glow);
                }
            }
            PaneFrame::Bordered => {
                // Fill + clean border from style.border.
                let border = self.base.style.border;
                if let Some(f) = fill {
                    cx.rect(b, f, border, radius, self.base.style.glow);
                } else if border.is_some() {
                    cx.rect(b, Color::TRANSPARENT, border, radius, None);
                }
            }
            PaneFrame::Bracketed => {
                // Fill + style.border + bracket accents on top.
                let border = self.base.style.border;
                if let Some(f) = fill {
                    cx.rect(b, f, border, radius, self.base.style.glow);
                } else if border.is_some() {
                    cx.rect(b, Color::TRANSPARENT, border, radius, None);
                }
                cx.bracket_frame(b, fill);
            }
        }

        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }

        // Title last, so the chip + its mask sit over the frame and content.
        if let Some(title) = &self.title {
            self.paint_title(cx, title);
        }
    }
}

impl Default for Pane {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Pane {}
impl StyleExt for Pane {}
impl Parent for Pane {}

/// Vertical space (logical px) a top-border title occupies **below** the pane's
/// top edge, at the default inherited font (the size a `Pane` resolves to when the
/// host doesn't override the layout base font). A host that paints its own content
/// inside a titled pane (e.g. a terminal grid) adds this to the pane's **top**
/// content padding so a shown title never overlaps the content. Matches the
/// geometry in [`Pane::paint_title`] (`chip_h - overhang`).
pub fn title_reserved_height() -> f32 {
    let chip_h = crate::layout::DEFAULT_BASE_FONT * TITLE_FONT_SCALE * MONO_LINE_RATIO;
    let overhang = (chip_h / 2.0).min(TITLE_OVERHANG_MAX as f32);
    chip_h - overhang
}

/// Truncate `text` to fit `max_w` (logical px) at the given per-char `advance`,
/// appending an ellipsis when it overflows. Char-based to match the naive
/// monospace measure used across this crate.
fn truncate_to_width(text: &str, advance: f32, max_w: f64) -> String {
    let per = f64::from(advance);
    let len = text.chars().count();
    if per <= 0.0 || (len as f64) * per <= max_w {
        return text.to_string();
    }
    match (max_w / per).floor() as usize {
        0 => String::new(),
        1 => "…".to_string(),
        n => {
            let mut s: String = text.chars().take(n - 1).collect();
            s.push('…');
            s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_to_width;

    #[test]
    fn keeps_text_that_fits_within_the_width() {
        // 5 chars × 10px = 50px fits within 60px.
        assert_eq!(truncate_to_width("codex", 10.0, 60.0), "codex");
    }

    #[test]
    fn ellipsizes_text_that_overflows_the_width() {
        // 35px ⇒ 3 chars fit; keep 2 + ellipsis.
        assert_eq!(truncate_to_width("codexer", 10.0, 35.0), "co…");
    }

    #[test]
    fn collapses_to_ellipsis_when_only_one_char_fits() {
        assert_eq!(truncate_to_width("codex", 10.0, 12.0), "…");
    }

    #[test]
    fn drops_label_when_not_even_an_ellipsis_fits() {
        // No room for a single glyph ⇒ empty (the icon still renders). Partial-
        // ellipsis clipping would look worse, so dropping the label is intentional.
        assert_eq!(truncate_to_width("codex", 10.0, 4.0), "");
    }
}
