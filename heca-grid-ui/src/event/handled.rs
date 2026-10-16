//! **"Was this mine?"** — the one answer every widget gives to every event.

// Named only in the docs below.
#[cfg(doc)]
use super::{Event, EventCx};

/// **The answer every widget gives to every event: "was this mine?"**
///
/// It is the only lever a widget has over the walk, and it does more than it looks like it does —
/// so read this before returning `Yes` out of habit.
///
/// # It is `stopPropagation` **and** `preventDefault` at once
///
/// If you know the DOM: returning `Yes` from
/// [`on_event_capture`](crate::component::Component::on_event_capture) is `stopPropagation()`
/// called in a capture listener — nothing below sees the event. Returning `Yes` from
/// [`on_event`](crate::component::Component::on_event) is the same in a bubble listener — no
/// ancestor sees it. The difference is that the DOM splits "nobody else handles this" from "and
/// the built-in behaviour must not run"; here there is one answer, and `Yes` means both. It is
/// closer to jQuery's `return false` than to `stopPropagation()` alone.
///
/// A [`Base`](crate::component::Base) handler says the same thing by name:
/// [`EventCx::stop_propagation`].
///
/// # A third meaning the DOM does not have: the host reads it
///
/// [`dispatch`](crate::component::dispatch) returns the accumulated answer, and the host uses it to
/// decide whether the input **also** belongs to whatever sits behind the tree. In heca that is the
/// terminal: `No` on a wheel is what lets the pane scroll instead of the sidebar, and `No` on a
/// right press is what lets the context menu open. So `Yes` is not a private decision — it is how a
/// widget tells the application "this input is spent".
///
/// **The bug that rule exists to stop:** an overlay returned `Yes` for every key it was offered,
/// including the ones it had no use for. `q`, catalogued and bound to `close_overlay` beside
/// `Escape`, did nothing at all while a layer was up — the layer swallowed it and the host never
/// got to resolve it. **Claim what you act on. Nothing else.**
///
/// # `Yes` on a [`PointerDown`](Event::PointerDown) does three things
///
/// 1. **stops the walk** — this widget's composed content never sees the press;
/// 2. **captures the pointer** — every move, and the release, come back to this widget wherever
///    the cursor goes (the DOM's `setPointerCapture`, without asking);
/// 3. **claims the click** — the [`Click`](Event::Click) that press turns into is delivered to
///    this widget rather than to the deepest thing under the cursor.
///
/// That is why a control does not write this by hand: it declares
/// [`Base::one_click_target`](crate::component::Base::one_click_target) and the router does all
/// three, **for the primary button only**. Nine widgets once wrote the claim themselves and all
/// nine claimed *every* button — which is how a right-click on a list row reached nothing at all,
/// while the same click on empty space opened a menu.
///
/// # Which answer to give
///
/// | Situation | Answer |
/// |---|---|
/// | I acted on this event | `Yes` |
/// | I looked and it is not mine | `No` — including from capture, which just means "I looked, carry on" |
/// | I observed it and others should still get it (a modifier broadcast, a hover cue) | `No` |
/// | I am a container and my child should decide | `No` — you do not forward anything; the framework already walked there |
///
/// # Events whose answer is ignored
///
/// [`PointerEnter`](Event::PointerEnter) / [`PointerLeave`](Event::PointerLeave) — leaving is an
/// announcement, and a widget must not be able to veto its neighbour's; the same for
/// [`PointerDownOutside`](Event::PointerDownOutside) (the press belongs to whatever it landed on),
/// [`Mount`](Event::Mount) / [`Unmount`](Event::Unmount), and
/// [`after_subtree`](crate::component::Component::after_subtree), which cannot consume an event or
/// revive a consumed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handled {
    /// This widget acted on the event: the walk stops here, the default is done, and the host is
    /// told the input is spent.
    Yes,
    /// Not this widget's: the walk carries on to whoever it is for.
    No,
}
