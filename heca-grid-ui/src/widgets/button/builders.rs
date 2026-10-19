//! The builder API: how a [`Button`] is constructed and configured — the
//! convenience constructors, the prop setters, and the accessors a container reads.

use super::*;
use crate::color::Color;
use crate::component::Component;
use crate::reactive::Signal;
use crate::style::{Align, Direction, Justify};
use crate::widgets::{Icon, Label};

#[heca_grid_ui_macros::props]
impl Button {
    /// An **empty** primary button — no content. Compose it with [`child`](Parent::child) /
    /// [`icon`](Self::icon).
    ///
    /// [`new`](Self::new) is the common case (a single label); this is the entry point when the
    /// content is a tree.
    pub fn empty() -> Self {
        let mut base = Base::new();
        base.focusable = true; // keyboard-focusable when enabled (Component::focusable)
        base.one_click_target = true; // and one click target (Base::one_click_target)
        // One control = one Tab stop: focus never descends into composed content.
        base.focus_barrier = true;
        // Content is laid out as a centered row; padding/gap derive from the size variant in
        // `remeasure`, and the variant itself is set via `LayoutExt::size`.
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Center;
        base.style.layout.justify = Justify::Center;
        let mut button = Self {
            base,
            variant: ButtonVariant::Primary,
            show_glow: true,
            show_border: true,
            progress: 0.0,
            flash: Flash::new(),
            on_click: None,
            active: false,
            icon_only: false,
            dangerous: false,
            label: String::new(),
            glyph: None,
        };
        button.remeasure();
        button
    }

    /// A primary button showing `label` — sugar for [`empty`](Self::empty) plus a bold
    /// [`Label`] child.
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        let mut b = Self::empty().child(Label::new(label.clone()).bold(true));
        b.label = label;
        b
    }

    /// Prepend a leading [`Icon`] — sugar for a child, so `Button::new("Save").icon(Glyph::Check)`
    /// holds `[Icon, Label]`. The icon inherits the button's state color and size variant.
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self.base.children.insert(0, Box::new(Icon::new(glyph)));
        self
    }

    /// **The button's own click, shareable.**
    ///
    /// A container that has to present this button as something else — a
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) turning it into a menu row when there is no room for it —
    /// runs *this*, so the two are not two paths over one action. It is a shared handle rather than
    /// an owned box for exactly that reason.
    pub fn click_handler(&self) -> Option<std::rc::Rc<dyn Fn()>> {
        self.on_click.clone()
    }

    /// **What this button's icon is**, or `None` when it has only words.
    ///
    /// Read by a container that has to render the button as something else — a
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) building the menu row a collapsed button becomes. It
    /// asks rather than reaching into `children`, so the answer cannot depend on how the content
    /// happens to be composed.
    pub fn glyph(&self) -> Option<Glyph> {
        self.glyph
    }

    /// **The words this button carries.** Every constructor takes them, so a button always has
    /// some — which is what makes a [`ButtonGroup`](crate::widgets::ButtonGroup) collapse readable. Answered
    /// from its own field, so it is the same whether the words are being shown or not.
    pub fn label(&self) -> String {
        self.label.clone()
    }

    /// Append an already-boxed component — the seam for a subtree built by a mapper
    /// (`realize(&ViewNode)` returns `Box<dyn Component>`, which is not itself `Component` and so
    /// goes through the same builder). Mirrors [`Dialog::body`](crate::widgets::Dialog::body).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    /// Convenience constructors, one per variant.
    pub fn primary(label: impl Into<String>) -> Self {
        Self::new(label)
    }
    pub fn secondary(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Secondary)
    }
    pub fn destructive(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Destructive)
    }
    pub fn outline(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Outline)
    }
    pub fn ghost(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Ghost)
    }
    pub fn link(label: impl Into<String>) -> Self {
        Self::new(label).variant(ButtonVariant::Link)
    }

    /// Mark the button as **held on** (toggled). When `true` it paints a persistent tone-tinted
    /// wash and a firm border under whatever its variant draws — the held version of a hover frame,
    /// matching the [`Toggle`](crate::widgets::Toggle) on-state — so it reads as an active *status* rather
    /// than a button that merely exists. Hover and press still layer on top, so an engaged button
    /// still brightens under the cursor.
    ///
    /// **The same state, and the same tokens, as [`IconButton::active`](crate::widgets::IconButton::active)**
    /// — a pane's zoom and float buttons look identical whether they are drawn as icons or as
    /// icons with words, which is what lets a [`ButtonGroup`](crate::widgets::ButtonGroup) hold them.
    #[heca_grid_ui_macros::prop]
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }

    /// **Which variant this button is**, so a container can tell one that was left at the default
    /// from one that named its own — see [`ButtonGroup::variant`](crate::widgets::ButtonGroup::variant).
    pub fn variant_of(&self) -> ButtonVariant {
        self.variant
    }

    /// **The hue this button reads in**, overriding what its variant would use — its content, its
    /// hover wash and its held-on frame.
    ///
    /// The same builder [`IconButton::accent`](crate::widgets::IconButton::accent) has, and for the same
    /// reason: a button can be *about* something dangerous without being drawn as a boxed
    /// destructive control. In a row of quiet ghost buttons, a `Destructive` variant is the odd one
    /// out — it carries a border the others do not — where a ghost button in the danger hue reads
    /// as a cue.
    #[heca_grid_ui_macros::prop]
    pub fn accent(self, c: Color) -> Self {
        crate::builders::ToneExt::accent(self, c)
    }

    /// **Show only the icon, keeping the words.** The label is not drawn and takes no space, and
    /// the button becomes square — the horizontal room a `Button` reserves exists for text, so a
    /// button with none is a button that is too wide by exactly that much.
    ///
    /// The words are still *carried*: they are what it says on hover, and what its row reads when a
    /// [`ButtonGroup`](crate::widgets::ButtonGroup) moves it into a menu. Nothing is lost by turning them
    /// off, which is what makes this the cheapest thing a group can give up.
    ///
    /// A button with no icon ignores it — hiding the words would leave an empty box.
    #[heca_grid_ui_macros::prop]
    pub fn icon_only(mut self, on: bool) -> Self {
        Component::set_icon_only(&mut self, on);
        self
    }

    /// Set the variant.
    #[heca_grid_ui_macros::prop]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self.dangerous = variant == ButtonVariant::Destructive;
        self
    }

    /// Set an explicit font size — overrides the inherited theme font + size scale.
    #[heca_grid_ui_macros::prop]
    pub fn font_size(mut self, fs: f32) -> Self {
        self.base.style.visual.font_size = fs;
        self.base.font = fs;
        self.remeasure();
        self
    }

    /// Enable or disable the glow — both the hover glow and the faint theme
    /// rest glow (`interaction.control_rest_glow`). Default: enabled.
    #[heca_grid_ui_macros::prop]
    pub fn glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Show or hide the border (default: shown for bordered variants).
    #[heca_grid_ui_macros::prop]
    pub fn bordered(mut self, enabled: bool) -> Self {
        self.show_border = enabled;
        self
    }

    /// Set the click callback.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(f));
        self.base.activatable = true; // and pickable — a letter runs this (Base::activatable)
        self
    }

    /// The hover-state signal.
    pub fn hovered(&self) -> Signal<bool> {
        self.base.pointer.hovered
    }
}
