//! The event vocabulary: what happened, said once, for every widget in the library.
//!
//! # Two halves, and the line between them
//!
//! **Raw input** is what a host has: a device moved, a button went down, a wheel turned. It is one
//! type — [`RawPointer`] inside [`Event::Raw`] — and a widget never sees it.
//!
//! **Resolved events** are what a widget handles: [`Event::Click`], [`Event::RightClick`],
//! [`Event::PointerEnter`], [`Event::Drop`]. The framework produces them from the raw stream
//! **once** ([`crate::pointer::route`]), hit-tests the target, and delivers them to that target and
//! then up its ancestors.
//!
//! That line is the whole design. Before it existed, every widget received every pointer event
//! wherever the pointer was, and each one re-derived the same four facts by hand: am I under the
//! cursor, was this press mine, is this the second click, did the pointer just leave me. Six
//! widgets carried their own copy of the hover test; [`Input`](crate::widgets::Input) carried its
//! own double-click clock; and a right-click could not reach a widget **at all**, because the press
//! carried no button — so the app rebuilt "what did you click" from a position and a registry of
//! strings, and the whole chain fell silent when one row forgot to declare its key.
//!
//! # Events say what happened, never what you should do about it
//!
//! [`Event::RightClick`], not `ContextMenu`. Naming an event after the response couples the
//! vocabulary to one reaction: Shift+F10 and the Menu key must be able to *emit* a right-click, and
//! a widget is free to answer one with something that is not a menu. The same rule is why there is
//! no `Event::Activate` here — that is a [`WidgetIntent`], resolved from a configurable binding.
//!
//! # What a widget author has to know
//!
//! Nothing. Put a widget in a tree and the events arrive:
//!
//! ```
//! use heca_grid_ui::prelude::*;
//!
//! let row = Row::new()
//!     .on_click(|_| println!("clicked"))
//!     .on_right_click(|e| println!("menu at {:?}", e.pos()));
//! ```
//!
//! …and inside a widget, the same events arrive as ordinary matches:
//!
//! ```ignore
//! fn on_event(&mut self, ev: &Event) -> Handled {
//!     match ev {
//!         // Already hit-tested: no `bounds.contains(pos)` here, ever.
//!         Event::Click(_) => { self.fire(); Handled::Yes }
//!         _ => Handled::No,
//!     }
//! }
//! ```

use crate::drag::DragItemId;
use heca_core::layout::{Point, Rectangle};
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

/// A renderer-agnostic keyboard key. No `winit` types leak into this crate; the
/// host maps its platform keys onto this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridKey {
    Char(char),
    Enter,
    Space,
    Tab,
    Escape,
    Backspace,
    Delete,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
}

impl GridKey {
    /// **The key a name means** — the one list, so no surface keeps its own.
    ///
    /// A key has more than one spelling in the wild (a config file writes `esc`, a platform reports
    /// `Escape`, a user types `left`), and every surface that mapped platform input to a `GridKey`
    /// wrote the list out again: heca's app kept one keyed by lowercase config name and its
    /// showcase kept another keyed by `winit::NamedKey`, twelve entries each, free to drift.
    /// Aliases are accepted here so a caller normalises nothing.
    ///
    /// A single character is itself: `"a"` is [`Char('a')`](GridKey::Char). Anything longer that is
    /// not named here is `None` — a function key, a dead key, a modifier on its own.
    ///
    /// ```
    /// use heca_grid_ui::GridKey;
    ///
    /// assert_eq!(GridKey::from_name("Escape"), Some(GridKey::Escape));
    /// assert_eq!(GridKey::from_name("esc"), Some(GridKey::Escape));
    /// assert_eq!(GridKey::from_name("ArrowLeft"), Some(GridKey::ArrowLeft));
    /// assert_eq!(GridKey::from_name("left"), Some(GridKey::ArrowLeft));
    /// assert_eq!(GridKey::from_name("a"), Some(GridKey::Char('a')));
    /// assert_eq!(GridKey::from_name("F5"), None);
    /// ```
    pub fn from_name(name: &str) -> Option<Self> {
        let lowered = name.to_lowercase();
        Some(match lowered.as_str() {
            "enter" | "return" => GridKey::Enter,
            "space" => GridKey::Space,
            "tab" => GridKey::Tab,
            "escape" | "esc" => GridKey::Escape,
            "backspace" => GridKey::Backspace,
            "delete" | "del" => GridKey::Delete,
            "arrowleft" | "left" => GridKey::ArrowLeft,
            "arrowright" | "right" => GridKey::ArrowRight,
            "arrowup" | "up" => GridKey::ArrowUp,
            "arrowdown" | "down" => GridKey::ArrowDown,
            "home" => GridKey::Home,
            "end" => GridKey::End,
            s => {
                let mut chars = s.chars();
                match (chars.next(), chars.next()) {
                    (Some(c), None) => GridKey::Char(c),
                    _ => return None,
                }
            }
        })
    }
}

