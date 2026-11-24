//! The builder API: how a [`Dialog`] is assembled — title, body, action buttons, dismissal and
//! open policy — and how its panel is composed from them the first time it is needed.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::builders::{IntoChildren, Parent, PlaceExt, PresenceExt as _};
use crate::component::Component;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::style::{Length, Track};
use crate::widgets::{Flex, Grid, Label, Overlay};

/// What a dialog does when it is asked to go away — shared by its builder and its overlay.
pub(super) type DismissFn = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// What the panel is built from — gathered by the builders, read once when it is built.
#[derive(Default)]
pub(super) struct Spec {
    pub(super) title: String,
    pub(super) body: Vec<crate::WidgetModel>,
    pub(super) actions: Vec<crate::WidgetModel>,
    pub(super) danger: bool,
    pub(super) default_action: Option<String>,
    /// The flag the dialog is up by — its own, or the caller's from `open(signal)`.
    pub(super) open: Option<Signal<bool>>,
    /// The panel's style as the caller wrote it — kept current by the dialog on every layout, read
    /// by the panel as it lays itself out.
    pub(super) panel_style: Rc<RefCell<crate::style::Style>>,
}

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
        // Root: a full-size passthrough so the overlay child fills the viewport.
        let mut base = Base::viewport_layer();
        let spec = Rc::new(RefCell::new(Spec {
            title: title.into(),
            open: Some(base.open),
            ..Spec::default()
        }));
        let dismissible = Rc::new(Cell::new(true));
        let on_dismiss: DismissFn = Rc::new(RefCell::new(None));
        let mut authored_style = crate::style::Style::default();
        authored_style.layout.padding = DIALOG_PAD.into();
        Self::attach_panel(&mut base, &spec, &dismissible, &on_dismiss);
        Self {
            base,
            authored_style,
            spec,
            dismissible,
            on_dismiss,
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
    pub fn panel_size(self, width: Length, height: Length) -> Self {
        let mut dialog = self.width(width).height(height);
        dialog.authored_style.layout.flex_basis = None;
        dialog
    }

    /// Override the theme's readable width cap. The viewport still limits narrow windows.
    #[heca_grid_ui_macros::prop]
    pub fn panel_max_width(self, width: Length) -> Self {
        self.max_width(width)
    }

    /// Append one or many body components, independently of action-builder order.
    /// Nested Flex/Grid layouts remain ordinary children; none is inferred to be a header.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn child(self, children: impl IntoChildren) -> Self {
        let mut children = children.into_children();
        for child in &mut children {
            Self::fit_body(child.as_mut());
        }
        self.spec.borrow_mut().body.extend(children);
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
    /// Order-free with [`body`](Dialog::body).
    #[heca_grid_ui_macros::prop]
    pub fn danger(self, on: bool) -> Self {
        self.spec.borrow_mut().danger = on;
        self
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
    /// fields breathe whether or not the body scrolls. Only defaults are filled in: an explicit
    /// width / grow / gap the caller set is respected.
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
    /// the panel bottom, in call order; the first is the **primary** one Enter presses.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn action(self, button: impl crate::builders::IntoComponent) -> Self {
        self.spec.borrow_mut().actions.push(button.into_component());
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
    /// question, not something a framework can infer. Unset, nothing is focused.
    ///
    /// It is the composed [`Overlay`]'s [`default_focus`](Overlay::default_focus), so a dialog and
    /// a surface **composed** from an overlay place the keyboard by the same rule.
    #[heca_grid_ui_macros::prop]
    pub fn default_action(self, key: impl Into<String>) -> Self {
        self.spec.borrow_mut().default_action = Some(key.into());
        self
    }

    /// `false` forces an explicit choice — a press outside is swallowed without dismissing (Esc
    /// still cancels). Pair with a cancel button so there's always a non-destructive way out.
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(self, on: bool) -> Self {
        self.dismissible.set(on);
        self
    }

    /// Set the callback fired when Esc or a press outside requests dismissal (respecting
    /// [`dismissible`](Dialog::dismissible)). The host wires this to its overlay-close path.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(self, f: impl Fn() + 'static) -> Self {
        *self.on_dismiss.borrow_mut() = Some(Box::new(f));
        self
    }

    /// Set the initial open state. A dialog born open puts the keyboard on its first control,
    /// without the ring — so Enter works at once, and the ring waits for keyboard navigation.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.base.open.set(open);
        // **Its own gesture adopts the state too**, not just the flag: anything asking whether it
        // is up gets the right answer before the first tick.
        self.base.presence.assume_open(open);
        self
    }

    /// **What opens this dialog** — a signal of your own, or the name of an action that opens it.
    ///
    /// ```ignore
    /// let confirming = signal(false);
    /// let d = Dialog::new("Close pane?").body(..).action(..).open(confirming);
    /// confirming.set(true);   // it appears
    /// ```
    ///
    /// See [`IntoOpen`](crate::builders::IntoOpen).
    #[heca_grid_ui_macros::prop]
    pub fn open(self, by: impl crate::builders::IntoOpen) -> Self {
        let own = self.base.open;
        by.into_open().apply(own, Self::follow_open, self)
    }

    /// **A handle to show and close this dialog from anywhere** — copyable, so it goes into any
    /// closure. See [`SurfaceHandle`](crate::widgets::SurfaceHandle).
    ///
    /// ```ignore
    /// let confirm = Dialog::new("Close pane?").body(..).action(..).handle();
    /// Button::new("Delete").on_click(move |_| confirm.show())
    /// ```
    pub fn handle(&self) -> crate::widgets::SurfaceHandle {
        crate::widgets::SurfaceHandle::new(self.base.open)
    }

    pub fn open_signal(&self) -> Signal<bool> {
        self.base.open
    }
}

