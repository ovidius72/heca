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

/// **What a label does when its text does not fit its box.**
///
/// Two ways to cut, because the two kinds of text read from opposite ends: a **label** is identified
/// by its beginning (`Move focus to the column…`), a **path** by its end (`…/projects/heca/src`).
/// Cutting a path at the tail throws away the only part anyone reads.
///
/// And [`None`](Ellipsis::None), which is the *opt-out*: keep the natural width and overflow. It
/// used to be the default, and nobody chose it — it was what the field held before truncation
/// existed. The result was that every author who might ever be squeezed had to know to ask for
/// cutting, and the ones who did not shipped text drawn across its neighbours (F003/P082/T438).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, heca_grid_ui_macros::PropName)]
pub enum Ellipsis {
    /// Keep the head, cut the tail: `Move focus to the col…`. **The default**, because most text is
    /// identified by how it starts.
    #[default]
    End,
    /// Keep the tail, cut the head: `…/heca/src`. What a path wants.
    Start,
    /// **Do not cut**: keep the natural width and paint outside the box if it does not fit. For the
    /// rare case where the text is what should size its container — and it is then *asked for*,
    /// rather than being what you get by forgetting.
    None,
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
    /// What this label does when its text does not fit — [`Ellipsis::End`] unless the caller says
    /// otherwise. **Not an `Option`**: [`Ellipsis::None`] *is* "do not cut", so there is one way to
    /// say it rather than two that can disagree.
    truncate: Ellipsis,
    /// Reflow the text onto as many lines as its width needs. Wins over
    /// [`truncate`](Self::truncate) when both are set.
    ///
    /// A [`Signal`] for the same reason [`bold`](Self::bold) is: an enclosing widget drives it from
    /// state. A list that reflows only its *selected* row flips this as the cursor moves, and it
    /// changes the label's **measure**, so the next layout pass re-measures it — which is exactly
    /// what makes the rows below shift.
    wrap: Signal<bool>,
    /// Character indices **in the source text** to draw as marks — a fuzzy match's hits. Empty (the
    /// default) means one text run, exactly as before marks existed.
    ///
    /// A [`Signal`] for the same reason [`bold`](Self::bold) is: an enclosing widget drives it from
    /// state. A search surface re-marks its rows on every keystroke, and rebuilding a hundred labels
    /// per character to change a colour is not a rebuild anyone can afford.
    marks: Signal<Vec<usize>>,
    /// The mark colour. Unset ⇒ the theme accent.
    mark_color: Option<Color>,
    /// Draw in the theme's muted colour — secondary text (a description under a title).
    muted: bool,
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
            truncate: Ellipsis::End,
            wrap: signal(false),
            marks: signal(Vec::new()),
            mark_color: None,
            muted: false,
        };
        label.remeasure();
        label
    }

    /// **Which end is lost** when the text does not fit — or [`Ellipsis::None`] to keep the natural
    /// width and overflow instead.
    ///
    /// Cutting is the default, so this is here to choose the *end*: a path wants
    /// [`Start`](Ellipsis::Start), everything else reads from its head.
    ///
    /// It used to do two more things — set `flex_shrink = 1` and force `min_width = 0` — because
    /// the cut is only reachable if the layout can hand the label less than its natural width, and
    /// nothing shrank by default while a label's "how narrow can you get?" answer was its longest
    /// word. Both are gone: shrinking is the default now, and the label answers that question
    /// honestly as **one character**. Forcing the floor to zero was the sledgehammer, and it cost a
    /// `Card` its title — a label whose floor is nothing resolves to nothing and draws no text at
    /// all (F003/P082/T438).
    ///
    /// The cut uses the **same** monospace cell the measure does ([`MONO_ADVANCE_RATIO`]), so the
    /// text ends exactly where the box does, and it is recomputed from the current bounds at every
    /// paint — a resize re-cuts with no rebuild.
    #[heca_grid_ui_macros::prop]
    pub fn truncate(mut self, mode: Ellipsis) -> Self {
        self.truncate = mode;
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
        self.wrap.set(wrap);
        // Shrinkable either way: a label that cannot be handed less than its natural width neither
        // wraps nor cuts, because there is never less width to work in.
        self.base.style.layout.flex_shrink = Some(1.0);
        self.base.style.layout.min_width = Some(Length::Px(0.0));
        self.remeasure();
        self
    }

    /// The wrap signal — reflow the label in place, without rebuilding it. Flipping it changes the
    /// label's measure, so the next layout pass re-measures and everything below it moves.
    pub fn wrap_signal(&self) -> Signal<bool> {
        self.wrap
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
    pub fn marks(self, indices: impl IntoIterator<Item = usize>) -> Self {
        self.marks.set(indices.into_iter().collect());
        self
    }

    /// The marks signal — re-mark the label in place, without rebuilding it. A search surface uses
    /// this to move the highlights as the query changes.
    pub fn marks_signal(&self) -> Signal<Vec<usize>> {
        self.marks
    }

    /// Draw in the theme's **muted** colour — secondary text, like a description under a title.
    ///
    /// Separate from [`color`](Self::color) because the theme is only resolved at paint: a caller
    /// building a tree cannot name the muted colour, and hardcoding one would not follow a theme
    /// change. Ignored when an explicit colour is set.
    #[heca_grid_ui_macros::prop]
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
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
    fn is_marked(&self, marks: &[usize], source: Option<usize>) -> bool {
        source.is_some_and(|i| marks.contains(&i))
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
        if self.wrap.get_untracked() {
            return wrap_indexed(&text, cells)
                .into_iter()
                .map(|line| line.into_iter().map(|(c, i)| (c, Some(i))).collect())
                .collect();
        }
        let whole = |chars: &[char]| -> Vec<(char, Option<usize>)> {
            chars.iter().enumerate().map(|(i, &c)| (c, Some(i))).collect()
        };
        let chars: Vec<char> = text.chars().collect();
        let mode = self.truncate;
        if mode == Ellipsis::None {
            return vec![whole(&chars)];
        }
        // **No box yet is not an empty box.** Bounds are zero until the layout pass fills them in,
        // and cutting to a zero-width box draws nothing at all — so a tree painted before it is laid
        // out would come out blank rather than merely mis-sized. Degenerate geometry means "no
        // answer yet" here exactly as it does in the picker's visibility rules (F003/P082/T438).
        if self.base.bounds.size.w <= 0.0 {
            return vec![whole(&chars)];
        }
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
                // Unreachable: returned above.
                Ellipsis::None => whole(&chars),
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
        let line_h = if self.wrap.get_untracked() { self.line_h() } else { bounds.size.h };
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
    /// **A label is what shows words**, so it answers this: the text is a signal, and writing it
    /// updates in place. This is what lets a host change what a retained tree says — a pane's
    /// foreground program, its git branch — without throwing the tree away and rebuilding it.
    fn set_text(&self, text: String) -> bool {
        use crate::reactive::SignalUpdate as _;
        self.text_signal().set(text);
        true
    }

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
        let fs = self.base.font;
        if self.wrap.get_untracked() {
            // **Deliberately no size.** A wrapped label's height is a function of the width it is
            // resolved to, which this method runs too early to know — writing the one-line height
            // here would have taffy believe it, and the extra lines would paint outside the box.
            // `measure_text` answers instead, once the width exists.
            self.base.style.layout.width = Length::Auto;
            self.base.style.layout.height = Length::Auto;
            return;
        }
        if self.truncate != Ellipsis::None {
            // **No definite width either.** A fixed natural width cannot be taken away on the cross
            // axis of a column, so a cutting label put in one simply overflowed its container and
            // never cut at all. `auto` + the measure path lets the box it is given decide, which is
            // the whole contract of truncation; the height is still exactly one line.
            self.base.style.layout.width = Length::Auto;
            self.base.style.layout.height = Length::Px(fs * MONO_LINE_RATIO);
            return;
        }
        let chars = text.chars().count() as f32;
        self.base.style.layout.width = Length::Px(chars * fs * MONO_ADVANCE_RATIO);
        self.base.style.layout.height = Length::Px(fs * MONO_LINE_RATIO);
    }

    /// A wrapping label is the one widget measured from its resolved width (see
    /// [`wrap`](Label::wrap)); every other case is sized by `remeasure` above.
    fn measure_text(&self) -> Option<crate::layout::TextMeasure> {
        let wrap = self.wrap.get_untracked();
        (wrap || self.truncate != Ellipsis::None).then(|| crate::layout::TextMeasure {
            text: self.text.get_untracked(),
            font: self.base.font,
            wrap,
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
            .or_else(|| self.muted.then(|| cx.theme().colors.muted))
            .or_else(|| cx.content_color())
            .unwrap_or_else(|| cx.theme().colors.foreground);
        let style = TextStyle::REGULAR
            .bold(self.bold.get_untracked())
            .italic(self.italic.get_untracked());
        let lines = self.drawn_indexed();
        let bounds = self.base.bounds;
        let runs = self.run_rects();
        let mark_color = self.mark_color.unwrap_or_else(|| cx.accent());
        let marks = self.marks.get_untracked();
        let wrapping = self.wrap.get_untracked();
        // A plain label draws into its whole box, exactly as before — the alignment and vertical
        // centring the renderer applies are unchanged. A wrapped one draws each line into its own
        // one-line slice, stacked from the top of the box.
        for (i, line) in lines.iter().enumerate() {
            let box_ = if wrapping {
                Rectangle::new(
                    Point::new(bounds.loc.x, bounds.loc.y + i as f64 * self.line_h()),
                    Size::new(bounds.size.w, self.line_h()),
                )
            } else {
                bounds
            };
            // **One run for the text, always.** The line is drawn whole, in the label's own
            // colour, and marked characters are then over-drawn in the mark colour on top.
            //
            // Splitting the line into runs at the mark boundaries was tried and is wrong: the
            // shaper drops a run's *leading* whitespace, so every run after the first landed a cell
            // early and the spaces walked — `Toggle Left Sidebar` drew as `ToggleLe ftSidebar`.
            // Scene-level tests could not see it, because the runs they assert were correct and the
            // loss happened in shaping.
            let text: String = line.iter().map(|(c, _)| *c).collect();
            cx.text(box_, &text, color, self.base.font, self.align, style);
            if marks.is_empty() {
                continue;
            }
            // Per character, not per run: a marked run starting with a space would lose it the same
            // way, and a single glyph cannot drift from its own position.
            let origin = runs.get(i).map_or(box_.loc.x, |r| r.loc.x);
            let cell = self.cell();
            for (col, (ch, source)) in line.iter().enumerate() {
                if !self.is_marked(&marks, *source) || ch.is_whitespace() {
                    continue;
                }
                let rect = Rectangle::new(
                    Point::new(origin + col as f64 * cell, box_.loc.y),
                    Size::new(cell, box_.size.h),
                );
                cx.text(
                    rect,
                    &ch.to_string(),
                    mark_color,
                    self.base.font,
                    TextAlign::Start,
                    style.bold(true),
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