/// Keyboard modifier state, renderer-agnostic. The host maps its platform
/// modifiers onto this and broadcasts changes via [`Event::ModifiersChanged`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// The Cmd/Super/Windows key.
    pub meta: bool,
}

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
/// The **item** is a [`DragItemId`], the opaque registry slot the app already hands a draggable
/// widget ([`ComponentExt::draggable`](crate::builders::ComponentExt::draggable)). The library moves it
/// around and hands it back; only the host knows what it means.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragEvent {
    /// What is being dragged — the source's [`Base::drag_source`](crate::component::Base::drag_source).
    pub item: DragItemId,
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

/// An event delivered to the component tree.
///
/// **Two halves.** [`Raw`](Self::Raw) is the host's input, resolved by
/// [`pointer::route`](crate::pointer::route) and never delivered to a widget. Everything else is
/// what a widget handles — see the [module docs](self) for why the line is drawn there.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    // ─────────────────────────── raw input (host → framework) ───────────────────────────
    /// **The host's raw pointer stream**, and the only pointer event a host constructs.
    ///
    /// The framework resolves it once — hit-test, hover transitions, press/release pairing, click
    /// runs, drag thresholds — and delivers the resolved events below. A widget matching on this
    /// is reading input the framework has not finished interpreting, and will re-derive something
    /// it is about to be told.
    Raw(RawPointer),

    // ───────────────────────────── keyboard ─────────────────────────────
    /// A key went down or up — delivered to the **focused** component (see
    /// [`dispatch`](crate::component::dispatch), which refuses to let an unfocused widget claim
    /// one on the way down).
    ///
    /// This is the *key*, not the *text* it produces: `Shift+2` is `Char('2')` here and `"@"` in
    /// [`TextInput`](Self::TextInput). A field inserts from `TextInput`; a shortcut reads this.
    Key { key: GridKey, pressed: bool },
    /// **Text the user committed** — a typed character, a pasted run, an IME composition result.
    ///
    /// Separate from [`Key`](Self::Key) because they answer different questions, and conflating
    /// them is why the host carried a fixup that re-derived the real character from a keyboard
    /// combo before handing it over, and why an IME could commit nothing at all.
    TextInput(String),
    /// Modifier keys changed — broadcast to the whole tree so widgets can track
    /// state (e.g. for word-wise editing). Observers should return `Handled::No`.
    ModifiersChanged(Modifiers),
    /// A **semantic widget intent** — the host-owned, configurable counterpart to raw
    /// keys, shared by every interactive widget. The host resolves the `[keys.widgets]`
    /// bindings (via a [`Keymap`](crate::keymap::Keymap)) into these, so widgets carry no
    /// hardcoded nav/edit keys. A focused text field still consumes its own raw keys first
    /// (field-first), so typing is never stolen. See [`WidgetIntent`] and
    /// `docs/widgets.md`.
    Widget(WidgetIntent),

    // ──────────────────── resolved pointer (framework → widget) ────────────────────
    /// A button went down on this widget (or a descendant). Consuming it makes this widget the
    /// **capture target**: every move and the release that end the gesture come here, wherever the
    /// cursor goes. That is what a scrollbar thumb needs, and it is no longer something a widget
    /// arranges for itself.
    PointerDown(PointerEvent),
    /// The button came up, ending the gesture that [`PointerDown`](Self::PointerDown) began.
    PointerUp(PointerEvent),
    /// The pointer moved, delivered to the capture target if there is one, otherwise to whatever
    /// is under it now.
    PointerMove(PointerEvent),
    /// The pointer came over this widget — sent once, to the whole ancestor chain of the widget
    /// under it, exactly like CSS `:hover`. Read [`Base::hovered`](crate::component::Base::hovered)
    /// for the state; handle this when entering has to *do* something.
    PointerEnter(PointerEvent),
    /// The pointer left this widget. Always delivered, including when the press that left it was
    /// consumed by somebody else: leaving is not something another widget can veto.
    PointerLeave(PointerEvent),
    /// Press and release on the same widget, with the **left** button. `click_count` is `1`.
    Click(PointerEvent),
    /// The second click of a run (`click_count == 2`). The first still arrived as a
    /// [`Click`](Self::Click) — a widget that acts on both gets one then the other, and none is
    /// held back waiting to see whether a second is coming.
    DoubleClick(PointerEvent),
    /// The third click of a run (`click_count == 3`), and every one after it.
    TripleClick(PointerEvent),
    /// Press and release on the same widget, with the **right** button.
    ///
    /// Named for what happened, not for the menu it usually opens: `Shift+F10` and the Menu key
    /// emit this too, and a widget may answer it with anything.
    RightClick(PointerEvent),
    /// Press and release on the same widget, with the **middle** button.
    MiddleClick(PointerEvent),
    /// A press landed **somewhere else** — the signal every popup needs to close itself.
    ///
    /// Broadcast to everything that is *not* on the target's ancestor path, so an open
    /// [`Select`](crate::widgets::Select) or menu hears the click that dismisses it without
    /// listening to presses over the whole window.
    PointerDownOutside(PointerEvent),
    /// The wheel turned over this widget. Carries the position (so it is routed, not broadcast)
    /// and the deltas in [`PointerEvent::delta_x`]/[`delta_y`](PointerEvent::delta_y).
    ///
    /// A region that cannot scroll the axis asked for declines, and the event bubbles to the next
    /// one out — which is what makes nested scroll areas work with nothing declared.
    Scroll(PointerEvent),

    // ───────────────────────────── drag and drop ─────────────────────────────
    /// A drag began on this widget: it declared a
    /// [`drag_source`](crate::component::Base::drag_source) and the pointer moved past the
    /// threshold while held.
    DragStart(DragEvent),
    /// The drag moved — delivered to the **source**, once per pointer move.
    Drag(DragEvent),
    /// The drag ended, whether it dropped on something or nothing. Delivered to the source after
    /// any [`Drop`](Self::Drop), so a source can always undo its own "being dragged" state.
    DragEnd(DragEvent),
    /// A drag came over this **drop target** — it declared a
    /// [`drop_target`](crate::component::Base::drop_target).
    DragEnter(DragEvent),
    /// The drag moved within this drop target; `side` follows the pointer, so an insertion marker
    /// can track it.
    DragOver(DragEvent),
    /// The drag left this drop target.
    DragLeave(DragEvent),
    /// The drag was released over this drop target.
    Drop(DragEvent),

    // ───────────────────────────── focus and lifetime ─────────────────────────────
    /// This widget gained keyboard focus. There was a `focused` **signal** and no event, so
    /// nothing could act on the moment — select-all on focus, opening a dropdown, arming a caret.
    Focus,
    /// This widget lost keyboard focus — the moment to commit an edit or close a popup.
    Blur,
    /// This widget entered a live tree: sent once, on the first layout pass that sees it.
    Mount,
    /// This widget is being dropped. Sent from [`Base`]'s destructor, so it fires when a rebuilt
    /// tree throws the old one away — the moment to release whatever the widget registered with
    /// the host. Only [`Base`] handlers see it; the widget itself is already coming apart.
    Unmount,
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

