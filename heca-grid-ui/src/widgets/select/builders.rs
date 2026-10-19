//! The builder API: how a [`Select`] is constructed and configured, plus the
//! accessors a host reads (the selected index and its label).

use super::*;
use crate::component::Component;
use crate::reactive::signal;
use crate::style::{Align, Direction};
use crate::widgets::choice::Choice;

#[heca_grid_ui_macros::props]
impl Select {
    /// An empty select — add options with [`option`](Select::option).
    pub fn empty() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        // One control = one Tab stop: focus never descends into the options.
        base.focus_barrier = true;
        // The options stack vertically. This is the flow taffy *measures* them in; `place_options`
        // then moves them into the overlay panel.
        base.style.layout.direction = Direction::Column;
        // Hug the widest option instead of stretching to fill a column parent (the width is
        // `Auto`, and the default cross-axis alignment is `Stretch`).
        base.style.layout.align_self = Some(Align::Start);
        let mut select = Self {
            base,
            option_states: Vec::new(),
            selected: signal(0),
            open: false,
            highlight: 0,
            scroll: 0,
            vis_rows: MAX_VISIBLE,
            open_up: false,
            natural_h: Vec::new(),
            viewport: Cell::new(Size::new(f64::INFINITY, f64::MAX)),
            on_change: None,
        };
        select.remeasure();
        select
    }

    /// A new select over plain-text `options`; the first is selected.
    ///
    /// Sugar: each string becomes a [`Choice::labeled`] child whose **value is the text itself**.
    /// It builds exactly the tree [`option`](Select::option) would — there is one option model, not
    /// a "simple mode".
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut select = Self::empty();
        for option in options {
            let text: String = option.into();
            select = select.option(Choice::labeled(text.clone(), text));
        }
        select
    }

    /// Append a composed option.
    ///
    /// Typed to [`Choice`] on purpose: the control keeps the row's selected-state
    /// [`Signal`](Choice::state) so it can drive the selection in place, and a `Box<dyn Component>`
    /// would have thrown that away. It is also the contract — the rows of a select **are** options.
    #[heca_grid_ui_macros::host_only("a composed value, not a scalar — built from `children`")]
    pub fn option(mut self, choice: Choice) -> Self {
        self.option_states.push(choice.state());
        self.base.children.push(Box::new(choice));
        self.sync_option_states();
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

    /// Select an initial option (clamped to the option count). Call it **after** the options.
    #[heca_grid_ui_macros::prop]
    pub fn selected(self, index: usize) -> Self {
        let i = index.min(self.count().saturating_sub(1));
        self.selected.set(i);
        self.sync_option_states();
        self
    }

    /// Set the change handler. Receives `Action::value("select-change",
    /// SignalData::Usize(index))` when the selection changes.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_change(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// The selected-index signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<usize> {
        self.selected
    }

    /// The currently selected index (untracked read).
    pub fn index(&self) -> usize {
        self.selected.get_untracked()
    }

    /// The selected option **as text** — its [accessible name](Component::text_summary), computed
    /// from the option's content: for `Choice::labeled` it is the label; for an option composed of
    /// an `Icon` + `Label("HIGH")` it is `"HIGH"`. Empty when the option has no text at all (an
    /// icon-only option). Use it to report the current value; the trigger itself renders the option,
    /// not this.
    pub fn selected_label(&self) -> String {
        self.base
            .children
            .get(self.selected.get_untracked())
            .and_then(|c| c.text_summary())
            .unwrap_or_default()
    }
}
