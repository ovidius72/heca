//! [`CommandPalette`] — a fuzzy command launcher (overlay).
//!
//! A centered-near-top overlay (same input-capturing contract as
//! [`Select`](super::Select): `overlay_active` +
//! `focusable` only while open, content **drawn + hit-tested manually** on the
//! overlay layer): a query line over a scrollable list of [`Command`]s. Typing
//! filters the list with a **fuzzy subsequence** match (chars in order),
//! **smart-case** (case-insensitive unless the query contains an uppercase
//! letter), ranked, with the matched characters highlighted in the accent.
//!
//! Each [`Command`] carries a label, an **optional** [`Glyph`] icon, an
//! **optional** keybinding hint, and an `on_run` callback; selecting one fires
//! the callback and closes. Navigation is built in (↑/↓ and **Ctrl+J / Ctrl+K**)
//! and also exposed as intents ([`select_next`](CommandPalette::select_next),
//! [`select_prev`](CommandPalette::select_prev),
//! [`run_selected`](CommandPalette::run_selected)) so a host can bind its own
//! configurable keys. Open/close is a host-owned [`Signal<bool>`](crate::reactive::Signal).

use crate::builders::LayoutExt;
use crate::style::WidgetSize;
use crate::component::{Base, Component, Event, Handled, Modifiers, PaintCx, WidgetIntent};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::widgets::{paint_panel_chrome, Glyph, Input, KeyCap, KeycapVariant, PanelChrome, PanelElevation};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

/// One command in a [`CommandPalette`].
pub struct Command {
    label: String,
    description: Option<String>,
    icon: Option<Glyph>,
    /// One entry per **binding**, each a chord of caps (`[λ] [⇧] [e]`). Several bindings stack, one
    /// per line — an action bound twice really is bound twice, and joining them into `"λ h / λ ←"`
    /// was both wider than the label and impossible to draw as chips.
    chords: Vec<Vec<KeyCap>>,
    on_run: Box<dyn Fn()>,
}

#[heca_grid_ui_macros::props]
impl Command {
    /// A command with `label` that runs `on_run` when selected.
    pub fn new(label: impl Into<String>, on_run: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            description: None,
            icon: None,
            chords: Vec::new(),
            on_run: Box::new(on_run),
        }
    }

    /// An optional second line, muted — what the command does.
    ///
    /// Filtering still matches the **label** only: the description explains a command the user has
    /// already found, and matching it would rank a command whose label the query never mentioned.
    #[heca_grid_ui_macros::prop]
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// An optional leading icon.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// Add one **binding**, as the caps of its chord — `[Text("λ"), Nf(Shift), Text("e")]` draws
    /// `[λ] [⇧] [e]`, right-aligned in the list's reserved shortcut column.
    ///
    /// Call it once per binding: a second call stacks a second row of chips under the first, rather
    /// than replacing it or running the two together on one line.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar")]
    pub fn keys(mut self, chord: impl IntoIterator<Item = KeyCap>) -> Self {
        let chord: Vec<KeyCap> = chord.into_iter().collect();
        if !chord.is_empty() {
            self.chords.push(chord);
        }
        self
    }
}

/// One filtered result: command index + match score + matched char indices.
struct Match {
    cmd: usize,
    score: i32,
    hits: Vec<usize>,
}

