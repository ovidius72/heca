//! [`Input`] — a single-line text field: the first widget with an editable text
//! buffer and a blinking caret. It is a **change widget** like
//! [`Toggle`](super::Toggle)/[`Checkbox`](super::Checkbox): each edit emits
//! `Action::value("input-change", SignalData::String(new))` to an
//! [`on_change`](Input::on_change) handler.
//!
//! Editing keys (delivered to the focused field): printable
//! [`Char`](crate::component::GridKey::Char)/Space insert at the caret;
//! Backspace/Delete remove; Left/Right move the caret. A pointer press places
//! the caret by x. Monospace advance (`font_size * MONO_ADVANCE_RATIO`) drives
//! both caret placement and click hit-testing.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, Modifiers, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Default field width (logical px); override via [`LayoutExt::width`].
const DEFAULT_WIDTH: f32 = 240.0;
/// Horizontal + vertical inner padding.
const PAD: f64 = 10.0;
/// Caret width (logical px).
const CARET_W: f64 = 1.5;
/// Border alpha at rest; firms to solid accent on focus.
const REST_BORDER_ALPHA: f32 = 150.0;
/// Caret blink period (seconds): visible for the first half, hidden the second.
const BLINK_PERIOD: f32 = 1.0;
/// Max gap (seconds) between clicks counted as part of one multi-click cycle.
const MULTI_CLICK: f32 = 0.4;
/// Selection highlight alpha.
const SELECTION_ALPHA: u8 = 70;

/// A single-line text input. Emits `input-change` with the full new text on each
/// edit.
pub struct Input {
    base: Base,
    /// The current text, exposed reactively via [`text`](Input::text).
    text: Signal<String>,
    /// Shown (muted) when empty and unfocused.
    placeholder: String,
    /// Caret position as a char index in `0..=text.chars().count()` (the moving
    /// head of a selection).
    cursor: usize,
    /// The fixed end of an active selection. The selection spans `anchor..cursor`
    /// (ordered); `None` (or `anchor == cursor`) means no selection.
    anchor: Option<usize>,
    /// Blink accumulator (seconds, wrapped to [`BLINK_PERIOD`]).
    blink: f32,
    /// Monotonic clock (seconds) advanced while focused, for click timing.
    clock: f32,
    /// Clock value at the last pointer press.
    last_click: f32,
    /// Consecutive-click counter driving the select cycle (word → all → clear).
    clicks: u8,
    /// Latest modifier state (tracked via [`Event::ModifiersChanged`]).
    mods: Modifiers,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Input {
    /// A new empty input.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(base.font * MONO_LINE_RATIO + 2.0 * PAD as f32);
        Self {
            base,
            text: signal(String::new()),
            placeholder: String::new(),
            cursor: 0,
            anchor: None,
            blink: 0.0,
            clock: 0.0,
            last_click: f32::NEG_INFINITY,
            clicks: 0,
            mods: Modifiers::default(),
            on_change: None,
        }
    }

