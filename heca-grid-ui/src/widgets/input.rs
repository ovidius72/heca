//! [`Input`] — a single-line text field: the first widget with an editable text
//! buffer and a blinking caret. It is a **change widget** like
//! [`Toggle`](super::Toggle)/[`Checkbox`](super::Checkbox): each edit emits
//! `Action::value("input-change", SignalData::String(new))` to an
//! [`on_change`](Input::on_change) handler.
//!
//! Editing keys (delivered to the focused field): printable
//! [`Char`](crate::component::GridKey::Char)/Space insert at the caret;
//! Backspace/Delete remove; Left/Right move the caret; Home/End jump to the line
//! ends (Ctrl/Cmd/Alt on these keys widen the granularity to line/word). A pointer
//! press places the caret by x. Monospace advance (`font_size * MONO_ADVANCE_RATIO`)
//! drives both caret placement and click hit-testing.
//!
//! The readline/select-all **shortcuts** are host-configured, not baked in
//! (`widget-keys-config`): Ctrl+h (delete back), Ctrl+u (delete to line start), and
//! Ctrl/Cmd+A (select all) arrive as the semantic [`Event::Widget`] intents
//! `EditDeleteBack` / `EditDeleteToLineStart` / `EditSelectAll`, which the host resolves
//! from the configurable `[keys.widgets]` `edit_delete_back` / `edit_delete_to_line_start` /
//! `edit_select_all` bindings.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, GridKey, Handled, Modifiers, PaintCx, WidgetIntent};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{Border, TextAlign, TextStyle};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};
use std::time::Instant;

/// Default field width (logical px); override via [`LayoutExt::width`].
const DEFAULT_WIDTH: f32 = 240.0;
/// Horizontal + vertical inner padding.
const PAD: f64 = 10.0;
/// Caret width (logical px).
const CARET_W: f64 = 1.5;
/// Caret blink period (seconds): visible for the first half, hidden the second.
const BLINK_PERIOD: f32 = 1.0;
/// Rest-glow spread radius (px) — the field's share of the theme rest halo
/// (`interaction.control_rest_glow` carries the intensity).
const GLOW_RADIUS: f32 = 12.0;
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
    /// When the current caret-blink cycle started. The caret is driven by real
    /// elapsed time (not the frame `dt`) so it blinks at a steady rate regardless of
    /// how sparsely the host redraws — see [`next_redraw`](Input::next_redraw).
    blink_origin: Instant,
    /// Caret visibility at the last paint, so `tick` damages the field only when the
    /// caret actually toggles (not every frame).
    last_caret: std::cell::Cell<bool>,
    /// Latest modifier state (tracked via [`Event::ModifiersChanged`]).
    mods: Modifiers,
    on_change: Option<Box<dyn Fn(Action)>>,
}

