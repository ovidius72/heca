//! **What happens to a widget**: a handler for each thing that can.

use crate::component::Component;

/// **What happens to a widget**, said the same way as how it is laid out or styled — one builder:
/// What happens to a widget is said the same way as how it is laid out or styled — one builder:
///
/// ```
/// use heca_grid_ui::prelude::*;
///
/// let row = Row::new()
///     .child(Label::new("pane-1"))
///     .on_click(|_| println!("selected"))
///     .on_right_click(|e| println!("menu at {:?}", e.pos()))
///     .on_pointer_enter(|_| println!("hovered"));
/// ```
///
/// **Handlers run in the bubble phase**, after the widget's descendants have had the event and
/// before the widget's own [`on_event`](Component::on_event) — so a handler sees what its children
/// declined, and can stop the widget's built-in behaviour by taking the event itself:
///
/// ```
/// use heca_grid_ui::prelude::*;
/// use heca_grid_ui::EventKind;
///
/// // The general form: full control over the walk.
/// let button = Button::primary("Delete").on(EventKind::Click, |cx| {
///     cx.stop_propagation();   // the button will not fire
/// });
/// ```
///
/// The named builders below consume the event (an `on_click` that let the click carry on to the
/// row behind it would be a surprise). Use [`on`](EventExt::on) with
/// [`EventCx`](crate::event::EventCx) when you want to observe without consuming.
pub trait EventExt: Component + Sized {
    /// Register `f` for `kind`. It receives an [`EventCx`](crate::event::EventCx) and consumes the
    /// event only if it calls [`stop_propagation`](crate::event::EventCx::stop_propagation).
    fn on(
        mut self,
        kind: crate::event::EventKind,
        f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static,
    ) -> Self {
        // **Wiring an action is what makes a widget pickable** (F003/P082/T441). Set here, in the one
        // place every generic listener goes through, rather than repeated in `on_click`,
        // `on_double_click`, `on_key_down` and `on_key_up` — a rule in four call sites is a rule in
        // the wrong place.
        //
        // Right-click and middle-click are deliberately absent: a right-click opens a context menu
        // rather than doing the thing, so it should not spend one of the 52 letters.
        use crate::event::EventKind as K;
        if matches!(kind, K::Click | K::DoubleClick | K::Key) {
            self.base_mut().activatable = true;
        }
        self.base_mut()
            .handlers
            .get_or_insert_with(Default::default)
            .add(kind, f);
        self
    }