/// Narrowest the panel is allowed to be — and it yields to a window narrower than itself.
const PANEL_MIN_W: f64 = 360.0;
/// The **most** of the window a panel may ever take, so it never touches the edges — the guard that
/// makes a small screen safe. Applied after the size variant's cap, and it wins.
const PANEL_VIEWPORT_FRAC: f64 = 0.92;
/// Panel top offset as a fraction of the viewport height (palettes sit high).
const TOP_FRAC: f64 = 0.12;
/// Room kept clear **below** the panel, as a fraction of the viewport height. `TOP_FRAC` is an
/// offset, not a margin: it guards the top and says nothing about the bottom, which is how the list
/// came to fill the window down to its last pixel and the final row ended up flush against the
/// screen edge.
///
/// Deliberately **wider than the gap [`PANEL_VIEWPORT_FRAC`] leaves at the sides** (4%). Matching
/// the sides was tried first and still read as touching: the panel's bottom edge landed on the top
/// of the bottom pane, so the gap disappeared into a boundary that was already there instead of
/// separating the panel from it.
const BOTTOM_FRAC: f64 = 0.08;
/// Panel inner padding.
const PAD: f64 = 12.0;
/// Vertical padding inside the query line and each result row.
const QUERY_PAD_Y: f64 = 9.0;
const ROW_PAD_Y: f64 = 6.0;
/// Horizontal inset of row content.
const ROW_PAD_X: f64 = 10.0;
/// Gap between an icon and the label.
const ICON_GAP: f64 = 10.0;
/// Command icon size as a fraction of the row font — a touch larger than the text, so the glyph
/// reads as the row's subject rather than as punctuation beside it.
const ICON_FONT_MUL: f32 = 1.3;
/// Gap between two keycap chips of one chord.
const CAP_GAP: f64 = 3.0;
/// Minimum gap between a label and the reserved shortcut column.
const SHORTCUT_GAP: f64 = 16.0;
/// Keycap font as a fraction of the row font — a chip reads as an annotation, not as content.
const KEYCAP_FONT_MUL: f32 = 0.85;
/// What a [`WidgetSize`] means for the panel: how wide it may get, and how many rows it shows.
///
/// The caller picks a **semantic** size (a `[settings] command_palette_size` of `small` / `normal` /
/// `large`); the widget owns the pixels, per the styling contract. Deliberately **width and row
/// count only** — the text stays at reading size at every variant, which is why this is a small
/// table here rather than `Style::size`, whose font scaling would cascade to the whole panel.
const fn panel_metrics(size: WidgetSize) -> (f64, usize) {
    match size {
        WidgetSize::Small => (560.0, 6),
        WidgetSize::Normal => (700.0, 8),
        WidgetSize::Large | WidgetSize::Header => (1000.0, 12),
    }
}

/// Fuzzy subsequence match. Returns `(score, matched_indices)` if every query
/// char appears in order in `text`. Higher score = better (consecutive,
/// start-of-word, and earliness bonuses).
fn fuzzy(query: &str, text: &str, case_sensitive: bool) -> Option<(i32, Vec<usize>)> {
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() {
        return Some((0, Vec::new()));
    }
    let t: Vec<char> = text.chars().collect();
    let norm = |c: char| {
        if case_sensitive {
            c
        } else {
            c.to_ascii_lowercase()
        }
    };
    let mut qi = 0;
    let mut hits = Vec::with_capacity(q.len());
    let mut score = 0i32;
    let mut prev: Option<usize> = None;
    for (ti, &tc) in t.iter().enumerate() {
        if norm(tc) == norm(q[qi]) {
            score += 1;
            if prev == Some(ti.wrapping_sub(1)) {
                score += 5; // consecutive run
            }
            if ti == 0 || !t[ti - 1].is_alphanumeric() {
                score += 8; // start of a word
            }
            hits.push(ti);
            prev = Some(ti);
            qi += 1;
            if qi == q.len() {
                // Prefer shorter / tighter matches.
                score -= (t.len() as i32 - q.len() as i32) / 4;
                return Some((score, hits));
            }
        }
    }
    None
}

/// A fuzzy command launcher.
pub struct CommandPalette {
    base: Base,
    commands: Vec<Command>,
    /// The query field — a real [`Input`], so editing (selection, word/line
    /// delete, multi-click, caret) comes for free. Driven manually since the
    /// palette is overlay-drawn: its bounds/font/focus are set at paint time.
    query: RefCell<Input>,
    selected: usize,
    scroll: usize,
    placeholder: String,
    /// How roomy the panel is — width and row count, never font. Set from the app's
    /// `[settings] command_palette_size`.
    panel_size: WidgetSize,
    open: Signal<bool>,
    modifiers: Modifiers,
    viewport: Cell<Size>,
    /// Panel rect cached at paint, so the caret blink can damage just the panel
    /// (the palette paints on the overlay layer, away from its layout `bounds`).
    panel: Cell<Rectangle>,
}

