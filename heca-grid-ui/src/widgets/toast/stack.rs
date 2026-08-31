//! [`ToastStack`] — an overlay that arranges a host-supplied set of notifications
//! into a corner stack.
//!
//! **Presentation only**, like every grid-ui widget: it does *not* own the
//! notification queue, lifetimes, auto-dismiss timers, or dedup — that business
//! logic is the **host application's** job. The host owns a
//! [`Signal<Vec<ToastSpec>>`](crate::reactive::Signal) (its render list); the
//! stack reflects it, **reconciling its cards by id** (so each keeps its hover,
//! flash and focus state and is never rebuilt), and reports interactions back via
//! [`on_dismiss(id)`](ToastStack::on_dismiss) / [`on_action(id)`](ToastStack::on_action).
//! The host then removes the id from its list (which reflows the rest).
//!
//! # The cards are real children
//!
//! They live in `base.children`, and **the engine lays them out**: the stack is a
//! viewport-sized box whose `justify`/`align` come from its corner, with its gap between the
//! cards and its margin as padding. Nothing here measures, positions, hit-tests or routes.
//!
//! It kept them outside the tree until F003/P096/T486, in a private list positioned by hand,
//! and three things followed from that — none of them fixable inside the card:
//!
//! - **No pick letters, ever.** The picker walks the laid-out tree, so a card that was not in it
//!   could not be lettered, and a notification's action was reachable by mouse alone.
//! - **No hover inside a card.** The framework marks hover along the hit-test target's ancestor
//!   chain; a hand-delivered move marks nothing, so a stacked card's action and × stayed dark
//!   under the pointer while an inline card's lit correctly.
//! - **Every layout and occlusion rule was a second copy**, and one had already drifted: the
//!   stack wrote a rect straight into the card's bounds, which placed the card and nothing
//!   inside it.
//!
//! Input contract, unchanged: it reports [`overlay_active`](Component::overlay_active) while it
//! has cards, so a host routes to it first; it **claims** what lands on a card — a click or a
//! move, because a card occludes the points it covers — and passes everything else through
//! (`Handled::No`), so cards block only the strip of UI they actually cover.

use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, PaintCx};
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::style::{Align, Direction, Justify, Length};
use crate::widgets::{Button, Toast, ToastPosition, ToastSpec};
use heca_core::layout::Point;
use std::rc::Rc;

/// Gap between stacked toasts.
const DEFAULT_GAP: f32 = 10.0;
/// Inset from the viewport edges.
const DEFAULT_MARGIN: f32 = 16.0;

/// One card's bookkeeping — its identity, and whether it is on its way out.
///
/// Parallel to `base.children` by index. It is two facts and not a widget handle: the card itself
/// is in the tree, where the engine, the pointer and the picker can all reach it.
struct Slot {
    id: u64,
    /// Its id has gone from the host's list and its exit is playing. It keeps its place until the
    /// gesture has finished, which is what gives the exit something to play over.
    leaving: bool,
}

/// Whether a card is still on screen — open, or dismissed and still leaving.
///
/// Asked of the component, not of a `Toast`: the answer lives in the [`Presence`] every animated
/// surface carries, so this works for whatever a future builder puts in the stack.
///
/// [`Presence`]: crate::animation::Presence
fn showing(card: &dyn Component) -> bool {
    card.presence()
        .map(|p| p.is_open() || p.is_leaving())
        .unwrap_or(true)
}

/// An overlay that stacks host-supplied toasts in a corner. Presentation only.
pub struct ToastStack {
    base: Base,
    items: Signal<Vec<ToastSpec>>,
    position: ToastPosition,
    gap: f32,
    margin: f32,
    on_dismiss: Option<Rc<dyn Fn(u64)>>,
    on_action: Option<Rc<dyn Fn(u64, &str)>>,
    /// **Is the pointer resting on one of the cards** — see [`hovered_signal`](Self::hovered_signal).
    hovered: Option<Signal<bool>>,
    /// One per child, same index — see [`Slot`].
    slots: Vec<Slot>,
}