    /// Explicit font size — overrides the inherited theme font.
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Set the initial text (caret lands at the end).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.set_value(value);
        self
    }

    /// Replace the text at runtime (caret to end, selection cleared) — e.g. to
    /// reset a reused field. Does **not** fire `on_change` (it's a host action,
    /// not a user edit).
    pub fn set_value(&mut self, value: impl Into<String>) {
        let s = value.into();
        self.cursor = s.chars().count();
        self.anchor = None;
        self.text.set(s);
    }

    /// Set the placeholder shown while empty and unfocused.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the change handler. Receives `Action::value("input-change",
    /// SignalData::String(new_text))` after every edit.
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// The text signal — bind UI to it reactively.
    pub fn text(&self) -> Signal<String> {
        self.text
    }

    /// The current text (untracked read).
    pub fn value_str(&self) -> String {
        self.text.get_untracked()
    }

    /// The selected char range `(start, end)` with `start < end`, if any.
    pub fn selection(&self) -> Option<(usize, usize)> {
        match self.anchor {
            Some(a) if a != self.cursor => Some((a.min(self.cursor), a.max(self.cursor))),
            _ => None,
        }
    }

    /// The selected text, if any.
    pub fn selected_text(&self) -> Option<String> {
        self.selection().map(|(s, e)| {
            self.text
                .get_untracked()
                .chars()
                .skip(s)
                .take(e - s)
                .collect()
        })
    }

    fn char_count(&self) -> usize {
        self.text.get_untracked().chars().count()
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }

    fn caret_visible(&self) -> bool {
        self.blink.rem_euclid(BLINK_PERIOD) < BLINK_PERIOD / 2.0
    }

    fn chars_vec(&self) -> Vec<char> {
        self.text.get_untracked().chars().collect()
    }

    /// Char index nearest pointer x (rounded, for caret placement).
    fn caret_index_at_x(&self, x: f64) -> usize {
        let advance = (self.base.font * MONO_ADVANCE_RATIO) as f64;
        let rel = (x - (self.base.bounds.loc.x + PAD)).max(0.0);
        let idx = if advance > 0.0 {
            (rel / advance).round() as usize
        } else {
            0
        };
        idx.min(self.char_count())
    }

    /// Char index under pointer x (floored, for word hit-testing).
    fn char_index_at_x(&self, x: f64, len: usize) -> usize {
        let advance = (self.base.font * MONO_ADVANCE_RATIO) as f64;
        let rel = (x - (self.base.bounds.loc.x + PAD)).max(0.0);
        let idx = if advance > 0.0 {
            (rel / advance).floor() as usize
        } else {
            0
        };
        idx.min(len.saturating_sub(1))
    }

    /// Remove the selected range (if any) from `chars`, moving the caret to its
    /// start. Returns whether anything was removed. Does not commit.
    fn drain_selection(&mut self, chars: &mut Vec<char>) -> bool {
        if let Some((s, e)) = self.selection() {
            let e = e.min(chars.len());
            let s = s.min(e);
            chars.drain(s..e);
            self.cursor = s;
            self.anchor = None;
            true
        } else {
            false
        }
    }

    /// Commit new text: store it, reset the blink, restart the click cycle, and
    /// emit `input-change`.
    fn commit(&mut self, text: String) {
        self.text.set(text.clone());
        self.blink = 0.0;
        self.clicks = 0;
        if let Some(f) = &self.on_change {
            f(Action::value("input-change", SignalData::String(text)));
        }
    }

    fn insert(&mut self, c: char) {
        let mut chars = self.chars_vec();
        self.drain_selection(&mut chars);
        let i = self.cursor.min(chars.len());
        chars.insert(i, c);
        self.cursor = i + 1;
        self.commit(chars.into_iter().collect());
    }

    /// Granularity for delete keys: Meta (Cmd) → to start/end, Ctrl/Alt → word,
    /// otherwise a single character.
    fn delete_granularity(&self) -> Granularity {
        if self.mods.meta {
            Granularity::Line
        } else if self.mods.ctrl || self.mods.alt {
            Granularity::Word
        } else {
            Granularity::Char
        }
    }

    /// Delete left of the caret at `gran` (char / word / to start).
    fn backspace(&mut self, gran: Granularity) {
        let mut chars = self.chars_vec();
        if self.drain_selection(&mut chars) {
            self.commit(chars.into_iter().collect());
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let start = match gran {
            Granularity::Char => self.cursor - 1,
            Granularity::Word => prev_word_boundary(&chars, self.cursor),
            Granularity::Line => 0,
        };
        chars.drain(start..self.cursor.min(chars.len()));
        self.cursor = start;
        self.commit(chars.into_iter().collect());
    }

    /// Delete right of the caret at `gran` (char / word / to end).
    fn delete(&mut self, gran: Granularity) {
        let mut chars = self.chars_vec();
        if self.drain_selection(&mut chars) {
            self.commit(chars.into_iter().collect());
            return;
        }
        if self.cursor >= chars.len() {
            return;
        }
        let end = match gran {
            Granularity::Char => self.cursor + 1,
            Granularity::Word => next_word_boundary(&chars, self.cursor),
            Granularity::Line => chars.len(),
        };
        chars.drain(self.cursor..end.min(chars.len()));
        self.commit(chars.into_iter().collect());
    }

    /// Handle an editing key. Returns whether it was consumed.
    fn handle_key(&mut self, key: GridKey) -> Handled {
        match key {
            // Cmd/Ctrl+A selects all; other modified chars are ignored so they
            // aren't typed as text.
            GridKey::Char(c)
                if (self.mods.ctrl || self.mods.meta) && c.eq_ignore_ascii_case(&'a') =>
            {
                self.select_all();
            }
            // Emacs/readline backspace bindings: Ctrl+H deletes one char back,
            // Ctrl+U deletes from the caret to the start of the line.
            GridKey::Char(c) if self.mods.ctrl && c.eq_ignore_ascii_case(&'h') => {
                self.backspace(Granularity::Char)
            }
            GridKey::Char(c) if self.mods.ctrl && c.eq_ignore_ascii_case(&'u') => {
                self.backspace(Granularity::Line)
            }
            GridKey::Char(_) if self.mods.ctrl || self.mods.meta => return Handled::No,
            GridKey::Char(c) => self.insert(c),
            GridKey::Space => self.insert(' '),
            GridKey::Backspace => self.backspace(self.delete_granularity()),
            GridKey::Delete => self.delete(self.delete_granularity()),
            GridKey::ArrowLeft => self.move_caret(true, self.granularity()),
            GridKey::ArrowRight => self.move_caret(false, self.granularity()),
            GridKey::Home => self.move_caret(true, Granularity::Line),
            GridKey::End => self.move_caret(false, Granularity::Line),
            _ => return Handled::No,
        }
        Handled::Yes
    }

    /// Select the entire text (caret at the end).
    fn select_all(&mut self) {
        let n = self.char_count();
        self.anchor = (n > 0).then_some(0);
        self.cursor = n;
        self.blink = 0.0;
        self.clicks = 0;
    }

    /// Movement granularity from the current modifiers: Ctrl/Cmd → to start/end,
    /// Alt → by word, otherwise by character.
    fn granularity(&self) -> Granularity {
        if self.mods.ctrl || self.mods.meta {
            Granularity::Line
        } else if self.mods.alt {
            Granularity::Word
        } else {
            Granularity::Char
        }
    }

    /// Move the caret left/right at `gran`. With Shift held the move
    /// **extends/shrinks** the selection (anchoring at the start position);
    /// without Shift it collapses any selection and moves the caret.
    fn move_caret(&mut self, left: bool, gran: Granularity) {
        self.blink = 0.0;
        self.clicks = 0;

        if self.mods.shift {
            // Begin anchoring at the caret if no selection is active yet.
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
        } else if let Some((s, e)) = self.selection() {
            // Plain arrow with a selection: collapse to the near edge, no move.
            self.cursor = if left { s } else { e };
            self.anchor = None;
            return;
        } else {
            self.anchor = None;
        }

        let chars = self.chars_vec();
        let n = chars.len();
        self.cursor = match (left, gran) {
            (true, Granularity::Char) => self.cursor.saturating_sub(1),
            (false, Granularity::Char) => (self.cursor + 1).min(n),
            (true, Granularity::Word) => prev_word_boundary(&chars, self.cursor),
            (false, Granularity::Word) => next_word_boundary(&chars, self.cursor),
            (true, Granularity::Line) => 0,
            (false, Granularity::Line) => n,
        };

        // Shrinking back onto the anchor clears the selection ("deselect").
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
    }
}

