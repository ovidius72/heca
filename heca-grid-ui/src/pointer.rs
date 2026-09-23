//! **The pointer stream, resolved once.**
//!
//! A host reports device facts: the pointer is here, a button went down, the wheel turned. Widgets
//! need something else entirely — *this widget was clicked*, *the pointer just left me*, *that was
//! the second click*, *a drag is passing over me*. Everything in between is this module.
//!
//! ```text
//! Event::Raw(RawPointer)
//!        │
//!        ├─ hit_test ─────────► the target, and the path of ancestors above it
//!        ├─ hover diff ───────► PointerEnter / PointerLeave
//!        ├─ press/release pair ► PointerDown / PointerUp / Click / DoubleClick / RightClick …
//!        ├─ drag threshold ───► DragStart / Drag / DragEnter / DragOver / DragLeave / Drop / DragEnd
//!        └─ delivery ─────────► capture down the path, then bubble back up
//! ```
//!
//! # Why the framework owns it
//!
//! It used to be every widget's job, so every widget did a piece of it, and each piece was a
//! private copy of the same four lines. Six widgets tested `bounds.contains(pos)` to know whether
//! they were hovered. [`Input`](crate::widgets::Input) kept its own clock to count clicks. A
//! scrollbar thumb tracked its own grab so a move outside its bounds still reached it. And nothing
//! could receive a right-click at all, because a press carried no button.
//!
//! None of that is a widget's business, and each copy is a place the rule can be wrong. It is one
//! rule; it lives here.
//!
//! # Where the state lives
//!
//! In the widgets, in [`PointerState`] on their [`Base`] — not in a router object the host has to
//! own and thread through. That is deliberate:
//!
//! - a host calls [`dispatch`](crate::component::dispatch) exactly as before, with no new type to
//!   create, hold, or hand around;
//! - **several trees never interfere** — heca mounts the chrome, every pane header, each viewport's
//!   widgets and every open layer as separate trees, and each carries its own hover and capture;
//! - a widget removed mid-gesture takes its own state with it, so a press whose widget is gone
//!   strands nothing.
//!
//! # Capture
//!
//! A widget that consumes a [`PointerDown`](Event::PointerDown) **captures the pointer**: every
//! move, and the release, come to it wherever the cursor goes, until the button is up. That is what
//! a scrollbar thumb, a slider and a rubber-band selection need, and it is why gating a release on
//! position welds a thumb to the cursor. The rule is here, once, instead of in each of them.

use crate::component::{Base, Component};
use crate::drag::DropSide;
use crate::event::{
    DragEvent, Event, EventKind, Handled, PointerButton, PointerEvent, RawPointer, RawPointerKind,
};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use heca_core::layout::{Point, Rectangle};
use std::cell::Cell;
use std::time::Instant;

/// Longest gap between two presses still counted as one click run (seconds).
///
/// The platform value on macOS and Windows alike is around 400–500 ms; the library picks one
/// rather than asking the host, because a double click that means different things in two trees of
/// the same app is not a setting anyone wants.
const MULTI_CLICK_SECS: f32 = 0.4;

/// How far the pointer may move between two presses and still continue a click run (logical px).
/// Past it the user is pointing at something else, however fast they were.
const MULTI_CLICK_SLOP: f64 = 4.0;

/// How far the pointer must travel while held before a press becomes a drag (logical px).
///
/// Same 8 px the app's own sidebar drag used, kept so a drag begins at the same distance wherever
/// it starts.
const DRAG_THRESHOLD: f64 = 8.0;

/// The pointer state the router keeps **on each widget**: hover, capture, the live press and its
/// click run, and whether a drag is in flight.
///
/// Every field is interior-mutable so the router can update it while it holds the tree immutably
/// for hit-testing. A widget reads [`hovered`](Self::hovered); the rest is the framework's.
#[derive(Debug)]
pub struct PointerState {
    /// `true` while the pointer is over this widget **or a descendant** — the CSS `:hover` rule,
    /// so a control is hovered when the label inside it is.
    ///
    /// A signal, not a flag, because widgets bind it: a
    /// [`Row`](crate::widgets::Row) hands it to a list's cursor, a
    /// [`Button`](crate::widgets::Button) eases its fill toward it.
    pub hovered: Signal<bool>,
    /// **When the pointer arrived**, or `None` while it is elsewhere. Stamped on the same
    /// transition that sets [`hovered`](Self::hovered), so the two cannot disagree.
    ///
    /// It exists because "how long has this been hovered" is not a question any one widget should
    /// answer for itself — a tooltip's reveal delay reads it, and before this the tooltip wrapper
    /// kept a private clock started from its own capture handler, which is the same shape as the
    /// six widgets that each tested `bounds.contains(pos)` before this module existed.
    hovered_since: Cell<Option<Instant>>,
    /// Set on the widget that consumed a [`PointerDown`](Event::PointerDown): moves and the
    /// release route here until the button comes up.
    capture: Cell<bool>,
    /// The live press this widget is the target of: where it landed, and with which button.
    press: Cell<Option<(Point, PointerButton)>>,
    /// The click run: when the last press landed and how many it made. Kept **after** the release,
    /// which is what makes the next press a double click.
    run: Cell<Option<(Instant, u32)>>,
    /// `true` while this widget is the source of a drag in flight.
    dragging: Cell<bool>,
    /// `true` while a drag in flight is over this drop target.
    drag_over: Cell<bool>,
    /// Where in this target the drag currently sits — before it, onto it, or after it — so the
    /// widget can draw the insertion line without measuring anything.
    drag_side: Cell<DropSide>,
    /// **Where the pointer is, while this widget is the source of a drag.** The one thing the
    /// picture that follows the cursor needs and layout cannot give: the source is still laid out
    /// where it was, and what follows the cursor is drawn somewhere else entirely.
    drag_pos: Cell<Point>,
}