impl ToastStack {
    /// A new stack bound to the host's `items` signal (the render list it owns).
    pub fn new(items: Signal<Vec<ToastSpec>>) -> Self {
        let mut base = Base::new();
        // Fill the viewport and let the engine anchor the column — the same shape an `Overlay`
        // uses to centre a modal. `justify`/`align` are set from the corner in `sync_anchor`.
        base.style.layout.width = Length::Pct(1.0);
        base.style.layout.height = Length::Pct(1.0);
        base.style.layout.direction = Direction::Column;
        base.style.layout.padding = DEFAULT_MARGIN;
        base.style.layout.gap = DEFAULT_GAP;
        let mut stack = Self {
            base,
            items,
            position: ToastPosition::default(),
            gap: DEFAULT_GAP,
            margin: DEFAULT_MARGIN,
            on_dismiss: None,
            on_action: None,
            hovered: None,
            slots: Vec::new(),
        };
        stack.sync_anchor();
        stack
    }

    /// Where in the window the stack sits (default [`ToastPosition::TopRight`]).
    ///
    /// **One vocabulary for the card and the stack** — `ToastPosition` is the same enum a single
    /// [`Toast`] places itself with, so "top right" means the same thing said either way. It
    /// replaced a separate four-member `ToastCorner`, which said the same thing in a second
    /// spelling and could not express the centre positions the card already had.
    #[heca_grid_ui_macros::prop]
    pub fn position(mut self, position: ToastPosition) -> Self {
        self.position = position;
        self.sync_anchor();
        self
    }

