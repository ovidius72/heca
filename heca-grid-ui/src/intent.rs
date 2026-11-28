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

pub(crate) type IntentSink = Rc<dyn Fn(heca_view::Intent)>;

/// **What receives the actions dispatched inside a container**, declared on it with
/// [`on_dispatch`](crate::builders::EventExt::on_dispatch).
///
/// The host seats each dock in a wrapper that knows which dock it is and how a gesture there is
/// judged, and declares this on it. Every widget inside then just names its action — a handler's
/// [`dispatch`](crate::event::EventCx::dispatch), a row's
/// [`on_click`](crate::builders::EventExt::on_click), a letter's
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

/// Run `f` as the widget whose route is `route`: a dispatch made inside goes to that route.
/// Nested calls restore the outer route, so a widget acting inside another's delivery cannot leak
/// its route past its own turn.
pub(crate) fn dispatching_to<T>(route: Option<DispatchHandler>, f: impl FnOnce() -> T) -> T {
    // A widget with no listener above it keeps whatever route its delivery already has — the one
    // a host set for this delivery ([`sending_to`]), never "none".
    let Some(route) = route else { return f() };
    let outer = ROUTE.with(|c| c.replace(Some(route)));
    let out = f();
    ROUTE.with(|c| *c.borrow_mut() = outer);
    out
}

/// **Run `f` as one delivery whose actions go to `route`** unless a nearer container listens
/// ([`on_dispatch`](crate::builders::EventExt::on_dispatch) still wins). For a host delivering
/// a gesture it can name the origin of — a `prefix+/` pick is a keyboard gesture wherever its
/// letter sits — so a widget that sends from inside it is judged as that gesture.
pub fn sending_to<T>(route: impl Fn(heca_view::Intent) + 'static, f: impl FnOnce() -> T) -> T {
    let route: DispatchHandler = Rc::new(route);
    let outer = ROUTE.with(|c| c.replace(Some(route)));
    let out = f();
    ROUTE.with(|c| *c.borrow_mut() = outer);
    out
}

/// Install the callback that **performs** what a widget asked for. Call once at startup.
///
/// The host decides what a name means and whether it is allowed; nothing here inspects the intent.
pub fn install_intent_sink(f: impl Fn(heca_view::Intent) + 'static) {
    INTENT_SINK.with(|c| *c.borrow_mut() = Some(Rc::new(f)));
}

/// The sink installed now, and putting one back — what a [`WindowContext`](crate::WindowContext)
/// keeps for its window.
pub(crate) fn intent_sink() -> Option<IntentSink> {
    INTENT_SINK.with(|c| c.borrow().clone())
}
pub(crate) fn set_intent_sink(sink: Option<IntentSink>) {
    INTENT_SINK.with(|c| *c.borrow_mut() = sink);
}