impl PointerState {
    /// Fresh state: not hovered, not captured, no press, no drag.
    pub fn new() -> Self {
        Self {
            hovered: signal(false),
            hovered_since: Cell::new(None),
            capture: Cell::new(false),
            press: Cell::new(None),
            run: Cell::new(None),
            dragging: Cell::new(false),
            drag_over: Cell::new(false),
            drag_side: Cell::new(DropSide::Onto),
            drag_pos: Cell::new(Point::new(0.0, 0.0)),
        }
    }

    /// Whether the pointer is over this widget or a descendant.
    pub fn is_hovered(&self) -> bool {
        self.hovered.get_untracked()
    }

    /// **How long the pointer has rested here**, in seconds, or `None` if it is not hovering.
    ///
    /// The one clock, read by anything whose behaviour is "after the pointer has been still for a
    /// while" — the tooltip reveal today.
    pub fn hovered_for(&self) -> Option<f32> {
        self.hovered_since
            .get()
            .map(|since| since.elapsed().as_secs_f32())
    }

    /// Start or clear the hover clock. Called by the hover walk on the same transition that sets
    /// [`hovered`](Self::hovered) — never by a widget.
    pub(crate) fn set_hovered_since(&self, now: Option<Instant>) {
        self.hovered_since.set(now);
    }

    /// Whether this widget is dragging (it is the source of a drag in flight).
    pub fn is_dragging(&self) -> bool {
        self.dragging.get()
    }

    /// Whether a drag in flight is currently over this drop target.
    pub fn is_drag_over(&self) -> bool {
        self.drag_over.get()
    }

    /// Where in this target the drag sits — meaningful only while
    /// [`is_drag_over`](Self::is_drag_over).
    pub fn drag_side(&self) -> DropSide {
        self.drag_side.get()
    }

    /// Where the pointer is — meaningful only while [`is_dragging`](Self::is_dragging).
    pub fn drag_pos(&self) -> Point {
        self.drag_pos.get()
    }
}

impl Default for PointerState {
    fn default() -> Self {
        Self::new()
    }
}

/// A path from a root to one node: the index of the child to take at each level. `[]` is the root
/// itself.
type Path = Vec<usize>;

/// Resolve `raw` against `root` and deliver whatever it means. The entry point
/// [`dispatch`](crate::component::dispatch) calls for every [`Event::Raw`].
///
/// Returns [`Handled::Yes`] if any widget consumed any of the events it produced — which is what a
/// host reads to decide whether the input also belongs to whatever sits behind the tree (in heca,
/// the terminal).
pub fn route(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    // **The framework fills in what is held down; the caller never does.** Modifiers are device
    // state, not a property of this event, and a host has more than one place it feeds a tree from
    // — so "attach the modifiers here" is a rule some call site always forgets, silently, because
    // "nothing held" is indistinguishable from "nobody asked". Read from the one place that is
    // told (F003/P097/T496).
    let raw = &RawPointer {
        modifiers: crate::event::modifiers(),
        ..*raw
    };
    match raw.kind {
        RawPointerKind::Moved => route_move(root, raw),
        RawPointerKind::Pressed => route_press(root, raw),
        RawPointerKind::Released => route_release(root, raw),
        RawPointerKind::Wheel => route_wheel(root, raw),
        RawPointerKind::Cancelled => {
            cancel(root, raw);
            Handled::No
        }
    }
}