impl Dialog {
    /// Follow `open`: the dialog's flag, and what its panel is built on.
    fn follow_open(mut self, open: Signal<bool>) -> Self {
        self.base.open = open;
        self.spec.borrow_mut().open = Some(open);
        self
    }

    /// The caller's panel style, handed to the panel — run on every layout, before the panel lays
    /// itself out, so a size given at any point is the one it is laid out at.
    pub(super) fn share_panel_style(&self) {
        let spec = self.spec.borrow();
        if *spec.panel_style.borrow() != self.authored_style {
            *spec.panel_style.borrow_mut() = self.authored_style;
        }
    }

    /// **The dialog's one child, its overlay and panel — built the first time it is needed** (the
    /// first layout, event or question about the keyboard), once every builder has run, from what
    /// they gathered. The same door as any child that follows a value: a list of one.
    fn attach_panel(
        base: &mut Base,
        spec: &Rc<RefCell<Spec>>,
        dismissible: &Rc<Cell<bool>>,
        on_dismiss: &DismissFn,
    ) {
        let (spec, dismissible, on_dismiss) =
            (spec.clone(), dismissible.clone(), on_dismiss.clone());
        crate::builders::IntoChildren::attach(
            crate::builders::BoundChildren::new(
                || vec![()],
                move |_| build(&mut spec.borrow_mut(), &dismissible, &on_dismiss),
            ),
            base,
        );
    }
}

/// **Compose the dialog**: the overlay that owns the layer and the keyboard, over the panel.
fn build(spec: &mut Spec, dismissible: &Rc<Cell<bool>>, on_dismiss: &DismissFn) -> Overlay {
    let open = spec.open.expect("a dialog always has the flag it is up by");
    let mut rows = Vec::new();
    let mut areas = Vec::new();
    let mut parts: Vec<crate::WidgetModel> = Vec::new();
    if !spec.title.is_empty() {
        rows.push(Track::Auto);
        areas.push(TITLE_AREA.to_string());
        parts.push(crate::WidgetModel::new(
            Label::new(std::mem::take(&mut spec.title)).area(TITLE_AREA),
        ));
    }
    if !spec.body.is_empty() {
        let mut body = std::mem::take(&mut spec.body);
        if spec.danger {
            for child in &mut body {
                child.base_mut().style.visual.content_tone = Some(crate::Tone::Danger);
            }
        }
        rows.push(Track::Fr(1.0));
        areas.push(BODY_AREA.to_string());
        parts.push(crate::WidgetModel::new(
            Flex::column().gap(DIALOG_GAP).area(BODY_AREA).child(body),
        ));
    }
    if !spec.actions.is_empty() {
        rows.push(Track::Auto);
        areas.push(ACTIONS_AREA.to_string());
        parts.push(crate::WidgetModel::new(
            Flex::row()
                .gap(DIALOG_BTN_GAP)
                .justify("end")
                .area(ACTIONS_AREA)
                .child(std::mem::take(&mut spec.actions)),
        ));
    }
    let layout = Grid::new()
        .template_column("1fr")
        .template_row(rows)
        .template_area(areas)
        .grow(1.0)
        .min_height(0.0)
        .gap(DIALOG_GAP)
        .area(LAYOUT_AREA)
        .child(parts);
    let (fire, fire_outside) = {
        let (esc, outside, dismissible) =
            (on_dismiss.clone(), on_dismiss.clone(), dismissible.clone());
        (
            move || {
                if let Some(f) = &*esc.borrow() {
                    f();
                }
            },
            move || {
                if dismissible.get()
                    && let Some(f) = &*outside.borrow()
                {
                    f();
                }
            },
        )
    };
    // **A dialog locks because it is a dialog.** It is asking a question, so nothing behind it is
    // reachable until you answer — the caller never says so, and cannot get it wrong.
    let mut overlay = Overlay::new()
        .blocking(true)
        .lock(true)
        // Its controls are a row of buttons: the arrows across it move the keyboard.
        .item_keys(true)
        .panel(component::DialogPanel::new(
            layout,
            spec.panel_style.clone(),
        ))
        .on_dismiss(fire)
        .on_outside_click(fire_outside)
        .open(open);
    let named = spec.default_action.take();
    if let Some(key) = &named {
        overlay = overlay.default_focus(key.clone());
    }
    if open.get_untracked() {
        // **Built already open, it is already up** — the overlay sees no opening to place the
        // keyboard at, so it is placed now, by the rule an opening follows: on the control the
        // dialog named, or on the first one without the ring, so Enter works at once.
        match named {
            Some(_) => overlay.place_default_focus(),
            None => Component::focus_first_quiet(&mut overlay),
        }
    }
    overlay
}
