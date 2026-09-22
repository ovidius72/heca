//! [`ButtonGroup`] — a row of related actions that fits the space it is given.
//!
//! A toolbar is not a `Flex` of buttons, because a `Flex` has no answer for the moment the room
//! runs out. Left alone, a row of icon buttons is squashed to slivers; told not to shrink, it
//! overflows its pane. Neither is a design — both are the layout doing the only thing it was asked.
//!
//! So this widget owns that question. It holds `Button` children and, as the space narrows, gives
//! things up in the order that costs least:
//!
//! 1. **the words** — the buttons become their icons, keeping their labels for the hover bubble;
//! 2. **the buttons themselves** — those that no longer fit move into a menu behind a single
//!    trailing `⋮`, where their labels are read again.
//!
//! **Children are `Button`s, and that is deliberate.** Every `Button` constructor takes its text,
//! so a button in a group cannot be built without words — which is what makes a collapsed row
//! readable, with nothing required of the author and no runtime check to forget. It is the same
//! reason [`Select`](super::Select) types its options to [`Choice`](super::Choice).
//!
//! **A collapsed row runs the button's own click**, through [`Component::activate`] — the one entry
//! every way of pressing a button already goes through. Not a copy of the handler, and not a second
//! handler on the group: the same one, so the visible button and the menu row cannot drift.

use crate::builders::{ComponentExt, LayoutExt, StyleExt};
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::SignalGet;
use crate::style::{Align, Direction, Justify};
use crate::widgets::button::ButtonVariant;
use crate::widgets::{Button, ContextMenu, Glyph, Menu, MenuItem};
use std::cell::Cell;
use std::rc::Rc;

/// **How much of each button is shown**, before the group has to give any of them up.
///
/// It governs stages 1 and 2 only. Collapsing into the menu still happens whenever the buttons
/// genuinely do not fit, whichever display is pinned — otherwise pinning [`Full`](Display::Full)
/// would bring back the squashing this widget exists to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum Display {
    /// Words while there is room for them, icons once there is not.
    ///
    /// ⚠️ **Not settled.** Taking the words off makes the row narrower, so the row then fits — and
    /// "it fits" is the condition for putting them back. Reading how far the row reached when it
    /// last had its words is meant to close that, and does not yet: at some widths the group still
    /// alternates between the two on successive layouts. Use [`IconOnly`](Self::IconOnly) or
    /// [`Full`](Self::Full) until it does.
    Auto,
    /// Always icons, however much room there is. The labels are still carried — they are what the
    /// hover bubble and the collapsed menu say. **The default**, because it is the one that is
    /// settled.
    #[default]
    IconOnly,
    /// Always words. The group will collapse into the menu sooner, because each button is wider.
    Full,
}

/// Extra room a button must have before the group takes it back — hysteresis, so the two
/// thresholds cannot meet and dragging across one width cannot strobe.
const STICKY_MARGIN: f64 = 12.0;

/// What the ⋮ is called — on hover, and as the title of the menu it opens.
const OVERFLOW_LABEL: &str = "More actions";

/// **Nothing this group holds gives way** — every button it arranges, including its own ⋮.
///
/// Shrinking a button has no answer: once it is an icon there is no label left to ellipse, so the
/// layout takes the room out of the box itself and the control becomes a sliver. Giving way is
/// precisely what this widget exists to replace — it hides what does not fit instead.
///
/// ⚠️ **It has to be every child, not just the ones an author adds.** The ⋮ was left shrinkable, and
/// so was the *only* thing in the row that could give: with a title beside the group competing for
/// the room, the layout squeezed the ⋮ rather than pushing a button out of the box — so the group
/// never saw an overflow, never moved anything into the menu, and the ⋮ absorbed the entire
/// shortfall. Measured at four to eight pixels wide beside a twenty-four pixel sibling, and barely
/// clickable (Antonio, driving, 2026-09-04). Stated here once so a future child cannot miss it.
fn never_gives_way(button: Button) -> Button {
    button.shrink(0.0)
}