    /// Gap between stacked toasts (logical px).
    #[heca_grid_ui_macros::prop]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self.base.style.layout.gap = gap;
        self
    }

    /// Inset from the viewport edges (logical px).
    #[heca_grid_ui_macros::prop]
    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self.base.style.layout.padding = margin;
        self
    }

    /// Called with the toast's id when its × is clicked. The host removes the id
    /// from its list (the stack reflows the rest).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn(u64) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self
    }

    /// Called with the toast's id **and the pressed action's key** when one of its actions is
    /// clicked. The key is the caller's own name for the act, straight from the spec — a card may
    /// offer several, so "which card" alone would not say what to do.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_action(mut self, f: impl Fn(u64, &str) + 'static) -> Self {
        self.on_action = Some(Rc::new(f));
        self
    }

    /// **Report whether the pointer is resting on a card**, into a signal the host owns.
    ///
    /// A stack is a place people *read* things and then reach for a button, so whoever owns the
    /// cards' lifetime usually wants to hold it still while the pointer is on one — otherwise a
    /// card can retire out from under the click that was aimed at it, which is exactly what a close
    /// button on a timed card invites.
    ///
    /// It is a **signal rather than a callback** because it is a state, not an act: the answer is
    /// true for as long as the pointer stays, whoever asks. The same shape
    /// [`open_when`](super::KeyHintGroup::open_when) reads, in the other direction.
    ///
    /// The stack does not decide what pausing means — it has no clock and no idea what a card's
    /// lifetime is. It reports the fact; the owner of the lifetime acts on it.
    ///
    /// **Hover is asked of the framework, never re-derived**: it is marked along the hit-test
    /// target's ancestor chain before a move is delivered, so this accounts for which card is on
    /// top and what is clipped away — neither of which a private `bounds.contains(pos)` could know.
    pub fn hovered_signal(mut self, hovered: Signal<bool>) -> Self {
        self.hovered = Some(hovered);
        self
    }

    /// Point the engine at the corner: which end of the column the cards pile against, and which
    /// side they hug. **This is the whole of the placement** — there is no arithmetic left.
    fn sync_anchor(&mut self) {
        let s = &mut self.base.style.layout;
        s.justify = match self.position.is_top() {
            true => Justify::Start,
            false => Justify::End,
        };
        s.align = if self.position.is_centered() {
            Align::Center
        } else if self.position.is_right() {
            Align::End
        } else {
            Align::Start
        };
    }

    /// Build a [`Toast`] from a spec, wiring its action/dismiss to the stack's
    /// id-tagged callbacks.
    ///
    /// **The stack does not choose how the action looks.** The spec carries the variant, so the
    /// developer raising a notification decides whether its action reads as primary, secondary or
    /// ghost — and one hardcoded face here cannot be wrong for every caller at once.
    fn build(&self, spec: &ToastSpec) -> Toast {
        let mut t = Toast::new(spec.title.clone()).severity(spec.severity);
        if let Some(g) = spec.icon {
            t = t.icon(g);
        }
        if let Some(b) = &spec.body {
            t = t.body_text(b.clone());
        }
        for action in &spec.actions {
            let cb = self.on_action.clone();
            let id = spec.id;
            let key = action.key.clone();
            t = t.action(
                Button::new(action.label.clone())
                    .variant(action.variant)
                    .on_click(move || {
                        if let Some(f) = &cb {
                            f(id, &key);
                        }
                    }),
            );
        }
        t = t.dismissible(spec.dismissible);
        if spec.dismissible {
            let cb = self.on_dismiss.clone();
            let id = spec.id;
            t = t.on_dismiss(move || {
                if let Some(f) = &cb {
                    f(id);
                }
            });
        }
        t
    }

    /// Reconcile the children against the host's list: ask a card whose id has gone to **leave**,
    /// drop it once its exit has played, build one for a spec that has none yet, and put the live
    /// cards back in the host's order.
    ///
    /// **A leaving card keeps its slot.** It is no longer in the host's list, so it has no place
    /// in that order — and moving it while it plays its exit would slide it sideways on the way
    /// out.
    fn reconcile(&mut self) {
        let specs = self.items.get_untracked();

        // 1. A card the host has dropped is asked to leave. Once, on the frame its id goes.
        for (slot, card) in self.slots.iter_mut().zip(self.base.children.iter_mut()) {
            if !slot.leaving && !specs.iter().any(|s| s.id == slot.id) {
                card.hide();
                slot.leaving = true;
            }
        }

        // 2. A card that has finished leaving is gone.
        self.drop_finished();

        // 3. A spec with no card yet gets one, closed, so its arrival plays.
        for spec in &specs {
            if !self.slots.iter().any(|s| s.id == spec.id) {
                let card = self.build(spec).opened(false);
                self.slots.push(Slot { id: spec.id, leaving: false });
                self.base.children.push(Box::new(card));
                // A new card has to be measured and placed before it can arrive.
                self.base.mark_needs_layout();
            }
        }

        // 4. The live cards take the host's order; the leaving ones hold the slots they are in.
        let mut order: Vec<usize> = (0..self.slots.len()).collect();
        let live: Vec<usize> = order.iter().copied().filter(|&i| !self.slots[i].leaving).collect();
        let mut wanted: Vec<usize> = live.clone();
        wanted.sort_by_key(|&i| {
            specs.iter().position(|s| s.id == self.slots[i].id).unwrap_or(usize::MAX)
        });
        for (at, from) in live.iter().zip(wanted) {
            order[*at] = from;
        }
        if order.iter().enumerate().any(|(at, from)| at != *from) {
            let mut slots: Vec<Option<Slot>> = self.slots.drain(..).map(Some).collect();
            let mut cards: Vec<Option<Box<dyn Component>>> =
                self.base.children.drain(..).map(Some).collect();
            for from in order {
                self.slots.push(slots[from].take().expect("each index is used once"));
                self.base
                    .children
                    .push(cards[from].take().expect("each index is used once"));
            }
        }

        // 5. Everything in the list is on screen. A card built closed opens on its first
        //    reconcile, which is what plays its arrival rather than cutting it in.
        for (slot, card) in self.slots.iter_mut().zip(self.base.children.iter_mut()) {
            if !slot.leaving && !showing(card.as_ref()) {
                card.open();
            }
        }
    }

    /// Drop the cards whose exit has played out.
    ///
    /// Called on both sides of the tick — before, so a card the host dropped on an earlier frame
    /// is gone, and after, so one whose gesture ended on *this* tick goes with it rather than
    /// lingering a frame.
    fn drop_finished(&mut self) {
        let mut i = 0;
        while i < self.slots.len() {
            if self.slots[i].leaving && !showing(self.base.children[i].as_ref()) {
                self.slots.remove(i);
                self.base.children.remove(i);
                // **The tree just changed shape, so the cards below must move up.** A repaint
                // cannot do it — they are still laid out around the card that has gone.
                self.base.mark_needs_layout();
            } else {
                i += 1;
            }
        }
    }

    /// Did the pointer land on one of the cards?
    ///
    /// **Asked of the framework, not re-derived.** Hover is marked along the target's ancestor
    /// chain before the move is delivered, so a card that answers `hovered` is the card the
    /// pointer is on — including which one is on top and what is clipped away, neither of which a
    /// private `bounds.contains(pos)` could know.
    fn on_a_card(&self) -> bool {
        self.base.children.iter().any(|c| c.base().hovered())
    }

    /// Publish the hover state, **only when it changes**.
    ///
    /// Writing every move would wake whatever reads the signal on every pixel of pointer travel,
    /// for an answer that is the same as it was.
    fn report_hover(&self, on_a_card: bool) {
        let Some(hovered) = self.hovered else { return };
        if hovered.get_untracked() != on_a_card {
            hovered.set(on_a_card);
        }
    }
}

