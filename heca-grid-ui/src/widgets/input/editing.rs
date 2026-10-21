use super::*;

impl Input {
    pub(super) fn char_count(&self) -> usize {
        self.text.get_untracked().chars().count()
    }

    pub(super) fn caret_visible(&self) -> bool {
        let phase = self
            .blink_origin
            .elapsed()
            .as_secs_f32()
            .rem_euclid(self.base.input_style.blink_period.max(f32::MIN_POSITIVE));
        self.base.input_style.blink_period <= 0.0 || phase < self.base.input_style.blink_period / 2.0
    }

    pub(super) fn chars_vec(&self) -> Vec<char> {
        self.text.get_untracked().chars().collect()
    }

    /// Inner padding scaled by the size variant (matches `remeasure` + paint).
    pub(super) fn pad(&self) -> f64 {
        self.base.input_style.padding as f64 * self.base.size_scale() as f64
    }

    /// Remove the selected range (if any) from `chars`, moving the caret to its
    /// start. Returns whether anything was removed. Does not commit.
    pub(super) fn drain_selection(&mut self, chars: &mut Vec<char>) -> bool {
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
    pub(super) fn commit(&mut self, text: String) {
        self.seen_text.clone_from(&text);
        self.text.set(text.clone());
        self.base.mark_needs_layout();
        self.blink_origin = Instant::now();
        if let Some(f) = &self.on_change {
            f(Action::value("input-change", SignalData::String(text)));
        }
    }

    pub(super) fn insert_text(&mut self, text: &str) {
        if text.is_empty() { return; }
        let mut chars = self.chars_vec();
        self.drain_selection(&mut chars);
        let at = self.cursor.min(chars.len());
        self.cursor = at + text.chars().count();
        chars.splice(at..at, text.chars());
        self.commit(chars.into_iter().collect());
    }

    pub(super) fn insert(&mut self, c: char) {
        let mut chars = self.chars_vec();
        self.drain_selection(&mut chars);
        let i = self.cursor.min(chars.len());
        chars.insert(i, c);
        self.cursor = i + 1;
        self.commit(chars.into_iter().collect());
    }

    /// Granularity for delete keys: Meta (Cmd) → to start/end, Ctrl/Alt → word,
    /// otherwise a single character.
    pub(super) fn delete_granularity(&self) -> Granularity {
        if crate::event::modifiers().meta {
            Granularity::Line
        } else if crate::event::modifiers().ctrl || crate::event::modifiers().alt {
            Granularity::Word
        } else {
            Granularity::Char
        }
    }

    /// Delete left of the caret at `gran` (char / word / to start).
    pub(super) fn backspace(&mut self, gran: Granularity) {
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
            Granularity::Line => self.line_start(),
        };
        chars.drain(start..self.cursor.min(chars.len()));
        self.cursor = start;
        self.commit(chars.into_iter().collect());
    }

    /// Delete right of the caret at `gran` (char / word / to end).
    pub(super) fn delete(&mut self, gran: Granularity) {
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
            Granularity::Line => self.line_end(),
        };
        chars.drain(self.cursor..end.min(chars.len()));
        self.commit(chars.into_iter().collect());
    }

}
