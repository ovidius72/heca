//! **Where a widget's act goes** — the host-installed sink every
//! [`EventCx::dispatch`](crate::event::EventCx::dispatch) posts to.
//!
//! The twin of the menu sink beside it, and for the same reason: a widget can say *what* it wants
//! done, but only the host can do it — it owns the action registry, the permission rules and the
//! confirmation gate. So that one step, and only that step, crosses back.
//!
//! Before this, a widget could act only through a closure its host handed it, which is why any
//! composition that did anything carried a struct of callbacks. A plugin could carry none of them,
//! so a plugin's widget could not act at all.

use std::cell::RefCell;
use std::rc::Rc;

type IntentSink = Box<dyn Fn(heca_view::Intent)>;

/// **What receives the actions dispatched inside a container**, declared on it with
/// [`on_dispatch`](crate::builders::EventExt::on_dispatch).
///
/// The host seats each dock in a wrapper that knows which dock it is and how a gesture there is
/// judged, and declares this on it. Every widget inside then just names its action — a handler's
/// [`dispatch`](crate::event::EventCx::dispatch), a row's
/// [`on_activate`](crate::widgets::Row::on_activate), a letter's
/// [`on_hint`](crate::builders::HintExt::on_hint) — and the action arrives addressed, with nothing
/// threaded through the widgets to do it.
pub type DispatchHandler = Rc<dyn Fn(heca_view::Intent)>;

thread_local! {
    /// Thread-local because the UI runs single-threaded, exactly like the menu sink.
    static INTENT_SINK: RefCell<Option<IntentSink>> = const { RefCell::new(None) };
    /// The route of the widget acting right now — set while its handlers and its own event code
    /// run ([`routed`]), so a dispatch made there goes where its container said.
    static ROUTE: RefCell<Option<DispatchHandler>> = const { RefCell::new(None) };
}

/// **Something a widget does when it is used**: a closure, or an action named as data.
///
/// What [`Row::on_activate`](crate::widgets::Row::on_activate) and its siblings take. A named
/// action is posted through [`dispatch`], so it follows the route of the container it sits in —
/// the line a plugin writes is the line heca writes:
///
/// ```ignore
/// Row::new().on_activate(Intent::new("focus_pane").arg("pane_id", 7))
/// ```
pub struct Act(Box<dyn Fn()>);

impl Act {
    /// Do it.
    pub fn run(&self) {
        (self.0)()
    }
}

impl<F: Fn() + 'static> From<F> for Act {
    fn from(f: F) -> Self {
        Self(Box::new(f))
    }
}

impl From<heca_view::Intent> for Act {
    fn from(intent: heca_view::Intent) -> Self {
        Self(Box::new(move || dispatch(intent.clone())))
    }
}

/// Run `f` as the widget whose route is `route`: a dispatch made inside goes to that route.
/// Nested calls restore the outer route, so a widget acting inside another's delivery cannot leak
/// its route past its own turn.
pub(crate) fn dispatching_to<T>(route: Option<DispatchHandler>, f: impl FnOnce() -> T) -> T {
    let outer = ROUTE.with(|c| c.replace(route));
    let out = f();
    ROUTE.with(|c| *c.borrow_mut() = outer);
    out
}

/// Install the callback that **performs** what a widget asked for. Call once at startup.
///
/// The host decides what a name means and whether it is allowed; nothing here inspects the intent.
pub fn install_intent_sink(f: impl Fn(heca_view::Intent) + 'static) {
    INTENT_SINK.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Post an intent: to the route of the widget acting now, if its container declared one, else to
/// the host. A no-op when there is neither.
pub(crate) fn dispatch(intent: heca_view::Intent) {
    // Cloned out first: the route may act, and acting may dispatch again.
    if let Some(route) = ROUTE.with(|c| c.borrow().clone()) {
        route(intent);
        return;
    }
    INTENT_SINK.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f(intent);
        }
    });
}

/// Whether a sink is installed — what lets a caller tell "nothing happened because nothing was
/// declared" from "nothing happened because there is no host".
pub fn has_intent_sink() -> bool {
    INTENT_SINK.with(|c| c.borrow().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::{EventExt, HintExt, Parent};
    use crate::widgets::{Flex, Row};
    use heca_view::Intent;

    /// The names of the actions one receiver got.
    type Got = Rc<RefCell<Vec<String>>>;

    /// What the host receives, and an empty list for a route to fill.
    fn routes() -> (Got, Got) {
        let host = Got::default();
        let seen = host.clone();
        install_intent_sink(move |intent| seen.borrow_mut().push(intent.action));
        (host, Got::default())
    }

    #[test]
    fn what_a_widget_names_goes_to_the_route_of_the_container_holding_it() {
        let (host, routed) = routes();
        let seen = routed.clone();
        let mut root = Flex::column()
            .on_dispatch(move |intent| seen.borrow_mut().push(intent.action))
            .child(Row::new().on_activate(Intent::new("activated")))
            .child(Row::new().on_hint(Intent::new("picked")))
            .child(Row::new().on_click(|ev| ev.dispatch(Intent::new("clicked"))));
        crate::pointer::fire_mounts(&mut root);
        for row in 0..3 {
            assert!(
                crate::hint::fire_hint(&mut root, &[row]),
                "row {row} is a target"
            );
        }
        assert_eq!(*routed.borrow(), ["activated", "picked", "clicked"]);
        assert!(
            host.borrow().is_empty(),
            "nothing inside a route reaches the host directly"
        );
    }

    #[test]
    fn without_a_route_a_named_action_reaches_the_host() {
        let (host, _) = routes();
        let mut root = Flex::column().child(Row::new().on_activate(Intent::new("activated")));
        crate::pointer::fire_mounts(&mut root);
        assert!(crate::hint::fire_hint(&mut root, &[0]));
        assert_eq!(*host.borrow(), ["activated"]);
        // And the route is gone once the widget's turn is over.
        assert!(ROUTE.with(|c| c.borrow().is_none()));
    }
}