/// The pointer moved: hover transitions first (so a widget's state is current before it is asked
/// to do anything), then the move itself, then whatever the drag machinery makes of it.
fn route_move(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let target = hit_test(root, raw.pos);
    // **A drag owns the pointer, so nothing hovers under it.** Hover says "the pointer is on you
    // and a click would land here", which is false mid-drag: what a release will do is decided by
    // the drop rules, not by what happens to be beneath the cursor. Leaving it on lit a pane card
    // as a column was dragged across it, which reads as "you may drop here" — and the drop then
    // went somewhere else entirely (Antonio, driving, 2026-09-01). Passing `None` also clears
    // whatever was lit when the drag began. The drop mark stays: that one is drawn from the
    // resolved target and is the only truthful feedback while a drag is in flight.
    let hover = match dragging_path(root).is_some() {
        true => None,
        false => target.as_deref(),
    };
    update_hover(root, hover, raw);

    let capture = capture_path(root);
    let mut handled = Handled::No;
    // A gesture in flight owns the pointer: it hears the move wherever the cursor went. Otherwise
    // the move belongs to whatever is under it now.
    if let Some(path) = capture.as_ref().or(target.as_ref()) {
        handled = deliver_targeted(root, path, Event::PointerMove(pointer_event(raw, 0)));
    }
    // A drag is driven from the widget the button went down on, **whether or not anything
    // consumed that press**: a draggable row that ignores presses is still draggable, and a
    // control that took the press can still be the thing being dragged.
    if let Some(path) = capture.or_else(|| press_path(root)) {
        handled = or(handled, drive_drag(root, &path, raw));
    }
    // And again after, because the move that *starts* a drag arrives while nothing is dragging yet:
    // the check above cannot know, so the row under the cursor would stay lit until the next move.
    if dragging_path(root).is_some() {
        update_hover(root, None, raw);
    }
    handled
}

/// A button went down: everything *not* under it hears that first (a popup closes itself on it),
/// then the target and its ancestors get the press.
fn route_press(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let target = hit_test(root, raw.pos);
    update_hover(root, target.as_deref(), raw);

    let outside = Event::PointerDownOutside(pointer_event(raw, 0));
    broadcast_outside(root, target.as_deref(), &outside);

    let Some(path) = target else {
        return Handled::No;
    };

    // The click run belongs to the widget the press landed on, so two presses on two widgets are
    // never a double click however quickly they follow each other.
    let count =
        {
            let node = node_at(root, &path);
            let base = node.base();
            let now = Instant::now();
            let continues =
                base.pointer.run.get().is_some_and(|(at, _)| {
                    now.duration_since(at).as_secs_f32() <= MULTI_CLICK_SECS
                }) && base
                    .pointer
                    .press
                    .get()
                    .map(|(p, _)| near(p, raw.pos))
                    .unwrap_or(true);
            let count = if continues {
                base.pointer.run.get().map_or(1, |(_, c)| c + 1)
            } else {
                1
            };
            base.pointer.run.set(Some((now, count)));
            base.pointer.press.set(Some((raw.pos, raw.button)));
            count
        };

    let handled = deliver_targeted(root, &path, Event::PointerDown(pointer_event(raw, count)));
    if handled == Handled::Yes {
        // Whoever took the press owns the rest of the gesture.
        set_capture(root, &path);
    }
    handled
}

/// The button came up: end the gesture, then — if the press and the release belong to the same
/// widget — say what the pair of them was.
fn route_release(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let target = hit_test(root, raw.pos);
    let up = Event::PointerUp(pointer_event(raw, 0));

    let capture = capture_path(root);
    let mut handled = match (&capture, &target) {
        (Some(path), _) => deliver_targeted(root, path, up),
        (None, Some(path)) => deliver_targeted(root, path, up),
        (None, None) => Handled::No,
    };

    if dragging_path(root).is_some() {
        handled = or(handled, finish_drag(root, raw));
    } else if let Some(path) = press_path(root) {
        // A click is a press and a release on the same widget — and it is delivered to **whoever
        // took the press**, not to the deepest widget under the cursor. A control that composes
        // its content (a button holding an icon and a label) consumes the press so its content
        // cannot; the click that press turns into is the control's for the same reason, and it
        // stops being something each control has to arrange by taking events away from itself.
        let same = target.as_ref() == Some(&path);
        let path = capture.clone().unwrap_or(path);
        let count = node_at(root, &path)
            .base()
            .pointer
            .run
            .get()
            .map_or(1, |(_, c)| c);
        if same && let Some(kind) = raw.button.click_kind() {
            let e = pointer_event(raw, count);
            // **Every click is a click.** The run only adds to it: the second click of a pair
            // arrives as a `Click` *and* a `DoubleClick`, in that order, the way the web does it.
            // Nothing is ever held back waiting to see whether another is coming — a button that
            // understands only single clicks must still fire on the second, and a widget waiting
            // on a maybe-double is a widget that responds late.
            let base = match kind {
                EventKind::RightClick => Event::RightClick(e),
                EventKind::MiddleClick => Event::MiddleClick(e),
                _ => Event::Click(e),
            };
            let (delivered, default_prevented) = delivering(|| deliver_targeted(root, &path, base));
            handled = or(handled, delivered);
            // **A declared menu opens because it was declared** — resolved by walking outwards
            // from the widget that was clicked to the nearest one carrying one.
            //
            // A widget that wants to answer the right-click *and* show something else says
            // `prevent_default`, the way a browser does. It used to be cancelled by
            // `stop_propagation` instead, which is a different question — that one is about who
            // *else* sees the event — so a widget that stopped the walk for an unrelated reason
            // silently lost its own menu, with nothing failing and no warning
            //.
            if kind == EventKind::RightClick
                && !default_prevented
                && crate::menu::open_declared_at(root, &path, raw.pos)
            {
                handled = Handled::Yes;
            }
            if kind == EventKind::Click && count >= 2 {
                let extra = if count == 2 {
                    Event::DoubleClick(e)
                } else {
                    Event::TripleClick(e)
                };
                handled = or(handled, deliver_path(root, &path, &extra));
            }
        }
    }

    clear_press(root);
    clear_capture(root);
    // Hover is re-derived from where the release left the pointer: a widget that opened or closed
    // something under the cursor must not stay lit because nothing moved afterwards.
    update_hover(root, target.as_deref(), raw);
    handled
}