/// A row of related actions that fits the space it is given.
pub struct ButtonGroup {
    base: Base,
    display: Display,
    variant: Option<ButtonVariant>,
    /// How many leading children are currently shown as buttons. The rest are in the menu.
    shown: Cell<usize>,
    /// **How wide the row reached when it last had its words** — the right edge the layout gave its
    /// last button, not a width this widget worked out.
    ///
    /// It is what stops the words flickering. Taking them off makes the row narrower, so the row
    /// then fits — and "it fits" is the condition for putting them back, which makes it too wide
    /// again. The question has to be *"is there room for the words"*, and the only honest answer is
    /// how much room they took when they were last shown.
    wanted_with_words: Cell<f64>,
    /// Whether the decision has been put into the tree at least once. Without it a group whose
    /// first decision happens to match its initial state never applies one, and the buttons keep
    /// the words the mode says they should not have.
    applied: Cell<bool>,
    /// Whether the buttons are currently showing their words. Held rather than recomputed at paint,
    /// because the widths measured this pass only mean something alongside the mode that produced
    /// them.
    words: Cell<bool>,
    /// Labels and glyphs, kept beside the children so the menu can be built without taking them
    /// apart. Read from each `Button` as it is added.
    entries: Vec<Entry>,
    /// **The menu of whatever did not fit**, refreshed when the arrangement changes and read by the
    /// ⋮ when it is opened — so the ⋮ itself is built once and never replaced.
    menu: Rc<std::cell::RefCell<ContextMenu>>,
}

struct Entry {
    label: String,
    glyph: Option<Glyph>,
    /// **The button's own click.** Held here so a menu row runs the same closure the button does,
    /// with nothing queued and nothing to drain.
    click: Option<Rc<dyn Fn()>>,
}

#[heca_grid_ui_macros::props]
impl ButtonGroup {
    /// An empty group.
    pub fn new() -> Self {
        let mut base = Base::new();
        // **It arranges with a `Flex`, like anything else would.** A widget that sets direction,
        // align and justify on its own base has quietly re-implemented a row — and then owns every
        // question a row already answers (gap tokens, alignment, wrapping). The group's job is
        // deciding *what* is in the row; `Flex` decides where those things sit.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        // **It takes the room that is left, and puts its buttons at the end of it.**
        //
        // Growing is what makes the room a *fact of the parent* rather than a result of the group's
        // own decision: a group that hugs its content is as wide as whatever it decided to show, so
        // asking it how much room there is returns the answer it just produced — hide a button and
        // the room shrinks, which is the reading that hid it.
        //
        // And it costs nothing in a header, because the buttons are aligned to the **end** of that
        // room: a title at the left and the actions hard against the right is what a growing group
        // with `Justify::End` already looks like, without the parent needing `SpaceBetween` to
        // arrange them.
        base.style.layout.flex_grow = 1.0;
        // **It stretches to the height it is given and centres its buttons inside that**, rather
        // than being a short box the parent has to place vertically. Placing it was where the
        // trouble was: with a sibling in the row, the group ended up hanging below the strip, and
        // its buttons — and the picker letters stamped over them — went with it. Filling removes
        // the question instead of answering it.
        // **Its buttons sit at the end of that room.** With the room being the group's own box, an
        // end-justified row needs no second box inside it — and a second box was actively harmful:
        // nested inside a grown parent it was placed against a height that was not the one it ended
        // up with, so the whole cluster hung below the strip and took the picker's letters with it
        // (Antonio, driving, 2026-09-03).
        base.style.layout.justify = Justify::End;
        let mut g = Self {
            base,
            display: Display::default(),
            variant: None,
            shown: Cell::new(usize::MAX),
            wanted_with_words: Cell::new(0.0),
            applied: Cell::new(false),
            words: Cell::new(true),
            entries: Vec::new(),
            menu: Rc::new(std::cell::RefCell::new(
                ContextMenu::new("button-group-overflow").child(Menu::new(OVERFLOW_LABEL, "")),
            )),
        };
        // The ⋮ exists from the start, hidden until something needs it — never added mid-layout.
        g.build_trigger();
        g
    }

