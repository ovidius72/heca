//! **Where a widget's act goes** — the host-installed sink every [`EventCx::dispatch`] posts to.
//!
//! The twin of the menu sink beside it, and for the same reason: a widget can say *what* it wants
//! done, but only the host can do it — it owns the action registry, the permission rules and the
//! confirmation gate. So that one step, and only that step, crosses back.
//!
//! Before this, a widget could act only through a closure its host handed it, which is why any
//! composition that did anything carried a struct of callbacks. A plugin could carry none of them,
//! so a plugin's widget could not act at all.

use std::cell::RefCell;

type IntentSink = Box<dyn Fn(heca_view::Intent)>;

thread_local! {
    /// Thread-local because the UI runs single-threaded, exactly like the menu sink.
    static INTENT_SINK: RefCell<Option<IntentSink>> = const { RefCell::new(None) };
}

/// Install the callback that **performs** what a widget asked for. Call once at startup.
///
/// The host decides what a name means and whether it is allowed; nothing here inspects the intent.
pub fn install_intent_sink(f: impl Fn(heca_view::Intent) + 'static) {
    INTENT_SINK.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Post an intent to the host. A no-op until a sink is installed.
pub(crate) fn dispatch(intent: heca_view::Intent) {
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