impl Event {
    /// A raw pointer move at `pos`.
    pub fn pointer_moved(pos: Point) -> Self {
        Self::Raw(RawPointer::new(RawPointerKind::Moved, pos))
    }

    /// A raw press of `button` at `pos`.
    pub fn pointer_pressed(pos: Point, button: PointerButton) -> Self {
        Self::Raw(RawPointer::new(RawPointerKind::Pressed, pos).with_button(button))
    }

    /// A raw release of `button` at `pos`.
    pub fn pointer_released(pos: Point, button: PointerButton) -> Self {
        Self::Raw(RawPointer::new(RawPointerKind::Released, pos).with_button(button))
    }

    /// A raw wheel turn at `pos`, in lines.
    pub fn wheel(pos: Point, delta_x: f32, delta_y: f32) -> Self {
        let mut raw = RawPointer::new(RawPointerKind::Wheel, pos);
        raw.delta_x = delta_x;
        raw.delta_y = delta_y;
        Self::Raw(raw)
    }

    /// The pointer left the window: clear hover, capture and any drag.
    pub fn pointer_cancelled() -> Self {
        Self::Raw(RawPointer::new(
            RawPointerKind::Cancelled,
            Point::new(f64::MIN, f64::MIN),
        ))
    }

    /// **Where this event happened**, if it happened anywhere in particular.
    ///
    /// Every pointer-carrying event answers with the cursor; a key, a text commit or a lifecycle
    /// event answers `None`, because they did not occur at a position and inventing one is how a
    /// panel ends up in the corner of the screen.
    pub fn position(&self) -> Option<Point> {
        match self {
            Self::Raw(r) => Some(r.pos),
            Self::PointerDown(p)
            | Self::PointerUp(p)
            | Self::PointerMove(p)
            | Self::PointerEnter(p)
            | Self::PointerLeave(p)
            | Self::Click(p)
            | Self::DoubleClick(p)
            | Self::TripleClick(p)
            | Self::RightClick(p)
            | Self::MiddleClick(p)
            | Self::PointerDownOutside(p)
            | Self::Scroll(p) => Some(p.pos),
            Self::DragStart(d)
            | Self::Drag(d)
            | Self::DragEnd(d)
            | Self::DragEnter(d)
            | Self::DragOver(d)
            | Self::DragLeave(d)
            | Self::Drop(d) => Some(d.pos),
            _ => None,
        }
    }

