//! What a handler is given and can say back — [`EventCx`], [`HandlerOutcome`] — and the handlers a
//! widget registers, [`Handlers`].

use super::{DragEvent, Event, EventKind, Handled, Modifiers, PointerEvent};
use heca_core::layout::Point;

/// **What a widget's handlers said about an event**: whether one of them took it, and whether one
/// of them asked the framework not to do its own thing afterwards.
///
/// Two answers rather than one because they are two questions. A widget that stops the walk is
/// saying nobody *else* should see this; a widget that prevents the default is saying the framework
/// should not act on it. Conflating them meant stopping the walk also cancelled a widget's own
/// declared context menu, silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandlerOutcome {
    pub handled: Handled,
    pub default_prevented: bool,
}

/// What a [`Base`](crate::component::Base) handler is given: the event, and the one lever it has
/// over the walk.
///
/// A handler is not asked to return anything. Most of them do their work and let the event carry
/// on; the ones that own it say so, once, by name — which is the difference between "I did
/// something" and "nobody else should".
pub struct EventCx<'a> {
    event: &'a Event,
    stop: bool,
    prevented: bool,
}

impl<'a> EventCx<'a> {
    /// Wrap `event` for delivery to a handler.
    pub fn new(event: &'a Event) -> Self {
        Self {
            event,
            stop: false,
            prevented: false,
        }
    }

    /// The event being delivered.
    pub fn event(&self) -> &'a Event {
        self.event
    }

    /// **Do something, by name.** The way a handler acts without a callback handed to it.
    ///
    /// ```ignore
    /// .on_click(|ev| {
    ///     if ev.modifiers().shift { return; }
    ///     ev.dispatch(Intent::new("focus_pane").arg("pane_id", id));
    /// })
    /// ```
    ///
    /// A widget used to be able to act only through a function its host passed in, so every
    /// composition that did anything carried a struct of callbacks — plumbing in the middle of what
    /// should read as a declaration. Naming the action instead means a plugin writes the identical
    /// line, and the act goes through the same gate a keypress does: the same permission rules, and
    /// the same confirmation on anything destructive.
    ///
    /// A no-op until a host installs a sink, so widget code and headless tests can always call it.
    pub fn dispatch(&self, intent: heca_view::Intent) {
        crate::intent::dispatch(intent);
    }

    /// The pointer payload, if this event has one.
    pub fn pointer(&self) -> Option<&PointerEvent> {
        self.event.pointer()
    }

    /// The drag payload, if this event has one.
    pub fn drag(&self) -> Option<&DragEvent> {
        self.event.drag()
    }

    /// Where it happened, for the events that happen somewhere.
    ///
    /// The common read at a call site — `on_right_click(|e| menu_at(e.pos()))` — so it is one call
    /// rather than a match on the payload.
    pub fn pos(&self) -> Option<Point> {
        self.event.position()
    }

    /// The modifiers held when it happened: what the event recorded when it carries a pointer or a
    /// drag, and otherwise what is [held right now](super::modifiers) — a key press with Shift down
    /// says Shift, never "nothing".
    pub fn modifiers(&self) -> Modifiers {
        self.event
            .pointer()
            .map(|p| p.modifiers)
            .or_else(|| self.event.drag().map(|d| d.modifiers))
            .unwrap_or_else(super::modifiers)
    }

    /// **This widget owns the event**: no ancestor sees it, and the widget's own
    /// [`on_event`](crate::component::Component::on_event) does not run.
    ///
    /// The named opt-out, deliberately: the walk used to be capture-first-wins with nothing able to
    /// stop it on purpose or let it continue on purpose, which is how a `Row` swallowed an `Enter`
    /// meant for the list it sits in.
    pub fn stop_propagation(&mut self) {
        self.stop = true;
    }

    /// Whether a handler called [`stop_propagation`](Self::stop_propagation).
    pub fn stopped(&self) -> bool {
        self.stop
    }

    /// **Do not do the thing the framework would have done.** The browser's `preventDefault`.
    ///
    /// The only default that exists today is opening the
    /// [`context_menu`](crate::builders::MenuExt::context_menu) a widget declared, on a
    /// right-click. So a widget that wants to answer a right-click *and* show its own menu instead
    /// of the declared one says this, and one that just wants to react says nothing.
    ///
    /// **It is separate from [`stop_propagation`](Self::stop_propagation) because they are
    /// different questions**, and one word doing both was a trap: stopping the walk — to keep
    /// something behind from also reacting — silently switched a widget's own declared menu off,
    /// with nothing failing and no warning. Stopping the walk is about who
    /// *else* sees the event; this is about what happens afterwards.
    pub fn prevent_default(&mut self) {
        self.prevented = true;
    }

    /// Whether a handler called [`prevent_default`](Self::prevent_default).
    pub fn default_prevented(&self) -> bool {
        self.prevented
    }
}

/// One registered handler: a callback that may take the event.
type Handler = Box<dyn FnMut(&mut EventCx<'_>)>;

/// A widget's registered event handlers, keyed by [`EventKind`].
///
/// Lives behind an `Option<Box<…>>` on [`Base`](crate::component::Base), so a widget that registers none costs one null
/// pointer. Several handlers may share a kind; they run in registration order, and the first to
/// call [`EventCx::stop_propagation`] ends the walk (the rest of *this* widget's handlers for that
/// kind still run — they were registered by the same author, on the same widget).
#[derive(Default)]
pub struct Handlers {
    entries: Vec<(EventKind, Handler)>,
}

impl Handlers {
    /// Register `f` for `kind`.
    pub fn add(&mut self, kind: EventKind, f: impl FnMut(&mut EventCx<'_>) + 'static) {
        self.entries.push((kind, Box::new(f)));
    }

    /// Whether anything is registered for `kind` — the cheap check the router makes before
    /// building an event nobody wants.
    pub fn has(&self, kind: EventKind) -> bool {
        self.entries.iter().any(|(k, _)| *k == kind)
    }

    /// Run every handler registered for this event's kind. Returns [`Handled::Yes`] if one of them
    /// stopped propagation.
    pub fn run(&mut self, ev: &Event) -> HandlerOutcome {
        let kind = ev.kind();
        let mut cx = EventCx::new(ev);
        for (k, f) in self.entries.iter_mut() {
            if *k == kind {
                f(&mut cx);
            }
        }
        HandlerOutcome {
            handled: if cx.stopped() {
                Handled::Yes
            } else {
                Handled::No
            },
            default_prevented: cx.default_prevented(),
        }
    }
}

impl std::fmt::Debug for Handlers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<_> = self.entries.iter().map(|(k, _)| k).collect();
        f.debug_struct("Handlers").field("kinds", &kinds).finish()
    }
}
