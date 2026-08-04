//! [`Label`] — a single run of text bound to a reactive signal.
//!
//! # Weight, slant, decorations
//! The four text attributes split cleanly in two, and the split is the interesting part:
//!
//! - **`bold` / `italic` are FONT attributes** — the shaper picks the glyphs. Bold selects the
//!   embedded family's real bold face; italic is a **synthesized oblique** (a glyph shear), because
//!   the embedded family ships no italic face and asking for a real one would substitute a
//!   *proportional* fallback and break the monospace metrics the whole grid rests on. See
//!   [`font`](crate::font).
//! - **`underline` / `strikethrough` are DECORATIONS** — a line is not a glyph, it is a rect. The
//!   label draws them itself, in its own resolved color, like any other chrome. Nothing about them
//!   reaches the shaper, which is why the renderer needed no change to gain them.

use crate::builders::LayoutExt;
use crate::color::Color;
use crate::component::{Base, Component, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO, mono_cells, wrap_indexed};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{TextAlign, TextStyle};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Decoration thickness as a fraction of the resolved font — so a rule under 10px text is hairline
/// and one under a 28px header is proportionate. Floored at 1px so it never vanishes.
const DECORATION_RATIO: f32 = 0.07;
/// How far **below the text's centre line** the underline sits, in font units — just clear of the
/// descenders of the embedded face.
const UNDERLINE_OFFSET_RATIO: f32 = 0.42;
/// How far **above the centre line** the strikethrough sits: a touch high, because a line through
/// the exact middle of the box reads as low against lowercase text (whose visual mass sits above the
/// box centre).
const STRIKE_OFFSET_RATIO: f32 = 0.06;

/// Which end of a label is cut when the text does not fit its box.
///
/// Two, because the two kinds of text read from opposite ends: a **label** is identified by its
/// beginning (`Move focus to the column…`), a **path** by its end (`…/projects/heca/src`). Cutting
/// a path at the tail throws away the only part anyone reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, heca_grid_ui_macros::PropName)]
pub enum Ellipsis {
    /// Keep the head, cut the tail: `Move focus to the col…`.
    End,
    /// Keep the tail, cut the head: `…/heca/src`.
    Start,
}

/// A text label. Its content is a [`Signal`], so updating it marks the label
/// dirty and triggers a repaint.
///
/// **Color is inherited when unset.** With no explicit [`color`](Label::color), the label paints
/// in the [content color](crate::component::PaintCx::with_content_color) published by an enclosing
/// control — which is how a `Label` composed inside a [`Button`](super::Button) tracks that
/// button's hover/disabled state without either widget knowing about the other. With no inherited
/// color either, it falls back to the theme foreground.
pub struct Label {
    base: Base,
    text: Signal<String>,
    seen_text: String,
    align: TextAlign,
    color: Option<Color>,
    /// Bold weight. A [`Signal`] because it can be **state-driven by an enclosing widget** — an
    /// [`Item`](super::Item) bolds its label while the row is active — and the inherited paint
    /// context carries only a color, not a weight. The parent flips this in its `tick`.
    bold: Signal<bool>,
    /// Slanted (a synthesized oblique — see the module docs). A [`Signal`] for the same reason as
    /// [`bold`](Self::bold): a parent can drive it from state without rebuilding the tree.
    italic: Signal<bool>,
    /// A rule under the text — drawn by this widget, not shaped.
    underline: Signal<bool>,
    /// A rule through the text — drawn by this widget, not shaped.
    strikethrough: Signal<bool>,
    /// Cut the text to its box instead of overflowing it. `None` (the default) is the historical
    /// behaviour: the label keeps its natural width and a container that cannot hold it overflows.
    truncate: Option<Ellipsis>,
    /// Reflow the text onto as many lines as its width needs. Mutually exclusive with
    /// [`truncate`](Self::truncate) — a label either cuts or wraps.
    wrap: bool,
    /// Character indices **in the source text** to draw as marks — a fuzzy match's hits. Empty (the
    /// default) means one text run, exactly as before marks existed.
    marks: Vec<usize>,
    /// The mark colour. Unset ⇒ the theme accent.
    mark_color: Option<Color>,
}

