//! [`Toast`] — a compact **notification card**: a bracket-framed surface with a
//! severity-colored leading [`Icon`](super::Icon), a strong title, optional small
//! body text, an optional inline **action** button, and an optional **×** dismiss
//! affordance.
//!
//! This is a **presentation widget only** — it holds *no* queue, timer, or global
//! state. Lifecycle (when a toast appears, how long it lives, auto-dismiss policy,
//! deduplication, sound) is the **host application's** job; the widget merely
//! *renders* one notification and *reports interactions* back via callbacks
//! ([`on_click`](Toast::on_click), [`on_action`](Toast::on_action),
//! [`on_dismiss`](Toast::on_dismiss)). The host removes it from its own store.
//!
//! Because it is an ordinary in-tree component (not overlay-drawn), it is equally
//! usable **inline** — e.g. as a notification row in a sidebar — as it will be
//! inside the overlay stack manager built on top of it. Severity maps to theme
//! tokens (`accent`/`success`/`warning`/`danger`), never baked-in literals.
//!
//! # Content is children
//! The card paints only its own chrome — the tinted surface, the bracket frame, the action's
//! button face, the press flash, the focus ring — and the **layout engine places the content**:
//! the leading icon, the text column, and the × are real children. It measured and placed them
//! itself until F003/P082/T481, and a card squeezed narrower than its own icon column then laid
//! its title out past its right edge, because a constant column width cannot consult the width the
//! card was actually given. Nothing here computes a position any more, and the card's sub-regions
//! are found the way every other widget finds them: the engine's bounds, and the pointer's own
//! routing.

mod position;
mod severity;
mod spec;
mod stack;

pub use position::ToastPosition;
pub use severity::ToastSeverity;
pub use spec::{ToastAction, ToastSpec};
pub use stack::ToastStack;

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, Event, GridKey, Handled, PaintCx};
use crate::animation::Animation;
use crate::effects::Flash;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::scene::TextAlign;
use crate::style::{Align, Direction, Length, WidgetSize};
use crate::widgets::{Ellipsis, Flex, Glyph, Icon, IconButton, Label};
use std::rc::Rc;

/// Inner padding.
const PAD: f32 = 13.0;
/// Gap between title, body, and the action row.
const GAP: f32 = 6.0;
/// Gap between the leading icon and the text column.
const ICON_GAP: f32 = 10.0;
/// Leading-icon size as a multiple of the resolved font (sits on the title line).
const ICON_SCALE: f32 = 1.1;
/// Body text multiplier relative to the title (which uses the resolved font).
const BODY_SCALE: f32 = 0.9;
/// Gap between two actions in the row under the body. The buttons' own height and padding are
/// theirs — a control brings its metrics with it.
const ACTION_GAP: f32 = 8.0;
/// Rest-glow spread radius (px) — the card's share of the theme rest halo. A
/// touch wider than the small controls (12): the toast is a card-sized surface
/// and a tight halo read visibly weaker beside them (user-reported).
const GLOW_RADIUS: f32 = 14.0;
/// Default card width.
const DEFAULT_WIDTH: f32 = 320.0;

/// Index of the leading icon / the text column / the × within `base.children`, and of the title /
/// body / action within that column. The card builds both levels, so it addresses them the way
/// [`Item`](super::Item) addresses its slots — by position, not by searching.
const ICON: usize = 0;
const COLUMN: usize = 1;
const DISMISS: usize = 2;
/// **An unfilled slot is removed from layout, not merely empty.** A zero-sized placeholder still
/// costs the row's gap, which is how a card with no leading icon started its text a gap further in
/// than a card with one. `hidden` is the engine's `display: none` (F003/P096/T483).
fn empty_slot() -> Box<dyn Component> {
    let mut slot = Flex::empty();
    slot.base_mut().set_hidden(true);
    Box::new(slot)
}

/// Within the column: the title, then the two slots a builder fills in later.
const TITLE: usize = 0;
const BODY: usize = 1;
const ACTION: usize = 2;

