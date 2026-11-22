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

use crate::FlashHandle;
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
    /// The tone the nearest of those above asks in — what a hint flashes with.
    pub tone_above: Option<Tone>,
    /// The tone the nearest of those below asks in.
    pub tone_below: Option<Tone>,
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

/// One edge's count: the number (what hides the pill at zero), the words its label shows, and the
/// flash it repeats in the tone of the row it stands for — all written where the region's facts
/// arrive.
#[derive(Clone, Copy)]
struct Count {
    n: Signal<usize>,
    text: Signal<String>,
    tone: Signal<Option<Tone>>,
    flash: FlashHandle,
}

impl Count {
    fn new() -> Self {
        Self {
            n: signal(0),
            text: signal(String::new()),
            tone: signal(None),
            flash: FlashHandle::new(),
        }
    }

    fn set(&self, n: usize, tone: Option<Tone>) {
        if self.n.get_untracked() != n {
            self.n.set(n);
            self.text.set(n.to_string());
        }
        if self.tone.get_untracked() != tone {
            self.tone.set(tone);
            match tone {
                Some(tone) => self.flash.repeat(tone),
                None => self.flash.clear(),
            }
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
                    .flash(count.flash)
                    .child(Label::new("").with_text_signal(count.text))
                    .on_click(move |_| {
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
        self.above.set(facts.attention_above, facts.tone_above);
        self.below.set(facts.attention_below, facts.tone_below);
    }
    fn take_reveal(&mut self) -> Option<RevealEdge> {
        self.request.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A pill flashes in the tone of the row it stands for**, and stops when nothing on its side
    /// asks any more.
    #[test]
    fn a_pill_repeats_the_tone_of_the_nearest_row_and_stops_with_it() {
        let mut hint = EdgeHint::new();
        let asking = |count: &Count| {
            crate::flash::attention(&{
                let mut base = Base::new();
                base.flash = Some(count.flash);
                base
            })
        };
        hint.scroll_facts(ScrollFacts {
            attention_below: 2,
            tone_below: Some(Tone::Warning),
            ..ScrollFacts::default()
        });
        assert_eq!(asking(&hint.below), Some(Tone::Warning));
        assert_eq!(asking(&hint.above), None, "nothing above: no flash");
        hint.scroll_facts(ScrollFacts::default());
        assert_eq!(asking(&hint.below), None, "nothing asks any more: it stops");
    }
}