#[heca_grid_ui_macros::props]
impl Label {
    /// A label showing `text`.
    pub fn new(text: impl Into<String>) -> Self {
        let base = Base::new();
        let text = signal(text.into());
        let seen_text = text.get_untracked();
        let mut label = Self {
            base,
            text,
            seen_text,
            align: TextAlign::Start,
            color: None,
            bold: signal(false),
            italic: signal(false),
            underline: signal(false),
            strikethrough: signal(false),
            truncate: None,
            wrap: false,
            marks: Vec::new(),
            mark_color: None,
        };
        label.remeasure();
        label
    }

    /// Cut the text to fit its box, with the ellipsis at `mode`'s end.
    ///
    /// **Two halves, and the second is the one that makes it work.** Cutting at paint time is only
    /// reached if the layout can hand the label *less* than its natural width, so a truncating label
    /// also declares itself shrinkable (`flex_shrink = 1.0`, `min_width = 0`). Without that it keeps
    /// its full measured width and simply overflows — the same trap `flex_grow` had: a size that
    /// cannot shrink does not participate in the squeeze.
    ///
    /// The cut uses the **same** monospace cell the measure does ([`MONO_ADVANCE_RATIO`]), so the
    /// text ends exactly where the box does, and it is recomputed from the current bounds at every
    /// paint — a resize re-cuts with no rebuild.
    #[heca_grid_ui_macros::prop]
    pub fn truncate(mut self, mode: Ellipsis) -> Self {
        self.truncate = Some(mode);
        self.wrap = false;
        self.base.style.layout.flex_shrink = Some(1.0);
        self.base.style.layout.min_width = Some(Length::Px(0.0));
        self.remeasure();
        self
    }