    /// **The bounds of the widget this event was delivered to**, as stamped by the router.
    ///
    /// The other half of "an author never picks an anchor": [`position`](Self::position) is where
    /// the pointer is, this is what it is on. A menu opened from a right-click hangs off the
    /// cursor; one opened from a widget hangs under the widget — and the handler picks neither,
    /// it just passes the event along.
    ///
    /// `None` for events that carry no pointer, and for a synthetic event nobody routed.
    pub fn target_bounds(&self) -> Option<Rectangle> {
        match self {
            Self::PointerDown(p)
            | Self::PointerUp(p)
            | Self::PointerMove(p)
            | Self::PointerEnter(p)
            | Self::PointerLeave(p)
            | Self::Click(p)
            | Self::DoubleClick(p)
            | Self::TripleClick(p)
            | Self::RightClick(p)
            | Self::MiddleClick(p)
            | Self::PointerDownOutside(p)
            | Self::Scroll(p) => p.target_bounds,
            _ => None,
        }
    }

    /// The same event, stamped with the bounds of the widget it is being delivered to.
    ///
    /// The router calls this once per delivery, which is why every handler can ask
    /// [`target_bounds`](Self::target_bounds) without any widget arranging for it. A non-pointer
    /// event is returned unchanged — there is nowhere to put it, and nothing that reads it.
    pub(crate) fn with_target_bounds(self, bounds: Rectangle) -> Self {
        let stamp = |mut p: PointerEvent| {
            p.target_bounds = Some(bounds);
            p
        };
        match self {
            Self::PointerDown(p) => Self::PointerDown(stamp(p)),
            Self::PointerUp(p) => Self::PointerUp(stamp(p)),
            Self::PointerMove(p) => Self::PointerMove(stamp(p)),
            Self::PointerEnter(p) => Self::PointerEnter(stamp(p)),
            Self::PointerLeave(p) => Self::PointerLeave(stamp(p)),
            Self::Click(p) => Self::Click(stamp(p)),
            Self::DoubleClick(p) => Self::DoubleClick(stamp(p)),
            Self::TripleClick(p) => Self::TripleClick(stamp(p)),
            Self::RightClick(p) => Self::RightClick(stamp(p)),
            Self::MiddleClick(p) => Self::MiddleClick(stamp(p)),
            Self::Scroll(p) => Self::Scroll(stamp(p)),
            // Deliberately NOT `PointerDownOutside`: it is broadcast to everything the press did
            // *not* land on, so "the target" is somebody else's widget and stamping each receiver
            // with its own bounds would be a lie about where the press was.
            other => other,
        }
    }

