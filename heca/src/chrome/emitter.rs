//! **How chrome widgets report a gesture back to the app** — the intent sink, and the click and
//! pick closures built on it.

use super::Intent;
use std::rc::Rc;

/// How a chrome widget reports a user action back to the app: it emits an
/// `InteractionIntent`, never a direct
/// state mutation. Handed to container providers through
/// [`ChromeCtx::emit_intent`](crate::providers::ChromeCtx::emit_intent).
#[derive(Clone)]
pub struct ChromeIntentEmitter {
    source: crate::app::interaction::InteractionSource,
    sink: Rc<
        dyn Fn(
            crate::app::interaction::InteractionSource,
            crate::app::interaction::InteractionIntent,
        ),
    >,
}

impl ChromeIntentEmitter {
    /// A sink that stamps everything it posts with `source`.
    pub(crate) fn new(
        event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
        source: crate::app::interaction::InteractionSource,
    ) -> Self {
        let event_proxy = event_proxy.clone();
        Self {
            source,
            sink: Rc::new(move |source, intent| {
                let _ = event_proxy
                    .send_event(crate::app::events::AppEvent::ChromeIntent { source, intent });
            }),
        }
    }

    /// A sink of your own — what a test uses to record what a component emitted.
    #[cfg(test)]
    pub(crate) fn of(
        source: crate::app::interaction::InteractionSource,
        sink: impl Fn(
            crate::app::interaction::InteractionSource,
            crate::app::interaction::InteractionIntent,
        ) + 'static,
    ) -> Self {
        Self {
            source,
            sink: Rc::new(sink),
        }
    }

    /// **Where intents from this tree come from.**
    ///
    /// It is readable, and that is the point: asking *"would this be allowed?"* and *running* it
    /// have to ask the identical question, and they cannot while one of them invents an answer.
    /// The `prefix+/` picker asked as `Keyboard` while the chrome tree dispatches as
    /// `MouseLeftSidebar`, so the two disagreed about the domain — the picker offered letters in
    /// `Container` that execution then refused in `Floating`.
    pub(crate) fn source(&self) -> crate::app::interaction::InteractionSource {
        self.source
    }

    /// Post an intent, stamped with this sink's source.
    pub(crate) fn fire(&self, intent: crate::app::interaction::InteractionIntent) {
        (self.sink)(self.source, intent)
    }
}

/// **The intent sink for one layer's own retained tree** — the exposé's cards, a modal's buttons, a
/// plugin panel's widgets.
///
/// Every intent it posts is stamped
/// [`InteractionSource::Surface`](crate::app::interaction::InteractionSource::Surface) with that
/// layer's id, which is how `domain_for` tells "the surface holding the keyboard acted on itself"
/// from "the user typed at the app behind it". The two arrive as the same event and only this
/// stamp separates them.
///
/// One function rather than a closure per call site, so no layer can claim a different source for
/// its intents.
///
/// Take the id from [`LayerRegistry::reserve_id`](super::LayerRegistry::reserve_id) when the tree
/// has to be built before the layer is inserted, which is the usual case.
pub(crate) fn layer_emitter(
    event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
    key: crate::app::interaction::SurfaceKey,
) -> ChromeIntentEmitter {
    ChromeIntentEmitter::new(
        event_proxy,
        crate::app::interaction::InteractionSource::Surface(key),
    )
}

/// **Fire a named gesture**: the closure that emits `intent` through this surface's chrome sink.
///
/// This is `realize`'s `emit(intent)` for native code — deliberately the same one line, so the two
/// authoring paths produce the same wiring:
///
/// ```ignore
/// // declarative (heca-view-realize):     native (here):
/// row.on_press(intent)                    row.on_activate(fires(mount, intent, emit))
/// row.on_hint(intent)                     row.on_hint(picks(mount, intent, emit))
/// ```
///
/// **Why a name and not a closure.** A closure is reachable from exactly one place: the gesture
/// that captured it. An [`Intent`] is an action id plus arguments, so the same declaration answers
/// a click, a menu entry, a keybinding and an RPC call, is routed by the action's own policy, and
/// passes the destructive-confirm gate — none of which a closure can be. It is also the only form
/// that crosses a plugin boundary, so a native row and a WASM row declare their gestures the same
/// way.
///
/// **`mount` is not ceremony — it is which element this is.** A widget is built inside one mounted
/// container and the gesture it declares carries that seating
/// (`SEAT_ARG`), so the same container seated twice has two rows that
/// each answer for themselves. It is a parameter rather than something a caller may add, because a
/// rule a caller has to remember is a rule that gets forgotten: without it the host had to resolve
/// the call back to an instance by guessing (focused seat, else last focused, else the first one
/// that declares the name), and a right-sidebar row was performed by the left-sidebar copy. Every
/// call site already holds its mount — it is what `BuildCx::container_id` hands a component.
///
/// A click and a `prefix+/` pick are different gestures — a click on a sidebar row means *go there
/// and leave*, a pick means *look at that one* — so each has its own builder: this one and
/// [`picks`].
pub fn fires(mount: &str, intent: Intent, emit: &ChromeIntentEmitter) -> impl Fn() + 'static {
    let seated = seated(mount, intent);
    let emit = emit.clone();
    move || {
        emit.fire(crate::app::interaction::InteractionIntent::View(
            seated.clone(),
        ))
    }
}

/// **What a `prefix+/` pick does** — [`fires`], plus the declaration of *what it is*.
///
/// The same closure, carried in a [`Hint`](heca_grid_ui::Hint) that keeps the [`Intent`] beside it.
/// A closure alone is opaque, and a host cannot ask its policy about an opaque thing — so a pick
/// whose action the domain would refuse would still be offered a letter that does nothing. With
/// the intent in hand, `chrome::active_hint_targets` drops the candidate
/// before the letter is spent.
///
/// Reach for it wherever a pick has a name. A pick that genuinely has none is still a plain closure
/// — it simply cannot be judged, and is offered as it always was.
pub fn picks(mount: &str, intent: Intent, emit: &ChromeIntentEmitter) -> heca_grid_ui::Hint {
    let seated = seated(mount, intent);
    let run = seated.clone();
    let emit = emit.clone();
    heca_grid_ui::Hint::of(seated, move || {
        emit.fire(crate::app::interaction::InteractionIntent::View(
            run.clone(),
        ))
    })
}

/// Stamp the seating onto an intent — **which element this is**, so the same container seated twice
/// has two rows that each answer for themselves. One definition, so a click and a pick of the same
/// row cannot be addressed differently.
fn seated(mount: &str, mut intent: Intent) -> Intent {
    intent.args.insert(
        crate::providers::SEAT_ARG.to_string(),
        heca_view::PropValue::Text(mount.to_string()),
    );
    intent
}