impl CommandPalette {
    /// A new, empty (closed) palette.
    pub fn new() -> Self {
        Self {
            base: Base::new(),
            commands: Vec::new(),
            query: RefCell::new(Input::new()),
            selected: 0,
            scroll: 0,
            placeholder: "Type a command…".to_string(),
            panel_size: WidgetSize::Normal,
            open: signal(false),
            modifiers: Modifiers::default(),
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
        }
    }

    /// Add a command.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn command(mut self, c: Command) -> Self {
        self.commands.push(c);
        self
    }

    /// How roomy the panel is: `Small` / `Normal` / `Large` decide its **maximum width and how many
    /// rows it shows**, and nothing else — the text stays at reading size at every size, which is why
    /// this is its own property rather than [`LayoutExt::size`](crate::builders::LayoutExt::size),
    /// whose font scaling would cascade through the whole panel.
    ///
    /// Whatever the variant, the panel never exceeds [`PANEL_VIEWPORT_FRAC`] of the window, so a
    /// small screen is safe by construction.
    #[heca_grid_ui_macros::prop]
    pub fn panel_size(mut self, size: WidgetSize) -> Self {
        self.panel_size = size;
        self
    }

    /// Set the empty-query placeholder text.
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set the initial open state.
    #[heca_grid_ui_macros::prop]
    pub fn open(self, open: bool) -> Self {
        self.open.set(open);
        self
    }

    /// The open-state signal — the host binds a trigger (e.g. Ctrl+K) to it.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
    }

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// The current query text (read from the [`Input`]).
    fn query_text(&self) -> String {
        self.query.borrow().value_str()
    }

    /// The current filtered + ranked results.
    fn results(&self) -> Vec<Match> {
        let query = self.query_text();
        let case_sensitive = query.chars().any(|c| c.is_uppercase());
        let mut out: Vec<Match> = self
            .commands
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                fuzzy(&query, &c.label, case_sensitive).map(|(score, hits)| Match {
                    cmd: i,
                    score,
                    hits,
                })
            })
            .collect();
        // Stable sort by score desc (filter_map preserved original order for ties).
        out.sort_by_key(|m| std::cmp::Reverse(m.score));
        out
    }

    /// Move the selection down (Ctrl+J / ↓), keeping it on-screen.
    pub fn select_next(&mut self) {
        let n = self.results().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(n - 1);
        self.follow_selection();
    }

    /// Move the selection up (Ctrl+K / ↑).
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.follow_selection();
    }

    /// Run the selected command (fires its callback) and close.
    pub fn run_selected(&mut self) {
        let results = self.results();
        if let Some(m) = results.get(self.selected) {
            (self.commands[m.cmd].on_run)();
        }
        self.close();
    }

    /// Keep `selected` within the visible scroll window.
    fn follow_selection(&mut self) {
        let visible = self.visible_rows(&self.results());
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    /// How many rows the panel shows: the size variant's count, **and never more than the window
    /// can hold with [`BOTTOM_FRAC`] still clear beneath it**.
    ///
    /// It measures the rows it would actually draw, from `scroll` onward, instead of dividing the
    /// room by a nominal row height. A row is two lines when the list is described and taller again
    /// when an action carries several bindings, so a one-line estimate over-counted and the panel ran
    /// off the bottom of a short window — which is what this looked like in the app.
    fn visible_rows(&self, results: &[Match]) -> usize {
        let (_, max_rows) = panel_metrics(self.panel_size);
        let vp = self.viewport.get();
        if !vp.h.is_finite() {
            return results.len().clamp(1, max_rows);
        }
        // Everything the panel spends before the first row: its own padding above and below the
        // query line, the query line itself, and the padding under the list.
        let chrome = 3.0 * PAD + self.line_h() + 2.0 * QUERY_PAD_Y;
        // The panel starts at `vh * TOP_FRAC`, so filling the remaining `1 - TOP_FRAC` put its
        // bottom exactly on the window's edge. `BOTTOM_FRAC` is the room kept clear under it.
        let mut room = (vp.h * (1.0 - TOP_FRAC - BOTTOM_FRAC) - chrome).max(0.0);
        let mut fits = 0usize;
        for m in results.iter().skip(self.scroll).take(max_rows) {
            let h = self.row_h(&self.commands[m.cmd]);
            if h > room && fits > 0 {
                break;
            }
            room -= h;
            fits += 1;
        }
        // Always at least one row: a window too short for even that is better showing a clipped row
        // than an empty panel.
        results.len().clamp(1, fits.max(1))
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
    }

    /// Does any command carry a description? Then **every** row is two lines high.
    ///
    /// One height for the whole list, not per row: `row_rect` is the same rectangle the paint, the
    /// hover-select and the click hit-test all compute, and rows of differing heights would make
    /// that an index-dependent running sum in three places. A described list simply leaves the
    /// second line of an undescribed row empty.
    fn two_line_rows(&self) -> bool {
        self.commands.iter().any(|c| c.description.is_some())
    }

    /// How many lines **this** row needs: its text (label, plus a description if the list has any)
    /// or its stack of bindings, whichever is taller.
    ///
    /// Per row, **not** one height for the list. The uniform version was simpler — `row_rect` is
    /// shared by the paint, the hover-select and the click hit-test, and one height makes it
    /// arithmetic instead of a running sum — but it charges every row for the worst row: one action
    /// bound three times (`paste_clipboard` is) made all 100 rows three lines tall, which is exactly
    /// the vertical sprawl this fixes. The running sum lives in [`row_rects`](Self::row_rects) and
    /// all three callers read it there, so it is still written once.
    fn row_lines(&self, cmd: &Command) -> usize {
        let text = if self.two_line_rows() { 2 } else { 1 };
        text.max(cmd.chords.len()).max(1)
    }

    /// Height of one result row.
    fn row_h(&self, cmd: &Command) -> f64 {
        self.line_h() * self.row_lines(cmd) as f64 + 2.0 * ROW_PAD_Y
    }

    /// The rectangle of every **visible** row, paired with its index in `results`.
    ///
    /// The one place row geometry is computed. Paint draws these, hover-select and the click
    /// hit-test read them — so a row can never be drawn in one place and clicked in another.
    fn row_rects(&self, results: &[Match], panel: Rectangle, list_top: f64) -> Vec<(usize, Rectangle)> {
        let mut out = Vec::new();
        let mut y = list_top;
        let window = results
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(self.visible_rows(results));
        for (ri, m) in window {
            let h = self.row_h(&self.commands[m.cmd]);
            out.push((
                ri,
                Rectangle::new(
                    Point::new(panel.loc.x + PAD, y),
                    Size::new(panel.size.w - 2.0 * PAD, h),
                ),
            ));
            y += h;
        }
        out
    }

    /// Width of one chord's chips, laid end to end.
    fn chord_w(&self, chord: &[KeyCap]) -> f64 {
        let font = self.keycap_font();
        let caps: f64 = chord.iter().map(|c| c.size(font).w).sum();
        caps + CAP_GAP * chord.len().saturating_sub(1) as f64
    }

    /// The width **reserved on the right of every row** for bindings — the widest chord in the whole
    /// list, so the labels of all rows end at the same place and the chips line up in a column
    /// instead of tracking each label's length.
    fn shortcut_col_w(&self) -> f64 {
        self.commands
            .iter()
            .flat_map(|c| c.chords.iter())
            .map(|chord| self.chord_w(chord))
            .fold(0.0, f64::max)
    }

    /// Keycap chips are drawn a touch smaller than the row text, like the context menu's quick-pick.
    fn keycap_font(&self) -> f32 {
        self.base.font * KEYCAP_FONT_MUL
    }

    fn close(&mut self) {
        self.open.set(false);
        self.query.borrow_mut().set_value("");
        self.selected = 0;
        self.scroll = 0;
    }

    /// Reset the filter/selection whenever the query changes.
    fn on_query_changed(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    /// Panel + query + first-row geometry for the current viewport + result count.
    fn layout(&self, results: &[Match]) -> (Rectangle, Rectangle, f64) {
        let vp = self.viewport.get();
        let line = self.line_h();
        let query_h = line + 2.0 * QUERY_PAD_Y;
        let n_results = results.len();
        let visible = self.visible_rows(results);
        // The panel is as tall as the rows it will actually show — summed, since a row with two
        // bindings is taller than one with none.
        let empty_row = line + 2.0 * ROW_PAD_Y;
        let list_h = match n_results {
            0 => empty_row,
            _ => results
                .iter()
                .skip(self.scroll)
                .take(visible)
                .map(|m| self.row_h(&self.commands[m.cmd]))
                .sum(),
        };

        let (want_w, _) = panel_metrics(self.panel_size);
        let panel_w = match vp.w.is_finite() {
            // **The size decides the width; the window only takes it away.** It used to be a
            // fraction of the viewport merely *capped* by the size, which made `normal` and `large`
            // identical on any window narrower than ~1550px — the fraction was below both caps, so
            // the setting did nothing on an ordinary screen.
            //
            // The floor yields to the window too: on a screen narrower than `PANEL_MIN_W` the panel
            // is as wide as fits rather than hanging off the edges.
            true => {
                let room = vp.w * PANEL_VIEWPORT_FRAC;
                want_w.min(room).max(PANEL_MIN_W.min(room))
            }
            false => want_w,
        };
        let panel_h = PAD + query_h + PAD + list_h + PAD;
        let (vw, vh) = if vp.w.is_finite() {
            (vp.w, vp.h)
        } else {
            (panel_w, panel_h)
        };
        let px = (vw - panel_w) / 2.0;
        let py = vh * TOP_FRAC;
        let panel = Rectangle::new(Point::new(px, py), Size::new(panel_w, panel_h));
        let query = Rectangle::new(
            Point::new(px + PAD, py + PAD),
            Size::new(panel_w - 2.0 * PAD, query_h),
        );
        let list_top = query.loc.y + query_h + PAD;
        (panel, query, list_top)
    }

    /// Cut `text` to what fits in `width`, ending in an ellipsis when it does not.
    ///
    /// A row's text has a hard right edge — the reserved shortcut column — and nothing clipped it:
    /// a long description simply ran under the keycaps and out of the panel. Measured in monospace
    /// cells, the way every other widget here measures text (`MONO_ADVANCE_RATIO`) and the same
    /// advance this widget already uses to place the match highlights, so the cut lands where the
    /// glyph does.
    fn fit(text: &str, width: f64, adv: f64) -> String {
        let cells = (width / adv).floor().max(0.0) as usize;
        if text.chars().count() <= cells {
            return text.to_string();
        }
        match cells {
            0 => String::new(),
            1 => "…".to_string(),
            _ => text.chars().take(cells - 1).collect::<String>() + "…",
        }
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for CommandPalette {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// An open palette grabs input for the whole viewport (typing, nav, outside
    /// click = dismiss), so every point is occluded while open — a host must not
    /// synthesize a page-level action (e.g. open a context menu) under it.
    fn overlay_occludes(&self, _pos: Point) -> bool {
        self.is_open()
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_open() {
            return;
        }
        self.viewport.set(cx.viewport());
        // NB: the panel's own surface fill + corner radius are read by the shared
        // `paint_panel_chrome`, so they are deliberately not pulled out here.
        let (background, accent, glow_c, foreground, muted, ctrl_radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.control_radius(),
            )
        };
        let font = self.base.font;
        let adv = (font * MONO_ADVANCE_RATIO) as f64;
        let line = self.line_h();
        let two_line = self.two_line_rows();
        // Measured once for the whole list, not per row — that is what makes the chips a column.
        let shortcut_col = self.shortcut_col_w();
        let results = self.results();
        let (panel, query, list_top) = self.layout(&results);
        // Remember the panel so the idle caret blink can damage just this rect.
        self.panel.set(panel);

        cx.with_overlay(|cx| {
            // Scrim + panel.
            let vp = self.viewport.get();
            let scrim = if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                panel
            };
            cx.rect(scrim, background.with_alpha(cx.theme().colors.interaction.scrim), None, 0.0, None);
            // The SHARED overlay panel chrome (drop shadow + theme surface fill +
            // bracket reticle) so the palette reads as the same surface as every
            // other overlay panel, plus its own accent edge and glow. The scrim
            // above is the blocking LAYER's, not part of the panel chrome.
            let panel_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            paint_panel_chrome(
                cx,
                panel,
                PanelChrome {
                    border: panel_border,
                    glow: Some(Glow {
                        color: glow_c,
                        radius: 12.0,
                        intensity: 0.3,
                    }),
                    elevation: PanelElevation::Panel,
                },
            );

            // Query line: a real Input, positioned + focused + painted manually
            // (it draws its own box, caret, selection, and text).
            {
                let mut q = self.query.borrow_mut();
                q.base_mut().bounds = query;
                q.base_mut().font = font;
                q.base_mut().focused.set(true); // so the caret shows + blinks
                q.paint(cx);
            }
            // The Input hides its placeholder while focused; draw ours when empty.
            if self.query_text().is_empty() {
                let q_text = Rectangle::new(
                    Point::new(query.loc.x + ROW_PAD_X, query.loc.y),
                    Size::new(query.size.w - 2.0 * ROW_PAD_X, query.size.h),
                );
                cx.text(
                    q_text,
                    &self.placeholder,
                    muted,
                    font,
                    TextAlign::Start,
                    TextStyle::REGULAR,
                );
            }

            // Result rows (the visible scroll window), from the one geometry helper.
            for (ri, row) in self.row_rects(&results, panel, list_top) {
                let m = &results[ri];
                let cmd = &self.commands[m.cmd];
                let is_sel = ri == self.selected;
                if is_sel {
                    let row_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_row_border));
                    cx.rect(row, accent.with_alpha(cx.theme().colors.interaction.panel_row_fill), row_border, ctrl_radius, None);
                    // Left accent bar.
                    cx.rect(
                        Rectangle::new(
                            Point::new(row.loc.x, row.loc.y + row.size.h * 0.2),
                            Size::new(2.5, row.size.h * 0.6),
                        ),
                        accent,
                        None,
                        1.0,
                        None,
                    );
                }
                // The line the label, its match highlights, the icon and the shortcut all share.
                // In a one-line list that is the whole row (unchanged); in a described list it is
                // the top line, with the description under it — so every element of the row keeps
                // one vertical rule instead of each re-deriving it.
                let first_line = match two_line {
                    true => Rectangle::new(
                        Point::new(row.loc.x, row.loc.y + ROW_PAD_Y),
                        Size::new(row.size.w, line),
                    ),
                    false => row,
                };
                // Icon. The column is reserved **whether or not this row has one**, so a list where
                // some commands carry a glyph and some do not still reads as one column of text —
                // indenting only the iconed rows left the others starting at the panel edge.
                let icon_x = row.loc.x + ROW_PAD_X;
                let isz = font * ICON_FONT_MUL;
                let text_x = icon_x + isz as f64 + ICON_GAP;
                if let Some(ch) = cmd.icon.and_then(|g| g.primary_char()) {
                    let irect = Rectangle::new(
                        Point::new(icon_x, first_line.loc.y),
                        Size::new(isz as f64, first_line.size.h),
                    );
                    cx.icon(
                        irect,
                        &ch.to_string(),
                        if is_sel { accent } else { muted },
                        isz,
                    );
                }
                // Label, then over-draw matched chars in accent. Its box stops short of the reserved
                // shortcut column, so text and chips can never share a pixel.
                let text_w = (row.loc.x + row.size.w - ROW_PAD_X - shortcut_col - SHORTCUT_GAP
                    - text_x)
                    .max(0.0);
                let lbl_rect = Rectangle::new(
                    Point::new(text_x, first_line.loc.y),
                    Size::new(text_w, first_line.size.h),
                );
                // Cut to the box rather than trusting it: `cx.text` draws the run it is given, so a
                // long label ran straight under the keycaps and out of the panel.
                let label = Self::fit(&cmd.label, text_w, adv);
                cx.text(
                    lbl_rect,
                    &label,
                    if is_sel {
                        foreground
                    } else {
                        muted.lerp(foreground, 0.7)
                    },
                    font,
                    TextAlign::Start,
                    TextStyle::REGULAR,
                );
                // Highlights follow the **drawn** text: an index past the cut has no glyph to
                // over-draw, and painting it anyway would stamp a letter onto the ellipsis.
                for &hi in &m.hits {
                    if let Some(ch) = label.chars().nth(hi) {
                        let hx = text_x + hi as f64 * adv;
                        let hrect = Rectangle::new(
                            Point::new(hx, first_line.loc.y),
                            Size::new(adv + 2.0, first_line.size.h),
                        );
                        cx.text(hrect, &ch.to_string(), accent, font, TextAlign::Start, TextStyle::BOLD);
                    }
                }
                // Bindings: one row of keycap chips per binding, stacked down the row, each
                // right-aligned to the same reserved column so every command's chips line up.
                let cap_font = self.keycap_font();
                let right_edge = row.loc.x + row.size.w - ROW_PAD_X;
                for (ci, chord) in cmd.chords.iter().enumerate() {
                    let line_top = first_line.loc.y + ci as f64 * line;
                    let mut x = right_edge - self.chord_w(chord);
                    for cap in chord {
                        let cs = cap.size(cap_font);
                        let chip = Rectangle::new(
                            Point::new(x, line_top + (line - cs.h) / 2.0),
                            cs,
                        );
                        // The bordered chip — the same primitive the context-menu quick-pick draws,
                        // never hand-rolled here.
                        cap.paint(
                            cx,
                            chip,
                            cap_font,
                            Some(if is_sel { accent } else { muted }),
                            KeycapVariant::Bordered,
                        );
                        x += cs.w + CAP_GAP;
                    }
                }
                // Description (optional), muted, on the second line — indented to the label so the
                // two read as one block.
                if let Some(desc) = &cmd.description {
                    let drect = Rectangle::new(
                        Point::new(text_x, first_line.loc.y + line),
                        Size::new(text_w, line),
                    );
                    let desc = Self::fit(desc, text_w, adv);
                    cx.text(drect, &desc, muted, font, TextAlign::Start, TextStyle::REGULAR);
                }
            }
        });
    }

    /// Owns its walk. While open it grabs the viewport — typing, nav and outside-click dismissal —
    /// and its command rows are drawn from data, not mounted as children.
    fn routes_own_subtree(&self) -> bool {
        true
    }

    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // Track modifiers even while closed; keep the query field's copy in sync
        // (it needs them for word/line delete). Observe, don't consume.
        if let Event::ModifiersChanged(m) = ev {
            self.modifiers = *m;
            crate::component::dispatch(&mut *self.query.borrow_mut(), ev);
            return Handled::No;
        }
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            // Nav is host-resolved from the configurable `[keys.widgets]` bindings and
            // arrives as a semantic `WidgetIntent` — the palette carries NO hardcoded nav
            // keys. Handled before the query field so a nav key never types. A vertical
            // list: `MenuUp`/`MenuDown` (not the horizontal `Item*`).
            Event::Widget(intent) => match intent {
                WidgetIntent::Dismiss => {
                    self.close();
                    Handled::Yes
                }
                WidgetIntent::Activate => {
                    self.run_selected();
                    Handled::Yes
                }
                WidgetIntent::MenuDown => {
                    self.select_next();
                    Handled::Yes
                }
                WidgetIntent::MenuUp => {
                    self.select_prev();
                    Handled::Yes
                }
                // **Everything else goes to the query field** — `Edit*` above all
                // (`edit_select_all`, `edit_delete_back`, `edit_delete_to_line_start`), which
                // `Input` already implements. Forwarded rather than listed, so an intent added to
                // the vocabulary later reaches the field without a change here; the palette owns
                // only the four it answers above. This is the same **field-first** rule
                // [`Dialog`](super::Dialog) applies through its `FocusManager` — the palette's query
                // lives off-tree, so it forwards by hand.
                _ => {
                    let before = self.query_text();
                    let handled = crate::component::dispatch(&mut *self.query.borrow_mut(), ev);
                    if self.query_text() != before {
                        self.on_query_changed();
                    }
                    handled
                }
            },
            Event::Key { pressed: true, .. } => {
                // The query field owns editing keys (typing, selection, char/word/line delete,
                // caret moves). Return **what the field did**: a single-line `Input` ignores
                // ArrowUp/Down/Enter (returns `No`), so those fall through to the host, which
                // resolves them to a `WidgetIntent` (MenuUp/MenuDown/Activate). Modal capture is
                // the host's job — do NOT hardcode `Handled::Yes` here.
                let before = self.query_text();
                let handled = crate::component::dispatch(&mut *self.query.borrow_mut(), ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            Event::PointerMoved { pos } => {
                // Hover-select a row.
                let results = self.results();
                let (panel, _q, list_top) = self.layout(&results);
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(*pos) {
                        self.selected = ri;
                        break;
                    }
                }
                Handled::Yes
            }
            Event::PointerPressed { pos } => {
                let results = self.results();
                let (panel, query_rect, list_top) = self.layout(&results);
                // A click on the query line places the caret / selects (the Input
                // needs its current bounds + font to hit-test the char position).
                if query_rect.contains(*pos) {
                    let font = self.base.font;
                    let mut q = self.query.borrow_mut();
                    q.base_mut().bounds = query_rect;
                    q.base_mut().font = font;
                    crate::component::dispatch(&mut *q, ev);
                    return Handled::Yes;
                }
                let mut ran = false;
                for (ri, row) in self.row_rects(&results, panel, list_top) {
                    if row.contains(*pos) {
                        self.selected = ri;
                        self.run_selected();
                        ran = true;
                        break;
                    }
                }
                // A click outside the panel dismisses.
                if !ran && !panel.contains(*pos) {
                    self.close();
                }
                Handled::Yes
            }
            // Swallow all other input while open.
            _ => Handled::Yes,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let open = self.is_open();
        // Advance the query's caret (it marks *its own* base on a blink flip, but it
        // lives off-tree so that mark is unobserved). Promote a flip to a palette
        // repaint of just the panel — no full-frame, no pegging frames every tick.
        let flipped = {
            let mut q = self.query.borrow_mut();
            q.base_mut().focused.set(open);
            q.tick(dt);
            let f = q.base().needs_paint();
            q.base().clear_needs_paint();
            f
        };
        if open && flipped {
            self.base.mark_needs_paint(); // collect_damage reads `damage_bounds` (the panel)
        }
        false
    }

    /// While open, wake the host for the query caret's next blink (instead of
    /// redrawing every frame). Closed: nothing pending.
    fn next_redraw(&self) -> Option<f32> {
        if self.is_open() {
            self.query.borrow().next_redraw()
        } else {
            None
        }
    }

    /// The palette paints its panel on the overlay layer, not at its layout `bounds`,
    /// so a caret-blink repaint must target the cached panel rect.
    fn damage_bounds(&self) -> Rectangle {
        if self.is_open() {
            self.panel.get()
        } else {
            self.base.bounds
        }
    }
}

