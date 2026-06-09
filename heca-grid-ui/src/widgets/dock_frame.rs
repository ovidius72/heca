//! [`DockFrame`] — a titled, collapsible frame for hosting a Dock's content.
//!
//! It is the bracket-framed shell an app-side Dock (e.g. `WorkspacesDock`) drops
//! its content into: a **title bar** (drag-handle grip + collapse chevron +
//! title, plus a header-controls slot the Dock fills with its own affordances,
//! e.g. a search field) over a **body** that holds the content. The whole frame
//! reuses [`Pane`](super::Pane)'s prominent corner brackets via the shared
//! [`PaintCx::bracket_frame`].
//!
//! Presentation-only and domain-neutral, like [`ItemGroup`](super::ItemGroup):
//! the header toggles an `expanded` [`Signal`] and (optionally) reports it via
//! [`on_toggle`](DockFrame::on_toggle); collapsing hides the body via
//! `style.hidden` (`display: none`), so it folds out of layout entirely. The
//! drag-handle grip is a visual affordance only — wiring it to the shipped
//! `drag/` framework is G6's job (see [`child`](DockFrame::child) seam).

use crate::action::{Action, SignalData};
use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{route_event, Base, Component, Event, Handled, PaintCx};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::{Align, Direction};
use crate::widgets::{Flex, Item, Label};

/// Chevron glyphs for expanded / collapsed states.
const CHEVRON_OPEN: &str = "▾";
const CHEVRON_CLOSED: &str = "▸";
/// Drag-handle grip glyph shown at the start of the title bar (G6 wires the drag).
const GRIP: &str = "⠿";
/// Gap between the grip and the chevron in the header's leading slot.
const LEADING_GAP: f32 = 8.0;

/// Index of the header (a [`Flex`] row) / body within `base.children`.
const HEADER: usize = 0;
const BODY: usize = 1;
/// Index of the controls slot within the header row (after the toggle [`Item`]).
const CONTROLS: usize = 1;

/// A titled, collapsible, bracket-framed container for a Dock.
pub struct DockFrame {
    base: Base,
    expanded: Signal<bool>,
    /// Text signal of the header's chevron glyph (flipped on toggle).
    chevron: Signal<String>,
    on_toggle: Option<Box<dyn Fn(Action)>>,
}

impl DockFrame {
    /// A new expanded frame titled `title`. Add body content with `.child(...)`
    /// and header controls with `.header(...)`.
    pub fn new(title: impl Into<String>) -> Self {
        let expanded = signal(true);
        let chevron_label = Label::new(CHEVRON_OPEN);
        let chevron = chevron_label.text_signal();

        // Leading slot: drag grip + chevron. Body visibility + chevron are synced
        // in `remeasure`/`event` (which have `&mut self`).
        let leading = Flex::row()
            .align(Align::Center)
            .gap(LEADING_GAP)
            .child(Label::new(GRIP))
            .child(chevron_label);
        // The toggle Item carries the title and flips `expanded` on activate; it
        // grows so the controls slot sits at the right edge.
        let toggle = Item::new(title)
            .leading(leading)
            .on_activate(move || expanded.set(!expanded.get_untracked()))
            .grow(1.0);
        // Header row: [toggle (grows), controls slot]. Children — not the toggle
        // Item — so an interactive control (e.g. a search Input) still receives
        // events: `event` routes to the header's children, controls-first.
        let header = Flex::row()
            .align(Align::Center)
            .child(toggle)
            .child(Flex::empty());

        // Body holds the dock content; folds out of layout when collapsed.
        let body = Flex::column();

        let mut base = Base::new();
        base.style.direction = Direction::Column;
        base.children.push(Box::new(header));
        base.children.push(Box::new(body));
        // Invariant relied on by `header`/`child`/`sync` index access below.
        debug_assert_eq!(base.children.len(), 2, "DockFrame children: [HEADER, BODY]");
        debug_assert_eq!(
            base.children[HEADER].base().children.len(),
            2,
            "DockFrame header children: [toggle, CONTROLS]"
        );
        Self { base, expanded, chevron, on_toggle: None }
    }

    /// Set the initial expanded state.
    pub fn expanded(self, open: bool) -> Self {
        self.expanded.set(open);
        self.chevron.set(chevron_for(open).to_string());
        self
    }

    /// Report toggles. Receives `Action::value("dock-toggle", Bool(expanded))`.
    pub fn on_toggle(mut self, f: impl Fn(Action) + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    /// Fill the header-controls slot — the Dock's own affordances (e.g. a search
    /// field). Interactive controls work: events reach the slot before the toggle.
    pub fn header(mut self, c: impl Component + 'static) -> Self {
        self.base.children[HEADER].base_mut().children[CONTROLS] = Box::new(c);
        self
    }

    /// Append body content (folds away when collapsed). This is also the seam G6
    /// uses to make the frame draggable via the shipped `drag/` framework.
    pub fn child(mut self, c: impl Component + 'static) -> Self {
        self.base.children[BODY].base_mut().children.push(Box::new(c));
        self
    }

    /// The expanded-state signal — bind UI to it reactively.
    pub fn state(&self) -> Signal<bool> {
        self.expanded
    }

    /// Apply the current `expanded` state to body visibility + chevron glyph.
    fn sync(&mut self) {
        let open = self.expanded.get_untracked();
        self.chevron.set(chevron_for(open).to_string());
        self.base.children[BODY].base_mut().style.hidden = !open;
    }
}

fn chevron_for(open: bool) -> &'static str {
    if open { CHEVRON_OPEN } else { CHEVRON_CLOSED }
}

impl Component for DockFrame {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Keep body visibility + chevron in sync with `expanded` every layout pass
    /// (covers the initial `.expanded(false)` state and external signal changes).
    fn remeasure(&mut self) {
        self.sync();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let radius = cx.theme().radius;
        let b = self.base.bounds;
        let fill = self.base.style.fill;

        // Background fill — rounded by theme radius.
        if let Some(f) = fill {
            cx.rect(b, f, None, radius, self.base.style.glow);
        }

        // Prominent flat corner-bracket frame (shared with Pane).
        cx.bracket_frame(b, fill);

        // Header + body draw themselves; the collapsed body has zero bounds.
        for child in &self.base.children {
            child.paint(cx);
        }
    }

    fn event(&mut self, ev: &Event) -> Handled {
        let was = self.expanded.get_untracked();
        // Default routing lets the header's toggle Item flip `expanded` on
        // click/Enter (and lets the controls slot consume events first).
        let handled = route_event(&mut self.base.children, ev);
        let now = self.expanded.get_untracked();
        if now != was {
            self.sync();
            if let Some(f) = &self.on_toggle {
                f(Action::value("dock-toggle", SignalData::Bool(now)));
            }
        }
        handled
    }
}

impl LayoutExt for DockFrame {}
impl StyleExt for DockFrame {}
