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
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::scene::{Border, TextAlign};
use crate::style::Length;
use heca_core::layout::{Point, Rectangle, Size};

/// Default field width (logical px); override via [`LayoutExt::width`].
const DEFAULT_WIDTH: f32 = 240.0;
/// Horizontal + vertical inner padding.
const PAD: f64 = 10.0;
/// Caret width (logical px).
const CARET_W: f64 = 1.5;
/// Field corner radius.
const RADIUS: f32 = 4.0;
/// Border alpha at rest; firms to solid accent on focus.
const REST_BORDER_ALPHA: f32 = 150.0;
/// Caret blink period (seconds): visible for the first half, hidden the second.
const BLINK_PERIOD: f32 = 1.0;

/// A single-line text input. Emits `input-change` with the full new text on each
/// edit.
pub struct Input {
    base: Base,
    /// The current text, exposed reactively via [`text`](Input::text).
    text: Signal<String>,
    /// Shown (muted) when empty and unfocused.
    placeholder: String,
    /// Caret position as a char index in `0..=text.chars().count()`.
    cursor: usize,
    /// Blink accumulator (seconds, wrapped to [`BLINK_PERIOD`]).
    blink: f32,
    on_change: Option<Box<dyn Fn(Action)>>,
}

impl Input {
    /// A new empty input.
    pub fn new() -> Self {
        let mut base = Base::new();
        base.style.font_size = 14.0;
        base.style.width = Length::Px(DEFAULT_WIDTH);
        base.style.height = Length::Px(base.style.font_size * MONO_LINE_RATIO + 2.0 * PAD as f32);
        Self {
            base,
            text: signal(String::new()),
            placeholder: String::new(),
            cursor: 0,
            blink: 0.0,
            on_change: None,
        }
    }

    /// Set the initial text (caret lands at the end).
    pub fn value(mut self, value: impl Into<String>) -> Self {
        let s = value.into();
        self.cursor = s.chars().count();
        self.text.set(s);
        self
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

    fn char_count(&self) -> usize {
        self.text.get_untracked().chars().count()
    }

    fn contains(&self, p: Point) -> bool {
        self.base.bounds.contains(p)
    }

    fn caret_visible(&self) -> bool {
        self.blink.rem_euclid(BLINK_PERIOD) < BLINK_PERIOD / 2.0
    }

    /// Commit new text: store it, reset the blink, and emit `input-change`.
    fn commit(&mut self, text: String) {
        self.text.set(text.clone());
        self.blink = 0.0;
        if let Some(f) = &self.on_change {
            f(Action::value("input-change", SignalData::String(text)));
        }
    }

    fn insert(&mut self, c: char) {
        let mut chars: Vec<char> = self.text.get_untracked().chars().collect();
        let i = self.cursor.min(chars.len());
        chars.insert(i, c);
        self.cursor = i + 1;
        self.commit(chars.into_iter().collect());
    }

    fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut chars: Vec<char> = self.text.get_untracked().chars().collect();
        let i = self.cursor - 1;
        if i < chars.len() {
            chars.remove(i);
            self.cursor = i;
            self.commit(chars.into_iter().collect());
        }
    }

    fn delete(&mut self) {
        let mut chars: Vec<char> = self.text.get_untracked().chars().collect();
        if self.cursor < chars.len() {
            chars.remove(self.cursor);
            self.commit(chars.into_iter().collect());
        }
    }

    /// Handle an editing key. Returns whether it was consumed.
    fn handle_key(&mut self, key: GridKey) -> Handled {
        match key {
            GridKey::Char(c) => self.insert(c),
            GridKey::Space => self.insert(' '),
            GridKey::Backspace => self.backspace(),
            GridKey::Delete => self.delete(),
            GridKey::ArrowLeft => {
                self.cursor = self.cursor.saturating_sub(1);
                self.blink = 0.0;
            }
            GridKey::ArrowRight => {
                self.cursor = (self.cursor + 1).min(self.char_count());
                self.blink = 0.0;
            }
            _ => return Handled::No,
        }
        Handled::Yes
    }
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

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let disabled = self.base.disabled.get_untracked();
        let focused = self.base.focused.get_untracked();
        let (surface, accent, muted, foreground) = {
            let t = cx.theme();
            (t.surface, t.accent, t.muted, t.foreground)
        };
        let b = self.base.bounds;
        let fs = self.base.style.font_size;

        // Field: dark fill; border firms muted → accent on focus.
        let p = if focused { 1.0 } else { 0.0 };
        let border_a = REST_BORDER_ALPHA + (255.0 - REST_BORDER_ALPHA) * p;
        let border = Border {
            color: muted.lerp(accent, p).with_alpha(border_a.round() as u8),
            width: 1.5,
        };
        cx.rect(b, surface, Some(border), RADIUS, None);

        // Text (left-aligned within the padded inner rect); placeholder when
        // empty and unfocused.
        let text_left = b.loc.x + PAD;
        let text_rect = Rectangle::new(
            Point::new(text_left, b.loc.y),
            Size::new((b.size.w - 2.0 * PAD).max(0.0), b.size.h),
        );
        let s = self.text.get_untracked();
        if s.is_empty() && !focused && !self.placeholder.is_empty() {
            cx.text(text_rect, &self.placeholder, muted, fs, TextAlign::Start, false);
        } else if !s.is_empty() {
            cx.text(text_rect, &s, foreground, fs, TextAlign::Start, false);
        }

        // Caret: a thin accent bar at the cursor (monospace advance).
        if focused && !disabled && self.caret_visible() {
            let advance = (fs * MONO_ADVANCE_RATIO) as f64;
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
            cx.dim(b, RADIUS);
        }

        // Focus-visible ring (keyboard focus only).
        if !disabled && self.base.focus_visible.get_untracked() && cx.theme().show_focus_border {
            cx.corner_brackets(b, accent);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerPressed { pos } if self.contains(*pos) => {
                // Place the caret by x (nearest character boundary).
                let advance = (self.base.style.font_size * MONO_ADVANCE_RATIO) as f64;
                let rel = (pos.x - (self.base.bounds.loc.x + PAD)).max(0.0);
                let idx = if advance > 0.0 {
                    (rel / advance).round() as usize
                } else {
                    0
                };
                self.cursor = idx.min(self.char_count());
                self.blink = 0.0;
                Handled::Yes
            }
            Event::Key { key, pressed: true } => self.handle_key(*key),
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        if self.base.focused.get_untracked() {
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