    /// Reflow the text onto as many lines as it needs, breaking on **word** boundaries.
    ///
    /// The hard half is that this changes the *measure*, not just the paint. Truncation cuts
    /// against a box the engine has already decided; a wrapped label's **height is a function of
    /// its resolved width**, which no fixed size can express. So a wrapping label reports no height
    /// of its own and is measured through taffy's measure path instead
    /// ([`Component::measure_text`](crate::component::Component::measure_text)) — the first widget
    /// in this crate to need it.
    ///
    /// Like [`truncate`](Self::truncate) it declares itself shrinkable (`flex_shrink = 1`,
    /// `min_width = 0`): a label that cannot be handed less than its natural width never wraps,
    /// because there is never less width to wrap into. It still *asks* for its full single-line
    /// width — taffy's max-content answer — so a wrapping label in a roomy parent looks exactly
    /// like a plain one.
    ///
    /// Mutually exclusive with `truncate`: a label either cuts or reflows, and the last of the two
    /// setters called wins.
    #[heca_grid_ui_macros::prop]
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        if wrap {
            self.truncate = None;
            self.base.style.layout.flex_shrink = Some(1.0);
            self.base.style.layout.min_width = Some(Length::Px(0.0));
        }
        self.remeasure();
        self
    }

    /// Draw these **source character indices** as marks — accented and bold — leaving the rest of
    /// the text in the label's own colour. A fuzzy match's hits, drawn by the widget that owns the
    /// text instead of over-painted by whoever did the matching.
    ///
    /// The indices are into the **source** string, and stay correct through a cut or a reflow: a
    /// truncated label drops the marks whose characters went and shifts the survivors, and a wrapped
    /// one carries them to the line the character landed on. An index past the end of the text is
    /// ignored — a caller's indices and a signal-driven text can disagree for a frame.
    ///
    /// Empty (the default) means the label paints exactly one text run, as it always has.
    #[heca_grid_ui_macros::host_only("a list of indices, not a scalar")]
    pub fn marks(mut self, indices: impl IntoIterator<Item = usize>) -> Self {
        self.marks = indices.into_iter().collect();
        self
    }

    /// The colour marked characters are drawn in. Unset ⇒ the theme accent.
    #[heca_grid_ui_macros::prop]
    pub fn mark_color(mut self, color: Color) -> Self {
        self.mark_color = Some(color);
        self
    }

    /// Is this drawn character marked? A character the label added itself (the ellipsis) carries no
    /// source index and is never marked.
    fn is_marked(&self, source: Option<usize>) -> bool {
        source.is_some_and(|i| self.marks.contains(&i))
    }

    /// The width of one monospace cell at the resolved font.
    fn cell(&self) -> f64 {
        (self.base.font * MONO_ADVANCE_RATIO) as f64
    }

    /// The height of one line at the resolved font.
    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    /// The lines as they will actually be drawn: one for a plain or truncated label, as many as the
    /// box needs for a wrapped one.
    ///
    /// The single answer to "what is on screen", so the glyphs, the rects the decorations follow,
    /// and any test all agree. The wrap is recomputed from the current bounds at every paint, using
    /// the same [`wrap_lines`] the layout measure used to decide the height — two spellings would
    /// put the text on a different number of lines than the box was sized for.
    fn drawn_lines(&self) -> Vec<String> {
        self.drawn_indexed()
            .into_iter()
            .map(|line| line.into_iter().map(|(c, _)| c).collect())
            .collect()
    }

    /// The drawn lines, each character paired with **its index in the source text** — `None` for a
    /// character the label added itself, which is only ever the ellipsis.
    ///
    /// The one place the cut and the reflow are applied, so the glyphs, the runs the decorations
    /// follow and the marks cannot disagree. The index is what lets a mark survive either
    /// transformation: a cut drops the marks whose characters went and shifts the survivors by
    /// construction, and a reflow carries each to the line its character landed on.
    fn drawn_indexed(&self) -> Vec<Vec<(char, Option<usize>)>> {
        let text = self.text.get_untracked();
        let cells = mono_cells(self.base.bounds.size.w, self.cell());
        if self.wrap {
            return wrap_indexed(&text, cells)
                .into_iter()
                .map(|line| line.into_iter().map(|(c, i)| (c, Some(i))).collect())
                .collect();
        }
        let whole = |chars: &[char]| -> Vec<(char, Option<usize>)> {
            chars.iter().enumerate().map(|(i, &c)| (c, Some(i))).collect()
        };
        let chars: Vec<char> = text.chars().collect();
        let Some(mode) = self.truncate else {
            return vec![whole(&chars)];
        };
        if chars.len() <= cells {
            return vec![whole(&chars)];
        }
        let cut = match cells {
            0 => Vec::new(),
            1 => vec![('…', None)],
            n => match mode {
                Ellipsis::End => whole(&chars[..n - 1])
                    .into_iter()
                    .chain(std::iter::once(('…', None)))
                    .collect(),
                // Count from the tail: keep the LAST n-1 chars, which is the half a path needs.
                // Their source indices come with them, so a mark still lands on its own character.
                Ellipsis::Start => {
                    let from = chars.len() - (n - 1);
                    std::iter::once(('…', None))
                        .chain((from..chars.len()).map(|i| (chars[i], Some(i))))
                        .collect()
                }
            },
        };
        vec![cut]
    }

    /// Text horizontal alignment.
    #[heca_grid_ui_macros::prop]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Explicit text color. Unset ⇒ the enclosing control's
    /// [content color](crate::component::PaintCx::with_content_color), else the theme foreground.
    /// Setting it opts the label **out** of that inheritance.
    #[heca_grid_ui_macros::prop]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Render the label with bold weight.
    #[heca_grid_ui_macros::prop]
    pub fn bold(self, bold: bool) -> Self {
        self.bold.set(bold);
        self
    }

    /// The bold-weight signal — set it to re-weight the label in place, without rebuilding the
    /// tree. An enclosing widget uses this to drive a state-dependent weight (e.g. an
    /// [`Item`](super::Item) bolding its label while active).
    pub fn bold_signal(&self) -> Signal<bool> {
        self.bold
    }

    /// Render the label slanted — a **synthesized oblique**, not a separate face (see the module
    /// docs): the glyphs are sheared, so the advances (and the monospace grid) are untouched.
    #[heca_grid_ui_macros::prop]
    pub fn italic(self, italic: bool) -> Self {
        self.italic.set(italic);
        self
    }

    /// The slant signal — flip it in place, no rebuild.
    pub fn italic_signal(&self) -> Signal<bool> {
        self.italic
    }

    /// Draw a rule **under** the text. A decoration, not a font attribute: the label paints it as a
    /// rect in its own resolved color, so it tints with the label (including a parent's inherited
    /// content color).
    #[heca_grid_ui_macros::prop]
    pub fn underline(self, underline: bool) -> Self {
        self.underline.set(underline);
        self
    }

    /// The underline signal — flip it in place, no rebuild (a link that underlines on hover).
    pub fn underline_signal(&self) -> Signal<bool> {
        self.underline
    }

    /// Draw a rule **through** the text (a struck-out / completed item).
    #[heca_grid_ui_macros::prop]
    pub fn strikethrough(self, strikethrough: bool) -> Self {
        self.strikethrough.set(strikethrough);
        self
    }

    /// The strikethrough signal — flip it in place, no rebuild.
    pub fn strikethrough_signal(&self) -> Signal<bool> {
        self.strikethrough
    }

    /// The rect of each **text run itself** inside the label's bounds — the decorations' reference,
    /// and the box each line is drawn into.
    ///
    /// The label's box can be wider than its text (a stretched parent), and `align` decides where
    /// each run sits in it, so a decoration must follow the run rather than span the box. Widths
    /// come from the same monospace measure the layout uses, which is what keeps rule and glyphs in
    /// agreement.
    ///
    /// **One rect per drawn line**, so a wrapped label's underline follows each line instead of one
    /// rule spanning the tallest box. A single-line label returns one rect covering its whole
    /// height — unchanged from before wrapping existed.
    fn run_rects(&self) -> Vec<Rectangle> {
        let bounds = self.base.bounds;
        let font = self.base.font as f64;
        // A wrapped label's box is N lines tall, so a run is one line; a plain label's run is its
        // whole box, which is what keeps a stretched single-line label centring as it always did.
        let line_h = if self.wrap { self.line_h() } else { bounds.size.h };
        self.drawn_lines()
            .iter()
            .enumerate()
            .map(|(i, line)| {
                // The **drawn** text, not the full string: a rule under a truncated label must span
                // the glyphs that are actually there, ellipsis included.
                let chars = line.chars().count() as f64;
                let width = (chars * font * MONO_ADVANCE_RATIO as f64).min(bounds.size.w);
                let x = match self.align {
                    TextAlign::Start => bounds.loc.x,
                    TextAlign::Center => bounds.loc.x + (bounds.size.w - width) / 2.0,
                    TextAlign::End => bounds.loc.x + bounds.size.w - width,
                };
                Rectangle::new(
                    Point::new(x, bounds.loc.y + i as f64 * line_h),
                    Size::new(width, line_h),
                )
            })
            .collect()
    }

    /// Explicit font size in logical px — overrides the inherited theme font.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, size: f32) -> Self {
        self.base.style.visual.font_size = size;
        self.base.font = size;
        self.remeasure();
        self
    }

    /// Semantic font multiplier relative to the inherited base font (header ≈ 2.0,
    /// caption ≈ 0.8). Scales with a global font change.
    #[heca_grid_ui_macros::prop]
    pub fn font_scale(mut self, scale: f32) -> Self {
        self.base.style.visual.font_scale = scale;
        self
    }

    /// The reactive text signal, so callers can update the label live.
    pub fn text_signal(&self) -> Signal<String> {
        self.text
    }
}