impl LayoutExt for CommandPalette {}

#[cfg(test)]
mod tests {
    use super::fuzzy;

    #[test]
    fn fuzzy_matches_subsequence_and_scores_consecutive_higher() {
        assert!(
            fuzzy("xyz", "abc", false).is_none(),
            "non-subsequence misses"
        );
        assert!(
            fuzzy("ace", "abcde", false).is_some(),
            "scattered subsequence matches"
        );
        assert_eq!(
            fuzzy("ce", "abcde", false).unwrap().1,
            vec![2, 4],
            "reports matched indices"
        );

        let consecutive = fuzzy("ab", "abxx", false).unwrap().0;
        let scattered = fuzzy("ab", "axbx", false).unwrap().0;
        assert!(
            consecutive > scattered,
            "a consecutive run outranks a scattered match"
        );

        // Empty query trivially matches (whole list shows).
        assert!(fuzzy("", "anything", false).is_some());
    }

    #[test]
    fn fuzzy_smart_case() {
        // Case-insensitive when allowed.
        assert!(fuzzy("git", "Git Push", false).is_some());
        // Case-sensitive: an uppercase query char won't match a lowercase target.
        assert!(fuzzy("G", "Git Push", true).is_some());
        assert!(fuzzy("G", "git pull", true).is_none());
    }
}
