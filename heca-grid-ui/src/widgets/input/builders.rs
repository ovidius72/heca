use super::*;
use crate::builders::Parent;

#[heca_grid_ui_macros::props]
impl Input {

    /// A new empty input.
    pub fn new() -> Self {
        let mut base = Base::container();
        base.focusable = true;
        let text = signal(String::new());
        let view = std::rc::Rc::new(std::cell::RefCell::new(content::View::new(text)));
        let content = content::Content::new(view.clone());
        let region = crate::ScrollRegion::new().horizontal().padding_y("none").child(content);
        view.borrow_mut().offset_x = region.scroll_offset_x();
        view.borrow_mut().offset_y = region.scroll_offset();
        base.children.push(crate::WidgetModel::new(region));
        Self { base, text, placeholder: String::new(), placeholder_on_focus: false, seen_text: String::new(), cursor: 0, anchor: None,
            blink_origin: Instant::now(), last_caret: std::cell::Cell::new(false),
            multiline: false, max_lines: None, view, on_change: None, on_submit: None }
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
        self.seen_text.clone_from(&s);
        self.text.set(s);
        self.base.mark_needs_layout();
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

    /// Keep an empty field's placeholder visible while focused (default false).
    #[heca_grid_ui_macros::prop]
    pub fn placeholder_on_focus(mut self, show: bool) -> Self { self.placeholder_on_focus = show; self }

    /// Set the change handler. Receives `Action::value("input-change",
    /// SignalData::String(new_text))` after every edit.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
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


    /// Enable a growing, wrapping multi-line field (default false). Enter inserts a newline.
    #[heca_grid_ui_macros::prop]
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self.base.children.clear();
        let region = if multiline { crate::ScrollRegion::new() } else { crate::ScrollRegion::new().horizontal() };
        let region = region.padding_y("none");
        self.view.borrow_mut().offset_x = region.scroll_offset_x();
        self.view.borrow_mut().offset_y = region.scroll_offset();
        self.base.children.push(crate::WidgetModel::new(region.child(content::Content::new(self.view.clone()))));
        self
    }

    /// Override the theme's maximum visible lines. At least one; extra content scrolls.
    #[heca_grid_ui_macros::prop]
    pub fn max_lines(mut self, lines: usize) -> Self { self.max_lines = Some(lines.max(1)); self }
}
