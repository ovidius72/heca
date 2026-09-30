use super::{Parent, Style};
use crate::{PropValue, ViewGlyph, ViewLabelSide, ViewMarker, ViewNode, ViewVariant, WidgetKind};

builder!(
    /// A tab strip plus panel. Its tabs are [`Choice`] children.
    Tabs => Tabs
);

builder!(
    /// One selectable option: a `value` plus composed content. The child of a [`Select`] or
    /// [`Tabs`], and usable alone.
    Choice => Choice
);

builder!(
    /// A button. Its content is its children, or the `text`/`icon` sugar when it has none.
    Button => Button
);

builder!(
    /// An icon-only button.
    IconButton => IconButton
);

builder_text!(
    /// A clickable status chip.
    BadgeButton => BadgeButton
);

builder!(
    /// A text field.
    Input => Input
);

builder!(
    /// A dropdown. Its options are [`Choice`] children.
    Select => Select
);

builder!(
    /// An on/off switch.
    Toggle => Toggle
);

builder!(
    /// A checkbox with a label.
    Checkbox => Checkbox
);

builder!(
    /// **A row of actions that gets out of its own way.** Its children are [`Button`]s.
    ///
    /// As the room runs out it shows icons instead of words, and whatever still does not fit
    /// collapses into a ⋮ menu running the same actions — none of which you write. Give each button
    /// **both** `text` and `icon`: the text is its menu row and its words on hover, the icon is what
    /// it shows once there is no room for words.
    ButtonGroup => ButtonGroup
);

builder!(
    /// One cell of an icon rail.
    RailCell => RailCell
);

builder!(
    /// A single selectable list row.
    Item => Item
);

impl Parent for Tabs {}
impl Parent for Choice {}
impl Parent for Button {}
impl Parent for Select {}
impl Parent for ButtonGroup {}
impl Parent for Item {}

impl Button {
    /// A leading icon.
    pub fn icon(self, glyph: ViewGlyph) -> Self {
        self.prop("icon", glyph)
    }
    /// Which visual variant — primary, destructive, and so on.
    pub fn variant(self, v: ViewVariant) -> Self {
        self.prop("variant", v)
    }
    /// A glow halo at rest.
    pub fn glowing(self, on: bool) -> Self {
        self.prop("glow", on)
    }
    /// Draw a border.
    pub fn bordered(self, on: bool) -> Self {
        self.prop("bordered", on)
    }
    /// **Held on** — a toggled status rather than a transient hover, painted as a persistent
    /// tone-tinted wash and a firm border under whatever the variant draws. The same state, and the
    /// same look, as an icon button's.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// **Show only the icon, keeping the words** — the label is not drawn and takes no space, and
    /// the button becomes square, since the horizontal room a button reserves exists for text. The
    /// words are still carried: they are what it says on hover, and what its row reads if a button
    /// group moves it into a menu. A button with no icon ignores it.
    pub fn icon_only(self, on: bool) -> Self {
        self.prop("icon_only", on)
    }
}

impl IconButton {
    /// The cell's square size in px.
    pub fn cell(self, px: f32) -> Self {
        self.prop("cell", px)
    }
    /// A glow halo.
    pub fn glowing(self, on: bool) -> Self {
        self.prop("glow", on)
    }
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
}

impl BadgeButton {
    /// Which visual variant.
    pub fn variant(self, v: ViewVariant) -> Self {
        self.prop("variant", v)
    }
}

impl Input {
    /// The text shown when the field is empty.
    pub fn placeholder(self, text: impl Into<String>) -> Self {
        self.prop("placeholder", PropValue::Text(text.into()))
    }
    /// The current value.
    pub fn value(self, text: impl Into<String>) -> Self {
        self.prop("value", PropValue::Text(text.into()))
    }
    /// The name this field reports under when the form is submitted.
    pub fn name(self, name: impl Into<String>) -> Self {
        self.prop("name", PropValue::Text(name.into()))
    }
}

impl Select {
    /// Which option is chosen, by position.
    pub fn selected(self, index: usize) -> Self {
        self.prop("selected", index)
    }
}

impl Tabs {
    /// Which tab is chosen, by position.
    pub fn selected(self, index: usize) -> Self {
        self.prop("selected", index)
    }
}

impl Choice {
    /// What this option means, independently of what it shows.
    pub fn value(self, value: impl Into<String>) -> Self {
        self.prop("value", PropValue::Text(value.into()))
    }
    /// Show as chosen.
    pub fn selected(self, on: bool) -> Self {
        self.prop("selected", on)
    }
}

impl Toggle {
    /// Whether it is on.
    pub fn on(self, on: bool) -> Self {
        self.prop("on", on)
    }
}

impl Checkbox {
    /// Whether it is ticked.
    pub fn checked(self, on: bool) -> Self {
        self.prop("checked", on)
    }
    /// The label beside the box.
    pub fn label(self, text: impl Into<String>) -> Self {
        self.prop("label", PropValue::Text(text.into()))
    }
    /// Which side the label sits on.
    pub fn label_side(self, side: ViewLabelSide) -> Self {
        self.prop("label_side", side)
    }
}

impl ButtonGroup {
    /// How wide each action is before the row starts collapsing (default
    /// [`IconOnly`](crate::ViewDisplay::IconOnly)).
    pub fn display(self, display: crate::ViewDisplay) -> Self {
        self.prop("display", display)
    }
    /// The look every action in the group takes, so it is said once rather than per button.
    pub fn variant(self, variant: ViewVariant) -> Self {
        self.prop("variant", variant)
    }
}

impl Item {
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
    /// How the active state is shown.
    pub fn marker(self, m: ViewMarker) -> Self {
        self.prop("marker", m)
    }
    /// Dim the row.
    pub fn muted(self, on: bool) -> Self {
        self.prop("muted", on)
    }
    /// Draw a border around the leading slot.
    pub fn leading_bordered(self, on: bool) -> Self {
        self.prop("leading_bordered", on)
    }
    /// Draw a border around the trailing slot.
    pub fn trailing_bordered(self, on: bool) -> Self {
        self.prop("trailing_bordered", on)
    }
}

impl RailCell {
    /// The cell's square size in px.
    pub fn cell_size(self, px: f32) -> Self {
        self.prop("cell_size", px)
    }
    /// Show as the current one.
    pub fn active(self, on: bool) -> Self {
        self.prop("active", on)
    }
}
