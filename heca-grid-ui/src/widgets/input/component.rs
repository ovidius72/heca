use super::*;

impl Component for Input {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn taffy_style(&self) -> taffy::Style {
        let mut layout = self.base.style.layout;
        if layout.width == Length::Auto {
            layout.width = Length::Px(self.base.input_style.width);
        }
        if layout.padding == crate::style::Space::default() {
            layout.padding = crate::style::Space::Px(self.pad() as f32);
        }
        let minimum = self.base.font * MONO_LINE_RATIO
            + layout.pad_top(self.base.font)
            + layout.pad_bottom(self.base.font);
        layout.min_height = match layout.min_height {
            None | Some(Length::Auto) => Some(Length::Px(minimum)),
            Some(Length::Px(height)) => Some(Length::Px(height.max(minimum))),
            other => other,
        };
        layout.to_taffy(self.base.font)
    }
    fn remeasure(&mut self) {
        let text = self.text.get_untracked();
        if text != self.seen_text {
            self.cursor = text.chars().count();
            self.anchor = None;
            self.seen_text = text;
            self.blink_origin = crate::clock::now();
        }
        self.cursor = self.cursor.min(self.char_count());
        let focused = self.base.focused.get_untracked();
        let mut view = self.view.borrow_mut();
        view.cursor = self.cursor;
        view.selection = self.selection();
        view.focused = focused;
        view.caret = self.caret_visible();
        view.multiline = self.multiline;
        view.placeholder_on_focus = self.placeholder_on_focus;
        view.font = self.base.font;
        view.caret_width = self.base.input_style.caret_width.max(0.0);
        let lines = if self.multiline {
            self.max_lines
                .unwrap_or(self.base.input_style.max_lines)
                .max(1)
        } else {
            1
        };
        let automatic_height = self.base.style.layout.height == Length::Auto;
        let max_height = Length::Px(self.base.font * MONO_LINE_RATIO * lines as f32);
        let Some(region) = self.base.child_in_area(super::builders::REGION_AREA) else {
            return;
        };
        let region = region.base_mut();
        region.style.layout.max_height = automatic_height.then_some(max_height);
        region.style.layout.flex_grow = 1.0;
        region.style.layout.width = Length::Percent(1.0);
        drop(view);
        self.hand_down_suggestion_style();
    }