/// The wheel turned: routed by position like every other pointer event, so a region that is not
/// under the cursor never scrolls, and a nested one that cannot scroll the axis asked for lets the
/// event bubble to the one outside it.
fn route_wheel(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let Some(path) = hit_test(root, raw.pos) else {
        return Handled::No;
    };
    deliver_targeted(root, &path, Event::Scroll(pointer_event(raw, 0)))
}

/// The pointer left the window, or the host is cancelling: nothing may stay hovered, captured,
/// pressed or dragging behind it.
fn cancel(root: &mut dyn Component, raw: &RawPointer) {
    update_hover(root, None, raw);
    if let Some(path) = dragging_path(root) {
        let item = drag_identity(root, &path);
        clear_drag_over(root);
        if let Some(item) = item {
            let ev = Event::DragEnd(DragEvent {
                item,
                pos: raw.pos,
                modifiers: raw.modifiers,
                side: DropSide::Onto,
            });
            let _ = deliver_path(root, &path, &ev);
        }
        node_at(root, &path).base().pointer.dragging.set(false);
    }
    clear_press(root);
    clear_capture(root);
}

// ─────────────────────────────── hit-testing ───────────────────────────────

/// The path to the **topmost, deepest** widget under `pos` — the pointer's target.
///
/// Three rules, and they are the same ones paint follows:
///
/// - **children before the parent**, last-added first, so what is drawn on top is what is hit;
/// - **an overlay wins over its siblings' order**: a dropdown panel drawn above a row that comes
///   after it in the child list still takes the click, because
///   [`overlay_occludes`](Component::overlay_occludes) says the panel owns that point;
/// - **a clipping widget's children are unreachable outside it** — content scrolled out of sight
///   stops being clickable, which is what [`clips_children`](Component::clips_children) means.
///
/// A widget's own bounds are tested **after** its children, and descent does not require the
/// parent to contain the point: a [`Select`](crate::widgets::Select)'s option list is a child
/// placed outside the trigger it belongs to, and it is still the thing under the cursor.
pub fn hit_test(root: &dyn Component, pos: Point) -> Option<Path> {
    if skip(root) {
        return None;
    }
    let Some(rect) = root.hit_bounds() else {
        // The widget declares it takes no input at all — a closed overlay, say. Its children are
        // laid out and would hit-test perfectly well, which is exactly the problem.
        return None;
    };
    if root.clips_children() && !rect.contains(pos) {
        return None;
    }
    let children = &root.base().children;
    // Pass 1: a subtree whose overlay covers this point, whatever its place in the child order.
    for (i, child) in children.iter().enumerate().rev() {
        if crate::component::overlay_occluded_at(child.as_ref(), pos)
            && let Some(mut sub) = hit_test(child.as_ref(), pos)
        {
            sub.insert(0, i);
            return Some(sub);
        }
    }
    for (i, child) in children.iter().enumerate().rev() {
        if let Some(mut sub) = hit_test(child.as_ref(), pos) {
            sub.insert(0, i);
            return Some(sub);
        }
    }
    // **A surface passes the pointer through where it covers nothing** — the browser's
    // `pointer-events: none` on a positioned wrapper, and the reason an author seats a surface and
    // writes nothing else. Its children have already had their turn above and keep everything that
    // lands on them; what is left is the surface's own box, and a surface's box is routinely much
    // bigger than what it draws (a notification stack spans the window so a corner means the
    // *screen's* corner). Claiming that box is how an empty, invisible surface came to swallow
    // every press in the app with nothing failing anywhere. A surface that means to swallow says so
    // in `overlay_occludes` — which is what `Overlay::blocking(true)` answers for the whole
    // viewport, so a modal is unaffected.
    if root.base().surface {
        return root.overlay_occludes(pos).then(Path::new);
    }
    rect.contains(pos).then(Path::new)
}