    /// **Add an action.** Its text is its label in the menu and its words on hover; its icon, if it
    /// has one, is what it shows once there is no room for words.
    #[heca_grid_ui_macros::host_only("a child — a description adds actions through `children`")]
    pub fn child(mut self, button: Button) -> Self {
        // Asked of the button, never read out of its content — what a button *is* must not depend
        // on how its content happens to be composed.
        self.entries.push(Entry {
            label: button.label(),
            glyph: button.glyph(),
            click: button.click_handler(),
        });
        // Its words are what it says on hover once it is showing only its icon, so the author
        // writes them once. An author who wants different words has already set a tooltip, and
        // that one stands.
        let button = if button.base().tooltip.is_none() {
            let label = button.label();
            button.tooltip(label)
        } else {
            button
        };
        // **The group's variant, through the one rule.** The widget decides what to keep and what
        // to take (`Component::set_variant`), so the group asks the same question of a child as it
        // does of its own ⋮ rather than testing a variant here: a button left at the default takes
        // the group's, and one that named its own keeps it whole — a destructive button in a quiet
        // row stays framed, because that is what the style says danger looks like.
        let mut button = never_gives_way(button);
        if let Some(v) = self.variant {
            Component::set_variant(&mut button, v);
        }
        // Before the ⋮, which is always the last child.
        let at = self.entries.len().saturating_sub(1);
        self.strip_mut()
            .base_mut()
            .children
            .insert(at, Box::new(button));
        // **Built in the mode it will be shown in.** Otherwise the first layout is of buttons with
        // their words, whatever the mode says, and the first decision is made from an arrangement
        // that was never going to be drawn — which is how a group pinned to icons collapsed itself
        // on the strength of widths it would never have (Antonio, driving, 2026-09-03).
        let icons = !self.words.get();
        // **The button just added — not the last child**, which is the ⋮ and always will be. Aimed
        // at the wrong one, every button kept its words whatever the mode said, and the first
        // decision was then made from widths that were never going to be drawn.
        if let Some(added) = self.strip_mut().base_mut().children.get_mut(at) {
            added.set_icon_only(icons);
        }
        self
    }

    /// **The variant the buttons in this group take** — declared once here rather than repeated on
    /// each child, because a group is one control and a row of buttons at different variants is not
    /// something anyone wants by accident.
    ///
    /// **A button that named its own variant keeps it.** The group's variant is what a child gets
    /// when it was left at the default, which is what lets a toolbar be uniformly quiet while its
    /// close button still reads as destructive — without either fact being written twice.
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = Some(variant);
        // **The ⋮ takes it too.** It is built in the constructor — before the group has been told
        // anything — so setting the variant only on later children left it the odd one out: a
        // `Primary` button among ghosts, drawing its glyph in the accent, which a pane dims when it
        // is not the active one. Its dots were invisible on every inactive pane (Antonio, driving,
        // 2026-09-03).
        let count = self.entries.len();
        if let Some(t) = self.base.children.get_mut(count) {
            t.set_variant(variant);
        }
        self
    }

    /// Whether the buttons show their words (default [`Display::Auto`]).
    #[heca_grid_ui_macros::prop]
    pub fn display(mut self, display: Display) -> Self {
        self.display = display;
        // **Start in the mode that was asked for**, so a group pinned to icons never renders its
        // words even once. Starting in the other one and correcting on the next pass is a visible
        // flash on the first frame, and a second one on every rebuild.
        let words = display != Display::IconOnly;
        self.words.set(words);
        self.set_words(words);
        self
    }

    /// How many of the children are buttons rather than menu rows, right now.
    pub fn shown_count(&self) -> usize {
        self.shown.get().min(self.entries.len())
    }

    /// Whether anything has been pushed into the menu.
    pub fn is_collapsed(&self) -> bool {
        self.shown_count() < self.entries.len()
    }
}

