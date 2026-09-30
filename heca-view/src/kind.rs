//! The closed widget vocabulary.

use serde::{Deserialize, Serialize};

/// **The vocabulary, written once**: the enum and [`WidgetKind::ALL`] come from the same list, so a
/// kind added here is enumerated everywhere that walks the vocabulary — nothing to keep in step.
macro_rules! widget_kinds {
    ($(#[$emeta:meta])* pub enum WidgetKind { $( $(#[$meta:meta])* $variant:ident, )* }) => {
        $(#[$emeta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum WidgetKind {
            $( $(#[$meta])* $variant, )*
        }

        impl WidgetKind {
            /// Every variant — the closed vocabulary, enumerable.
            ///
            /// The host checks **coverage** against this: `realize` has a test that walks it and
            /// asserts each kind maps to a real widget, which is what stops a newly-added kind from
            /// silently rendering an empty container.
            pub const ALL: &'static [WidgetKind] = &[$(WidgetKind::$variant),*];

            /// This kind's position in [`ALL`](Self::ALL).
            #[allow(dead_code)]
            pub(crate) fn ordinal(self) -> usize {
                self as usize
            }
        }
    };
}

widget_kinds! {
/// The closed widget vocabulary. Covers the whole grid-ui set: **containers** hold
/// children, **leaves** are terminal. `realize` maps each to its grid-ui widget (arms are
/// filled in incrementally, starting with what the confirm dialog needs).
pub enum WidgetKind {
    // ── Containers ──
    /// A vertical box. Plain arrangement — no focus, no hover, no activation.
    VStack,
    /// A horizontal box. Plain arrangement — see [`Row`](WidgetKind::Row) for the interactive one.
    HStack,
    /// A **clickable, selectable** container for arbitrary content: hover tint, active state with
    /// a marker, press flash, focus ring, activation by mouse and by Enter/Space.
    ///
    /// This is `heca_grid_ui::Row`. The name used to belong to the plain horizontal box, which is
    /// now [`HStack`](WidgetKind::HStack) — so the library's `Row` and this one finally mean the
    /// same thing. Before that rename the interactive row had no declarative spelling at all, and
    /// `docs/chrome-and-ui.md` shipped an example writing this widget's behaviour against the box
    /// that cannot do it.
    Row,
    Grid,
    /// **A grid of cards with a cursor** — the shape a picker surface is: an exposé, a palette of
    /// tiles, a plugin's chooser (F003/P097/T501).
    ///
    /// Each child is one card, in reading order, and its own `key` is what activation hands back.
    /// The cursor is the widget's — arrow keys move it, hovering moves it, Enter activates, Escape
    /// dismisses — and a described grid gets all of that with nothing declared but the cards.
    ///
    /// **The lit card is not something a description wires.** Natively a caller hands the grid each
    /// card's own state signal; a description cannot name another node's signal, so the realizer
    /// makes that connection itself — it is the one building both the card and the cell. That is
    /// why this can be described at all while a `ScrollBar` cannot.
    CardGrid,
    Card,
    Scroll,
    Panel,
    Surface,
    /// Grouped list of items (`ItemGroup`).
    ItemGroup,
    /// A titled, collapsible dock frame.
    DockFrame,
    /// **A surface over the page**: a scrim, a panel holding the children, and — the reason a
    /// description can raise one at all — how it **arrives and leaves** (`animation_named`).
    ///
    /// The panel is the children: one child is the panel, several are stacked into one. Native
    /// code hands the widget a live animation, including a type this model has never heard of; a
    /// description names a built-in and gets the same gesture (`heca_grid_ui::NamedAnimation`).
    Overlay,
    /// A row of column/pane markers.
    MarkerGroup,
    /// A tab strip + panel.
    Tabs,
    /// One selectable **option**: a `value` plus arbitrary composed content. The children of a
    /// [`Select`](WidgetKind::Select) / [`Tabs`](WidgetKind::Tabs) — and usable on its own.
    Choice,
    /// **A picker over its own children**: open it and every pickable node beneath wears a letter,
    /// typing one runs that node's `hint`. A transparent wrapper the rest of the time.
    ///
    /// `opens_on` names the verb that opens it (`"mypanel.pick"`), and config binds the key to that
    /// name — so a described surface owns a picker on the same terms the exposé does, instead of
    /// only contributing targets to heca's (`heca_grid_ui::widgets::KeyHintGroup`).
    KeyHintGroup,

    // ── Leaves ──
    Label,
    Button,
    IconButton,
    Badge,
    BadgeButton,
    Tag,
    Icon,
    Input,
    Select,
    Toggle,
    Checkbox,
    StatusDot,
    Gauge,
    ScrollBar,
    Alert,
    Toast,
    RailCell,
    /// A single selectable list row.
    Item,
    /// A thin themed divider line.
    Separator,
    /// **Something is happening and nobody knows for how long** — an indeterminate ring
    /// (F003/P097/T501).
    ///
    /// It takes no properties of its own: it animates itself off the frame clock, and its diameter
    /// is `width`/`height` like any other node's. Reach for [`Progress`](WidgetKind::Progress)
    /// instead the moment you can say *how far along* — a spinner is what you show when you cannot.
    Spinner,
    /// **How far along something is**, `0.0..=1.0` in the `value` prop (F003/P097/T501).
    ///
    /// The fill eases toward whatever it is given, so a described tree re-sent with a new `value`
    /// animates rather than jumping, with nothing declared.
    Progress,
    /// **A keyboard glyph** from the embedded Nerd Font — ⇧ ⌃ ⌥ ⌘, Enter, Escape, the arrows
    /// (F003/P097/T501).
    ///
    /// Its own `glyph` vocabulary ([`ViewNfGlyph`]), because it is its own font. It is what lets a
    /// plugin draw a shortcut the way heca's own key hints do, rather than typing a character its
    /// user's font may not carry.
    NfIcon,
    /// **A row of actions that gets out of its own way** (F003/P097/T501).
    ///
    /// Its children are [`Button`](WidgetKind::Button) nodes. As the room runs out it shows icons
    /// instead of words, and whatever still does not fit collapses into a ⋮ menu that runs the same
    /// actions — none of which an author writes. A button's own text becomes its menu row and its
    /// words on hover, so it is written once.
    ///
    /// The group is why a tooltip and a hint placement had to stop being wrappers: it holds
    /// **typed** buttons, and wrapping one changes what it is.
    ButtonGroup,
    /// **A thing, said in a line or a few** — a status pip, an icon, a title with a quieter suffix,
    /// and lines under it. `heca_grid_ui::Tile`. It only arranges: every part is a child, placed by
    /// its `slot` prop — `status`, `icon`, `title`, `suffix` — and a child with no slot is a line
    /// under the head. Put it in a `Row` for selection and a click.
    Tile,
}
}