/// Hidden and invisible subtrees have stale bounds and take no input — the same filter paint,
/// focus and drag resolution use.
fn skip(c: &dyn Component) -> bool {
    !c.base().visible.get_untracked()
        || c.base().style.layout.hidden
        // Decoration the pointer passes through — see `Base::pointer_transparent`.
        || c.base().pointer_transparent
}

// ─────────────────────────────── hover ───────────────────────────────

/// Set [`PointerState::hovered`] on the target's whole ancestor chain and clear it everywhere
/// else, emitting [`PointerEnter`](Event::PointerEnter) / [`PointerLeave`](Event::PointerLeave) on
/// the transitions.
///
/// **Leaving is never vetoed.** Enter and leave are announcements, not offers: a widget cannot
/// consume its neighbour's leave, so the walk delivers both to everything that changed and takes
/// no answer.
fn update_hover(root: &mut dyn Component, target: Option<&[usize]>, raw: &RawPointer) {
    let e = pointer_event(raw, 0);
    hover_walk(root, target, &e);
}

fn hover_walk(node: &mut dyn Component, target: Option<&[usize]>, e: &PointerEvent) {
    let on_path = target.is_some();
    let was = node.base().pointer.hovered.get_untracked();
    if was != on_path {
        node.base().pointer.hovered.set(on_path);
        // The clock starts and stops with the hover itself, so nothing downstream has to observe
        // enter/leave to keep its own copy in step.
        node.base()
            .pointer
            .set_hovered_since(on_path.then(Instant::now));
        node.base().mark_needs_paint();
        let ev = if on_path {
            Event::PointerEnter(*e)
        } else {
            Event::PointerLeave(*e)
        };
        deliver_self(node, &ev);
    }
    let next = target.and_then(|p| p.split_first());
    let n = node.base().children.len();
    for i in 0..n {
        let sub = match next {
            Some((head, rest)) if *head == i => Some(rest),
            _ => None,
        };
        let child = &mut node.base_mut().children[i];
        hover_walk(child.as_mut(), sub, e);
    }
}

// ─────────────────────────────── delivery ───────────────────────────────

/// Deliver `ev` to exactly one widget — capture, handlers, then the widget — for the events that
/// are **about** that widget rather than about a place: enter, leave, focus, mount. There is no
/// path to walk, and a widget sees them through the same two hooks as everything else.
fn deliver_self(node: &mut dyn Component, ev: &Event) {
    if node.on_event_capture(ev) == Handled::Yes {
        return;
    }
    if node.base_mut().run_handlers(ev).handled == Handled::Yes {
        return;
    }
    let _ = node.on_event(ev);
}