impl Default for ButtonGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ButtonGroup {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        cx.paint_base(&self.base);
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }

    /// **What fits is what the layout placed inside the box.**
    ///
    /// Nothing here measures a child or works out a budget. The buttons are laid out at their own
    /// size inside a box the parent shrinks, so anything that does not fit is simply *placed past
    /// the edge* — and that is read, not computed. It is the same thing the pane header does for
    /// its own height: ask the finished layout, never arithmetic standing in for it.
    fn on_layout(&mut self) {
        let count = self.entries.len();
        if count == 0 {
            return;
        }
        // The left edge of the box the parent gave this group.
        let left = self.base.bounds.loc.x;

        // **Words first, and from the same question**: with words on, does anything land outside?
        // A button with no icon has nothing to fall back to, so its words stay whatever the room.
        // **What does not fit is placed outside the box, and that is what is read.**
        //
        // The buttons sit at the *end* of the room, so anything too wide for it spills off the
        // **left** — the same way an end-aligned row overflows in a browser. Counting them is the
        // whole of the decision: no widths are added up and no budget is worked out.
        let outside = |group: &Self| -> usize {
            group
                .strip()
                .base()
                .children
                .iter()
                .filter(|c| !c.base().style.layout.hidden && c.base().bounds.size.w > 0.0)
                .filter(|c| c.base().bounds.loc.x < left - EPS)
                .count()
        };
        // While the words are up, remember how far the row reached — read off the last button the
        // layout placed, so it is the layout's answer and not a sum of parts.
        if self.words.get() {
            let shown_kids: Vec<&Box<dyn Component>> = self
                .strip()
                .base()
                .children
                .iter()
                .filter(|c| !c.base().style.layout.hidden && c.base().bounds.size.w > 0.0)
                .collect();
            if let (Some(first), Some(last)) = (shown_kids.first(), shown_kids.last()) {
                // From where the layout started the row to where it ended it — the span it took,
                // read off both ends. Not the group's own box: end-aligned, the row's right edge is
                // the box's whatever the content, so the box says nothing about what was wanted.
                let span = (last.base().bounds.loc.x + last.base().bounds.size.w)
                    - first.base().bounds.loc.x;
                self.wanted_with_words.set(span);
            }
        }

        let words = match self.display {
            Display::Full => true,
            Display::IconOnly => false,
            Display::Auto => {
                if self.entries.iter().any(|e| e.glyph.is_none()) {
                    // Nothing to fall back to; hiding the words would leave empty boxes.
                    true
                } else if self.words.get() {
                    outside(self) == 0
                } else {
                    // Back on only when there is room for what they took last time — not merely
                    // because the row fits without them, which it always does.
                    let room = self.base.bounds.size.w;
                    self.wanted_with_words.get() > 0.0 && room >= self.wanted_with_words.get()
                }
            }
        };
        if words != self.words.get() {
            self.words.set(words);
            self.set_words(words);
            self.base.mark_needs_paint();
            // **Ask for the pass this decision needs, and get it before anything is painted.** The
            // engine settles a layout that prompted a change rather than leaving it to the next
            // frame, so what appears is the arrangement decided here — never the one it replaced.
            self.base.mark_needs_layout();
            // The widths have changed, so what fits must be read from the layout that follows —
            // deciding it now would be deciding it from the arrangement being replaced.
            return;
        }

        // **However many are outside, that many give way** — taken from the trailing end, so the row
        // keeps its leading actions and the menu takes the rest. They are all buttons in one row, so
        // dropping the last N frees exactly the room the first N were overflowing by; the count
        // comes from the layout and nothing here adds a width to another.
        let shown_now = self.shown.get().min(count);
        let over = outside(self);
        let mut fits = shown_now.saturating_sub(over);
        // The ⋮ appears the moment anything is in the menu, and it needs room of its own — so the
        // first time it does, one more gives way to it.
        if over > 0 && shown_now == count {
            fits = fits.saturating_sub(1);
        }
        // **And it takes them back when the room returns.** Giving way was one-way: a group that had
        // collapsed stayed collapsed however wide the pane grew afterwards (Antonio, driving,
        // 2026-09-03).
        //
        // One at a time, and only against **visible slack** — the gap the layout left between the
        // group's edge and the first thing in it. Asking for a button back costs more room than
        // giving one up released, so the two thresholds cannot meet and a drag across the boundary
        // cannot strobe.
        if over == 0 && shown_now < count {
            let first = self
                .strip()
                .base()
                .children
                .iter()
                .find(|c| !c.base().style.layout.hidden && c.base().bounds.size.w > 0.0)
                .map(|c| c.base().bounds.loc.x)
                .unwrap_or(left);
            let slack = first - left;
            let widest = self
                .strip()
                .base()
                .children
                .iter()
                .filter(|c| !c.base().style.layout.hidden)
                .map(|c| c.base().bounds.size.w)
                .fold(0.0_f64, f64::max);
            if widest > 0.0 && slack > widest + STICKY_MARGIN {
                fits += 1;
            }
        }

        if fits != shown_now || !self.applied.get() {
            self.applied.set(true);
            self.shown.set(fits);
            self.apply();
            self.base.mark_needs_paint();
            // The row now holds different buttons, so it must be placed again before it is drawn —
            // the engine settles it in this same pass. Without it the row was painted once with the
            // arrangement this line just replaced, which is the flash a caller kept seeing on every
            // rebuild and could do nothing about.
            self.base.mark_needs_layout();
        }
    }
}