    /// This event's kind — the key a [`Base`] handler is registered under.
    pub fn kind(&self) -> EventKind {
        match self {
            Self::Raw(r) => match r.kind {
                RawPointerKind::Moved => EventKind::PointerMove,
                RawPointerKind::Pressed => EventKind::PointerDown,
                RawPointerKind::Released => EventKind::PointerUp,
                RawPointerKind::Wheel => EventKind::Scroll,
                RawPointerKind::Cancelled => EventKind::PointerLeave,
            },
            Self::Key { .. } => EventKind::Key,
            Self::TextInput(_) => EventKind::TextInput,
            Self::ModifiersChanged(_) => EventKind::ModifiersChanged,
            Self::Widget(_) => EventKind::Widget,
            Self::PointerDown(_) => EventKind::PointerDown,
            Self::PointerUp(_) => EventKind::PointerUp,
            Self::PointerMove(_) => EventKind::PointerMove,
            Self::PointerEnter(_) => EventKind::PointerEnter,
            Self::PointerLeave(_) => EventKind::PointerLeave,
            Self::Click(_) => EventKind::Click,
            Self::DoubleClick(_) => EventKind::DoubleClick,
            Self::TripleClick(_) => EventKind::TripleClick,
            Self::RightClick(_) => EventKind::RightClick,
            Self::MiddleClick(_) => EventKind::MiddleClick,
            Self::PointerDownOutside(_) => EventKind::PointerDownOutside,
            Self::Scroll(_) => EventKind::Scroll,
            Self::DragStart(_) => EventKind::DragStart,
            Self::Drag(_) => EventKind::Drag,
            Self::DragEnd(_) => EventKind::DragEnd,
            Self::DragEnter(_) => EventKind::DragEnter,
            Self::DragOver(_) => EventKind::DragOver,
            Self::DragLeave(_) => EventKind::DragLeave,
            Self::Drop(_) => EventKind::Drop,
            Self::Focus => EventKind::Focus,
            Self::Blur => EventKind::Blur,
            Self::Mount => EventKind::Mount,
            Self::Unmount => EventKind::Unmount,
        }
    }

    /// The pointer payload, for the events that carry one.
    pub fn pointer(&self) -> Option<&PointerEvent> {
        match self {
            Self::PointerDown(p)
            | Self::PointerUp(p)
            | Self::PointerMove(p)
            | Self::PointerEnter(p)
            | Self::PointerLeave(p)
            | Self::Click(p)
            | Self::DoubleClick(p)
            | Self::TripleClick(p)
            | Self::RightClick(p)
            | Self::MiddleClick(p)
            | Self::PointerDownOutside(p)
            | Self::Scroll(p) => Some(p),
            _ => None,
        }
    }

