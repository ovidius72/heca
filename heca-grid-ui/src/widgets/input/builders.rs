use super::*;
use crate::builders::{EventExt, Parent, PlaceExt, PresenceExt, ToneExt, TooltipExt};

/// The input's scrolling part, found by name: it holds the text and moves it.
pub(super) const REGION_AREA: &str = "region";

/// **The scrolling part around the text** — one line that scrolls sideways, or a growing block that
/// scrolls down when `multiline` — with the view following its offsets.
fn text_region(
    view: &std::rc::Rc<std::cell::RefCell<content::View>>,
    multiline: bool,
) -> crate::WidgetModel {
    let region = if multiline {
        crate::ScrollRegion::new()
    } else {
        crate::ScrollRegion::new().horizontal()
    };
    let region = region.padding_y("none");
    view.borrow_mut().offset_x = region.scroll_offset_x();
    view.borrow_mut().offset_y = region.scroll_offset();
    crate::WidgetModel::new(
        region
            .child(content::Content::new(view.clone()))
            .area(REGION_AREA),
    )
}

#[heca_grid_ui_macros::props]
impl Input {
    /// A new empty input.
    pub fn new() -> Self {
        let mut base = Base::container();
        base.focusable = true;
        let text = signal(String::new());
        let placeholder = signal(String::new());
        let view = std::rc::Rc::new(std::cell::RefCell::new(content::View::new(
            text,
            placeholder,
        )));
        base.children.push(text_region(&view, false));
        Self {
            base,
            text,
            placeholder,
            placeholder_on_focus: false,
            seen_text: String::new(),
            cursor: 0,
            anchor: None,
            blink_origin: crate::clock::now(),
            last_caret: std::cell::Cell::new(false),
            multiline: false,
            max_lines: None,
            view,
            on_change: None,
            on_submit: None,
            clear_requested: std::rc::Rc::new(std::cell::Cell::new(false)),
            suggest: None,
        }
    }