    fn on_placed(&mut self) {
        self.hand_down_caret();
    }
    fn paint(&self, cx: &mut PaintCx) {
        let focused = self.base.focused.get_untracked();
        {
            let mut view = self.view.borrow_mut();
            view.focused = focused;
            view.caret = self.caret_visible();
            view.cursor = self.cursor;
            view.selection = self.selection();
        }
        let colors = &cx.theme().colors;
        let border = Border {
            color: if focused {
                cx.accent()
            } else {
                colors
                    .muted
                    .with_alpha(colors.interaction.control_rest_border)
            },
            width: colors.border_width,
        };
        cx.rect(
            self.base.bounds,
            colors.surface,
            Some(border),
            colors.control_radius(),
            cx.rest_glow(self.base.input_style.glow_radius),
        );
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
        if !self.base.disabled.get_untracked()
            && self.base.shows_focus_ring()
            && cx.theme().colors.show_focus_border
        {
            cx.focus_ring(
                self.base.bounds,
                cx.theme().colors.effective_focus_ring(),
                cx.theme().colors.control_radius(),
            );
        }
    }
    /// The scrolling child takes its thumb and track gestures first. Text presses and editing
    /// bubble here; the field remains the keyboard owner.
    ///
    /// **A field types because it is focused, and for no other reason.** It used to declare that it
    /// took raw keys and typed text whether or not it held focus; both flags are gone, along with
    /// the question they answered. Keys and typed text are delivered to the focus owner, so a
    /// mounted-but-unfocused field is silent without saying so, and a focused one hears everything
    /// without asking.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        // The × of a clearable field was clicked: its click bubbles here, and the field empties.
        if self.clear_requested.take() {
            self.clear();
            self.base.mark_needs_layout();
            return Handled::Yes;
        }
        // A suggestion row was clicked: its click bubbles here, and its item goes in.
        if let Some(insert) = self.suggest.as_ref().and_then(|s| s.picked.get_untracked()) {
            let suggest = self.suggest.as_ref().expect("picked from it");
            suggest.picked.set(None);
            if let Some(index) = suggest.index_of(&insert) {
                self.choose_suggestion(index);
            }
            self.follow_suggestions();
            self.base.mark_needs_layout();
            return Handled::Yes;
        }
        let handled = match ev {
            // The open suggestion list has the keys it uses, and a press on one of its rows.
            Event::Key { key, pressed: true } if self.yields_to_suggestions(*key) => {
                return Handled::No;
            }
            Event::Widget(intent) if self.suggestion_intent(*intent) == Handled::Yes => {
                Handled::Yes
            }
            Event::Widget(WidgetIntent::EditSubmit) => {
                if let Some(submit) = &self.on_submit {
                    submit(Action::value(
                        "input-submit",
                        SignalData::String(self.value_str()),
                    ));
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            // The **click run comes with the event**. This widget kept its own clock and its own
            // counter to work out that a press was the second one; both are gone, and with them
            // the chance of a field that counts clicks differently from everything else.
            //
            // The caret lands on the press (so a drag-select would start from the right place),
            // and the selection cycle reads `click_count`: 1 = caret, 2 = word, 3 = all,
            // 4 = deselect and start again.
            Event::PointerDown(p) => {
                self.blink_origin = crate::clock::now();
                self.anchor = None;
                self.cursor = self.view.borrow().index_at(p.pos, self.text_origin(), true);
                Handled::Yes
            }
            Event::DoubleClick(p) => {
                let chars = self.chars_vec();
                let idx = self
                    .view
                    .borrow()
                    .index_at(p.pos, self.text_origin(), false)
                    .min(chars.len().saturating_sub(1));
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
                self.cursor = self.view.borrow().index_at(p.pos, self.text_origin(), true);
                Handled::Yes
            }
            // Typed text, IME commits and pastes: one event, whatever produced them. The field no
            // longer reconstructs a character from a key combo, and the host no longer patches the
            // key it sends so that reconstruction comes out right.
            Event::TextInput(text) => {
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                let text: String = text
                    .chars()
                    .filter(|ch| self.multiline || *ch != '\n')
                    .collect();
                self.insert_text(&text);
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
            Event::Widget(WidgetIntent::ClearInput) => {
                self.clear();
                Handled::Yes
            }
            _ => Handled::No,
        };
        if handled == Handled::Yes {
            self.follow_suggestions();
            self.base.mark_needs_layout();
        }
        handled
    }

    // The caret is driven by real time (`blink_origin`), not the frame `dt`. `tick`
    // does no continuous animation; it only damages the field when the caret actually
    // toggles, so a blink repaints just the input's rect (not the whole scene). The
    // wake at the next toggle is scheduled via `next_redraw`.
    fn tick(&mut self, dt: f32) -> bool {
        let mut moving = self.base.tick_presence(dt);
        for child in &mut self.base.children {
            moving |= child.tick(dt);
        }
        if self.base.focused.get_untracked() {
            let vis = self.caret_visible();
            if vis != self.last_caret.get() {
                self.last_caret.set(vis);
                self.base.mark_needs_paint();
            }
        }
        moving
    }

    fn next_redraw(&self) -> Option<f32> {
        // Its own tooltip is still pending whether or not the caret is blinking — an unfocused
        // field with a tip would otherwise never wake to show it.
        let tip = crate::widgets::tooltip::wake(&self.base);
        if !self.base.focused.get_untracked() {
            return tip;
        }
        if self.base.input_style.blink_period <= 0.0 {
            return tip;
        }
        // Time until the caret flips: the next half-`self.base.input_style.blink_period.max(f32::MIN_POSITIVE)` boundary.
        let phase = self
            .blink_origin
            .elapsed()
            .as_secs_f32()
            .rem_euclid(self.base.input_style.blink_period.max(f32::MIN_POSITIVE));
        let half = self.base.input_style.blink_period.max(f32::MIN_POSITIVE) / 2.0;
        let caret = if phase < half {
            half - phase
        } else {
            self.base.input_style.blink_period.max(f32::MIN_POSITIVE) - phase
        };
        crate::component::soonest_redraw(tip, Some(caret))
    }
}

impl Input {
    /// **Hand the suggestion list the field's theme values** before layout — how many items, how
    /// narrow — since they change its size.
    fn hand_down_suggestion_style(&mut self) {
        let Some(suggest) = &self.suggest else {
            return;
        };
        let style = self.base.input_style;
        let limit = style.suggestions.max(1);
        if suggest.limit.get_untracked() != limit {
            suggest.limit.set(limit);
        }
        let width = Length::Px(style.suggestions_width.max(0.0));
        if suggest.width.get_untracked() != width {
            suggest.width.set(width);
        }
    }

    /// **Tell the suggestion list where the caret is on screen**, once layout has finished and the
    /// field is where it will be drawn: the text's start, plus the caret's row and column, less how
    /// far the text has scrolled. Whether the list is up, and where it goes — below, or above when
    /// there is no room — is the list's own.
    fn hand_down_caret(&mut self) {
        let Some(suggest) = &self.suggest else {
            return;
        };
        let caret = {
            let view = self.view.borrow();
            let (row, column) = view.position(self.cursor);
            let cell = f64::from(view.font * MONO_ADVANCE_RATIO);
            let line = f64::from(view.font * MONO_LINE_RATIO);
            let origin = self.text_origin();
            Rectangle::new(
                Point::new(
                    origin.x + column as f64 * cell - f64::from(view.offset_x.get_untracked()),
                    origin.y + row as f64 * line - f64::from(view.offset_y.get_untracked()),
                ),
                Size::new(cell, line),
            )
        };
        if suggest.caret.get_untracked() != caret {
            suggest.caret.set(caret);
        }
    }

    /// Whether the suggestion list is open with something in it.
    fn suggesting(&self) -> bool {
        self.suggest
            .as_ref()
            .is_some_and(|suggest| suggest.is_open())
    }

    /// **The field's own keys the open list needs**: the arrows, Enter, Tab and Escape would move
    /// the caret, add a line, move focus or leave — while the list is open the field declines them,
    /// so the keymap turns them into the list's intents (below) the way it does for any list.
    fn yields_to_suggestions(&self, key: GridKey) -> bool {
        let plain = {
            let m = crate::event::modifiers();
            !(m.ctrl || m.meta || m.alt || m.shift)
        };
        plain
            && self.suggesting()
            && matches!(
                key,
                GridKey::ArrowUp
                    | GridKey::ArrowDown
                    | GridKey::Enter
                    | GridKey::Tab
                    | GridKey::Escape
            )
    }

    /// **The open list answers the list intents**, whatever keys the user bound to them: up and
    /// down (`menu_up` / `menu_down` — the arrows, Ctrl+k / Ctrl+j) move the choice, `activate` and
    /// `complete` (Enter, Tab) put it in, `dismiss` (Escape) closes the list.
    fn suggestion_intent(&mut self, intent: WidgetIntent) -> Handled {
        if !self.suggesting() {
            return Handled::No;
        }
        let Some(suggest) = self.suggest.as_mut() else {
            return Handled::No;
        };
        match intent {
            WidgetIntent::MenuDown => suggest.step(true),
            WidgetIntent::MenuUp => suggest.step(false),
            WidgetIntent::Dismiss => suggest.dismiss(),
            WidgetIntent::Activate | WidgetIntent::Complete => {
                let chosen = suggest.selected.get_untracked();
                self.choose_suggestion(chosen);
            }
            _ => return Handled::No,
        }
        Handled::Yes
    }

    /// **Put suggestion `index` in place of the open word** and close the list.
    fn choose_suggestion(&mut self, index: usize) {
        let Some((range, text)) = self
            .suggest
            .as_ref()
            .and_then(|suggest| suggest.replacement(index, self.cursor))
        else {
            return;
        };
        let mut chars = self.chars_vec();
        let range = range.start.min(chars.len())..range.end.min(chars.len());
        self.anchor = None;
        self.cursor = range.start + text.chars().count();
        chars.splice(range, text.chars());
        self.commit(chars.into_iter().collect());
    }

    /// The suggestions follow the caret after anything that moved it or changed the text.
    fn follow_suggestions(&mut self) {
        let chars = self.chars_vec();
        let selecting = self.selection().is_some();
        let caret = self.cursor;
        if let Some(suggest) = self.suggest.as_mut() {
            suggest.follow(&chars, caret, selecting);
        }
    }

    /// Where the text starts on screen — the top-left of its scrolling part, which the pointer's
    /// position is read against.
    fn text_origin(&self) -> Point {
        self.base
            .child_area(super::builders::REGION_AREA)
            .map_or(self.base.bounds.loc, |region| region.base().bounds.loc)
    }
}