/// **Send an action from inside a widget's callback** — the one way a callback that is handed a
/// value rather than the event (a fold's new state, the card a grid chose, a toast's id, a
/// scrollbar's position) names what it does:
///
/// ```ignore
/// DockFrame::new(name).on_toggle(move |_| send(Intent::new("toggle_workspace_collapsed").arg("ws_idx", i)))
/// ```
///
/// It goes exactly where [`EventCx::dispatch`](crate::event::EventCx::dispatch) and `on_press` go:
/// to the listener of the dock or layer holding the widget acting now, else to the host. So the
/// callback carries no emitter, mount or seat. Called outside any event, it reaches the host.
pub fn send(intent: heca_view::Intent) {
    dispatch(intent);
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
            .child(Row::new().on_press(Intent::new("activated")))
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

    /// **What a widget sends on the way in — from its capture code — takes its route too.** A grid
    /// that chooses the card under its cursor on `Activate` answers in capture; that send once fell
    /// through to the host with no origin, and the app refused a layer's own choice as an act on
    /// the page behind it.
    #[test]
    fn what_a_widget_sends_from_its_capture_goes_to_its_route() {
        struct Chooser {
            base: crate::component::Base,
        }
        impl crate::component::Component for Chooser {
            fn base(&self) -> &crate::component::Base {
                &self.base
            }
            fn base_mut(&mut self) -> &mut crate::component::Base {
                &mut self.base
            }
            fn on_event_capture(&mut self, ev: &crate::event::Event) -> crate::event::Handled {
                if matches!(
                    ev,
                    crate::event::Event::Widget(crate::event::WidgetIntent::Activate)
                ) {
                    send(Intent::new("chosen"));
                    return crate::event::Handled::Yes;
                }
                crate::event::Handled::No
            }
        }
        let (host, routed) = routes();
        let seen = routed.clone();
        let mut root = Flex::column()
            .on_dispatch(move |intent| seen.borrow_mut().push(intent.action))
            .child(Chooser {
                base: crate::component::Base::new(),
            });
        crate::pointer::fire_mounts(&mut root);
        crate::component::deliver_to_path(
            &mut root,
            &[0],
            &crate::event::Event::Widget(crate::event::WidgetIntent::Activate),
        );
        assert_eq!(*routed.borrow(), ["chosen"]);
        assert!(
            host.borrow().is_empty(),
            "nothing reached the host directly"
        );
    }

    #[test]
    fn without_a_route_a_named_action_reaches_the_host() {
        let (host, _) = routes();
        let mut root = Flex::column().child(Row::new().on_press(Intent::new("activated")));
        crate::pointer::fire_mounts(&mut root);
        assert!(crate::hint::fire_hint(&mut root, &[0]));
        assert_eq!(*host.borrow(), ["activated"]);
        // And the route is gone once the widget's turn is over.
        assert!(ROUTE.with(|c| c.borrow().is_none()));
    }

    /// **A callback handed a value sends to its container too** — here a dock frame's fold, which
    /// it reports from `after_subtree`, after the click inside it was handled.
    #[test]
    fn a_fold_reported_after_the_subtree_reaches_the_container() {
        use crate::widgets::DockFrame;
        let (host, routed) = routes();
        let seen = routed.clone();
        let mut root = Flex::column()
            .on_dispatch(move |intent| seen.borrow_mut().push(intent.action))
            .child(DockFrame::new("Lobby").on_toggle(|_| send(Intent::new("folded"))));
        crate::pointer::fire_mounts(&mut root);
        crate::layout::LayoutEngine::new().compute(&mut root, crate::Size::new(300.0, 200.0));
        fn path_to_area(
            node: &dyn crate::component::Component,
            area: &str,
            at: &mut Vec<usize>,
        ) -> bool {
            if node.base().grid_area.as_deref() == Some(area) {
                return true;
            }
            for (i, child) in node.base().children.iter().enumerate() {
                at.push(i);
                if path_to_area(child.as_ref(), area, at) {
                    return true;
                }
                at.pop();
            }
            false
        }
        let mut path = Vec::new();
        assert!(path_to_area(&root, "toggle", &mut path));
        let _ = crate::component::click(&mut root, &path);
        assert_eq!(*routed.borrow(), ["folded"]);
        assert!(host.borrow().is_empty());
    }

    /// **A host can say where one delivery's actions go** — a pick is a keyboard gesture wherever
    /// its letter sits — and a nearer listener still answers for its own widgets.
    #[test]
    fn a_hosts_route_takes_what_no_nearer_listener_does() {
        let (host, inner) = routes();
        let seen = inner.clone();
        let mut root = Flex::column()
            .child(Row::new().on_hint(|| send(Intent::new("bare"))))
            .child(
                Flex::column()
                    .on_dispatch(move |intent| seen.borrow_mut().push(intent.action))
                    .child(Row::new().on_hint(|| send(Intent::new("listened")))),
            );
        crate::pointer::fire_mounts(&mut root);
        let picked = Got::default();
        for path in [vec![0], vec![1, 0]] {
            let seen = picked.clone();
            sending_to(
                move |intent| seen.borrow_mut().push(intent.action),
                || {
                    assert!(crate::hint::fire_hint(&mut root, &path));
                },
            );
        }
        assert_eq!(
            *picked.borrow(),
            ["bare"],
            "the host's route took the bare widget's action"
        );
        assert_eq!(
            *inner.borrow(),
            ["listened"],
            "the nearer listener kept its own"
        );
        assert!(host.borrow().is_empty());
        assert!(
            ROUTE.with(|c| c.borrow().is_none()),
            "and the route ends with the delivery"
        );
    }
}