/// How far a caret move travels.
#[derive(Clone, Copy)]
enum Granularity {
    Char,
    Word,
    Line,
}

/// The `(start, end)` char range of the word at `idx`: a maximal run of
/// like-classed (whitespace vs non-whitespace) characters.
fn word_bounds(chars: &[char], idx: usize) -> (usize, usize) {
    if chars.is_empty() {
        return (0, 0);
    }
    let i = idx.min(chars.len() - 1);
    let target = !chars[i].is_whitespace();
    let mut start = i;
    while start > 0 && chars[start - 1].is_whitespace() != target {
        start -= 1;
    }
    let mut end = i + 1;
    while end < chars.len() && chars[end].is_whitespace() != target {
        end += 1;
    }
    (start, end)
}

/// The start of the word before `cursor`: skip whitespace, then the word, going
/// left (so Ctrl/Alt+Backspace removes the word plus any space before the caret).
fn prev_word_boundary(chars: &[char], cursor: usize) -> usize {
    let mut i = cursor.min(chars.len());
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !chars[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

/// The end of the word after `cursor`: skip whitespace, then the word, going
/// right (so Ctrl/Alt+Delete removes the word plus following space).
fn next_word_boundary(chars: &[char], cursor: usize) -> usize {
    let mut i = cursor;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}

impl Component for Input {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn focusable(&self) -> bool {
        !self.base.disabled.get_untracked()
    }

    /// Field height tracks the resolved font.
    fn remeasure(&mut self) {
        self.base.style.height = Length::Px(self.base.font * MONO_LINE_RATIO + 2.0 * PAD as f32);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let focused = self.base.focused.get_untracked();
        let (surface, accent, muted, foreground, radius, bw) = {
            let t = cx.theme();
            (t.surface, t.accent, t.muted, t.foreground, t.control_radius(), t.border_width)
        };
        let b = self.base.bounds;
        let fs = self.base.font;

        // Field: dark fill; border firms muted → accent on focus. Radius + border
        // width come from the theme so global settings scale this proportionally.
        let p = if focused { 1.0 } else { 0.0 };
        let border_a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        cx.rect(b, surface, Some(border), radius, None);

        // Text (left-aligned within the padded inner rect); placeholder when
        // empty and unfocused.
        let text_left = b.loc.x + PAD;
        let text_rect = Rectangle::new(
            Point::new(text_left, b.loc.y),
            Size::new((b.size.w - 2.0 * PAD).max(0.0), b.size.h),
        );
        let advance = (fs * MONO_ADVANCE_RATIO) as f64;

        // Selection highlight behind the text.
        if let Some((sel_s, sel_e)) = self.selection() {
            let ch = fs as f64;
            let sel = Rectangle::new(
                Point::new(
                    text_left + sel_s as f64 * advance,
                    b.loc.y + (b.size.h - ch) / 2.0,
                ),
                Size::new((sel_e - sel_s) as f64 * advance, ch),
            );
            cx.rect(sel, accent.with_alpha(SELECTION_ALPHA), None, 1.0, None);
        }

        let s = self.text.get_untracked();
        if s.is_empty() && !focused && !self.placeholder.is_empty() {
            cx.text(
                text_rect,
                &self.placeholder,
                muted,
                fs,
                TextAlign::Start,
                false,
            );
        } else if !s.is_empty() {
            cx.text(text_rect, &s, foreground, fs, TextAlign::Start, false);
        }

        // Caret: a thin accent bar at the cursor (hidden while text is selected).
        if focused && !disabled && self.selection().is_none() && self.caret_visible() {
            let caret_x = text_left + self.cursor as f64 * advance;
            let ch = fs as f64;
            let caret = Rectangle::new(
                Point::new(caret_x, b.loc.y + (b.size.h - ch) / 2.0),
                Size::new(CARET_W, ch),
            );
            cx.rect(caret, accent, None, 0.0, None);
        }

        // Dim when disabled.
        if disabled {
            cx.dim(b, radius);
        }

        // Focus-visible ring (keyboard focus only).
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        // Track modifiers even when disabled is irrelevant; observe, don't consume.
        if let Event::ModifiersChanged(m) = ev {
            self.mods = *m;
            return Handled::No;
        }
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerPressed { pos } if self.contains(*pos) => {
                // Multi-click cycle: 1 = caret, 2 = word, 3 = all, 4 = clear.
                let multi = (self.clock - self.last_click) <= MULTI_CLICK;
                self.last_click = self.clock;
                self.clicks = if multi { self.clicks + 1 } else { 1 };
                self.blink = 0.0;
                match self.clicks {
                    2 => {
                        let chars = self.chars_vec();
                        let idx = self.char_index_at_x(pos.x, chars.len());
                        let (s, e) = word_bounds(&chars, idx);
                        self.anchor = (e > s).then_some(s);
                        self.cursor = e;
                    }
                    3 => {
                        let n = self.char_count();
                        self.anchor = (n > 0).then_some(0);
                        self.cursor = n;
                    }
                    n if n >= 4 => {
                        // Deselect and restart the cycle.
                        self.anchor = None;
                        self.cursor = self.caret_index_at_x(pos.x);
                        self.clicks = 0;
                    }
                    _ => {
                        self.anchor = None;
                        self.cursor = self.caret_index_at_x(pos.x);
                    }
                }
                Handled::Yes
            }
            Event::Key { key, pressed: true } => self.handle_key(*key),
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        if self.base.focused.get_untracked() {
            self.clock += dt;
            self.blink = (self.blink + dt).rem_euclid(BLINK_PERIOD);
            true
        } else {
            false
        }
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Input {}