impl Component for Label {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// The label's text — a `Label` is the leaf that gives a composed subtree its
    /// [accessible name](Component::text_summary), so a control can render the text of content it
    /// does not own (a [`Select`](super::Select) trigger showing the chosen option).
    fn text_summary(&self) -> Option<String> {
        Some(self.text.get_untracked())
    }

    /// Naive monospace measure from the resolved font ([`Base::font`]).
    fn remeasure(&mut self) {
        let text = self.text.get_untracked();
        self.seen_text = text.clone();
        if self.wrap {
            // **Deliberately no size.** A wrapped label's height is a function of the width it is
            // resolved to, which this method runs too early to know — writing the one-line height
            // here would have taffy believe it, and the extra lines would paint outside the box.
            // `measure_text` answers instead, once the width exists.
            self.base.style.layout.width = Length::Auto;
            self.base.style.layout.height = Length::Auto;
            return;
        }
        let chars = text.chars().count() as f32;
        let fs = self.base.font;
        self.base.style.layout.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO);
        self.base.style.layout.height = Length::Px(fs * MONO_LINE_RATIO);
    }

    /// A wrapping label is the one widget measured from its resolved width (see
    /// [`wrap`](Label::wrap)); every other case is sized by `remeasure` above.
    fn measure_text(&self) -> Option<crate::layout::TextMeasure> {
        self.wrap.then(|| crate::layout::TextMeasure {
            text: self.text.get_untracked(),
            font: self.base.font,
        })
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        cx.paint_base(&self.base);
        // Own color → the enclosing control's inherited content color → the theme foreground.
        // The middle step is what makes a composed label track its parent's state (a Button's
        // hover sweep / disabled fade) with no wiring between the two widgets.
        let color = self
            .color
            .or_else(|| cx.content_color())
            .unwrap_or_else(|| cx.theme().colors.foreground);
        let style = TextStyle::REGULAR
            .bold(self.bold.get_untracked())
            .italic(self.italic.get_untracked());
        let lines = self.drawn_indexed();
        let bounds = self.base.bounds;
        let runs = self.run_rects();
        let mark_color = self.mark_color.unwrap_or_else(|| cx.theme().colors.accent);
        // A plain label draws into its whole box, exactly as before — the alignment and vertical
        // centring the renderer applies are unchanged. A wrapped one draws each line into its own
        // one-line slice, stacked from the top of the box.
        for (i, line) in lines.iter().enumerate() {
            let box_ = if self.wrap {
                Rectangle::new(
                    Point::new(bounds.loc.x, bounds.loc.y + i as f64 * self.line_h()),
                    Size::new(bounds.size.w, self.line_h()),
                )
            } else {
                bounds
            };
            if self.marks.is_empty() {
                // **One run, as always.** An unmarked label must not be split into pieces: the
                // renderer shapes a run at a time, and slicing every label into characters would
                // cost the whole tree for a feature almost nothing uses.
                let text: String = line.iter().map(|(c, _)| *c).collect();
                cx.text(box_, &text, color, self.base.font, self.align, style);
                continue;
            }
            // Marked: draw runs of same-markedness, each placed by hand from the line's aligned
            // origin. `run_rects` already resolved where the text sits inside the box for this
            // `align`, so each piece is drawn at `Start` from there rather than re-deriving it.
            let origin = runs.get(i).map_or(box_.loc.x, |r| r.loc.x);
            let cell = self.cell();
            let mut col = 0usize;
            while col < line.len() {
                let marked = self.is_marked(line[col].1);
                let start = col;
                while col < line.len() && self.is_marked(line[col].1) == marked {
                    col += 1;
                }
                let piece: String = line[start..col].iter().map(|(c, _)| *c).collect();
                let rect = Rectangle::new(
                    Point::new(origin + start as f64 * cell, box_.loc.y),
                    Size::new((col - start) as f64 * cell, box_.size.h),
                );
                cx.text(
                    rect,
                    &piece,
                    if marked { mark_color } else { color },
                    self.base.font,
                    TextAlign::Start,
                    if marked { style.bold(true) } else { style },
                );
            }
        }

        // Decorations. Drawn here, in the same resolved color as the glyphs, over the *runs* (not
        // the box — a run is where the text actually is; see `run_rects`), one per drawn line. An
        // empty label has a zero-width run and so draws nothing.
        let underline = self.underline.get_untracked();
        let strikethrough = self.strikethrough.get_untracked();
        if !(underline || strikethrough) {
            return;
        }
        let font = self.base.font;
        let thickness = (font * DECORATION_RATIO).max(1.0) as f64;
        for run in self.run_rects() {
            if run.size.w <= 0.0 {
                continue;
            }
            let mid_y = run.loc.y + run.size.h / 2.0;
            let mut rule = |y: f64| {
                cx.rect(
                    Rectangle::new(Point::new(run.loc.x, y), Size::new(run.size.w, thickness)),
                    color,
                    None,
                    0.0,
                    None,
                );
            };
            if underline {
                rule(mid_y + (font * UNDERLINE_OFFSET_RATIO) as f64);
            }
            if strikethrough {
                rule(mid_y - (font * STRIKE_OFFSET_RATIO) as f64 - thickness / 2.0);
            }
        }
    }

    fn tick(&mut self, _dt: f32) -> bool {
        let next = self.text.get_untracked();
        if next != self.seen_text {
            self.seen_text = next;
            self.remeasure();
            self.base.mark_needs_paint();
        }
        false
    }
}

// Gives `Label` the shared builders — notably `size` (the size variant) and
// width/height — even though it has no children.
impl LayoutExt for Label {}
