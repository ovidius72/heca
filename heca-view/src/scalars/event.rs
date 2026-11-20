//! The events a node can bind.

named_set! {
    /// **The names an event travels under** — the keys of a node's [`Events`](crate::Events), and
    /// the one place they are spelled. Everything that binds or reads an event goes through this,
    /// so no crate carries a string literal that has to agree with another's.
    ///
    /// The map itself stays keyed by text: a plugin ships JSON, and a name it invents is ignored
    /// rather than rejected. This is the list of the names `realize` reads.
    pub enum ViewEvent {
        /// Activated — a click, or Enter / Space on a focused button, row or standalone `Choice`.
        Press => "press",
        /// What a leader-key pick does to the node, when that is not simply what a click does.
        /// Unbound, a pick falls back to [`Press`](ViewEvent::Press).
        Hint => "hint",
        /// The value changed — `Input`, `Toggle`, `Checkbox`; on a `Select` / `Tabs` the intent
        /// carries the chosen option's `value`.
        Change => "change",
        /// Submit an Input through its configurable editing action.
        Submit => "submit",
        /// A collapsible group folded or unfolded — the intent carries the new state in
        /// `args["expanded"]`.
        Toggle => "toggle",
        /// A surface asked to go away — an `Overlay`'s scrim or Escape, a `Toast`'s close.
        Dismiss => "dismiss",
        /// A `Toast`'s own action button.
        Action => "action",
        /// A `CardGrid`'s chosen card was activated — the card's `key` travels as the `key` arg.
        Activate => "activate",
        /// A `CardGrid`'s cursor moved to another card — the card's `key` travels as the `key` arg.
        Move => "move",
        /// A `Splitter`'s edge was dragged — the intent carries `delta`: how many px it moved along
        /// its axis since the last report, sent at every pointer move while held.
        Resize => "resize",
        // ── Every event the native API has, under the native builders' own names ──────────────
        //
        // The names below are `heca_grid_ui::EVENT_NAMES` — the one table the native
        // `Base::listen_named` and `realize` both read — and a test holds the two lists equal, so a
        // widget can never have an event a plugin cannot bind. They work on **every** kind: any
        // widget can be clicked, hovered, focused or dragged.
        /// A key went down on the focused widget, or one inside it (`on_key_down`).
        KeyDown => "key_down",
        /// A key came up (`on_key_up`).
        KeyUp => "key_up",
        /// Text was committed — typed, pasted or composed (`on_text_input`).
        TextInput => "text_input",
        /// A modifier key was pressed or released (broadcast to the whole tree).
        ModifiersChanged => "modifiers_changed",
        /// A configurable widget intent resolved from the key bindings reached the widget.
        Widget => "widget",
        /// A button went down on the widget (`on_pointer_down`).
        PointerDown => "pointer_down",
        /// The button came up (`on_pointer_up`).
        PointerUp => "pointer_up",
        /// The pointer moved over the widget (`on_pointer_move`).
        PointerMove => "pointer_move",
        /// Hover was re-checked after the tree changed under a still pointer.
        PointerRecheck => "pointer_recheck",
        /// The pointer came over the widget (`on_pointer_enter`).
        PointerEnter => "pointer_enter",
        /// The pointer left the widget (`on_pointer_leave`).
        PointerLeave => "pointer_leave",
        /// A left click landed on the widget (`on_click`). `press` is this plus the pick.
        Click => "click",
        /// The second click of a run (`on_double_click`).
        DoubleClick => "double_click",
        /// The third click of a run (`on_triple_click`).
        TripleClick => "triple_click",
        /// A right click landed on the widget (`on_right_click`).
        RightClick => "right_click",
        /// A middle click landed on the widget (`on_middle_click`).
        MiddleClick => "middle_click",
        /// A press landed somewhere that is not this widget or inside it
        /// (`on_pointer_down_outside`).
        PointerDownOutside => "pointer_down_outside",
        /// The wheel turned over the widget (`on_scroll`).
        Scroll => "scroll",
        /// A drag began on the widget (`on_drag_start`).
        DragStart => "drag_start",
        /// The drag the widget started moved (`on_drag`).
        Drag => "drag",
        /// The drag the widget started ended (`on_drag_end`).
        DragEnd => "drag_end",
        /// A drag came over this drop target (`on_drag_enter`).
        DragEnter => "drag_enter",
        /// A drag moved within this drop target (`on_drag_over`).
        DragOver => "drag_over",
        /// A drag left this drop target (`on_drag_leave`).
        DragLeave => "drag_leave",
        /// A drag was released over this drop target (`on_drop`).
        Drop => "drop",
        /// A drag this target accepts began, anywhere in the window (`on_drag_in_flight`).
        DragInFlight => "drag_in_flight",
        /// The drag this target was told about is over (`on_drag_settled`).
        DragSettled => "drag_settled",
        /// The widget gained keyboard focus (`on_focus_gained`).
        FocusGained => "focus_gained",
        /// The widget lost keyboard focus (`on_focus_lost`).
        FocusLost => "focus_lost",
        /// The widget entered a live tree (`on_mount`).
        Mount => "mount",
        /// The widget is being dropped (`on_unmount`).
        Unmount => "unmount",
    }
}

impl ViewEvent {
    /// **The events a widget raises that are not a native event** — a value changed, a group
    /// folded, a toast was dismissed. Their meaning belongs to the widget that raises them, so the
    /// names are `realize`'s to read per kind. Every other name in [`ALL`](Self::ALL) is a native
    /// event, bound the same way on any widget.
    ///
    /// `hint` is both: it is the native pick event, and `realize` reads it as what a pick does.
    pub const WIDGET: &'static [ViewEvent] = &[
        ViewEvent::Press,
        ViewEvent::Change,
        ViewEvent::Submit,
        ViewEvent::Toggle,
        ViewEvent::Dismiss,
        ViewEvent::Action,
        ViewEvent::Activate,
        ViewEvent::Move,
        ViewEvent::Resize,
    ];
}

impl AsRef<str> for ViewEvent {
    fn as_ref(&self) -> &str {
        self.name()
    }
}

impl From<ViewEvent> for String {
    fn from(event: ViewEvent) -> String {
        event.name().to_string()
    }
}