/// A compact notification card. Presentation only — the host owns lifecycle.
pub struct Toast {
    base: Base,
    severity: ToastSeverity,
    /// Leading glyph; defaults to the severity glyph, `None` hides the icon.
    icon: Option<Glyph>,
    show_icon: bool,
    /// The title child's text signal (the child owns the text; this is the handle callers get).
    title: Signal<String>,
    /// Whether a slot has been filled — an empty one is removed from layout entirely, so it costs
    /// neither space nor a gap.
    has_body: bool,
    has_actions: bool,
    /// Whether the × dismiss affordance is shown (default `true`).
    dismissible: bool,
    on_click: Option<Box<dyn Fn()>>,
    on_dismiss: Option<Rc<dyn Fn()>>,
    /// The whole-card press flash. The actions and the × are real controls that flash themselves,
    /// so this is only ever the card's own press.
    flash: Flash,
    // **How it arrives and leaves, and whether it is up, are both on `Base` now** — the same two
    // things every component carries. They were a `Presence` and a `Signal<bool>` of this widget's
    // own, which is what made showing and closing a capability only a card and an overlay had.
}

#[heca_grid_ui_macros::props]
impl Toast {
    /// A new info toast showing `title`. Add body text with [`body`](Toast::body),
    /// an action with [`action`](Toast::action), severity via the convenience
    /// constructors, and wire dismissal with [`on_dismiss`](Toast::on_dismiss).
    pub fn new(title: impl Into<String>) -> Self {
        let mut base = Base::new();
        base.style.layout.width = Length::Px(DEFAULT_WIDTH);
        base.style.layout.direction = Direction::Row;
        base.style.layout.align = Align::Start; // icon, text and × all sit on the title line
        base.style.layout.padding = (PAD).into();
        base.style.layout.gap = (ICON_GAP).into();

        // The title is a real child, so the card composes like every other widget: the engine
        // lays the three columns out, each paints itself, and the text can be cut by the label's
        // own ellipsis instead of being drawn wherever a computed origin happened to land.
        let title = Label::new(title).align(TextAlign::Start).truncate(Ellipsis::End);
        let title_signal = title.text_signal();
        let column = Flex::column()
            .grow(1.0)
            .gap(GAP)
            .child(title)
            .child(empty_slot()) // BODY
            .child(empty_slot()); // ACTION

        base.children.push(empty_slot()); // ICON
        base.children.push(Box::new(column));
        base.children.push(empty_slot()); // DISMISS

        let mut toast = Self {
            base,
            severity: ToastSeverity::Info,
            icon: None,
            show_icon: true,
            title: title_signal,
            has_body: false,
            has_actions: false,
            dismissible: true,
            on_click: None,
            on_dismiss: None,
            flash: Flash::new(),
        };
        // A card built and put in a tree is on screen; a caller that wants it to arrive says
        // `.default_open(false)` and then `show()`.
        //
        // **A notification slides in by default**, because that is what a notification does: it
        // arrives from the edge it lives on rather than materialising in place. Any other gesture
        // is one builder away — `animation(Animation::Fade)`, `Animation::of(mine)` — and
        // `Animation::None` is the cut. Set on the gesture every component carries, which is where
        // it lives now rather than in a field of this widget's own.
        toast.base.presence.set_animation(Animation::Slide.build());
        toast.base.presence.assume_open(true);
        crate::reactive::SignalUpdate::set(&toast.base.open, true);
        toast.sync_icon();
        toast.sync_dismiss();
        toast.remeasure();
        toast
    }