/// Deliver `ev` down `path` and back up: [`on_event_capture`](Component::on_event_capture) on the
/// way in, the target, then each ancestor's handlers and
/// [`on_event`](Component::on_event) on the way out.
///
/// This is the target-and-bubble model, and it is what makes a widget's own routing unnecessary: a
/// container on the path is *on the path*, so it sees the event without forwarding anything, and a
/// container that is not gets nothing to forward. A widget stops the walk by returning
/// [`Handled::Yes`], or a handler by calling
/// [`EventCx::stop_propagation`](crate::event::EventCx::stop_propagation).
pub fn deliver_path(node: &mut dyn Component, path: &[usize], ev: &Event) -> Handled {
    if node.on_event_capture(ev) == Handled::Yes {
        return Handled::Yes;
    }
    if claims_primary_press(node, ev) {
        return Handled::Yes;
    }
    if let Some((head, rest)) = path.split_first() {
        let from_subtree = match node.base_mut().children.get_mut(*head) {
            Some(child) => deliver_path(child.as_mut(), rest, ev),
            None => Handled::No,
        };
        // Always, consumed or not: a container that reports what its subtree did must hear about
        // the click the row inside it took.
        node.after_subtree(ev, from_subtree);
        if from_subtree == Handled::Yes {
            return Handled::Yes;
        }
    }
    if node.base_mut().run_handlers(ev).handled == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// Whether this widget takes the press because it is
/// [one click target](crate::component::Base::one_click_target).
///
/// **The primary button only.** A control claims the press that leads to its own activation;
/// every other button carries on past it, which is what lets a right-click reach whatever answers
/// one. Nine widgets used to write this claim by hand and all nine claimed every button.
fn claims_primary_press(node: &dyn Component, ev: &Event) -> bool {
    let base = node.base();
    base.one_click_target
        && !base.disabled.get_untracked()
        && matches!(ev, Event::PointerDown(p) if p.button == PointerButton::Left)
}

/// Deliver `ev` to every node that is **not** on `path` — how a popup hears the press that
/// dismisses it without watching the whole window.
fn broadcast_outside(node: &mut dyn Component, path: Option<&[usize]>, ev: &Event) {
    if path.is_none() {
        // Through the widget's own hooks, capture included: a menu answers this in capture like
        // everything else it handles, and delivering it any other way is how "click away to close"
        // silently stopped working.
        deliver_self(node, ev);
    }
    let next = path.and_then(|p| p.split_first());
    let n = node.base().children.len();
    for i in 0..n {
        let sub = match next {
            Some((head, rest)) if *head == i => Some(rest),
            _ => None,
        };
        let child = &mut node.base_mut().children[i];
        broadcast_outside(child.as_mut(), sub, ev);
    }
}

// ─────────────────────────────── drag ───────────────────────────────

/// A move while a press is live: start a drag once the pointer has travelled far enough, then keep
/// the source and whatever it is over informed.
fn drive_drag(root: &mut dyn Component, press: &[usize], raw: &RawPointer) -> Handled {
    let Some((source_path, item)) = drag_source_on(root, press) else {
        return Handled::No;
    };
    let dragging = node_at(root, &source_path).base().pointer.dragging.get();
    if !dragging {
        let origin = node_at(root, &source_path)
            .base()
            .pointer
            .press
            .get()
            .or_else(|| node_at(root, press).base().pointer.press.get());
        let far = origin.is_some_and(|(p, _)| dist(p, raw.pos) >= DRAG_THRESHOLD);
        if !far {
            return Handled::No;
        }
        node_at(root, &source_path)
            .base()
            .pointer
            .dragging
            .set(true);
        let ev = Event::DragStart(drag_event(&item, raw, DropSide::Onto));
        let _ = deliver_path(root, &source_path, &ev);
    }

    let kind = node_at(root, &source_path).base().drag_kind.clone();
    let hit = crate::drag::resolve_at_for(root, raw.pos, kind.as_deref());
    {
        // The source draws what follows the cursor, so it is the source that must know where the
        // cursor is: its own bounds still say where it was picked up from.
        let p = &node_at(root, &source_path).base().pointer;
        p.drag_pos.set(raw.pos);
    }
    let side = hit.as_ref().map_or(DropSide::Onto, |h| h.side);
    update_drag_over(
        root,
        hit.as_ref().map(|h| (h.path.clone(), h.side)),
        &item,
        raw,
    );
    deliver_path(
        root,
        &source_path,
        &Event::Drag(drag_event(&item, raw, side)),
    )
}

/// The release that ends a drag: the target it landed on hears [`Drop`](Event::Drop), then the
/// source hears [`DragEnd`](Event::DragEnd) — always, so a source can undo its own state whether
/// or not anything accepted it.
fn finish_drag(root: &mut dyn Component, raw: &RawPointer) -> Handled {
    let Some(source_path) = dragging_path(root) else {
        return Handled::No;
    };
    let Some(item) = drag_identity(root, &source_path) else {
        return Handled::No;
    };
    let kind = node_at(root, &source_path).base().drag_kind.clone();
    let hit = crate::drag::resolve_at_for(root, raw.pos, kind.as_deref());
    let mut handled = Handled::No;
    if let Some(hit) = hit.as_ref() {
        let ev = Event::Drop(drag_event(&item, raw, hit.side));
        handled = deliver_path(root, &hit.path, &ev);
        // **What happens to a drop nobody took is the host's**, the same way an unclaimed
        // right-click with a declared menu is. A row can say it accepts drops; it cannot move a
        // pane into another workspace. So this crosses back once, with both identities already
        // resolved — and a row that wants to answer for itself still wins, because it consumed the
        // event above and never reaches here.
        if handled == Handled::No {
            crate::drag::sink::present(crate::drag::Dropped {
                source: item.clone(),
                target: hit.key.clone(),
                side: hit.side,
                action: crate::drag::DropAction::held(),
                modifiers: raw.modifiers,
            });
            handled = Handled::Yes;
        }
    }
    clear_drag_over(root);
    node_at(root, &source_path)
        .base()
        .pointer
        .dragging
        .set(false);
    let side = hit.as_ref().map_or(DropSide::Onto, |h| h.side);
    let ev = Event::DragEnd(drag_event(&item, raw, side));
    or(handled, deliver_path(root, &source_path, &ev))
}

/// Move the "a drag is over me" flag to `now`, emitting enter/leave/over as it goes.
fn update_drag_over(
    root: &mut dyn Component,
    now: Option<(Path, DropSide)>,
    item: &str,
    raw: &RawPointer,
) {
    let previous = drag_over_path(root);
    // **The node the walk found, not a second search for its name.** Two seatings of one container
    // give their rows the same name, so a search lights whichever comes first — the other sidebar.
    let current = now.as_ref().map(|(path, _)| path.clone());
    if previous != current
        && let Some(path) = previous.clone()
    {
        node_at(root, &path).base().pointer.drag_over.set(false);
        let ev = Event::DragLeave(drag_event(item, raw, DropSide::Onto));
        let _ = deliver_path(root, &path, &ev);
    }
    if let Some(path) = current {
        let side = now.map_or(DropSide::Onto, |(_, s)| s);
        node_at(root, &path).base().pointer.drag_side.set(side);
        if previous.as_ref() != Some(&path) {
            node_at(root, &path).base().pointer.drag_over.set(true);
            let ev = Event::DragEnter(drag_event(item, raw, side));
            let _ = deliver_path(root, &path, &ev);
        }
        let ev = Event::DragOver(drag_event(item, raw, side));
        let _ = deliver_path(root, &path, &ev);
    }
}

/// The innermost drag source at or above the pressed widget, and its path.
fn drag_source_on(root: &dyn Component, press: &[usize]) -> Option<(Path, String)> {
    let mut node = root;
    let mut best: Option<(Path, String)> = None;
    let mut here = Path::new();
    if let Some(id) = drag_identity(root, &here) {
        best = Some((here.clone(), id));
    }
    for i in press {
        let Some(child) = node.base().children.get(*i) else {
            break;
        };
        node = child.as_ref();
        here.push(*i);
        if let Some(id) = drag_identity(root, &here) {
            best = Some((here.clone(), id));
        }
    }
    best
}

// ─────────────────────────────── per-node state ───────────────────────────────

/// The path to the widget holding pointer capture, if any.
fn capture_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.capture.get())
}

