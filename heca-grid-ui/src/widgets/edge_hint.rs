//! **`EdgeHint` — "more below that wants you"**: pinned in a [`ScrollRegion`], it shows how many
//! of the region's rows that want attention (a repeating [flash](crate::FlashHandle)) are scrolled out
//! of sight above and below — a caret and the count — and a click brings the nearest one into view and
//! activates it, as picking it with a letter would.
//!
//! ```ignore
//! ScrollRegion::new()
//!     .child(rows)                      // rows that flash with `repeat` while they want attention
//!     .pinned(EdgeHint::new())          // stays on the edges, above the content
//! ```
//!
//! Nothing is declared on the rows: wanting attention *is* a repeating flash. The region tells the
//! hint what is out of sight ([`Component::scroll_facts`]) and the hint asks the region to reveal
//! ([`Component::take_reveal`]); both are trait methods, so any pinned widget can do the same.

use std::cell::Cell;
use std::rc::Rc;

use crate::component::{Base, Component};
use crate::prelude::*;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::style::Length;
use crate::widgets::{Button, Flex, Glyph, Grid, Label};

/// What a [`ScrollRegion`] tells the widgets pinned in it, each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollFacts {
    /// Rows wanting attention scrolled out of sight above.
    pub attention_above: usize,
    /// Rows wanting attention scrolled out of sight below.
    pub attention_below: usize,
}

/// Which way a pinned widget asks its region to reveal the nearest row wanting attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevealEdge {
    Above,
    Below,
}

/// **The two edge pills**: how many rows wanting attention a [`ScrollRegion`]
/// it is pinned in has out of sight above and below, and a click that reveals the nearest.
pub struct EdgeHint {
    base: Base,
    above: Count,
    below: Count,
    request: Rc<Cell<Option<RevealEdge>>>,
}

/// One edge's count: the number (what hides the pill at zero) and the words its label shows,
/// both written where the region's facts arrive.
#[derive(Clone, Copy)]
struct Count {
    n: Signal<usize>,
    text: Signal<String>,
}

impl Count {
    fn new() -> Self {
        Self {
            n: signal(0),
            text: signal(String::new()),
        }
    }

    fn set(&self, n: usize) {
        if self.n.get_untracked() != n {
            self.n.set(n);
            self.text.set(n.to_string());
        }
    }
}

impl EdgeHint {
    pub fn new() -> Self {
        let (above, below) = (Count::new(), Count::new());
        let request = Rc::new(Cell::new(None));
        let pill = |count: Count, edge: RevealEdge, glyph: Glyph| {
            let request = request.clone();
            Flex::row().justify("center").child(
                Button::empty()
                    .size(WidgetSize::Small)
                    .icon(glyph)
                    .child(Label::new("").with_text_signal(count.text))
                    .on_click(move || {
                        request.set(Some(edge));
                        crate::request_frame();
                    })
                    .hidden(move || count.n.get() == 0),
            )
        };
        let mut base = Base::new();
        // **Passes the pointer through where it draws nothing**, so the rows under it stay clickable.
        base.surface = true;
        base.children.push(
            Grid::new()
                .template_row("auto 1fr auto")
                .width(Length::FULL)
                .height(Length::FULL)
                .padding(Spacing::Xs)
                .child(pill(above, RevealEdge::Above, Glyph::CaretUp))
                .child(Flex::empty())
                .child(pill(below, RevealEdge::Below, Glyph::CaretDown))
                .into(),
        );
        Self {
            base,
            above,
            below,
            request,
        }
    }
}

impl Default for EdgeHint {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for EdgeHint {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn paint(&self, cx: &mut crate::PaintCx) {
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }
    fn scroll_facts(&mut self, facts: ScrollFacts) {
        self.above.set(facts.attention_above);
        self.below.set(facts.attention_below);
    }
    fn take_reveal(&mut self) -> Option<RevealEdge> {
        self.request.take()
    }
}