    /// Convenience constructors, one per severity.
    pub fn info(title: impl Into<String>) -> Self {
        Self::new(title)
    }
    pub fn success(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Success)
    }
    pub fn warning(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Warning)
    }
    pub fn danger(title: impl Into<String>) -> Self {
        Self::new(title).severity(ToastSeverity::Danger)
    }

    /// Set the severity (hue + default leading glyph).
    #[heca_grid_ui_macros::prop]
    pub fn severity(mut self, severity: ToastSeverity) -> Self {
        self.severity = severity;
        self.sync_icon();
        self
    }

    /// Override the leading glyph (default: the severity glyph).
    #[heca_grid_ui_macros::prop]
    pub fn icon(mut self, glyph: Glyph) -> Self {
        self.icon = Some(glyph);
        self.show_icon = true;
        self.sync_icon();
        self
    }

    /// Hide the leading icon entirely.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn no_icon(mut self) -> Self {
        self.show_icon = false;
        self.sync_icon();
        self
    }

    /// **The card's body — anything you composed.** A `Flex`, a `Grid`, a realized subtree: the
    /// card gives it the column under the title and lets the engine lay it out.
    ///
    /// The slot is painted under the theme's **muted** token, so unstyled text inside reads as
    /// secondary without the caller saying so — and content that carries its own colour keeps it,
    /// the same way a `.badge-danger` stays red inside a coloured parent.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn body(mut self, body: impl crate::builders::IntoComponent) -> Self {
        let body = body.into_component();
        self.has_body = true;
        self.column_mut()[BODY] = body;
        self
    }

    /// Sugar for the common body: one line of small text. It builds the `Label` the caller would
    /// have built, so there is one code path and not two.
    #[heca_grid_ui_macros::prop]
    pub fn body_text(self, body: impl Into<String>) -> Self {
        let label = Label::new(body)
            .align(TextAlign::Start)
            .truncate(Ellipsis::End)
            .font_scale(BODY_SCALE);
        self.body(label)
    }

    /// **Add an action — a real control, appended to the row under the body.**
    ///
    /// The caller says *what* the action is (`Button::outline("Retry").on_click(…)`); the card says
    /// where it sits and what hue it takes, publishing its severity so the control tones itself.
    /// Call it again for a second action: a notification that can be retried *and* inspected needs
    /// two, and nothing here counts them.
    ///
    /// **No pick declaration is needed.** Anything actionable is lettered by `prefix+/` already, so
    /// a button dropped in here is keyboard-reachable the moment the card is in the tree.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn action(mut self, action: impl crate::builders::IntoComponent) -> Self {
        let action = action.into_component();
        if !self.has_actions {
            self.has_actions = true;
            self.column_mut()[ACTION] = Box::new(
                Flex::row()
                    .gap(ACTION_GAP)
                    // The row hugs its buttons — a column would otherwise stretch it edge to edge
                    // and the actions would read as a banner rather than as things to press.
                    .align_self("start")
                    // And when the card is too narrow for them all, they take a second line
                    // rather than being squeezed to ellipses or laid out past the edge.
                    .wrap(true)
                    // **One more gap above the actions than between the text lines.** The title and
                    // the body are one block of prose; the buttons are a different kind of thing,
                    // and sharing the prose spacing read as a third line of text.
                    .margin_top(Length::Px(GAP))
                    // **A notification's actions are compact controls.** Declared on the row, not on
                    // the button, because the size variant cascades: a caller who sizes their own
                    // button still wins, which is what keeps the choice theirs.
                    .size(WidgetSize::Small),
            );
        }
        self.column_mut()[ACTION].base_mut().children.push(action);
        self
    }

    /// Where this card sits in the box that holds it — a corner or an edge, resolved to auto
    /// margins by the engine. Unset, it sits wherever its parent puts it.
    #[heca_grid_ui_macros::prop]
    pub fn position(mut self, position: ToastPosition) -> Self {
        let (left, right, top, bottom) = position.margins();
        let s = &mut self.base.style.layout;
        s.margin_left = left;
        s.margin_right = right;
        s.margin_top = top;
        s.margin_bottom = bottom;
        self
    }

    /// Whether the × dismiss affordance is shown (default `true`).
    #[heca_grid_ui_macros::prop]
    pub fn dismissible(mut self, on: bool) -> Self {
        self.dismissible = on;
        self.sync_dismiss();
        self
    }

    /// Make the whole card clickable. It fires for a press the action button and the × did not
    /// take — they are children, and a child takes its own press.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(f));
        self.base.activatable = true; // and pickable — a letter runs this (Base::activatable)
        self.base.focusable = true; // a clickable toast is focusable (Component::focusable)
        self
    }

    /// Set the callback fired when the × is clicked. The host removes the toast.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(f));
        self.sync_dismiss();
        self
    }

    /// The title text signal (set it to update reactively).
    pub fn title_signal(&self) -> Signal<String> {
        self.title
    }

    /// Whether it starts on screen. Order-independent with
    /// [`animation`](Toast::animation), exactly as an `Overlay`'s is.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.base.open.set(open);
        self.base.presence.assume_open(open);
        self
    }

    /// **The open-state signal a host binds.** Flipping it is the same act as calling
    /// [`open`](Toast::open) / [`hide`](Toast::hide) — the arrival or the exit plays either way —
    /// so a host that keeps its state in signals drives the card without holding it.
    pub fn open_signal(&self) -> Signal<bool> {
        self.base.open
    }

    /// **How it arrives and leaves** — `Animation::Fade`, `ZoomFade`, or one of your own. Undeclared
    /// it cuts: on screen the frame it opens, gone the frame it hides.
    #[heca_grid_ui_macros::host_only("an animation is a behaviour object, not a value static data carries")]
    pub fn animation(mut self, animation: crate::animation::Animation) -> Self {
        self.base.presence.set_animation(animation.build());
        self
    }

    /// **Put it on screen.** With an animation declared it plays; already up, or still on its way
    /// out, and nothing happens.
    pub fn show(&mut self) {
        if self.base.presence.enter() {
            self.base.open.set(true);
        }
    }

    /// **Dismiss it.** With an animation the card stays laid out, inert, until the gesture has
    /// played out — which is what lets a host drop it from a list only once it has actually gone.
    pub fn close(&mut self) {
        self.base.presence.leave();
        self.base.open.set(false);
    }

    /// Open it if it is closed, dismiss it if it is open.
    pub fn toggle(&mut self) {
        match self.base.presence.is_open() {
            true => self.close(),
            false => self.show(),
        }
    }

    /// Whether it is on screen — **including while it is leaving**, which is when it is still drawn
    /// and no longer interactive.
    pub fn is_showing(&self) -> bool {
        self.base.presence.is_open() || self.base.presence.is_leaving()
    }

    /// Paint the text column one slot at a time, each under the colour that slot means: the title
    /// carries the severity, the body is secondary text, the actions are controls that tone
    /// themselves from what the card published. The card built both levels, so it addresses them
    /// the way `Item` addresses its slots.
    fn paint_column(&self, cx: &mut PaintCx, tone: crate::color::Color, muted: crate::color::Color) {
        let column = self.column();
        let paint = |cx: &mut PaintCx, i: usize, colour: crate::color::Color| {
            if let Some(child) = column.children.get(i) {
                cx.with_content_color(colour, |cx| {
                    crate::component::paint_child(child.as_ref(), cx);
                });
            }
        };
        paint(cx, TITLE, tone);
        paint(cx, BODY, muted);
        paint(cx, ACTION, tone);
    }

    /// The text column's own children — the card built both levels, so it addresses them
    /// directly rather than searching for them.
    fn column_mut(&mut self) -> &mut Vec<Box<dyn Component>> {
        &mut self.base.children[COLUMN].base_mut().children
    }

    fn column(&self) -> &Base {
        self.base.children[COLUMN].base()
    }

    /// Rebuild the leading icon slot. The glyph is the card's own or the severity's; hiding it
    /// leaves an empty slot, which takes no width and no gap.
    fn sync_icon(&mut self) {
        let glyph = self.icon.unwrap_or_else(|| self.severity.default_glyph());
        self.base.children[ICON] = if self.show_icon {
            // A share of the card's font, not a px: the icon then tracks a font or size-variant
            // change with nothing to keep in step by hand.
            let mut icon = Icon::new(glyph);
            icon.base_mut().style.visual.font_scale = ICON_SCALE;
            Box::new(icon)
        } else {
            empty_slot()
        };
    }

    /// Rebuild the × slot — **a real [`IconButton`](super::IconButton)**, so its hover frame, its
    /// press flash and its focus ring are its own. The card used to paint that face and read the
    /// child's hover state to decide how bright it was; a control already knows.
    fn sync_dismiss(&mut self) {
        self.base.children[DISMISS] = if self.dismissible {
            let dismiss = self.on_dismiss.clone();
            Box::new(IconButton::new(Icon::new(Glyph::Close)).on_click(move || {
                if let Some(f) = &dismiss {
                    f();
                }
            }))
        } else {
            empty_slot()
        };
    }

    fn activate_body(&mut self) {
        self.flash.trigger();
        if let Some(f) = &self.on_click {
            f();
        }
    }
}

