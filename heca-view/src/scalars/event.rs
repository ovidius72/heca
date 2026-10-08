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
    }
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