/// The path to the widget a live press landed on, if any.
fn press_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.press.get().is_some())
}

/// **Is a drag in flight anywhere in this tree?**
///
/// The one question a host asks about a drag it does not own: while something is being carried, the
/// cursor changes shape, nothing hovers, and a move must not reach the program in a pane. The app
/// used to answer it from a drag machine of its own; the framework runs the gesture, so the
/// framework answers (F003/P097/T496).
pub fn dragging(root: &dyn Component) -> bool {
    dragging_path(root).is_some()
}

/// The path to the widget currently dragging, if any.
fn dragging_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.dragging.get())
}

/// The path to the drop target a drag is currently over, if any.
fn drag_over_path(root: &dyn Component) -> Option<Path> {
    find(root, &|c| c.base().pointer.drag_over.get())
}

/// The path to the drop target registered under `id`.
/// The dragged identity of the widget at `path`, when it is a drag source at all — the same
/// answer [`drag::source_at`](crate::drag::source_at) gives, so the gesture and the resolution
/// cannot disagree about what is being carried.
fn drag_identity(root: &dyn Component, path: &[usize]) -> Option<String> {
    let node = node_at(root, path);
    if !node.is_drag_source() {
        return None;
    }
    node.base()
        .key
        .clone()
        .or_else(|| crate::nav::identity_of(root, path))
}

/// Depth-first search for the first node satisfying `f`, returning its path.
fn find(node: &dyn Component, f: &dyn Fn(&dyn Component) -> bool) -> Option<Path> {
    if f(node) {
        return Some(Path::new());
    }
    for (i, child) in node.base().children.iter().enumerate() {
        if let Some(mut sub) = find(child.as_ref(), f) {
            sub.insert(0, i);
            return Some(sub);
        }
    }
    None
}

/// The node `path` leads to. The path came from a walk of this same tree, so a missing step can
/// only mean the tree changed underneath it — in which case the nearest node standing is the
/// honest answer.
fn node_at<'a>(root: &'a dyn Component, path: &[usize]) -> &'a dyn Component {
    let mut node = root;
    for i in path {
        match node.base().children.get(*i) {
            Some(child) => node = child.as_ref(),
            None => break,
        }
    }
    node
}

fn set_capture(root: &dyn Component, path: &[usize]) {
    node_at(root, path).base().pointer.capture.set(true);
}

fn clear_capture(root: &dyn Component) {
    walk(root, &|c| c.base().pointer.capture.set(false));
}

