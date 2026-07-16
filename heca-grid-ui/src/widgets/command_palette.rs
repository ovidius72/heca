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
use crate::component::{Base, Component, Event, Handled, Modifiers, PaintCx, WidgetIntent};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Glow, TextAlign, TextStyle};
use crate::widgets::{Glyph, Input};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

/// One command in a [`CommandPalette`].
pub struct Command {
    label: String,
    icon: Option<Glyph>,
    key: Option<String>,
    on_run: Box<dyn Fn()>,
}

impl Command {
    /// A command with `label` that runs `on_run` when selected.
    pub fn new(label: impl Into<String>, on_run: impl Fn() + 'static) -> Self {
        Self {
            label: label.into(),
            icon: None,
            key: None,
            on_run: Box::new(on_run),
        }
    }

    /// An optional leading icon.
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self
    }

    /// An optional right-aligned keybinding hint (e.g. `"⌘K"`).
    pub fn key(mut self, hint: impl Into<String>) -> Self {
        self.key = Some(hint.into());
        self
    }
}

/// One filtered result: command index + match score + matched char indices.
struct Match {
    cmd: usize,
    score: i32,
    hits: Vec<usize>,
}

/// Panel width as a fraction of the viewport, with absolute bounds.
const PANEL_W_FRAC: f64 = 0.55;
const PANEL_MIN_W: f64 = 360.0;
const PANEL_MAX_W: f64 = 600.0;
/// Panel top offset as a fraction of the viewport height (palettes sit high).
const TOP_FRAC: f64 = 0.12;
/// Panel inner padding.
const PAD: f64 = 12.0;
/// Vertical padding inside the query line and each result row.
const QUERY_PAD_Y: f64 = 9.0;
const ROW_PAD_Y: f64 = 6.0;
/// Horizontal inset of row content.
const ROW_PAD_X: f64 = 10.0;
/// Gap between an icon and the label.
const ICON_GAP: f64 = 10.0;
/// Maximum result rows shown at once (the list scrolls beyond this).
const MAX_VISIBLE: usize = 8;

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
            open: signal(false),
            modifiers: Modifiers::default(),
            viewport: Cell::new(Size::new(f64::MAX, f64::MAX)),
            panel: Cell::new(Rectangle::from_size(Size::new(0.0, 0.0))),
        }
    }

    /// Add a command.
    pub fn command(mut self, c: Command) -> Self {
        self.commands.push(c);
        self
    }

    /// Set the empty-query placeholder text.
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Set the initial open state.
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
        self.follow_selection(n);
    }

    /// Move the selection up (Ctrl+K / ↑).
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.follow_selection(self.results().len());
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
    fn follow_selection(&mut self, n: usize) {
        let visible = self.visible_rows(n);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + visible {
            self.scroll = self.selected + 1 - visible;
        }
    }

    fn visible_rows(&self, n: usize) -> usize {
        n.clamp(1, MAX_VISIBLE)
    }

    fn line_h(&self) -> f64 {
        (self.base.font * MONO_LINE_RATIO) as f64
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
    fn layout(&self, n_results: usize) -> (Rectangle, Rectangle, f64, f64, usize) {
        let vp = self.viewport.get();
        let line = self.line_h();
        let query_h = line + 2.0 * QUERY_PAD_Y;
        let row_h = line + 2.0 * ROW_PAD_Y;
        let visible = self.visible_rows(n_results.max(1));
        let list_h = if n_results == 0 {
            row_h
        } else {
            visible as f64 * row_h
        };

        let cap = if vp.w.is_finite() {
            (vp.w * PANEL_W_FRAC).clamp(PANEL_MIN_W, PANEL_MAX_W)
        } else {
            PANEL_MAX_W
        };
        let panel_w = cap;
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
        (panel, query, list_top, row_h, visible)
    }

    fn row_rect(
        &self,
        panel: Rectangle,
        list_top: f64,
        row_h: f64,
        visible_idx: usize,
    ) -> Rectangle {
        Rectangle::new(
            Point::new(panel.loc.x + PAD, list_top + visible_idx as f64 * row_h),
            Size::new(panel.size.w - 2.0 * PAD, row_h),
        )
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
        let (background, surface, accent, glow_c, foreground, muted, ctrl_radius, radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.surface,
                t.colors.accent,
                t.colors.glow,
                t.colors.foreground,
                t.colors.muted,
                t.colors.control_radius(),
                t.colors.border_radius,
            )
        };
        let font = self.base.font;
        let adv = (font * MONO_ADVANCE_RATIO) as f64;
        let results = self.results();
        let (panel, query, list_top, row_h, visible) = self.layout(results.len());
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
            let panel_border = cx.border(accent.with_alpha(cx.theme().colors.interaction.panel_border));
            cx.rect(
                panel,
                surface,
                panel_border,
                radius,
                Some(Glow {
                    color: glow_c,
                    radius: 12.0,
                    intensity: 0.3,
                }),
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

            // Result rows (the visible scroll window).
            for vi in 0..visible {
                let ri = self.scroll + vi;
                let Some(m) = results.get(ri) else { break };
                let cmd = &self.commands[m.cmd];
                let row = self.row_rect(panel, list_top, row_h, vi);
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
                // Icon (optional).
                let mut text_x = row.loc.x + ROW_PAD_X;
                if let Some(g) = cmd.icon {
                    if let Some(ch) = g.primary_char() {
                        let isz = font * 1.05;
                        let irect = Rectangle::new(
                            Point::new(text_x, row.loc.y),
                            Size::new(isz as f64, row.size.h),
                        );
                        cx.icon(
                            irect,
                            &ch.to_string(),
                            if is_sel { accent } else { muted },
                            isz,
                        );
                    }
                    text_x += font as f64 * 1.05 + ICON_GAP;
                }
                // Label, then over-draw matched chars in accent.
                let lbl_rect = Rectangle::new(
                    Point::new(text_x, row.loc.y),
                    Size::new(row.size.w, row.size.h),
                );
                cx.text(
                    lbl_rect,
                    &cmd.label,
                    if is_sel {
                        foreground
                    } else {
                        muted.lerp(foreground, 0.7)
                    },
                    font,
                    TextAlign::Start,
                    TextStyle::REGULAR,
                );
                for &hi in &m.hits {
                    if let Some(ch) = cmd.label.chars().nth(hi) {
                        let hx = text_x + hi as f64 * adv;
                        let hrect = Rectangle::new(
                            Point::new(hx, row.loc.y),
                            Size::new(adv + 2.0, row.size.h),
                        );
                        cx.text(hrect, &ch.to_string(), accent, font, TextAlign::Start, TextStyle::BOLD);
                    }
                }
                // Keybinding hint (optional), right-aligned.
                if let Some(k) = &cmd.key {
                    let krect = Rectangle::new(
                        Point::new(row.loc.x, row.loc.y),
                        Size::new(row.size.w - ROW_PAD_X, row.size.h),
                    );
                    cx.text(krect, k, muted, font, TextAlign::End, TextStyle::REGULAR);
                }
            }
        });
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Track modifiers even while closed; keep the query field's copy in sync
        // (it needs them for word/line delete). Observe, don't consume.
        if let Event::ModifiersChanged(m) = ev {
            self.modifiers = *m;
            self.query.borrow_mut().event(ev);
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
                _ => Handled::No,
            },
            Event::Key { pressed: true, .. } => {
                // The query field owns editing keys (typing, selection, char/word/line delete,
                // caret moves). Return **what the field did**: a single-line `Input` ignores
                // ArrowUp/Down/Enter (returns `No`), so those fall through to the host, which
                // resolves them to a `WidgetIntent` (MenuUp/MenuDown/Activate). Modal capture is
                // the host's job — do NOT hardcode `Handled::Yes` here.
                let before = self.query_text();
                let handled = self.query.borrow_mut().event(ev);
                if self.query_text() != before {
                    self.on_query_changed();
                }
                handled
            }
            Event::PointerMoved { pos } => {
                // Hover-select a row.
                let results = self.results();
                let (panel, _q, list_top, row_h, visible) = self.layout(results.len());
                for vi in 0..visible {
                    let ri = self.scroll + vi;
                    if ri >= results.len() {
                        break;
                    }
                    if self.row_rect(panel, list_top, row_h, vi).contains(*pos) {
                        self.selected = ri;
                        break;
                    }
                }
                Handled::Yes
            }
            Event::PointerPressed { pos } => {
                let results = self.results();
                let (panel, query_rect, list_top, row_h, visible) = self.layout(results.len());
                // A click on the query line places the caret / selects (the Input
                // needs its current bounds + font to hit-test the char position).
                if query_rect.contains(*pos) {
                    let font = self.base.font;
                    let mut q = self.query.borrow_mut();
                    q.base_mut().bounds = query_rect;
                    q.base_mut().font = font;
                    q.event(ev);
                    return Handled::Yes;
                }
                let mut ran = false;
                for vi in 0..visible {
                    let ri = self.scroll + vi;
                    if ri >= results.len() {
                        break;
                    }
                    if self.row_rect(panel, list_top, row_h, vi).contains(*pos) {
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