    /// The drag payload, for the events that carry one.
    pub fn drag(&self) -> Option<&DragEvent> {
        match self {
            Self::DragStart(d)
            | Self::Drag(d)
            | Self::DragEnd(d)
            | Self::DragEnter(d)
            | Self::DragOver(d)
            | Self::DragLeave(d)
            | Self::Drop(d) => Some(d),
            _ => None,
        }
    }
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

/// The **name** of an event, with none of its payload — what a [`Base`] handler is keyed by.
///
/// One kind per [`Event`] variant, except that the raw pointer stream maps onto the resolved kind
/// it produces: a handler is registered for `Click`, never for "a raw press".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Key,
    TextInput,
    ModifiersChanged,
    Widget,
    PointerDown,
    PointerUp,
    PointerMove,
    PointerEnter,
    PointerLeave,
    Click,
    DoubleClick,
    TripleClick,
    RightClick,
    MiddleClick,
    PointerDownOutside,
    Scroll,
    DragStart,
    Drag,
    DragEnd,
    DragEnter,
    DragOver,
    DragLeave,
    Drop,
    Focus,
    Blur,
    Mount,
    Unmount,
}

/// A **semantic widget intent** — one shared vocabulary every interactive widget speaks
/// instead of hardcoding keys (the host maps `[keys.widgets]` → these via a
/// [`Keymap`](crate::keymap::Keymap)). Split by **axis**: horizontal (`Item*`), vertical
/// (`Menu*`), the shared `Activate`/`Dismiss`, and text-field edits (`Edit*`).
///
/// A single key may resolve to **several** intents (e.g. `Ctrl+h` → `EditDeleteBack`
/// *then* `ItemPrevious`); the host delivers them in order to the focused widget, which
/// consumes the one it understands (an `Input` deletes, a `Tabs`/`Dialog` moves) — so the
/// overload disambiguates by focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetIntent {
    /// Horizontal previous (left) — `Tabs`, a `Dialog`'s button row. `item_previous`.
    ItemPrevious,
    /// Horizontal next (right) — `Tabs`, a `Dialog`'s button row. `item_next`.
    ItemNext,
    /// Vertical up — menus, `Select` lists, the command palette. `menu_up`.
    MenuUp,
    /// Vertical down — menus, `Select` lists, the command palette. `menu_down`.
    MenuDown,
    /// An **older** past query — a search surface's history, walked backwards. `menu_history_up`.
    ///
    /// Vocabulary, not a palette feature: any surface that searches has a history worth recalling.
    /// Up/down rather than previous/next because the table splits by axis (`item_*` is horizontal,
    /// `menu_*` vertical) and because beside `item_previous`, a "history_prev" reads ambiguously —
    /// an older entry, or one row up?
    MenuHistoryUp,
    /// A **newer** past query, and past the newest, the draft the walk interrupted.
    /// `menu_history_down`.
    MenuHistoryDown,
    /// Activate / commit / submit the current entry or primary action. `activate`.
    Activate,
    /// Dismiss / cancel / close the overlay. `dismiss`.
    Dismiss,
    /// [`Input`](crate::widgets::Input): delete one char before the caret. `edit_delete_back`.
    EditDeleteBack,
    /// [`Input`](crate::widgets::Input): delete from the caret to line start. `edit_delete_to_line_start`.
    EditDeleteToLineStart,
    /// [`Input`](crate::widgets::Input): select the whole field. `edit_select_all`.
    EditSelectAll,

    // ── Scrolling a scroll area from the keyboard (F003/P011/T012) ──
    //
    // These are **semantic**, not keys: the host says "one page back", the widget decides what a
    // page is and clamps the result, because it is the only thing that knows its viewport and its
    // content. Delivering `PageUp` as a key instead would have needed `GridKey` to grow variants
    // and would have put paging arithmetic in the app.
    //
    // Both axes, because a region can be horizontal or two-axis. A region that cannot scroll the
    // axis asked for declines, so a nested one still gets its turn — the same rule the wheel
    // follows.
    /// Scroll a scroll area one page back, vertically.
    ScrollPageUp,
    /// Scroll a scroll area one page on, vertically.
    ScrollPageDown,
    /// Jump a scroll area to the top.
    ScrollToTop,
    /// Jump a scroll area to the bottom.
    ScrollToBottom,
    /// Scroll a scroll area one page back, horizontally.
    ScrollPageLeft,
    /// Scroll a scroll area one page on, horizontally.
    ScrollPageRight,
    /// Jump a scroll area to its left edge.
    ScrollToLeftEdge,
    /// Jump a scroll area to its right edge.
    ScrollToRightEdge,
}

