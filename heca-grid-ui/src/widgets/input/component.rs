use super::*;

impl Component for Input {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn taffy_style(&self) -> taffy::Style {
        let mut layout = self.base.style.layout;
        if layout.width == Length::Auto { layout.width = Length::Px(self.base.input_style.width); }
        if layout.padding == crate::style::Space::default() {
            layout.padding = crate::style::Space::Px(self.pad() as f32);
        }
        let minimum = self.base.font * MONO_LINE_RATIO
            + layout.pad_top(self.base.font) + layout.pad_bottom(self.base.font);
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
            self.blink_origin = Instant::now();
        }
        self.cursor = self.cursor.min(self.char_count());
        let focused = self.base.focused.get_untracked();
        let mut view = self.view.borrow_mut();
        view.cursor = self.cursor;
        view.selection = self.selection();
        view.focused = focused;
        view.caret = self.caret_visible();
        view.multiline = self.multiline;
        view.placeholder.clone_from(&self.placeholder);
        view.placeholder_on_focus = self.placeholder_on_focus;
        view.font = self.base.font;
        view.caret_width = self.base.input_style.caret_width.max(0.0);
        let lines = if self.multiline { self.max_lines.unwrap_or(self.base.input_style.max_lines).max(1) } else { 1 };
        let automatic_height = self.base.style.layout.height == Length::Auto;
        let region = self.base.children[0].base_mut();
        region.style.layout.max_height = automatic_height
            .then_some(Length::Px(self.base.font * MONO_LINE_RATIO * lines as f32));
        region.style.layout.flex_grow = 1.0;
        region.style.layout.width = Length::Percent(1.0);
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
        let border = Border { color: if focused { cx.accent() } else { colors.muted.with_alpha(colors.interaction.control_rest_border) },
            width: colors.border_width };
        cx.rect(self.base.bounds, colors.surface, Some(border), colors.control_radius(),
            cx.rest_glow(self.base.input_style.glow_radius));
        for child in &self.base.children { crate::component::paint_child(child.as_ref(), cx); }
        if !self.base.disabled.get_untracked() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            cx.focus_ring(self.base.bounds, cx.theme().colors.effective_focus_ring(), cx.theme().colors.control_radius());
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
        let handled = match ev {
            Event::Widget(WidgetIntent::EditSubmit) => {
                if let Some(submit) = &self.on_submit {
                    submit(Action::value("input-submit", SignalData::String(self.value_str())));
                    Handled::Yes
                } else { Handled::No }
            }
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
                self.cursor = self.view.borrow().index_at(p.pos, self.base.children[0].base().bounds.loc, true);
                Handled::Yes
            }
            Event::DoubleClick(p) => {
                let chars = self.chars_vec();
                let idx = self.view.borrow().index_at(p.pos, self.base.children[0].base().bounds.loc, false).min(chars.len().saturating_sub(1));
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
                self.cursor = self.view.borrow().index_at(p.pos, self.base.children[0].base().bounds.loc, true);
                Handled::Yes
            }
            // Typed text, IME commits and pastes: one event, whatever produced them. The field no
            // longer reconstructs a character from a key combo, and the host no longer patches the
            // key it sends so that reconstruction comes out right.
            Event::TextInput(text) => {
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                let text: String = text.chars().filter(|ch| self.multiline || *ch != '\n').collect();
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
            _ => Handled::No,
        };
        if handled == Handled::Yes { self.base.mark_needs_layout(); }
        handled
    }

    // The caret is driven by real time (`blink_origin`), not the frame `dt`. `tick`
    // does no continuous animation; it only damages the field when the caret actually
    // toggles, so a blink repaints just the input's rect (not the whole scene). The
    // wake at the next toggle is scheduled via `next_redraw`.
    fn tick(&mut self, dt: f32) -> bool {
        let mut moving = self.base.tick_presence(dt);
        for child in &mut self.base.children { moving |= child.tick(dt); }
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
        if self.base.input_style.blink_period <= 0.0 { return tip; }
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
