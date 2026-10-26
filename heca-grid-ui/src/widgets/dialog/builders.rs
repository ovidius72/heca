//! The builder API: how a [`Dialog`] is assembled — title, body, action buttons,
//! dismissal and open policy — and the body-shaping helpers those builders use.

use super::*;
use crate::builders::{IntoChildren, Parent, PlaceExt, PresenceExt as _};
use crate::component::Component;
use crate::reactive::{Signal, SignalUpdate};
use crate::style::{Length, Track};
use crate::widgets::{Flex, Grid, Label, Overlay, Surface};

#[heca_grid_ui_macros::props]
impl Dialog {
    /// Preferred width of a content-sized panel, relative to its resolved font.
    /// Content may need more room; a narrow viewport may offer less.
    pub const BASELINE_WIDTH: Length = Length::Em(22.0);

    /// A new (closed) dialog titled `title`. Add content with [`child`](Dialog::child) and
    /// buttons with [`action`](Dialog::action), in either order.
    /// Content-sized panels start at a preferred `22em` width, grow for larger content,
    /// and give way to the viewport cap. [`panel_size`](Dialog::panel_size) overrides it.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        let mut layout = Grid::new()
            .template_column("1fr")
            .grow(1.0)
            .min_height(0.0)
            .gap(DIALOG_GAP)
            .area(LAYOUT_AREA);
        if !title.is_empty() {
            layout = layout
                .template_row("auto")
                .template_area(TITLE_AREA)
                .child(Label::new(title).area(TITLE_AREA));
        }
        // Surface owns the panel box; Grid arranges its content inside that box.
        let panel = Surface::new()
            .min_width(Self::BASELINE_WIDTH)
            .padding(DIALOG_PAD)
            .child(layout);

        // The base Overlay owns the layer presentation: blocking (scrim + swallow),
        // viewport centering, drop shadow, panel fill, bracket reticle. Its open
        // signal IS the dialog's open signal.
        // **A dialog locks because it is a dialog.** It is asking a question, so nothing behind it
        // is reachable until you answer — the caller never says so, and cannot get it wrong. A
        // modeless one (find/replace, a properties panel) turns it off with `.lock(false)`.
        // Root: a full-size passthrough so the overlay child fills the viewport.
        let mut base = Base::viewport_layer();

        // **One flag, handed down.** The dialog is up when `Base::open` says so — the flag every
        // component carries — and the overlay it composes *follows* that same signal rather than
        // owning one of its own. This single line is what used to be five forwarded methods: show,
        // close and the gesture all work on the dialog because they work on every component, and
        // the surface underneath simply agrees.
        let overlay = Overlay::new()
            .blocking(true)
            .lock(true)
            .panel(panel)
            .open_when(base.open);
        base.children.push(Box::new(overlay));