/// What a [`Base`] handler is given: the event, and the one lever it has over the walk.
///
/// A handler is not asked to return anything. Most of them do their work and let the event carry
/// on; the ones that own it say so, once, by name — which is the difference between "I did
/// something" and "nobody else should".
pub struct EventCx<'a> {
    event: &'a Event,
    stop: bool,
}

impl<'a> EventCx<'a> {
    /// Wrap `event` for delivery to a handler.
    pub fn new(event: &'a Event) -> Self {
        Self { event, stop: false }
    }

    /// The event being delivered.
    pub fn event(&self) -> &'a Event {
        self.event
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

    /// The modifiers held when it happened.
    pub fn modifiers(&self) -> Modifiers {
        self.event
            .pointer()
            .map(|p| p.modifiers)
            .or_else(|| self.event.drag().map(|d| d.modifiers))
            .unwrap_or_default()
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
}

/// **The text a keystroke committed, if it committed any** — the one rule that decides whether a
/// key is *typing* or a *shortcut*.
///
/// A host has the platform's two facts (the key, and the text it produced) and has to turn them
/// into [`Event::TextInput`] plus [`Event::Key`]. Which chords count as typing is not a decision
/// each host should make: heca's app and its showcase made it separately, and the showcase simply
/// never made it at all — so the identical `CommandPalette` typed in one and was deaf in the other.
///
/// Space **is** typing: a field must be able to type one. A focused button still activates on it,
/// because a button does not consume `TextInput` and the key follows right behind.
///
/// ```
/// use heca_grid_ui::{typed_text, Modifiers};
///
/// let plain = Modifiers::default();
/// assert_eq!(typed_text(Some("a"), plain).as_deref(), Some("a"));
/// assert_eq!(typed_text(Some(" "), plain).as_deref(), Some(" "));
/// // A shortcut is not typing, and neither is a control character.
/// assert!(typed_text(Some("a"), Modifiers { ctrl: true, ..plain }).is_none());
/// assert!(typed_text(Some("\u{1}"), plain).is_none());
/// assert!(typed_text(None, plain).is_none());
/// ```
pub fn typed_text(key_text: Option<&str>, mods: Modifiers) -> Option<String> {
    let text = key_text?;
    if mods.ctrl || mods.meta || text.is_empty() {
        return None;
    }
    text.chars().all(|c| !c.is_control()).then(|| text.to_string())
}

/// A widget's registered event handlers, keyed by [`EventKind`].
///
/// Lives behind an `Option<Box<…>>` on [`Base`], so a widget that registers none costs one null
/// pointer. Several handlers may share a kind; they run in registration order, and the first to
/// call [`EventCx::stop_propagation`] ends the walk (the rest of *this* widget's handlers for that
/// kind still run — they were registered by the same author, on the same widget).
/// One registered handler: a callback that may take the event.
type Handler = Box<dyn FnMut(&mut EventCx<'_>)>;

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
    pub fn run(&mut self, ev: &Event) -> Handled {
        let kind = ev.kind();
        let mut cx = EventCx::new(ev);
        for (k, f) in self.entries.iter_mut() {
            if *k == kind {
                f(&mut cx);
            }
        }
        if cx.stopped() { Handled::Yes } else { Handled::No }
    }
}

impl std::fmt::Debug for Handlers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kinds: Vec<_> = self.entries.iter().map(|(k, _)| k).collect();
        f.debug_struct("Handlers").field("kinds", &kinds).finish()
    }
}