impl Component for ToastStack {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Overlay-active while it has cards, so a host gives it input first — and passes everything
    /// through that misses them.
    fn overlay_active(&self) -> bool {
        !self.base.children.is_empty()
    }

    /// **Anchor to the window, not to the slot a parent happened to give.**
    ///
    /// A notification corner means the *screen's* corner. A host mounts its layers however it
    /// likes — the showcase stacks them in a column, where five viewport-sized siblings each get a
    /// fifth of the height — so a stack that trusted its own box put "top right" two thirds of the
    /// way down the window. It reads the viewport the layout pass stamped on every node
    /// ([`Base::viewport`]) and moves the finished group to the corner it belongs to.
    ///
    /// **The arrangement is still entirely the engine's**: the column, the gap, the margin and
    /// which end the cards pile against are all resolved before this runs. All that is left is
    /// which corner of the finished group meets which corner of the window — and bounds stay
    /// honest, because the cards move with it (`shift_subtree`), exactly as an `Overlay` places
    /// its panel.
    fn on_layout(&mut self) {
        let vp = self.base.viewport;
        if !vp.w.is_finite() || !vp.h.is_finite() || self.base.children.is_empty() {
            return;
        }
        let b = self.base.bounds;
        let dx = if self.position.is_centered() {
            (vp.w - b.size.w) / 2.0 - b.loc.x
        } else if self.position.is_right() {
            vp.w - (b.loc.x + b.size.w)
        } else {
            -b.loc.x
        };
        let dy = match self.position.is_top() {
            true => -b.loc.y,
            false => vp.h - (b.loc.y + b.size.h),
        };
        if dx != 0.0 || dy != 0.0 {
            crate::component::shift_subtree(self, dx, dy);
        }
    }

    /// A **card** occludes the points it covers (the stack draws above the page), even though the
    /// stack as a whole does not grab input: a host must not synthesize a page-level action (a
    /// context menu) under one. Points between and outside the cards are not occluded.
    ///
    /// It is the cards' own laid-out bounds, not a private layout pass — the engine placed them,
    /// and this reads where they landed.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.base
            .children
            .iter()
            .any(|c| showing(c.as_ref()) && c.base().bounds.contains(pos))
    }

    /// **A card claims the pointer over it, and the stack claims nothing else.**
    ///
    /// The cards are children, so the framework hit-tests them, hovers them and delivers to them.
    /// What is left for the stack is the one thing only it knows: that a point over a card belongs
    /// to the card and must not also reach the page behind. A move or a press that missed every
    /// card bubbles out unclaimed, which is how the UI underneath keeps working.
    fn on_event(&mut self, ev: &Event) -> Handled {
        match ev {
            Event::PointerMove(_) | Event::PointerDown(_) | Event::Click(_) => {
                let on_a_card = self.on_a_card();
                self.report_hover(on_a_card);
                match on_a_card {
                    true => Handled::Yes,
                    false => Handled::No,
                }
            }
            // **The pointer leaving is a hover ending, and no move reports it.** Moving off a card
            // onto empty space is covered above — `on_a_card` simply goes false — but the pointer
            // leaving the window *over* a card sends no further moves at all. Without this the
            // stack stays hovered for good, and whatever holds still for it never resumes.
            Event::PointerLeave(_) => {
                self.report_hover(self.on_a_card());
                Handled::No
            }
            _ => Handled::No,
        }
    }

    /// The cards paint on the scene's **overlay layer**, above the page, in tree order.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || self.base.children.is_empty() {
            return;
        }
        cx.with_overlay(|cx| {
            for card in self.base.children.iter() {
                crate::component::paint_child(card.as_ref(), cx);
            }
        });
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.reconcile();
        let mut animating = false;
        for card in self.base.children.iter_mut() {
            animating |= card.tick(dt);
        }
        self.drop_finished();
        animating
    }
}

impl LayoutExt for ToastStack {}
