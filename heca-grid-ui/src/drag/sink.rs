//! **Where a drop goes when no widget took it.**
//!
//! The twin of the menu sink beside it ([`menu::install_menu_sink`](crate::menu::install_menu_sink)),
//! and for the same reason. A widget owns the gesture — it says it can be dragged, it says it
//! accepts drops, and the framework runs the whole thing — but *what a drop means* is often the
//! host's: moving a pane between workspaces is not something a row can do to itself. So an
//! unclaimed drop crosses back, once, exactly as an unclaimed right-click with a declared menu
//! does.
//!
//! **A widget that handles its own drop still wins.** This is the fallback, not the path: if
//! [`on_drop`](crate::builders::ComponentExt::on_drop) consumed the event, nothing arrives here.
//!
//! Why a sink rather than a handler each row writes: the alternative is the same three lines copied
//! into every row that can be dropped on, and a fourth copy in every plugin that ever wants one —
//! the rule living at the call sites instead of in the middle. Here a row says
//! `.draggable()` and `.drop_target()` and writes nothing else, and a plugin's row is identical
//! because there is nothing for it to miss.

use crate::drag::DropSide;
use crate::event::Modifiers;
use std::cell::RefCell;

/// A drop nothing in the tree claimed: what was carried, what it landed on, and how.
///
/// Both names are widget **identities** — declared with
/// [`key`](crate::builders::ComponentExt::key), or derived from the widget's content when it
/// declared none. The library never interprets them; the host knows what its own names mean, which
/// is what lets a plugin's rows take part without the host knowing what they are.
#[derive(Clone, Debug, PartialEq)]
pub struct Dropped {
    /// The dragged widget's identity.
    pub source: String,
    /// The identity of the widget it was released over.
    pub target: String,
    /// Where in the target it landed — before it, onto it, or after it.
    pub side: DropSide,
    /// The modifiers held at the release. The library has no opinion on what they mean; heca reads
    /// Shift as "swap these two".
    pub modifiers: Modifiers,
}

type DropSink = Box<dyn Fn(Dropped)>;

thread_local! {
    /// Host-installed sink every unclaimed drop posts to. Thread-local because the UI runs
    /// single-threaded, exactly like the menu sink and the frame-request hook beside it.
    static DROP_SINK: RefCell<Option<DropSink>> = const { RefCell::new(None) };
}

/// Install the callback that **acts on** a drop no widget took. Call once at startup.
///
/// ```ignore
/// heca_grid_ui::drag::install_drop_sink(move |d| {
///     // Arrives already resolved: what was dragged, what it landed on, and which side.
///     queue.borrow_mut().push(d);
/// });
/// ```
///
/// The host will usually queue it and act in its own frame, because acting needs the mutable app
/// state a sink does not have — the same shape the menu sink uses.
pub fn install_drop_sink(f: impl Fn(Dropped) + 'static) {
    DROP_SINK.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Hand a drop to the host. A no-op until a sink is installed, so widget code and headless tests
/// can always call it.
pub(crate) fn present(dropped: Dropped) {
    DROP_SINK.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f(dropped);
        }
    });
}

/// Whether a sink is installed — what a host-less test asserts against, and what tells
/// "nothing happened because nothing was dropped" from "nothing happened because there is no host".
pub fn has_drop_sink() -> bool {
    DROP_SINK.with(|c| c.borrow().is_some())
}
