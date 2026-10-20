//! **Everything that can happen to a widget** — [`Event`] — and its payload-free name,
//! [`EventKind`].

use super::{
    DragEvent, GridKey, HintEvent, Modifiers, PointerButton, PointerEvent, RawPointer,
    RawPointerKind, WidgetIntent,
};
use heca_core::layout::{Point, Rectangle};

/// An event delivered to the component tree.
///
/// **Two halves.** [`Raw`](Self::Raw) is the host's input, resolved by
/// [`pointer::route`](crate::pointer::route) and never delivered to a widget. Everything else is
/// what a widget handles — see the [module docs](super) for why the line is drawn there.
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
    /// **A letter from a picker resolved to a widget** — the pick, as an event.
    ///
    /// It travels the same walk everything else does: capture down the picked widget's ancestor
    /// chain, the target, then back up, with `stop_propagation` ending it. There is no second
    /// dispatch path for picks.
    ///
    /// **The widget's own [`on_hint`](crate::builders::HintExt::on_hint) fires in the target
    /// phase only** — a pane that is pickable *and* holds pickable rows must not answer for a row.
    /// What bubbles is this event, which is the **delegation** seam: an ancestor listening with
    /// `.on(EventKind::Hint, …)` sees which widget was picked and may take the pick from it. That
    /// ancestor declares nothing and gets no letter of its own.
    Hint(HintEvent),

    // ──────────────────── resolved pointer (framework → widget) ────────────────────
    /// A button went down on this widget (or a descendant). Consuming it makes this widget the
    /// **capture target**: every move and the release that end the gesture come here, wherever the
    /// cursor goes. That is what a scrollbar thumb needs, and it is no longer something a widget
    /// arranges for itself.
    PointerDown(PointerEvent),
    /// The button came up, ending the gesture that [`PointerDown`](Self::PointerDown) began. The
    /// widget holding the pointer also gets one when the gesture is cancelled — the pointer left the
    /// window — so whatever it keeps for a drag ends either way. Never a click: a click is a press
    /// and a release on one widget, and a cancel is neither.
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
    /// [`draggable`](crate::component::Base::draggable) and the pointer moved past the
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
    /// **A drag this drop target accepts is now in flight** — told once, when it starts, to every
    /// drop target whose [`accepts`](crate::builders::DragExt::accepts) takes what is being
    /// carried, wherever it sits and whether or not it is shown. A target that only exists for the
    /// length of a drag (a landing place) appears on this and goes on [`DragSettled`](Self::DragSettled).
    DragInFlight(DragEvent),
    /// **The drag that was in flight is over**, dropped or not — told to the same targets.
    DragSettled(DragEvent),

    // ───────────────────────────── focus and lifetime ─────────────────────────────
    /// This widget gained keyboard focus. There was a `focused` **signal** and no event, so
    /// nothing could act on the moment — select-all on focus, opening a dropdown, arming a caret.
    Focus,
    /// This widget lost keyboard focus — the moment to commit an edit or close a popup.
    Blur,
    /// This widget entered a live tree: sent once, on the first layout pass that sees it.
    Mount,
    /// This widget is being dropped. Sent from [`Base`](crate::component::Base)'s destructor, so it
    /// fires when a rebuilt tree throws the old one away — the moment to release whatever the
    /// widget registered with the host. Only [`Base`](crate::component::Base) handlers see it; the
    /// widget itself is already coming apart.
    Unmount,
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
            _ => self
                .pointer()
                .map(|p| p.pos)
                .or_else(|| self.drag().map(|d| d.pos)),
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
        self.pointer()?.target_bounds
    }

    /// The same event, stamped with the bounds of the widget it is being delivered to.
    ///
    /// The router calls this once per delivery, which is why every handler can ask
    /// [`target_bounds`](Self::target_bounds) without any widget arranging for it. A non-pointer
    /// event is returned unchanged — there is nowhere to put it, and nothing that reads it.
    pub(crate) fn with_target_bounds(mut self, bounds: Rectangle) -> Self {
        // Deliberately NOT `PointerDownOutside`: it is broadcast to everything the press did *not*
        // land on, so "the target" is somebody else's widget and stamping each receiver with its own
        // bounds would be a lie about where the press was.
        if !matches!(self, Self::PointerDownOutside(_))
            && let Some(p) = self.pointer_mut()
        {
            p.target_bounds = Some(bounds);
        }
        self
    }

    /// This event's kind — the key a [`Base`](crate::component::Base) handler is registered under.
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
            Self::Hint(_) => EventKind::Hint,
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
            Self::DragInFlight(_) => EventKind::DragInFlight,
            Self::DragSettled(_) => EventKind::DragSettled,
            Self::Focus => EventKind::Focus,
            Self::Blur => EventKind::Blur,
            Self::Mount => EventKind::Mount,
            Self::Unmount => EventKind::Unmount,
        }
    }

    /// The pointer payload, for the events that carry one.
    ///
    /// **The one list of which events carry a pointer**, with its writable twin
    /// `pointer_mut` beside it. Where an event happened and which widget it
    /// was delivered to are read through here, so a new pointer event is added in these two places
    /// and nowhere else.
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

    /// [`pointer`](Self::pointer), writable — the same list, kept beside it.
    fn pointer_mut(&mut self) -> Option<&mut PointerEvent> {
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
            | Self::Drop(d)
            | Self::DragInFlight(d)
            | Self::DragSettled(d) => Some(d),
            _ => None,
        }
    }
}

/// The **name** of an event, with none of its payload — what a [`Base`](crate::component::Base)
/// handler is keyed by.
///
/// One kind per [`Event`] variant, except that the raw pointer stream maps onto the resolved kind
/// it produces: a handler is registered for `Click`, never for "a raw press".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Key,
    TextInput,
    ModifiersChanged,
    Widget,
    /// A picker's letter resolved to a widget — see [`Event::Hint`].
    Hint,
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
    DragInFlight,
    DragSettled,
    Focus,
    Blur,
    Mount,
    Unmount,
}