        Self {
            base,
            dismissible: true,
            on_dismiss: None,
            danger: false,
            panel_max_width: None,
        }
    }

    /// Give the dialog panel an explicit size instead of letting it hug its content.
    ///
    /// The reason you usually want it: **a [`ScrollRegion`](crate::widgets::ScrollRegion) only
    /// scrolls when its parent bounds it.** A panel that sizes to its content just grows
    /// with a long [`body`](Dialog::body), so nothing overflows and no scrollbar appears.
    /// Give the panel a height and the body scrolls inside it — while the title and the
    /// action row stay put, because only the body is inside the region:
    ///
    /// ```ignore
    /// Dialog::new("Pick a container")
    ///     .panel_size(Length::Percent(0.5), Length::Percent(0.6))   // 50% × 60% of the VIEWPORT
    ///     .body(ScrollRegion::new().child(long_list))       // only the body scrolls
    ///     .action(Button::new("Cancel"))
    /// ```
    ///
    /// `Length::Auto` on width restores content sizing with the baseline; on height it hugs
    /// content. A non-auto width overrides the baseline. A [`Percent`](Length::Percent)
    /// resolves against the **viewport** (the composed [`Overlay`] fills it).
    #[heca_grid_ui_macros::host_only(
        "takes more than one value, which a single property cannot carry"
    )]
    pub fn panel_size(mut self, width: Length, height: Length) -> Self {
        let style = &mut self.panel_mut().style.layout;
        style.width = width;
        style.height = height;
        style.flex_basis = None;
        self
    }

    /// Override the theme's readable width cap. The viewport still limits narrow windows.
    #[heca_grid_ui_macros::prop]
    pub fn panel_max_width(mut self, width: Length) -> Self {
        self.panel_max_width = Some(width);
        self
    }

    /// Append one or many body components, independently of action-builder order.
    /// Nested Flex/Grid layouts remain ordinary children; none is inferred to be a header.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn child(mut self, children: impl IntoChildren) -> Self {
        let mut children = children.into_children();
        if children.is_empty() {
            return self;
        }
        for child in &mut children {
            Self::fit_body(child.as_mut());
        }
        if self.part_mut(BODY_AREA).is_none() {
            let body = Flex::column().gap(DIALOG_GAP).area(BODY_AREA);
            let layout = self.layout_mut().base_mut();
            let before_actions = layout.children.iter().position(|part| {
                part.base().grid_area.as_deref() == Some(ACTIONS_AREA)
            }).unwrap_or(layout.children.len());
            layout.children.insert(before_actions, Box::new(body));
        }
        self.part_mut(BODY_AREA)
            .expect("the body was just created")
            .base_mut().children.extend(children);
        self.tone_body();
        self.sync_template();
        self
    }

    /// Compatibility spelling for [`child`](Dialog::child). It accepts one or many children.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn body(self, children: impl IntoChildren) -> Self {
        self.child(children)
    }

    /// **A destructive question**: the body says its consequence in the theme's danger colour.
    ///
    /// The dialog's own rule, so no caller colours a message by hand: the body gets
    /// [`content_tone(Tone::Danger)`](crate::builders::ToneExt::content_tone), and every text
    /// inside it without a colour of its own follows. The title and the buttons keep theirs — a
    /// destructive button already says so through its own variant.
    ///
    /// Order-free with [`body`](Dialog::body): whichever comes second applies it.
    #[heca_grid_ui_macros::prop]
    pub fn danger(mut self, on: bool) -> Self {
        self.danger = on;
        self.tone_body();
        self
    }

    /// Put the danger tone on the body, when there is one and the dialog is destructive. Never
    /// clears a tone: a body that set its own keeps it when the dialog is not destructive.
    fn tone_body(&mut self) {
        if self.danger && let Some(body) = self.part_mut(BODY_AREA) {
            for child in &mut body.base_mut().children {
                child.base_mut().style.visual.content_tone = Some(crate::Tone::Danger);
            }
        }
    }

    /// Make the body behave the way a dialog body always should, so no caller has
    /// to remember it: **fill the panel's width** (instead of hugging its content
    /// and sitting to the left) and **take the space left between the title and the
    /// action row** (instead of the row floating up under a short body).
    ///
    /// The second part is what makes a scrollable body work: with a bounded panel
    /// ([`panel_size`](Dialog::panel_size)) the body gets the leftover height, so a
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) inside it has a real viewport to
    /// overflow — and the title and buttons stay put because only the body flexes.
    /// With an unsized panel there is no leftover space, so this changes nothing.
    ///
    /// It also gives the body a default **gap between its children** so a dialog's
    /// fields breathe whether or not the body scrolls — the same spacing a
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) body already applies to itself, now
    /// applied to a plain container body too, so the modal is consistent either
    /// way. (A label + its control should be grouped into one tight unit, e.g. a
    /// `Field`, so this larger gap falls *between* fields, never between a label
    /// and its own input.)
    ///
    /// Only defaults are filled in: an explicit width / grow / gap the caller set
    /// is respected.
    fn fit_body(body: &mut dyn Component) {
        let style = &mut body.base_mut().style.layout;
        if style.width == Length::Auto {
            style.width = Length::Percent(1.0);
        }
        if style.flex_grow == 0.0 {
            style.flex_grow = 1.0;
        }
        if style.gap == crate::style::Space::Px(0.0) {
            style.gap = Spacing::Md.into();
        }
    }

    /// Append an action **button** (a real [`Button`](crate::widgets::Button) the caller has already
    /// wired with its `on_click` + hint target). Buttons live in a right-aligned row along
    /// the panel bottom, in call order.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn action(mut self, button: impl Component + 'static) -> Self {
        if self.part_mut(ACTIONS_AREA).is_none() {
            // Lazily create the right-aligned action row on first use.
            let row = Flex::row().gap(DIALOG_BTN_GAP).justify("end").area(ACTIONS_AREA);
            self.layout_mut().base_mut().children.push(Box::new(row));
        }
        self.part_mut(ACTIONS_AREA).expect("the action row was just created")
            .base_mut()
            .children
            .push(Box::new(button));
        self.sync_template();
        self
    }

    /// **Which action the keyboard starts on**, named by that button's `key`.
    ///
    /// ```ignore
    /// Dialog::new("Close pane?")
    ///     .action(Button::new("Cancel").key("cancel"))
    ///     .action(Button::destructive("Close").key("close"))
    ///     .default_action("cancel")          // Enter is the safe one
    /// ```
    ///
    /// The author's call, and only the author's: which button is safe is a fact about *this*
    /// question, not something a framework can infer. Unset, nothing is focused — a deliberate
    /// choice a dialog can keep making.
    ///
    /// It delegates to [`Overlay::default_focus`], so a dialog and a surface **composed** from an
    /// overlay place the keyboard by the same rule rather than two that drift.
    #[heca_grid_ui_macros::prop]
    pub fn default_action(mut self, key: impl Into<String>) -> Self {
        let key = key.into();
        // The composed `Overlay` is this dialog's only child, and it is what holds the keyboard.
        self.base.children[0].set_default_focus(&key);
        self
    }

    /// `false` forces an explicit choice — Esc / scrim are swallowed without dismissing.
    /// Pair with a cancel button so there's always a non-destructive way out.
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self
    }

    /// Set the callback fired when Esc or a scrim click requests dismissal (respecting
    /// [`dismissible`](Dialog::dismissible)). The host wires this to its overlay-close path.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    /// Set the initial open state (focusing the first focusable — the safe default when the
    /// caller orders `[Cancel, …, Confirm]`).
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.base.open.set(open);
        // **Its own gesture adopts the state too**, not just the flag. Setting only the flag left
        // the two disagreeing for a frame — the flag said up, the component's own record said
        // closed — so anything asking whether it was up (a host driving the layer stack, an exit
        // still playing) got the wrong answer until the first tick caught it up.
        //
        // Its own, and not the composed overlay's: arriving and leaving belong to every component
        // now, so a caller holding a `Dialog` asks the `Dialog`, and the surface underneath
        // follows the one flag it was handed.
        self.base.presence.assume_open(open);
        if open {
            // Focus the safe-default (first) button so Enter works — but WITHOUT the ring; it
            // appears only once the user navigates by keyboard (focus-visible).
            self.base.children[0].focus_first_quiet();
        }
        self
    }

    /// The open-state signal — the host binds this to show/hide the dialog.
    /// **Follow a signal of your own** — the dialog is up exactly when it is true.
    ///
    /// ```ignore
    /// let confirming = signal(false);
    /// let d = Dialog::new("Close pane?").body(..).action(..).open_when(confirming);
    /// confirming.set(true);   // it appears
    /// ```
    ///
    /// Delegates to the composed [`Overlay`], which holds the state.
    #[heca_grid_ui_macros::host_only(
        "a live signal; a description carries a starting value, `open`"
    )]
    pub fn open_when(mut self, open: Signal<bool>) -> Self {
        let overlay = std::mem::replace(&mut self.base.children[0], Box::new(Flex::column()));
        self.base.children[0] = overlay;
        self.base.open = open;
        self.base.children[0].follow_open(open);
        self
    }

    /// **A handle to show and close this dialog from anywhere** — copyable, so it goes into any
    /// closure. See [`SurfaceHandle`](crate::widgets::SurfaceHandle).
    ///
    /// ```ignore
    /// let confirm = Dialog::new("Close pane?").body(..).action(..).handle();
    /// Button::new("Delete").on_click(move || confirm.show())
    /// ```
    pub fn handle(&self) -> crate::widgets::SurfaceHandle {
        crate::widgets::SurfaceHandle::new(self.base.open)
    }

    pub fn open_signal(&self) -> Signal<bool> {
        self.base.open
    }

    /// The panel container (the composed [`Overlay`]'s single child), mutably.
    fn panel_mut(&mut self) -> &mut Base {
        self.base.children[0].base_mut().children[0].base_mut()
    }

    /// Search only this panel's parts: a nested dialog may use the same area names.
    pub(super) fn part_mut(&mut self, name: &str) -> Option<&mut Box<dyn Component>> {
        self.layout_mut().base_mut().child_in_area(name)
    }

    fn layout_mut(&mut self) -> &mut Box<dyn Component> {
        self.panel_mut().child_in_area(LAYOUT_AREA)
            .expect("the panel owns its layout")
    }

    /// Rebuild the template during construction, retaining the panel's Base and live children.
    /// Absent parts contribute neither a row nor a gap; body content takes the remaining space.
    fn sync_template(&mut self) {
        let slot = self.layout_mut();
        let mut rows = Vec::new();
        let mut areas = Vec::new();
        for part in &slot.base().children {
            if let Some(name) = &part.base().grid_area {
                rows.push(if name == BODY_AREA { Track::Fr(1.0) } else { Track::Auto });
                areas.push(name.clone());
            }
        }
        let mut panel = Grid::new().template_column("1fr").template_row(rows).template_area(areas);
        std::mem::swap(panel.base_mut(), slot.base_mut());
        *slot = Box::new(panel);
    }
}