    /// A left click landed on this widget (or a descendant that did not take it).
    fn on_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Click, f)
    }


    /// A second click in the same run. The first still arrived as a
    /// [`click`](EventExt::on_click).
    fn on_double_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DoubleClick, f)
    }


    /// A third click, and any beyond it (`click_count` says which).
    fn on_triple_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::TripleClick, f)
    }


    /// A right click landed on this widget.
    ///
    /// This is the whole of "a widget can have its own menu": the widget hears the click, on
    /// itself, with the position — no registry of row identities, no hit-test at the host, and
    /// nothing to forget to declare.
    fn on_right_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::RightClick, f)
    }


    /// A middle click landed on this widget.
    fn on_middle_click(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::MiddleClick, f)
    }


    /// A button went down on this widget. Stopping propagation here **captures the pointer**: the
    /// moves and the release that end the gesture come here wherever the cursor goes.
    fn on_pointer_down(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerDown, f)
    }


    /// The button came up.
    fn on_pointer_up(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerUp, f)
    }


    /// The pointer moved over this widget, or anywhere while this widget holds capture.
    fn on_pointer_move(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerMove, f)
    }


    /// The pointer came over this widget. Several widgets on one path enter together, so stopping
    /// propagation here is almost always wrong.
    fn on_pointer_enter(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerEnter, f)
    }


    /// The pointer left this widget.
    fn on_pointer_leave(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::PointerLeave, f)
    }


    /// The wheel turned over this widget. Stop propagation to keep it from also scrolling whatever
    /// contains this widget.
    fn on_scroll(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Scroll, f)
    }


    /// A press landed somewhere that is **not** this widget or a descendant — how a popup closes
    /// itself. The press belongs to whatever it landed on, so do not claim it.
    fn on_pointer_down_outside(
        self,
        f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static,
    ) -> Self {
        self.on(crate::event::EventKind::PointerDownOutside, f)
    }


    /// A key went **down** on this widget — it holds the keyboard, or it contains what does.
    ///
    /// The other half of "an event listener in every widget": intercepting a key, a quick-pick
    /// letter or a shortcut is the same one line as intercepting a click, on the same argument,
    /// with the same way to say the event is yours.
    ///
    /// Down and up are separate builders because they are separate events everywhere else — the
    /// DOM's `keydown` / `keyup`, and every toolkit that copies it. One handler with a `pressed`
    /// flag inside makes every caller write the same `if`, which is a rule in N call sites rather
    /// than in the API.
    ///
    /// ```ignore
    /// card.on_key_up(move |cx| {
    ///     if let Event::Key { key: GridKey::Char('x'), .. } = cx.event() {
    ///         delete(id);
    ///         cx.stop_propagation();   // …or let it bubble to the container
    ///     }
    /// })
    /// ```
    fn on_key_down(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on_key_when(true, f)
    }


    /// A key came **up** on this widget. See [`on_key_down`](EventExt::on_key_down).
    fn on_key_up(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on_key_when(false, f)
    }


    /// Shared by the two above: one registration on `EventKind::Key`, gated on the half it wants.
    /// Private so there is no third spelling of "a key happened".
    #[doc(hidden)]
    fn on_key_when(
        self,
        want_pressed: bool,
        mut f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static,
    ) -> Self {
        self.on(crate::event::EventKind::Key, move |cx| {
            if matches!(cx.event(), crate::event::Event::Key { pressed, .. } if *pressed == want_pressed)
            {
                f(cx);
            }
        })
    }


    /// Text the user committed — typed, pasted, or composed by an IME.
    fn on_text_input(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::TextInput, f)
    }


    /// This widget gained keyboard focus.
    ///
    /// Named `on_focus_gained`, not `on_focus`, because [`Component::on_focus`] is the widget's
    /// own hook for the same moment and two same-named methods on one type is a puzzle nobody
    /// should have to solve at a call site.
    fn on_focus_gained(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Focus, f)
    }


    /// This widget lost keyboard focus — the moment to commit an edit or close a popup. Named
    /// `on_focus_lost` for the same reason as [`on_focus_gained`](EventExt::on_focus_gained).
    fn on_focus_lost(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Blur, f)
    }


    /// This widget entered a live tree (fired on its first layout pass).
    fn on_mount(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Mount, f)
    }


    /// This widget is being dropped — the tree that held it was rebuilt or thrown away. Use it to
    /// release what the widget registered with the host.
    fn on_unmount(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Unmount, f)
    }


    /// A drag began on this widget (it declared a [`draggable`](super::DragExt::draggable) id and the
    /// pointer travelled past the threshold).
    fn on_drag_start(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragStart, f)
    }


    /// The drag this widget started moved.
    fn on_drag(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Drag, f)
    }


    /// The drag this widget started ended, dropped or not.
    fn on_drag_end(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragEnd, f)
    }


    /// A drag came over this drop target.
    fn on_drag_enter(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragEnter, f)
    }


    /// A drag moved within this drop target — `side` follows the pointer.
    fn on_drag_over(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragOver, f)
    }


    /// A drag left this drop target.
    fn on_drag_leave(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragLeave, f)
    }


    /// A drag this drop target accepts has started, anywhere in the window.
    fn on_drag_in_flight(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragInFlight, f)
    }


    /// The drag this drop target was told about is over.
    fn on_drag_settled(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::DragSettled, f)
    }


    /// A drag was released over this drop target.
    fn on_drop(self, f: impl FnMut(&mut crate::event::EventCx<'_>) + 'static) -> Self {
        self.on(crate::event::EventKind::Drop, f)
    }


    fn on_action(mut self, name: impl Into<String>, f: impl Fn() + 'static) -> Self {
        self.base_mut().actions.push(crate::hint::DeclaredAction {
            name: name.into(),
            run: Box::new(f),
        });
        self
    }
}

impl<T: Component + Sized> EventExt for T {}