impl Component for Toast {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable when the card itself is clickable (Enter/Space activates it).
    ///
    /// **The card's height is the engine's answer, not a sum written here.** It hugs its content:
    /// a body line or an action row makes it taller because those children exist, not because a
    /// formula was updated to account for them.
    /// **A closed card takes no space.** Not merely invisible: an inline card in a sidebar
    /// collapses rather than leaving a hole where it used to be. It stays laid out while it is
    /// *leaving*, which is what gives the exit something to play over.
    fn remeasure(&mut self) {
        self.base.set_hidden(!self.is_showing());
    }

    /// This surface's arrival and exit — what a host drives, and what it carries across a rebuild.
    fn show(&mut self) {
        Toast::show(self);
    }

    fn close(&mut self) {
        Toast::close(self);
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let (surface, muted, card_radius, toast_tint) = {
            let t = cx.theme();
            (t.colors.surface, t.colors.muted, t.colors.border_radius, t.colors.interaction.toast_tint)
        };
        // The severity says which token; the token is read from the theme in front of us, so a
        // reload re-tones a card that is already on screen.
        let tone = self.severity.tone(cx.theme());
        let b = self.base.bounds;

        // Surface: severity-tinted fill + the shared Pane/DockFrame corner-bracket
        // reticle frame (GridCN fidelity — same as the Modal panel, #79). The theme
        // rest glow (`PaintCx::rest_glow`) keeps toasts scaling with `glow_size`
        // (T011) — in the THEME glow color, not the severity tone: the bracket
        // frame is always accent, and a danger-red halo under a blue frame blends
        // to a muddy purple fringe (user-reported).
        let glow = cx.rest_glow(GLOW_RADIUS);
        // Everything the card draws goes through the arrival/exit frame, so a fade or a zoom takes
        // the whole card — chrome and content — rather than half of it.
        let frame = self.base.presence.frame();
        frame.apply(cx, b.loc, |cx| {
        cx.rect(b, surface.lerp(tone, toast_tint as f32 / 255.0), None, card_radius, glow);
        cx.bracket_frame(b);

        // **Everything else is published, not painted.** The severity reaches the content as an
        // inherited colour and the controls as an inherited tone, so the icon, the title, the
        // actions and the × all take the card's hue without the card drawing any of them — and a
        // control's hover, press and focus ring are its own, which is what they are for.
        cx.with_accent(tone, |cx| {
            cx.with_content_color(tone, |cx| {
                crate::component::paint_child(self.base.children[ICON].as_ref(), cx);
            });
            self.paint_column(cx, tone, muted);
            // The × rests quiet in the muted token and lights up on its own.
            cx.with_content_color(muted, |cx| {
                crate::component::paint_child(self.base.children[DISMISS].as_ref(), cx);
            });
        });

        // The card's own press — `on_click` or the keyboard. The actions and the × flash
        // themselves, so this is never their press.
        let amount = self.flash.amount();
        if amount > 0.0 {
            cx.flash(b, amount * 0.4, card_radius);
        }
        // Focus ring when clickable + focused (theme-aware shift of the toast tone).
        if self.focusable() && self.base.shows_focus_ring() && cx.theme().colors.show_focus_border {
            let ring = cx.theme().colors.focus_ring_tone(tone);
            cx.focus_ring(b, ring, card_radius);
        }
        });
    }

    /// **The card takes what its children declined.** The action button and the × are children and
    /// take their own press, so a press that reaches here landed on the card itself — which is
    /// exactly what `on_click` means. Bubble, not capture: capturing would take the press before
    /// the two controls the card is built from ever saw it.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if self.base.disabled.get_untracked() {
            return Handled::No;
        }
        match ev {
            Event::PointerDown(_) if self.on_click.is_some() => {
                self.activate_body();
                Handled::Yes
            }
            Event::Key { key: GridKey::Enter | GridKey::Space, pressed: true }
                if self.focusable() =>
            {
                self.activate_body();
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        // A bound signal is as good as a call: one flip of it is an arrival or a dismissal, and
        // neither may be the only way the animation starts.
        self.base.presence.follow(self.base.open.get_untracked());
        let mut animating = self.base.tick_presence(dt);
        animating |= self.flash.tick(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        // When used inline (in-tree), damage just our own rect on a press flash. In a
        // `ToastStack` the toast paints on the overlay layer and the stack owns the
        // damage region, so this mark is simply unobserved there.
        if animating {
            self.base.mark_needs_paint();
        }
        animating
    }
}

impl LayoutExt for Toast {}