/// Half a pixel: a right edge exactly on the boundary is inside it, and rounding must not read as
/// overflow.
const EPS: f64 = 0.5;

impl ButtonGroup {
    /// The group **is** the strip that arranges the buttons — they are its own children.
    ///
    /// Named `strip` rather than `row`: `row` is CSS's `grid-row` on every widget
    /// ([`LayoutExt::row`]), and a private accessor should not take a word the whole library uses.
    fn strip(&self) -> &dyn Component {
        self
    }
    fn strip_mut(&mut self) -> &mut dyn Component {
        self
    }

    /// **Put the decision into the tree**: which buttons are buttons, whether they show their
    /// words, and whether the ⋮ is there.
    ///
    /// One function, called from the one place the decision is made, so the tree can never be a
    /// frame out of step with `shown`.
    fn apply(&mut self) {
        let count = self.entries.len();
        let shown = self.shown.get().min(count);
        let words = match self.display {
            Display::Full => true,
            Display::IconOnly => false,
            // A button with no icon has nothing to fall back to, so words stay: dropping them
            // would leave an empty box.
            Display::Auto => shown == count && self.entries.iter().all(|e| e.glyph.is_some()),
        };
        for i in 0..count {
            let Some(child) = self.strip_mut().base_mut().children.get_mut(i) else {
                continue;
            };
            // A collapsed button is `hidden`, not merely invisible: the layout must give it no
            // space at all, or the row it left would still be as wide as it was.
            child.base_mut().set_hidden(i >= shown);
        }
        self.set_words(words);
        self.set_trigger(shown < count);
    }

    /// **Tell each button whether it is showing its words.** The group decides *what* is shown; the
    /// button owns *how it looks* when it is — including that a button with no words is square,
    /// which is not a number this widget should be computing.
    fn set_words(&mut self, words: bool) {
        let count = self.entries.len();
        for i in 0..count {
            let Some(child) = self.strip_mut().base_mut().children.get_mut(i) else {
                continue;
            };
            child.set_icon_only(!words);
        }
    }