fn clear_press(root: &dyn Component) {
    walk(root, &|c| c.base().pointer.press.set(None));
}

fn clear_drag_over(root: &dyn Component) {
    walk(root, &|c| c.base().pointer.drag_over.set(false));
}

fn walk(node: &dyn Component, f: &dyn Fn(&dyn Component)) {
    f(node);
    for child in &node.base().children {
        walk(child.as_ref(), f);
    }
}

/// Clear every widget's hover in `root` — what a host calls when the pointer leaves its window and
/// no further move is coming. Equivalent to routing [`Event::pointer_cancelled`].
pub fn clear_hover(root: &mut dyn Component) {
    let raw = RawPointer::new(RawPointerKind::Cancelled, Point::new(f64::MIN, f64::MIN));
    update_hover(root, None, &raw);
}

// ─────────────────────────────── small helpers ───────────────────────────────

fn pointer_event(raw: &RawPointer, click_count: u32) -> PointerEvent {
    PointerEvent {
        pos: raw.pos,
        button: raw.button,
        modifiers: raw.modifiers,
        click_count,
        delta_x: raw.delta_x,
        delta_y: raw.delta_y,
        // Filled in by `deliver_targeted` on the way down — the router knows the target, the
        // constructor does not.
        target_bounds: None,
    }
}

/// The laid-out bounds of the node `path` leads to.
fn bounds_at(root: &dyn Component, path: &[usize]) -> Rectangle {
    let mut node = root;
    for i in path {
        match node.base().children.get(*i) {
            Some(child) => node = child.as_ref(),
            None => break,
        }
    }
    node.base().bounds
}

/// Deliver `ev` down `path`, **stamped with the target widget's bounds**.
///
/// The one place the stamp happens, so every routed pointer event carries it and no widget has to
/// arrange for its own. [`deliver_path`] stays the raw walk underneath.
fn deliver_targeted(root: &mut dyn Component, path: &[usize], ev: Event) -> Handled {
    let ev = ev.with_target_bounds(bounds_at(root, path));
    deliver_path(root, path, &ev)
}

thread_local! {
    /// **Whether any handler in the delivery just made asked the framework not to act.**
    ///
    /// Recorded as the event is delivered rather than found by asking again: a second walk would
    /// run every handler twice and fire its side effects twice. Reset before each delivery, read
    /// straight after — the walk is synchronous and single-threaded, so nothing can interleave.
    static DEFAULT_PREVENTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Note that a handler asked the framework not to do its own thing.
pub(crate) fn note_default_prevented() {
    DEFAULT_PREVENTED.with(|c| c.set(true));
}

/// Run `f` as one delivery and report whether anything in it prevented the default.
fn delivering<T>(f: impl FnOnce() -> T) -> (T, bool) {
    DEFAULT_PREVENTED.with(|c| c.set(false));
    let out = f();
    (out, DEFAULT_PREVENTED.with(std::cell::Cell::get))
}

fn drag_event(item: &str, raw: &RawPointer, side: DropSide) -> DragEvent {
    DragEvent {
        item: item.to_string(),
        pos: raw.pos,
        modifiers: raw.modifiers,
        side,
    }
}

fn near(a: Point, b: Point) -> bool {
    dist(a, b) <= MULTI_CLICK_SLOP
}

fn dist(a: Point, b: Point) -> f64 {
    let (dx, dy) = (a.x - b.x, a.y - b.y);
    (dx * dx + dy * dy).sqrt()
}

fn or(a: Handled, b: Handled) -> Handled {
    if a == Handled::Yes || b == Handled::Yes {
        Handled::Yes
    } else {
        Handled::No
    }
}

/// Fire [`Event::Mount`] on every widget in `root` that has not had one yet, and mark it mounted.
/// Called by the layout pass, which is the first moment a tree is live and laid out.
pub(crate) fn fire_mounts(node: &mut dyn Component) {
    if !node.base().mounted.get() {
        node.base().mounted.set(true);
        let ev = Event::Mount;
        let _ = node.base_mut().run_handlers(&ev);
        let _ = node.on_event(&ev);
    }
    let n = node.base().children.len();
    for i in 0..n {
        let child = &mut node.base_mut().children[i];
        fire_mounts(child.as_mut());
    }
}

impl Base {
    /// Run this widget's registered handlers for `ev`, if it has any.
    pub(crate) fn run_handlers(&mut self, ev: &Event) -> crate::event::HandlerOutcome {
        match self.handlers.as_mut() {
            Some(h) => {
                let out = h.run(ev);
                if out.default_prevented {
                    note_default_prevented();
                }
                out
            }
            None => crate::event::HandlerOutcome {
                handled: Handled::No,
                default_prevented: false,
            },
        }
    }
}
