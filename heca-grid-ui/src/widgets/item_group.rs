//! [`ItemGroup`] — a collapsible group of rows: a clickable header
//! (chevron + label) over a set of child rows that fold away when collapsed.
//!
//! Presentation-only and domain-neutral: the header toggles an `expanded`
//! [`Signal`] and (optionally) reports it via [`on_toggle`](ItemGroup::on_toggle);
//! collapsed children leave layout via `style.hidden` (`display: none`), so they
//! take no space. A container (e.g. a sidebar Dock) composes these around
//! [`Item`](super::Item) rows.

use crate::action::{Action, SignalData};
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, route_event};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::Direction;
use crate::widgets::{Item, Label};

/// Chevron glyphs for expanded / collapsed states.
const CHEVRON_OPEN: &str = "▾";
const CHEVRON_CLOSED: &str = "▸";

/// Index of the header row within `base.children`; group rows follow.
const HEADER: usize = 0;

/// A collapsible group: header row + foldable child rows.
pub struct ItemGroup {
    base: Base,
    expanded: Signal<bool>,
    /// Text signal of the header's chevron glyph (flipped on toggle).
    chevron: Signal<String>,
    on_toggle: Option<Box<dyn Fn(Action)>>,
}

#[heca_grid_ui_macros::props]
impl ItemGroup {
    /// A new expanded group titled `label`. Add rows with `.child(...)`.
    pub fn new(label: impl Into<String>) -> Self {
        let expanded = signal(true);
        let chevron_label = Label::new(CHEVRON_OPEN);
        let chevron = chevron_label.text_signal();
        // Header: clickable Item that flips `expanded` on activate. Sibling-row
        // visibility + chevron are synced in `event()` (which has `&mut self`).
        let header = Item::new(label)
            .leading(chevron_label)
            .on_activate(move || expanded.set(!expanded.get_untracked()));

        let mut base = Base::new();
        base.style.layout.direction = Direction::Column;
        base.children.push(Box::new(header));
        Self {
            base,
            expanded,
            chevron,
            on_toggle: None,
        }
    }

    /// Set the initial expanded state.
    #[heca_grid_ui_macros::prop]
    pub fn expanded(self, open: bool) -> Self {
        self.expanded.set(open);
        self.chevron.set(chevron_for(open).to_string());
        self
    }

    /// Report toggles. Receives `Action::value("group-toggle", Bool(expanded))`.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_toggle(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    /// Append a group row (folds away when collapsed).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn child(mut self, c: impl Component + 'static) -> Self {
        self.base.children.push(Box::new(c));
        self
    }

    /// The expanded-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.expanded
    }

    /// Apply the current `expanded` state to row visibility + chevron glyph.
    fn sync(&mut self) {
        let open = self.expanded.get_untracked();
        self.chevron.set(chevron_for(open).to_string());
        for row in self.base.children.iter_mut().skip(HEADER + 1) {
            row.base_mut().style.layout.hidden = !open;
        }
    }
}

fn chevron_for(open: bool) -> &'static str {
    if open { CHEVRON_OPEN } else { CHEVRON_CLOSED }
}

impl Component for ItemGroup {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Keep row visibility + chevron in sync with `expanded` every layout pass
    /// (covers the initial `.expanded(false)` state and external signal changes).
    fn remeasure(&mut self) {
        self.sync();
    }

    fn event(&mut self, ev: &Event) -> Handled {
        let was = self.expanded.get_untracked();
        // Default routing lets the header Item flip `expanded` on click/Enter.
        let handled = route_event(&mut self.base.children, ev);
        let now = self.expanded.get_untracked();
        if now != was {
            self.sync();
            if let Some(f) = &self.on_toggle {
                f(Action::value("group-toggle", SignalData::Bool(now)));
            }
        }
        handled
    }
}

impl LayoutExt for ItemGroup {}
