//! **Target-and-bubble delivery.** How a resolved event reaches a widget: capture down the path,
//! the target, then each ancestor's handlers on the way back up. The router decides *which* widget
//! an event is for; this is the one way a widget ever hears it.

use crate::component::{Base, Component};
use crate::event::{Event, Handled, PointerButton};
use crate::reactive::SignalGet;
use heca_core::layout::Rectangle;

/// Deliver `ev` to exactly one widget — capture, handlers, then the widget — for the events that
/// are **about** that widget rather than about a place: enter, leave, focus, mount. There is no
/// path to walk, and a widget sees them through the same two hooks as everything else.
pub(super) fn deliver_self(node: &mut dyn Component, ev: &Event) {
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
pub(super) fn broadcast_outside(node: &mut dyn Component, path: Option<&[usize]>, ev: &Event) {
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
pub(super) fn deliver_targeted(root: &mut dyn Component, path: &[usize], ev: Event) -> Handled {
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
fn note_default_prevented() {
    DEFAULT_PREVENTED.with(|c| c.set(true));
}

/// Run `f` as one delivery and report whether anything in it prevented the default.
pub(super) fn delivering<T>(f: impl FnOnce() -> T) -> (T, bool) {
    DEFAULT_PREVENTED.with(|c| c.set(false));
    let out = f();
    (out, DEFAULT_PREVENTED.with(std::cell::Cell::get))
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
