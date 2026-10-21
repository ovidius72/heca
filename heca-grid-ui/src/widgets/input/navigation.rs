use super::*;

impl Input {
    /// Handle a plain editing key. Returns whether it was consumed. The emacs/readline
    /// shortcuts (Ctrl+h delete, Ctrl+u clear, Ctrl/Cmd+A select-all) are **not** here —
    /// they are host-configured (`[keys.widgets]` `edit_*` bindings) and arrive as the
    /// [`Event::Widget`] `Edit*` intents. A modified char is
    /// ignored so it is never typed as text (and so the host's shortcut resolution can act on it).
    pub(super) fn handle_key(&mut self, key: GridKey) -> Handled {
        match key {
            // **Text is not a key.** Typing arrives as [`Event::TextInput`] — the character the
            // user meant, including anything an IME composed and anything pasted — so a raw
            // `Char` is only ever a shortcut here, and falls through unhandled to whoever resolves
            // those. This is what the host's "deliver the real character, not the lowercased combo
            // key" fixup existed to paper over.
            GridKey::Char(_) | GridKey::Space => return Handled::No,
            GridKey::Enter if self.multiline && !crate::event::modifiers().ctrl
                && !crate::event::modifiers().meta && !crate::event::modifiers().alt => self.insert('\n'),
            GridKey::ArrowUp if self.multiline => self.move_vertical(true),
            GridKey::ArrowDown if self.multiline => self.move_vertical(false),
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
    pub(super) fn select_all(&mut self) {
        let n = self.char_count();
        self.anchor = (n > 0).then_some(0);
        self.cursor = n;
        self.blink_origin = Instant::now();
    }

    /// Movement granularity from the current modifiers: Ctrl/Cmd → to start/end,
    /// Alt → by word, otherwise by character.
    pub(super) fn granularity(&self) -> Granularity {
        if crate::event::modifiers().ctrl || crate::event::modifiers().meta {
            Granularity::Line
        } else if crate::event::modifiers().alt {
            Granularity::Word
        } else {
            Granularity::Char
        }
    }

    /// Move the caret left/right at `gran`. With Shift held the move
    /// **extends/shrinks** the selection (anchoring at the start position);
    /// without Shift it collapses any selection and moves the caret.
    pub(super) fn move_caret(&mut self, left: bool, gran: Granularity) {
        self.blink_origin = Instant::now();

        if crate::event::modifiers().shift {
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
            (true, Granularity::Line) => self.line_start(),
            (false, Granularity::Line) => self.line_end(),
        };

        // Shrinking back onto the anchor clears the selection ("deselect").
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
    }
    pub(super) fn line_start(&self) -> usize {
        self.chars_vec().iter().take(self.cursor).rposition(|ch| *ch == '\n').map_or(0, |index| index + 1)
    }
    pub(super) fn line_end(&self) -> usize {
        let chars = self.chars_vec();
        chars.iter().skip(self.cursor).position(|ch| *ch == '\n').map_or(chars.len(), |index| self.cursor + index)
    }
    fn move_vertical(&mut self, up: bool) {
        let view = self.view.borrow();
        let rows = view.rows();
        let (row, col) = view.position(self.cursor);
        let row = if up { row.saturating_sub(1) } else { (row + 1).min(rows.len() - 1) };
        let next = rows[row].start + col.min(rows[row].len());
        drop(view);
        if crate::event::modifiers().shift { self.anchor.get_or_insert(self.cursor); }
        else { self.anchor = None; }
        self.cursor = next;
        self.blink_origin = Instant::now();
    }

}

/// How far a caret move travels.
#[derive(Clone, Copy)]
pub(super) enum Granularity {
    Char,
    Word,
    Line,
}

/// The `(start, end)` char range of the word at `idx`: a maximal run of
/// like-classed (whitespace vs non-whitespace) characters.
pub(super) fn word_bounds(chars: &[char], idx: usize) -> (usize, usize) {
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
pub(super) fn prev_word_boundary(chars: &[char], cursor: usize) -> usize {
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
pub(super) fn next_word_boundary(chars: &[char], cursor: usize) -> usize {
    let mut i = cursor;
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    while i < chars.len() && !chars[i].is_whitespace() {
        i += 1;
    }
    i
}