    /// **Suggestions under the caret.** Typing `trigger` at the start of a word opens a list of
    /// `items` under the caret, narrowed by fuzzy search as the word grows; each is drawn by `row`
    /// and, when chosen, `insert` gives the text that replaces the word (trigger included). While
    /// the list is open it answers the list intents, from whatever keys they are bound to: up /
    /// down (the arrows, Ctrl+k / Ctrl+j) move through it, `activate` / `complete` (Enter, Tab)
    /// choose, `dismiss` (Escape) closes it until the word changes; a click on an item chooses it.
    /// How many show at once and how narrow it may be are the theme's (`input_style.suggestions`,
    /// `input_style.suggestions_width`).
    ///
    /// `items` is a fixed list or one that changes (a `Signal`, a `Memo`, a function reading
    /// signals), followed while the list is open.
    ///
    /// ```ignore
    /// Input::new()
    ///     .multiline(true)
    ///     .suggest('@', members, |m| Label::new(&m.handle), |m| format!("@{} ", m.handle))
    /// ```
    #[heca_grid_ui_macros::host_only("closures cannot cross as static properties")]
    pub fn suggest<T: 'static, C: crate::builders::IntoComponent + 'static>(
        mut self,
        trigger: char,
        items: impl crate::builders::IntoSuggestions<T>,
        row: impl Fn(&T) -> C + 'static,
        insert: impl Fn(&T) -> String + 'static,
    ) -> Self {
        let items = items.into_suggestions();
        let (row, insert) = (std::rc::Rc::new(row), std::rc::Rc::new(insert));
        let source: std::rc::Rc<dyn Fn() -> Vec<suggest::Candidate>> =
            std::rc::Rc::new(move || {
                items()
                    .into_iter()
                    .map(|item| {
                        let item = std::rc::Rc::new(item);
                        let text = insert(&item);
                        let row = row.clone();
                        suggest::Candidate {
                            insert: text,
                            row: std::rc::Rc::new(move || row(&item).into_component()),
                        }
                    })
                    .collect()
            });
        let suggest = suggest::Suggest::new(trigger, source);
        self.base
            .children
            .push(crate::WidgetModel::new(suggest.list()));
        self.suggest = Some(suggest);
        self
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
    pub fn value(mut self, value: impl crate::builders::IntoTextValue) -> Self {
        // A `Signal<String>` becomes the field's own, so what is typed is written into it; a fixed
        // or derived text is shown in it.
        value.into_text_value().bind(&self.text);
        let text = self.text.get_untracked();
        self.cursor = text.chars().count();
        self.anchor = None;
        self.seen_text = text;
        self.base.mark_needs_layout();
        self
    }

    /// Replace the text at runtime (caret to end, selection cleared) — e.g. to
    /// reset a reused field. Does **not** fire `on_change` (it's a host action,
    /// not a user edit).
    pub fn set_value(&mut self, value: impl Into<String>) {
        let s = value.into();
        self.cursor = s.chars().count();
        self.anchor = None;
        self.seen_text.clone_from(&s);
        self.text.set(s);
        self.base.mark_needs_layout();
    }

    /// The placeholder text shown now (empty when unset).
    pub fn placeholder_str(&self) -> String {
        self.placeholder.get_untracked()
    }

    /// **The placeholder shown while empty and unfocused** — a fixed text, or one that changes
    /// (a `Signal<String>`, a `Memo`, a closure through [`TextValue`](crate::builders::TextValue)),
    /// which the field follows with nothing rebuilt: a search field whose hint names what it
    /// searches changes the hint, not the field.
    ///
    /// ```ignore
    /// let mode = signal("Search commands".to_string());
    /// Input::new().placeholder(mode)     // later: mode.set("Search panes".into())
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn placeholder(self, placeholder: impl crate::builders::IntoTextValue) -> Self {
        placeholder.into_text_value().show_in(self.placeholder);
        self
    }

    /// Keep an empty field's placeholder visible while focused (default false).
    #[heca_grid_ui_macros::prop]
    pub fn placeholder_on_focus(mut self, show: bool) -> Self {
        self.placeholder_on_focus = show;
        self
    }

    /// Set the change handler. Receives `Action::value("input-change",
    /// SignalData::String(new_text))` after every edit.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// **An × at the field's right end that empties it** — shown while the field holds text, gone
    /// when it is empty, with the [`IconButton`](crate::widgets::IconButton)'s hover from the theme.
    /// A click empties the field and leaves the keyboard in it (the field
    /// [keeps the keyboard](crate::component::Base::keeps_keyboard), so the × gets its click and
    /// not the keyboard), and reports the change through
    /// [`on_change`](Self::on_change) exactly as typing does. The keyboard twin is `clear_input` in
    /// `[keys.widgets]` (`WidgetIntent::ClearInput`), which works on every field, clearable or not.
    ///
    /// ```ignore
    /// Input::new().value(query).clearable(true).on_change(|action| search(action))
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn clearable(mut self, clearable: bool) -> Self {
        if !clearable {
            return self;
        }
        let text = self.text;
        let requested = self.clear_requested.clone();
        self.base.style.layout.align = "center".into();
        // The field keeps the keyboard: a press on the × clicks it and leaves the keyboard here.
        self.base.keeps_keyboard = true;
        self.base.children.push(crate::WidgetModel::new(
            crate::widgets::IconButton::new(
                crate::widgets::Icon::new(crate::widgets::Glyph::XCircle)
                    .content_tone(crate::Tone::Muted),
            )
            .size(crate::style::WidgetSize::Small)
            .tooltip("Clear")
            .hidden(move || text.get().is_empty())
            .on_click(move |_| requested.set(true)),
        ));
        self
    }

    /// Handle the configurable `EditSubmit` intent. Emits `input-submit` with the full value.
    /// No handler means the intent bubbles to the enclosing component. No key is bound by default.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_submit(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_submit = Some(Box::new(f));
        self
    }

    /// The text signal — bind UI to it reactively.
    pub fn text_signal(&self) -> Signal<String> {
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

    /// Enable a growing, wrapping multi-line field (default false). Enter inserts a newline.
    #[heca_grid_ui_macros::prop]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self.base.children.clear();
        self.base.children.push(text_region(&self.view, multiline));
        self
    }

    /// Override the theme's maximum visible lines. At least one; extra content scrolls.
    #[heca_grid_ui_macros::prop]
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = Some(lines.max(1));
        self
    }
}
