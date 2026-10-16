//! What a pointer event carries — the button, the resolved pointer, drag and pick details — and the
//! raw device input a host hands in.

// Named only in the docs below.
#[cfg(doc)]
use super::Event;
use super::{EventKind, Modifiers};
use heca_core::layout::{Point, Rectangle};

/// Which pointer button an event is about.
///
/// A press used to carry none, so a widget could not tell a right-click from a left one — which is
/// why right-click was an app-level gesture rebuilt from a position, and why no widget could own
/// its own menu. `Other` keeps a five-button mouse from being silently reported as a left click.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    /// Any further physical button, by index as the host reports it.
    Other(u16),
}

impl PointerButton {
    /// The click event this button produces: [`Event::Click`] for `Left`,
    /// [`Event::RightClick`] for `Right`, [`Event::MiddleClick`] for `Middle`.
    /// `Other` produces none — a widget that wants those reads [`Event::PointerUp`].
    pub fn click_kind(self) -> Option<EventKind> {
        match self {
            Self::Left => Some(EventKind::Click),
            Self::Right => Some(EventKind::RightClick),
            Self::Middle => Some(EventKind::MiddleClick),
            Self::Other(_) => None,
        }
    }
}

/// Everything a resolved pointer event carries: where, which button, which modifiers, and — for
/// a click — how many in a row.
///
/// One payload for the whole pointer vocabulary, so a widget reads `e.pos` the same way whether it
/// matched a [`Click`](Event::Click), a [`PointerEnter`](Event::PointerEnter) or a
/// [`Scroll`](Event::Scroll). **The position is already hit-tested**: a widget receiving one is the
/// target or an ancestor of it, so `bounds.contains(e.pos)` is never the question.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerEvent {
    /// Where the pointer is, in logical pixels.
    pub pos: Point,
    /// The button this event is about. Moves, wheels and enter/leave report the button that is
    /// held (or [`PointerButton::Left`] when none is).
    pub button: PointerButton,
    /// The modifier keys held at the moment the host produced the raw event.
    pub modifiers: Modifiers,
    /// How many clicks in this run: `1` for a [`Click`](Event::Click), `2` for a
    /// [`DoubleClick`](Event::DoubleClick), `3` for a [`TripleClick`](Event::TripleClick), and `4`
    /// and up for further clicks in the same run (a widget with a 4-state cycle reads this rather
    /// than counting). `0` for events that are not clicks.
    pub click_count: u32,
    /// Wheel movement in **lines** — non-zero only on [`Scroll`](Event::Scroll). Positive
    /// `delta_y` scrolls the content down, positive `delta_x` scrolls it right. The host maps
    /// device deltas and any modifier convention (`Shift`+wheel → horizontal) onto these, so a
    /// widget reads the axis it wants and never tracks a modifier to find it.
    pub delta_x: f32,
    /// Wheel movement in lines along Y. See [`delta_x`](Self::delta_x).
    pub delta_y: f32,
    /// **The device's own pixels, when it reports pixels** (a trackpad) — the DOM's `deltaMode`,
    /// as a value: `Some` means the device counts pixels and this is the movement in the same
    /// convention as [`delta_x`](Self::delta_x)/[`delta_y`](Self::delta_y); `None` means it counts
    /// notches (a wheel) and the line deltas are the whole story.
    ///
    /// The line deltas are always filled in, from pixels by a fixed ratio when need be, so a
    /// widget that only wants "how much" never reads this. One that scrolls by its own unit — a
    /// terminal, by its cell height — reads it, because dividing a ratio back out of the lines
    /// would mean knowing the host's constant.
    pub delta_pixels: Option<(f32, f32)>,
    /// **The laid-out bounds of the widget this event is being delivered to**, stamped by the
    /// router at delivery.
    ///
    /// It is what lets a handler anchor something to the widget it fired on without the author
    /// measuring anything — `ContextMenu::show(ev)` reads it to hang a menu under the row that was
    /// right-clicked. `None` on a synthetic event nobody routed.
    pub target_bounds: Option<Rectangle>,
}

impl PointerEvent {
    /// A pointer event at `pos` with the left button, no modifiers and no click run — the shape
    /// tests and synthetic callers want.
    pub fn at(pos: Point) -> Self {
        Self {
            pos,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
            click_count: 0,
            delta_x: 0.0,
            delta_y: 0.0,
            delta_pixels: None,
            target_bounds: None,
        }
    }