    /// Add, update or drop the trailing ⋮ that opens the menu of what did not fit.
    /// **The ⋮, built once — at construction, before anything is laid out.**
    ///
    /// ⚠️ Not created while deciding what fits: that happens *inside* the layout walk, so a widget
    /// added there has no bounds yet and the frame that paints next draws it at the window's origin
    /// — a small thing flickering in the corner of the scrolling area whenever the header was
    /// rebuilt, which a resize does continuously (Antonio, driving, 2026-09-03).
    fn build_trigger(&mut self) {
        // ⚠️ **Built once, then shown or hidden.** Making a fresh one each time the arrangement
        // changed put a brand-new widget into the tree with no bounds yet, and the frame that
        // painted next drew it at the window's origin — a small thing flickering in the corner
        // of the scrolling area while a divider was dragged (Antonio, driving, 2026-09-03).
        //
        // **A `Button`, and icon-only like its siblings.** It was an `IconButton` — a different
        // control with its own padding — and then a `Button` that still carried the horizontal
        // room a label needs, so it was wider than the buttons beside it and its picker letter
        // was placed as though it were a large target: above the ⋮ while every other letter sat
        // below its button. A group's own affordance has to be one of the things the group
        // arranges, on the same terms.
        let menu = self.menu.clone();
        // **The group's own affordance is one of the things it arranges, on the same terms** — so
        // it goes through the same rule its children do and never gives way either.
        let trigger = never_gives_way(
            Button::empty()
                .icon(Glyph::DotsThreeVertical)
                .variant(self.variant.unwrap_or_default())
                .icon_only(true)
                .tooltip(OVERFLOW_LABEL),
        );
        // **The anchor comes out of the gesture, never chosen by an author** — the event carries
        // where it happened, and the menu reads it from there.
        let trigger = ComponentExt::on_click(trigger, {
            let menu = menu.clone();
            move |ev: &mut crate::event::EventCx<'_>| menu.borrow().show(ev.event())
        });
        // Nothing is declared about picking it. It is a button: a letter reaches it for that
        // reason alone, and picking it runs this same click.
        self.base.children.push(Box::new(trigger));
    }

    fn set_trigger(&mut self, needed: bool) {
        let count = self.entries.len();
        // **Whatever went into the menu, the menu says so** — refreshed here rather than rebuilt
        // with the ⋮, so the widget itself is only ever made once.
        *self.menu.borrow_mut() = self.overflow_menu();
        if let Some(t) = self.strip_mut().base_mut().children.get_mut(count) {
            t.base_mut().set_hidden(!needed);
        }
    }

    /// The menu of everything that did not fit — each row the button's own label, its own icon, and
    /// its own click.
    fn overflow_menu(&self) -> ContextMenu {
        let shown = self.shown.get().min(self.entries.len());
        let mut menu = Menu::new(OVERFLOW_LABEL, "Actions that did not fit here");
        for entry in self.entries.iter().skip(shown) {
            let run = entry.click.clone();
            let mut item = MenuItem::new()
                .label(entry.label.clone())
                .on_click(move || {
                    // **The button's own closure, run here.** It used to write down which button was
                    // chosen and wait for the group to drain it on its next event — but the menu is in a
                    // layer, so no event reaches the group when a row is clicked, and the action sat
                    // there for seconds until something unrelated wandered past (Antonio, 2026-09-03).
                    if let Some(run) = run.as_ref() {
                        run();
                    }
                });
            if let Some(g) = entry.glyph {
                item = item.icon(g);
            }
            menu = menu.child(item);
        }
        ContextMenu::new("button-group-overflow").child(menu)
    }
}

impl LayoutExt for ButtonGroup {}
impl StyleExt for ButtonGroup {}