#[heca_grid_ui_macros::props]
impl Input {
    /// A new empty input.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.style.layout.width = Length::Px(DEFAULT_WIDTH);
        base.style.layout.height = Length::Px(base.font * MONO_LINE_RATIO + 2.0 * PAD as f32);
        Self {
            base,
            text: signal(String::new()),
            placeholder: String::new(),
            cursor: 0,
            anchor: None,
            blink_origin: Instant::now(),
            last_caret: std::cell::Cell::new(false),
            mods: Modifiers::default(),
            on_change: None,
        }
    }

    /// Explicit font size — overrides the inherited theme font.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.visual.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Set the initial text (caret lands at the end).
    #[heca_grid_ui_macros::prop]
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

    /// The placeholder text as set (empty when unset).
    pub fn placeholder_str(&self) -> &str {
        &self.placeholder
    }

    /// Set the placeholder shown while empty and unfocused.
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the change handler. Receives `Action::value("input-change",
    /// SignalData::String(new_text))` after every edit.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
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


    fn caret_visible(&self) -> bool {
        let phase = self
            .blink_origin
            .elapsed()
            .as_secs_f32()
            .rem_euclid(BLINK_PERIOD);
        phase < BLINK_PERIOD / 2.0
    }

    fn chars_vec(&self) -> Vec<char> {
        self.text.get_untracked().chars().collect()
    }

    /// Inner padding scaled by the size variant (matches `remeasure` + paint).
    fn pad(&self) -> f64 {
        PAD * self.base.size_scale() as f64
    }

    /// Char index nearest pointer x (rounded, for caret placement).
    fn caret_index_at_x(&self, x: f64) -> usize {
        let advance = (self.base.font * MONO_ADVANCE_RATIO) as f64;
        let rel = (x - (self.base.bounds.loc.x + self.pad())).max(0.0);
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
        let rel = (x - (self.base.bounds.loc.x + self.pad())).max(0.0);
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

    /// Commit new text: store it, reset the blink, and emit `input-change`.
    fn commit(&mut self, text: String) {
        self.text.set(text.clone());
        self.blink_origin = Instant::now();
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

    /// Handle a plain editing key. Returns whether it was consumed. The emacs/readline
    /// shortcuts (Ctrl+h delete, Ctrl+u clear, Ctrl/Cmd+A select-all) are **not** here —
    /// they are host-configured (`[keys.widgets]` `edit_*` bindings) and arrive as the
    /// [`Event::Widget`](crate::component::Event::Widget) `Edit*` intents. A modified char is
    /// ignored so it is never typed as text (and so the host's shortcut resolution can act on it).
    fn handle_key(&mut self, key: GridKey) -> Handled {
        match key {
            // **Text is not a key.** Typing arrives as [`Event::TextInput`] — the character the
            // user meant, including anything an IME composed and anything pasted — so a raw
            // `Char` is only ever a shortcut here, and falls through unhandled to whoever resolves
            // those. This is what the host's "deliver the real character, not the lowercased combo
            // key" fixup existed to paper over.
            GridKey::Char(_) | GridKey::Space => return Handled::No,
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
        self.blink_origin = Instant::now();
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
        self.blink_origin = Instant::now();

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

    /// Field height tracks the resolved font + size-scaled padding.
    fn remeasure(&mut self) {
        let pad = self.pad() as f32;
        self.base.style.layout.height = Length::Px(self.base.font * MONO_LINE_RATIO + 2.0 * pad);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let focused = self.base.focused.get_untracked();
        let (surface, accent, muted, foreground, radius, bw, ia) = {
            let t = cx.theme();
            (
                t.colors.surface,
                t.colors.accent,
                t.colors.muted,
                t.colors.foreground,
                t.colors.control_radius(),
                t.colors.border_width,
                t.colors.interaction,
            )
        };
        let b = self.base.bounds;
        let fs = self.base.font;

        // Field: dark fill; border firms muted → accent on focus. Radius + border
        // width come from the theme so global settings scale this proportionally.
        // A faint theme rest glow (`interaction.control_rest_glow`) gives the field
        // the shared neon identity at rest; `glow_size` scales it (T011).
        let p = if focused { 1.0 } else { 0.0 };
        let rest_border = ia.control_rest_border as f32;
        let border_a = rest_border + (255.0 - rest_border) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: bw,
        };
        let glow = if disabled { None } else { cx.rest_glow(GLOW_RADIUS) };
        cx.rect(b, surface, Some(border), radius, glow);

        // Text (left-aligned within the padded inner rect); placeholder when
        // empty and unfocused.
        let pad = self.pad();
        let text_left = b.loc.x + pad;
        let text_rect = Rectangle::new(
            Point::new(text_left, b.loc.y),
            Size::new((b.size.w - 2.0 * pad).max(0.0), b.size.h),
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
            cx.rect(sel, accent.with_alpha(ia.selection), None, 1.0, None);
        }

        let s = self.text.get_untracked();
        if s.is_empty() && !focused && !self.placeholder.is_empty() {
            cx.text(
                text_rect,
                &self.placeholder,
                muted,
                fs,
                TextAlign::Start,
                TextStyle::REGULAR,
            );
        } else if !s.is_empty() {
            cx.text(text_rect, &s, foreground, fs, TextAlign::Start, TextStyle::REGULAR);
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

        // Focus ring — shown whenever focused (theme-aware color, outside the box).
        if !disabled && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.effective_focus_ring();
            cx.focus_ring(b, ring, radius);
        }
    }

    /// Capture, not bubble: this control owns the input that lands on it. Its content is composed
    /// children, and they must never take the press first — the control is one click target.
    ///
    /// **A field types because it is focused, and for no other reason.** It used to declare that it
    /// took raw keys and typed text whether or not it held focus; both flags are gone, along with
    /// the question they answered. Keys and typed text are delivered to the focus owner, so a
    /// mounted-but-unfocused field is silent without saying so, and a focused one hears everything
    /// without asking.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // Track modifiers even when disabled is irrelevant; observe, don't consume.
        if let Event::ModifiersChanged(m) = ev {
            self.mods = *m;
            return Handled::No;
        }
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            // The **click run comes with the event**. This widget kept its own clock and its own
            // counter to work out that a press was the second one; both are gone, and with them
            // the chance of a field that counts clicks differently from everything else.
            //
            // The caret lands on the press (so a drag-select would start from the right place),
            // and the selection cycle reads `click_count`: 1 = caret, 2 = word, 3 = all,
            // 4 = deselect and start again.
            Event::PointerDown(p) => {
                self.blink_origin = Instant::now();
                self.anchor = None;
                self.cursor = self.caret_index_at_x(p.pos.x);
                Handled::Yes
            }
            Event::DoubleClick(p) => {
                let chars = self.chars_vec();
                let idx = self.char_index_at_x(p.pos.x, chars.len());
                let (s, e) = word_bounds(&chars, idx);
                self.anchor = (e > s).then_some(s);
                self.cursor = e;
                Handled::Yes
            }
            Event::TripleClick(p) if p.click_count == 3 => {
                let n = self.char_count();
                self.anchor = (n > 0).then_some(0);
                self.cursor = n;
                Handled::Yes
            }
            Event::TripleClick(p) => {
                // The fourth click and beyond: deselect, caret where the pointer is.
                self.anchor = None;
                self.cursor = self.caret_index_at_x(p.pos.x);
                Handled::Yes
            }
            // Typed text, IME commits and pastes: one event, whatever produced them. The field no
            // longer reconstructs a character from a key combo, and the host no longer patches the
            // key it sends so that reconstruction comes out right.
            Event::TextInput(text) => {
                for c in text.chars() {
                    self.insert(c);
                }
                Handled::Yes
            }
            Event::Key { key, pressed: true } => self.handle_key(*key),
            // Host-resolved editing shortcuts (`[keys.widgets]` `edit_*` → `WidgetIntent`). The
            // plain keys stay in `handle_key`; only these shortcuts are configurable. Nav intents
            // (`Item*`/`Menu*`) are ignored so they fall through to any surrounding widget.
            Event::Widget(WidgetIntent::EditDeleteBack) => {
                self.backspace(Granularity::Char);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::EditDeleteToLineStart) => {
                self.backspace(Granularity::Line);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::EditSelectAll) => {
                self.select_all();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    // The caret is driven by real time (`blink_origin`), not the frame `dt`. `tick`
    // does no continuous animation; it only damages the field when the caret actually
    // toggles, so a blink repaints just the input's rect (not the whole scene). The
    // wake at the next toggle is scheduled via `next_redraw`.
    fn tick(&mut self, _dt: f32) -> bool {
        if self.base.focused.get_untracked() {
            let vis = self.caret_visible();
            if vis != self.last_caret.get() {
                self.last_caret.set(vis);
                self.base.mark_needs_paint();
            }
        }
        false
    }

    fn next_redraw(&self) -> Option<f32> {
        if !self.base.focused.get_untracked() {
            return None;
        }
        // Time until the caret flips: the next half-`BLINK_PERIOD` boundary.
        let phase = self
            .blink_origin
            .elapsed()
            .as_secs_f32()
            .rem_euclid(BLINK_PERIOD);
        let half = BLINK_PERIOD / 2.0;
        Some(if phase < half {
            half - phase
        } else {
            BLINK_PERIOD - phase
        })
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExt for Input {}