    /// The same event stamped with the delivery target's bounds — what the router does on the way
    /// in, and what a test does to stand in for it.
    pub fn with_target_bounds(mut self, bounds: Rectangle) -> Self {
        self.target_bounds = Some(bounds);
        self
    }

    /// The same event with `button` instead.
    pub fn with_button(mut self, button: PointerButton) -> Self {
        self.button = button;
        self
    }

    /// The same event with `modifiers` instead.
    pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}

/// What a drag event carries: the dragged item, where the pointer is, and — on a drop — which
/// half of the target it landed on.
///
/// The **item** is the dragged widget's own name — what it declared with
/// [`IdentityExt::key`](crate::builders::IdentityExt::key), the same identity the keyboard cursor
/// and the right-click target read. The library moves it around and hands it back; only the host
/// knows what it means.
#[derive(Debug, Clone, PartialEq)]
pub struct DragEvent {
    /// What is being dragged — the source's own [`Base::key`](crate::component::Base::key).
    pub item: String,
    /// Where the pointer is, in logical pixels.
    pub pos: Point,
    /// The modifier keys held (a copy-drag is `alt` in most conventions; the library has no
    /// opinion and reports what was down).
    pub modifiers: Modifiers,
    /// Where in the target the pointer sits — `Before`/`After` for a list insertion, `Onto` for a
    /// drop **into** it. Computed from the target's bounds and the drop axis, so every drop target
    /// gets an insertion point without measuring anything.
    pub side: crate::drag::DropSide,
}

/// **Which widget a pick landed on** — what a delegating ancestor needs, and nothing more.
///
/// A pick is *nominal*, not spatial: it names a widget rather than a point, so this carries the
/// picked widget's own identity ([`Base::key`](crate::component::Base::key)) rather than a cursor
/// position. The bounds ride along because that is what a container wants in order to scroll the
/// picked row into view or draw over it — the same thing
/// [`Event::target_bounds`](Event::target_bounds) answers for a pointer event.
#[derive(Clone, Debug, PartialEq)]
pub struct HintEvent {
    /// The picked widget's [`key`](crate::builders::IdentityExt::key), when it declares one.
    ///
    /// `None` is ordinary — most widgets need no key (an ordinary button, an icon), and one is
    /// required only for an item in a collection. A delegating container that iterates a collection
    /// is exactly the case that has one.
    pub key: Option<String>,
    /// The picked widget's laid-out rect, as the letter was drawn over it.
    pub bounds: Rectangle,
}

/// A pointer event exactly as the host produced it — a device fact, before the framework has
/// worked out what it means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawPointer {
    /// What the device did.
    pub kind: RawPointerKind,
    /// Where the pointer is, in logical pixels.
    pub pos: Point,
    /// The button for a press/release; ignored for moves and wheels.
    pub button: PointerButton,
    /// Modifiers held at the time. A host that broadcasts
    /// [`ModifiersChanged`](Event::ModifiersChanged) still fills this in — an event that carries
    /// its own modifiers cannot be read against a state that has moved on since.
    pub modifiers: Modifiers,
    /// Wheel deltas in lines, for [`RawPointerKind::Wheel`].
    pub delta_x: f32,
    /// Wheel deltas in lines, for [`RawPointerKind::Wheel`].
    pub delta_y: f32,
    /// The device's own pixels, when it reports pixels — see [`PointerEvent::delta_pixels`].
    pub delta_pixels: Option<(f32, f32)>,
}

/// What the pointing device did — the four things a host can actually observe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RawPointerKind {
    Moved,
    Pressed,
    Released,
    Wheel,
    /// The pointer left the window (or the host is cancelling the gesture): clears hover and ends
    /// any capture or drag. Without it a widget stays lit after the cursor has gone.
    Cancelled,
}

impl RawPointer {
    /// A raw event of `kind` at `pos`, left button, no modifiers, no wheel delta.
    pub fn new(kind: RawPointerKind, pos: Point) -> Self {
        Self {
            kind,
            pos,
            button: PointerButton::Left,
            modifiers: Modifiers::default(),
            delta_x: 0.0,
            delta_y: 0.0,
            delta_pixels: None,
        }
    }

    /// The same raw event with `button` instead.
    pub fn with_button(mut self, button: PointerButton) -> Self {
        self.button = button;
        self
    }

    /// The same raw event with `modifiers` instead.
    pub fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }
}
