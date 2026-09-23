//! [`Overlay`] — the base overlay surface: scrim + panel chrome + viewport
//! positioning, shared by every overlay widget.
//!
//! **Blocking is a property of this layer, not a per-widget reimplementation**
//! (the T009 overlay rework): a *blocking* overlay paints a dimming scrim over
//! the whole viewport and swallows outside input (modal — [`Dialog`](super::Dialog));
//! a non-blocking one lets outside input fall through (light-dismiss popovers).
//! Positioning is a [property](OverlayPosition) of this layer: the default
//! [`Center`](OverlayPosition::Center) fills the viewport (`Percent(1.0)`²) and
//! **centers** its single panel child with real taffy layout, so every
//! descendant gets true bounds (hint picker + pointer hit-testing need them);
//! [`Anchored`](OverlayPosition::Anchored) instead hangs the panel off a trigger
//! rect (below/flip-above/clamp — [`place_anchored`]) for dropdown/popover
//! specializations. (The `Select`/`Tooltip`/`ContextMenu` widgets still own their
//! placement today; converting their panel *presentation* to compose an anchored
//! `Overlay` is the follow-up — the placement authority now lives here.)
//!
//! **Composition, not inheritance.** A specialized overlay widget ([`Dialog`](super::Dialog))
//! *composes* an `Overlay` as its subtree — the `Overlay` owns presentation
//! (scrim, shadow, panel fill, bracket reticle) and geometry
//! ([`overlay_occludes`](Component::overlay_occludes)); the specialization owns
//! its content and behaviour (focus trap, keyboard, dismissal policy) and
//! intercepts events *before* the `Overlay`'s own standalone handling runs.
//! Used directly (a host mounting an arbitrary — e.g. `realize`d — panel), the
//! `Overlay`'s own event handling provides the standard layer semantics:
//! nested-overlay-first routing, outside-click callback, blocking swallow.
//!
//! Paint goes through [`PaintCx::with_overlay`], so an overlay opened *inside*
//! this panel (a `Select` dropdown in a modal body) records a **deeper** scene
//! segment and composites above everything this overlay draws — see
//! [`Scene::overlay_segments`](crate::scene::Scene::overlay_segments).
//!
//! **This surface is three files**: the widget here, [what a panel looks like](chrome) and
//! [where a panel goes](place).

mod chrome;
mod place;

pub use chrome::{PanelChrome, PanelElevation, paint_panel_chrome};
pub use place::{
    AnchorSide, BesideSide, DEFAULT_ANCHOR_GAP, OverlayPosition, place_anchored, place_anchored_on,
    place_at_point, place_beside,
};

use crate::animation::Animation;
use crate::builders::LayoutExt;
use crate::component::{Base, Component, Event, Handled, PaintCx, paint_child, shift_subtree};
use crate::event::WidgetIntent;
use crate::reactive::{Signal, SignalGet, SignalUpdate};
use crate::style::{Align, Justify, Length};
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::Cell;

/// Gap kept between an overlay panel and the window edge, so a maxed-out panel's
/// border (and its glow) is never shaved off by the viewport boundary.
///
/// Public because a full-window overlay whose content has to **fit** must know how much of the
/// window it does not get: the exposé sizes its cards to fit the room it has, and not knowing about
/// this margin made it overshoot by `2 × 24` on both axes — a whole workspace row off the bottom,
/// four failed corrections deep, because the missing term was outside the code doing the fitting
/// (F003/P082/T420). Read it rather than repeating the number.
pub const VIEWPORT_MARGIN: f32 = 24.0;
/// How far a blocking scrim reaches before a viewport has been cached (logical px).
const SCRIM_REACH: f64 = 1.0e9;

/// The base overlay surface: a viewport-filling, centering layer that paints a panel (its single
/// child) with the shared overlay chrome — optional scrim, drop shadow, theme surface fill, and the
/// bracket reticle.
///
/// Build with [`Overlay::new`], hand it the panel via [`panel`](Overlay::panel) (or
/// a mapper-produced `Box<dyn Component>` goes through the same builder), and raise it
/// with [`open`](Overlay::open) / [`hide`](Overlay::hide) / [`toggle`](Overlay::toggle) — or bind
/// [`open_signal`](Overlay::open_signal), which a composing widget does.
/// [`blocking`](Overlay::blocking) selects the layer policy: blocking (default) = scrim + swallow
/// outside input; non-blocking = outside input falls through (light dismiss).
/// [`animation`](Overlay::animation) says how it arrives and leaves; without one it simply appears
/// and goes.
/// **A surface you can show and close from anywhere** — copyable, so it goes into any closure.
///
/// ```ignore
/// let confirm = Dialog::new("Close pane?").body(..).action(..);
/// Button::new("Delete").on_click(move || confirm.show())
/// ```
///
/// `show`/`hide` on the widget itself take `&mut self`, so a closure living inside a button can
/// never hold one while the surface sits beside it. That is why every caller reached for the raw
/// signal instead. This is the signal with the two verbs on it, and nothing else.
#[derive(Clone, Copy)]
pub struct SurfaceHandle {
    open: Signal<bool>,
}

impl SurfaceHandle {
    /// Wrap a surface's open signal.
    pub fn new(open: Signal<bool>) -> Self {
        Self { open }
    }

    /// Put it on screen.
    pub fn show(&self) {
        crate::reactive::SignalUpdate::set(&self.open, true);
    }

    /// Take it off screen.
    pub fn close(&self) {
        crate::reactive::SignalUpdate::set(&self.open, false);
    }

    /// Show it if it is closed, close it if it is up.
    pub fn toggle(&self) {
        let now = crate::reactive::SignalGet::get_untracked(&self.open);
        crate::reactive::SignalUpdate::set(&self.open, !now);
    }

    /// Whether it is up.
    pub fn is_open(&self) -> bool {
        crate::reactive::SignalGet::get_untracked(&self.open)
    }

    /// The signal itself, for binding something else to the same state.
    pub fn signal(&self) -> Signal<bool> {
        self.open
    }
}

pub struct Overlay {
    base: Base,
    /// Blocking layer policy: scrim + swallow outside input (modal). `false` ⇒
    /// no scrim; outside input falls through after the outside-click callback.
    blocking: bool,
    /// Whether what is behind this surface is blurred — see [`frosted`](Overlay::frosted).
    frosted: bool,
    /// Fired when a press lands outside the panel — the standalone dismissal
    /// hook (a composing widget usually implements its own policy instead).
    on_outside_click: Option<Box<dyn Fn()>>,
    /// **What the dismiss key means to this surface**, when it means anything.
    ///
    /// An open overlay already holds the keyboard (`base.focused = open`), so the key arrives here
    /// whatever composes it — it was simply dropped, because this widget answered pointer events
    /// and nothing else. Every surface built on top therefore wrote its own dismissal: `Dialog`,
    /// `ContextMenu` and `CommandPalette` each have one, three copies of the same sentence, and a
    /// surface **composed** rather than built — which is what a plugin writes — had none at all
    /// and could not be closed from the keyboard (F003/P097/T502).
    ///
    /// Unset means unset: the key is left alone and passes to whatever composes this overlay, so
    /// the three widgets above keep answering it exactly as they did.
    on_dismiss: Option<Box<dyn Fn()>>,
    /// **Keyboard focus among the panel's focusable descendants.**
    ///
    /// Tab is the universal focus primitive — not a rebindable binding — and it belongs to
    /// whatever *contains* the focusables. That is this widget: an open overlay already holds the
    /// keyboard, and its panel is the subtree to walk. It lived only in `Dialog`, so a surface
    /// composed rather than built could not be tabbed through at all (F003/P097/T502).
    focus: crate::focus::FocusManager,
    /// **Which of its own controls the keyboard starts on**, by the name that control goes by.
    ///
    /// The author's call, not the framework's: a confirm wants the safe button, a form wants its
    /// first field, and a menu wants nothing at all. Unset means nothing is focused, which is what
    /// every surface did before there was a way to say otherwise.
    default_focus: Option<String>,
    /// How the panel is placed: centered (default) or anchored to a trigger rect.
    position: OverlayPosition,
    /// Explicit panel size, applied to the panel child's style. `None` (default)
    /// leaves the panel to size itself from its content.
    panel_size: Option<(Length, Length)>,
    /// Last-seen viewport, cached during paint (scrim rect + anchored placement).
    viewport: Cell<Size>,
}

#[heca_grid_ui_macros::props]
impl Overlay {
    /// A new (closed) blocking overlay with an empty panel slot.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Fill the viewport and center the panel on both axes — real taffy
        // centering, so every descendant gets true bounds.
        base.style.layout.width = Length::Percent(1.0);
        base.style.layout.height = Length::Percent(1.0);
        base.style.layout.justify = Justify::Center;
        base.style.layout.align = Align::Center;
        // Breathing room between the panel and the window edge. It doubles as the
        // inset for the viewport cap in `apply_panel_size`: the panel's `Percent(1.0)`
        // max resolves against this padded content box, so even a huge panel keeps
        // this margin and its border/glow is never shaved by the window edge.
        base.style.layout.padding = (VIEWPORT_MARGIN).into();
        // The flag every component carries, not one of this widget's own — see `Base::open`.
        let open = base.open;
        // **An open layer holds the keyboard.** Binding `open` to `Base::focused` is the whole of
        // how keys and intents reach a panel: the framework delivers them down the focus owner's
        // ancestor chain, so an open overlay is on that path and a closed one is not. It replaces
        // this widget forwarding every key and intent into its panel by hand — the forwarding that
        // had to exist because the overlay had taken over its own subtree's walk in the first place.
        base.focused = open;
        Self {
            base,
            blocking: true,
            frosted: false,
            on_outside_click: None,
            on_dismiss: None,
            focus: crate::focus::FocusManager::new(),
            default_focus: None,
            position: OverlayPosition::Center,
            panel_size: None,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
            // No animation until one is declared: a surface that never asked for one is a cut,
            // costs nothing, and needs no special case anywhere.
        }
    }

    /// Set the **panel** — the single child this layer centers and decorates.
    /// The caller owns the panel's internal layout (padding, gaps, children);
    /// the overlay owns the chrome around it. Replaces any previous panel.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn panel(mut self, panel: impl crate::builders::IntoComponent) -> Self {
        self.base.children.clear();
        self.base.children.push(panel.into_component());
        self.apply_panel_size();
        self
    }

    /// Like [`panel`](Overlay::panel) but takes an already-boxed component —
    /// for a panel produced by a mapper returning `Box<dyn Component>` (e.g.
    /// `heca`'s `realize(ViewNode)`).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    /// Give the panel an explicit size instead of letting it hug its content.
    ///
    /// The main reason to want this: **a [`ScrollRegion`](super::ScrollRegion)
    /// only scrolls when its parent bounds it.** A panel that sizes to its content
    /// simply grows with a long body, so nothing ever overflows and no scrollbar
    /// appears. Give the panel a height and the body can scroll inside it.
    ///
    /// `Length::Auto` on an axis means "as before" (hug the content). A
    /// [`Percent`](Length::Percent) resolves against the **viewport**, because the
    /// `Overlay` itself fills it — so `Percent(0.8)` is 80% of the viewport, not 80%
    /// of anything the caller laid out.
    ///
    /// Call order does not matter: this is stored on the overlay and re-applied
    /// whenever the panel is (re)set by [`panel`](Overlay::panel) /
    /// [`panel_boxed`](Overlay::panel_boxed).
    ///
    /// ```ignore
    /// // A modal that is 60% of the viewport wide and 70% tall, whose body scrolls.
    /// Overlay::new()
    ///     .panel_size(Length::Percent(0.6), Length::Percent(0.7))
    ///     .panel(Flex::column().child(ScrollRegion::new().child(long_content)))
    /// ```
    #[heca_grid_ui_macros::host_only(
        "takes more than one value, which a single property cannot carry"
    )]
    pub fn panel_size(mut self, width: Length, height: Length) -> Self {
        self.panel_size = Some((width, height));
        self.apply_panel_size();
        self
    }

    /// Push the configured panel size onto the panel child's style, and **always**
    /// cap the panel at the viewport.
    ///
    /// The cap is unconditional, not part of `panel_size`: the `Overlay` fills the
    /// viewport, so `Percent(1.0)` here *is* the window. Without it a fixed `Px` panel
    /// (or a big content-sized one) draws larger than the window and gets cut off
    /// by the screen edge on both sides — a dialog must never be bigger than the
    /// thing it is centered in.
    fn apply_panel_size(&mut self) {
        let Some(panel) = self.base.children.first_mut() else {
            return;
        };
        let style = &mut panel.base_mut().style.layout;
        style.max_width = Some(Length::Percent(1.0));
        style.max_height = Some(Length::Percent(1.0));
        if let Some((w, h)) = self.panel_size {
            style.width = w;
            style.height = h;
        }
    }

    /// Layer policy: `true` (default) = modal — dimming scrim + outside input
    /// swallowed; `false` = light layer — no scrim, outside input falls through.
    #[heca_grid_ui_macros::prop]
    pub fn blocking(mut self, blocking: bool) -> Self {
        self.blocking = blocking;
        self
    }

    /// **Blur what is behind this surface.** The counterpart of the scrim: a scrim tints what is
    /// underneath, a frost takes its detail away, and a surface may want either, both or neither.
    ///
    /// ```ignore
    /// Overlay::new().blocking(true).frosted(true).panel(map)   // the exposé's backdrop
    /// ```
    ///
    /// **Strength is the theme's**, not the caller's — `overlay_frost_radius`, beside the scrim
    /// alpha it is the counterpart of. A theme that wants a flat backdrop sets it to `0` and every
    /// frosted surface answers together; nothing is decided at a call site. Same bargain as every
    /// other token: the caller picks the semantic, the widget owns the pixels.
    ///
    /// **It blurs exactly what it occludes** — the viewport when [`blocking`](Overlay::blocking),
    /// the panel alone when not — and it **fades with the surface that asked for it**, so a
    /// dissolving overlay's backdrop dissolves with it instead of holding the session out of focus
    /// and snapping sharp in one frame.
    ///
    /// The blur is GPU work, so this widget only **records the request** at its place in the
    /// drawing order and the host performs it (`docs/surface-compositor.md` § 0.5). That is why a
    /// frosted surface needs nothing from whoever places it — no registry, no declaration, no host
    /// pass keyed on it.
    #[heca_grid_ui_macros::prop]
    pub fn frosted(mut self, frosted: bool) -> Self {
        self.frosted = frosted;
        self
    }

    /// Set the panel placement (default [`OverlayPosition::Center`]).
    #[heca_grid_ui_macros::host_only(
        "an anchored placement carries a host-computed trigger rect, which static data cannot \
         supply; a description gets the centered placement"
    )]
    pub fn position(mut self, position: OverlayPosition) -> Self {
        self.position = position;
        self
    }

    /// Anchor the panel to a trigger `rect` (dropdown/popover placement): below,
    /// flipped above when no room, left-edge aligned, clamped into the viewport —
    /// see [`place_anchored`]. Uses [`DEFAULT_ANCHOR_GAP`]; pair with a
    /// non-[`blocking`](Overlay::blocking) layer for a light-dismiss popover.
    #[heca_grid_ui_macros::host_only("a host-computed anchor rect, not authorable data")]
    pub fn anchored(mut self, rect: Rectangle) -> Self {
        self.position = OverlayPosition::Anchored {
            anchor: rect,
            gap: DEFAULT_ANCHOR_GAP,
        };
        self
    }

    /// Update the anchor rect of an [`Anchored`](OverlayPosition::Anchored)
    /// overlay in place (a host re-anchoring to a moved trigger). No-op in
    /// [`Center`](OverlayPosition::Center) mode.
    pub fn set_anchor(&mut self, rect: Rectangle) {
        if let OverlayPosition::Anchored { anchor, .. } = &mut self.position
            && *anchor != rect
        {
            *anchor = rect;
            self.place_panel();
        }
    }

    /// Place the panel for an anchored overlay by baking the placement offset into
    /// the panel child's bounds (the same subtree-shift trick `Select`/`ScrollRegion`
    /// use). Idempotent: the target is absolute, so re-running never compounds.
    /// A no-op in [`Center`](OverlayPosition::Center) mode (taffy centers there).
    fn place_panel(&mut self) {
        let OverlayPosition::Anchored { anchor, gap } = self.position else {
            return;
        };
        let Some(child) = self.base.children.first_mut() else {
            return;
        };
        let current = child.base().bounds;
        if current.size == Size::new(0.0, 0.0) {
            return; // Not laid out yet — nothing to place.
        }
        let target = place_anchored(anchor, current.size, self.viewport.get(), gap);
        let dx = target.loc.x - current.loc.x;
        let dy = target.loc.y - current.loc.y;
        if dx != 0.0 || dy != 0.0 {
            shift_subtree(child.as_mut(), dx, dy);
            self.base.mark_needs_paint();
        }
    }

    /// **Whether it starts up.** A surface *born* open is already there and plays no arrival.
    ///
    /// To follow state you already hold, use [`open_when`](Overlay::open_when) instead — this one
    /// is a starting value, which is all a description can carry.
    ///
    /// Order-independent: `.default_open(true).animation(..)` and `.animation(..).default_open(true)` are the
    /// same surface.
    #[heca_grid_ui_macros::prop]
    pub fn default_open(mut self, open: bool) -> Self {
        self.base.open.set(open);
        self.base.presence.assume_open(open);
        self
    }

    /// **Follow a signal of your own** — the surface is up exactly when it is true.
    ///
    /// ```ignore
    /// let editing = signal(false);
    /// Overlay::new().panel(body).open_when(editing);
    /// editing.set(true);      // it appears
    /// ```
    ///
    /// This is the direction that was missing. The widget owned a signal and lent it out through
    /// [`open_signal`](Overlay::open_signal), so a caller could drive *its* state but never hand it
    /// *theirs* — backwards from how state is held everywhere else here.
    ///
    /// `base.focused` is bound to whichever signal is in force — that binding is the whole of how
    /// keys reach a panel — so adopting yours rebinds it rather than leaving it pointed at a signal
    /// nobody writes any more.
    #[heca_grid_ui_macros::host_only(
        "a live signal; a description carries a starting value, `opened`"
    )]
    pub fn open_when(mut self, open: Signal<bool>) -> Self {
        self.base.open = open;
        self.base.focused = open;
        self.base
            .presence
            .assume_open(crate::reactive::SignalGet::get_untracked(&open));
        self
    }

    /// **Which control the keyboard starts on**, by the name that control goes by.
    ///
    /// ```ignore
    /// Overlay::new().panel(body).default_focus("Cancel")
    /// ```
    ///
    /// Yours to decide, because only you know which control is safe: a confirm starts on the
    /// button that changes nothing, a form on its first field, a menu on neither. Unset leaves the
    /// keyboard where it was.
    ///
    /// **No `key` required.** The name is the control's declared `key` when it has one and the
    /// words it reads by when it does not — see [`FocusManager::focus_named`].
    #[heca_grid_ui_macros::prop]
    pub fn default_focus(mut self, name: impl Into<String>) -> Self {
        self.default_focus = Some(name.into());
        self
    }

    /// **Put it on screen.** If an [`animation`](Overlay::animation) was declared it plays;
    /// if not, it is simply up. Already up, or still on its way out, and nothing happens.
    ///
    /// Opening is also where the keyboard is placed, when the surface said where it should go —
    /// here rather than at construction, because a surface is built once and opened many times.
    pub fn show(&mut self) {
        // Refuses while the exit is still playing, exactly as `Presence::enter` does — asked here
        // so the open flag is not raised on a surface that is still going away.
        if self.base.presence.is_leaving() {
            return;
        }
        self.base.open.set(true);
        self.settle();
    }

    /// **The one place this surface reacts to becoming open or closed**, whoever asked.
    ///
    /// There are two ways in and they must not each carry their own version of what opening means:
    /// a caller says [`show`](Overlay::show), or the open flag is simply set — by
    /// [`SurfaceHandle`], by [`open_when`](Overlay::open_when), by a host binding its own state.
    /// Reacting in the verb alone is what left the second way half-done: the arrival played,
    /// because [`tick`](Component::tick) follows the flag every frame, but the keyboard was never
    /// put where the surface said, because that lived in `show` and nothing else called it. A
    /// surface raised by a signal came up with the keyboard nowhere.
    ///
    /// So the rule lives here and is asked at both moments: `show` runs it at once (the keyboard
    /// must not wait for the next frame), and `tick` runs it every frame for everyone else.
    fn settle(&mut self) {
        let was_open = self.base.presence.is_open();
        self.base.presence.follow(self.base.open.get_untracked());
        if !was_open && self.base.presence.is_open() {
            self.place_default_focus();
        }
    }

    /// Put the keyboard where [`default_focus`](Overlay::default_focus) said, if that control is
    /// there. Goes **through the focus manager**, which is the point: writing the control's
    /// `focused` signal by hand leaves the manager's own position unset, so the next Tab is spent
    /// moving to the first control instead of the next one and the first press appears to do
    /// nothing (Antonio, driving, 2026-09-07).
    fn place_default_focus(&mut self) {
        let Some(name) = self.default_focus.clone() else {
            return;
        };
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.focus_named(panel.as_mut(), &name);
        }
    }

    /// **Dismiss it.** With an animation this begins the exit and the surface stays on screen,
    /// inert, until the gesture has played out; with none, it is gone now.
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

    /// The open-state signal — the host (or composing widget) binds this.
    /// **A handle to show and close this surface from anywhere.** See [`SurfaceHandle`].
    pub fn handle(&self) -> SurfaceHandle {
        SurfaceHandle::new(self.base.open)
    }

    pub fn open_signal(&self) -> Signal<bool> {
        self.base.open
    }

    /// **How this surface arrives and leaves.**
    ///
    /// ```ignore
    /// Overlay::new().panel(body).animation(Animation::Fade)
    /// Overlay::new().panel(body).animation(Animation::ZoomFade)          // the exposé's gesture
    /// Overlay::new().panel(body).animation(Animation::Zoom.from(0.8))    // tuned
    /// Overlay::new().panel(body).animation(Animation::of(MyWhirl::new()))// …or one you wrote
    /// ```
    ///
    /// The `Overlay` owns appearing and disappearing as a **concept** — it already owns the scrim,
    /// the panel chrome, the placement and whether it is open — and hands the *how* to an
    /// [`Animation`]. So [`show`](Component::show) and [`hide`](Component::hide) stop being
    /// instant: a dismissed overlay stays on screen, inert, until its exit has played out.
    ///
    /// **One builder, both authors.** A description names the same animation, because
    /// [`Animation`]'s variants *are* the vocabulary — `{"kind": "overlay", "props":
    /// {"animation": "zoom_fade"}}`. An unknown name leaves the surface with its default rather
    /// than failing: the model is untrusted input.
    ///
    /// Declared, never decided by whoever dismisses it: whether a surface dissolves or cuts is a
    /// property *of the surface*. A full-screen map that vanishes mid-keystroke reads as a glitch;
    /// a context menu that lingers reads as lag.
    #[heca_grid_ui_macros::prop]
    pub fn animation(mut self, animation: Animation) -> Self {
        self.base.presence.set_animation(animation.build());
        self
    }

    /// Called when a press lands **outside** the panel (standalone use; a
    /// composing widget usually intercepts the press and applies its own
    /// dismissal policy instead).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    /// **What the dismiss key does to this surface.**
    ///
    /// ```ignore
    /// Overlay::new().panel(my_panel).on_dismiss(move || close())
    /// ```
    ///
    /// The keys already arrive — an open overlay holds them. This is what makes one of them mean
    /// something, and it is on the overlay rather than in each surface built from it, so a
    /// composed surface closes on Escape without its author writing anything.
    #[heca_grid_ui_macros::host_only("a callback, not a scalar — behaviour crosses as an Intent")]
    pub fn on_dismiss(mut self, f: impl Fn() + 'static) -> Self {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    #[heca_grid_ui_macros::host_only("a callback, not a scalar")]
    pub fn on_outside_click(mut self, f: impl Fn() + 'static) -> Self {
        self.on_outside_click = Some(Box::new(f));
        self
    }

    /// The panel's laid-out bounds (valid after layout; zero before).
    pub fn panel_bounds(&self) -> Rectangle {
        self.base
            .children
            .first()
            .map(|c| c.base().bounds)
            .unwrap_or_else(|| Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0)))
    }

    /// **How far a blocking surface reaches** — the whole viewport, or the panel alone before one
    /// has been seen. The scrim and the frosted backdrop are the same reach said twice, so they
    /// read it from one place rather than each deriving it.
    fn scrim_rect(&self, panel: Rectangle) -> Rectangle {
        let vp = self.viewport.get();
        match vp.w.is_finite() {
            true => Rectangle::new(Point::new(0.0, 0.0), vp),
            false => panel,
        }
    }

    fn is_open(&self) -> bool {
        self.base.open.get_untracked()
    }

    /// **Is there still something to draw?** Open, or dismissed and still playing its exit.
    ///
    /// The distinction that makes an animated dismissal possible at all: input follows
    /// [`is_open`](Self::is_open) — a dissolving surface holds nothing, or the map would refuse
    /// every act on the pane it exists to let you choose — while *painting* follows this.
    fn is_present(&self) -> bool {
        self.is_open() || self.base.presence.is_leaving()
    }

    /// The fixed point an animation's scale works about: the middle of this surface, so it grows
    /// from and shrinks toward its own centre rather than a corner. For a centered layer that is
    /// the middle of the window (it fills it); for an anchored one it is the panel, which is the
    /// only part of it there is.
    fn scale_origin(&self) -> Point {
        let r = match self.position {
            OverlayPosition::Center => self.base.bounds,
            OverlayPosition::Anchored { .. } => self.panel_bounds(),
        };
        Point::new(r.loc.x + r.size.w / 2.0, r.loc.y + r.size.h / 2.0)
    }
}

impl Default for Overlay {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Overlay {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Focusable only while open, so a host's overlay scan can route input here
    /// when the `Overlay` is mounted directly (a composing widget like `Dialog`
    /// is found first in pre-order and intercepts instead).
    fn focusable(&self) -> bool {
        self.is_open()
    }

    fn overlay_active(&self) -> bool {
        self.is_open()
    }

    /// Layout just reset the panel to its taffy-computed position; in
    /// [`Anchored`](OverlayPosition::Anchored) mode, re-place it against the
    /// trigger rect (idempotent — see [`place_panel`](Overlay::place_panel)).
    /// [`Center`](OverlayPosition::Center) mode keeps taffy's centering untouched.
    fn on_layout(&mut self) {
        self.place_panel();
    }

    /// **A layer is not scrolled into view.** `Base::focused` says this widget holds the keyboard
    /// while it is open, and `wants_visible` defaults to exactly that — so an enclosing
    /// `ScrollRegion` would scroll the page to wherever this widget's layout node happens to sit,
    /// every frame it is open. A layer draws over the page; the page does not come to it.
    fn wants_visible(&self) -> bool {
        false
    }

    /// This surface's arrival and exit — what a host drives, and what it carries across a rebuild.
    fn show(&mut self) {
        Overlay::show(self);
    }

    fn set_default_focus(&mut self, name: &str) {
        self.default_focus = Some(name.to_string());
    }

    fn follow_open(&mut self, open: Signal<bool>) {
        self.base.open = open;
        self.base.focused = open;
        self.base
            .presence
            .assume_open(crate::reactive::SignalGet::get_untracked(&open));
    }

    fn advance_focus(&mut self, forward: bool) {
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.advance(panel.as_mut(), forward);
        }
    }

    fn focus_first_quiet(&mut self) {
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.focus_first_quiet(panel.as_mut());
        }
    }

    fn focus_at_trapped(&mut self, pos: heca_core::layout::Point) {
        if let Some(panel) = self.base.children.first_mut() {
            self.focus.focus_at_trapped(panel.as_mut(), pos);
        }
    }

    fn close(&mut self) {
        Overlay::close(self);
    }

    /// Advance the arrival or exit, then the subtree.
    ///
    /// Runs [`settle`](Overlay::settle) because a **composing** widget drives the same surface
    /// through [`open_signal`](Overlay::open_signal) — one flip of that signal is an arrival or a
    /// dismissal exactly as a host's call is, and neither may be the only way an animation starts,
    /// or the only way the keyboard is placed.
    ///
    /// (It named `Component::set_open` before, which does not exist and never has.)
    fn tick(&mut self, dt: f32) -> bool {
        self.settle();
        let mut animating = self.base.tick_presence(dt);
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// The overlay's **input** surface: nothing at all while closed (the panel is still in the
    /// tree and still laid out, and must be completely inert), the whole viewport while
    /// **blocking** (the scrim owns every point), and just the panel otherwise, so a press beside a
    /// non-blocking layer reaches the page behind it.
    fn hit_bounds(&self) -> Option<Rectangle> {
        if !self.is_open() {
            return None;
        }
        if self.blocking {
            let vp = self.viewport.get();
            return Some(if vp.w.is_finite() {
                Rectangle::new(Point::new(0.0, 0.0), vp)
            } else {
                Rectangle::new(
                    Point::new(-SCRIM_REACH, -SCRIM_REACH),
                    Size::new(2.0 * SCRIM_REACH, 2.0 * SCRIM_REACH),
                )
            });
        }
        Some(self.panel_bounds())
    }

    /// A **blocking** overlay occludes the whole viewport (its scrim owns every
    /// point); a non-blocking one occludes only the panel itself.
    fn overlay_occludes(&self, pos: Point) -> bool {
        self.is_open() && (self.blocking || self.panel_bounds().contains(pos))
    }

    /// Paints while it is **present** — open, or dismissed and still leaving — and paints
    /// everything it draws under its [`Animation`]'s frame.
    ///
    /// One place, for the whole surface: the scrim, the panel chrome and every descendant go
    /// through the same [`AnimationFrame::apply`](crate::animation::AnimationFrame::apply), so no widget inside knows an animation is
    /// running and a new animation needs no drawing code anywhere. A frame is something done *to*
    /// a picture — it never re-measures, never moves bounds, and therefore never changes what is
    /// clickable.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() || !self.is_present() {
            return;
        }
        self.viewport.set(cx.viewport());
        let (background, scrim_a, frost_radius) = {
            let t = cx.theme();
            (
                t.colors.background,
                t.colors.interaction.scrim,
                t.colors.overlay_frost_radius,
            )
        };
        let panel = self.panel_bounds();

        let frame = self.base.presence.frame();

        // **The frost is recorded here, in the BASE segment, before anything this surface draws.**
        //
        // Two placements matter and neither is arbitrary:
        //
        // - **Not inside `with_overlay`.** Everything this surface draws goes to the deferred
        //   overlay band so it composites above its siblings; a backdrop must sit in the base
        //   segment instead, because the host performs it between flushing the base and flushing
        //   the overlay bands — which is exactly "after everything beneath me, before me".
        // - **Faded by the animation but NOT scaled.** `frame.apply` also scales and translates,
        //   and a blurred *region* that zooms is not what "blur what is behind me" means. The
        //   opacity alone is wanted: it is what makes the backdrop dissolve with the surface that
        //   asked for it.
        //
        // It blurs what it occludes — the viewport when blocking, the panel alone when not —
        // mirroring `overlay_occludes`, so the frost and the input policy can never disagree about
        // this surface's reach.
        if self.frosted {
            let reach = match self.blocking {
                true => self.scrim_rect(panel),
                false => panel,
            };
            cx.with_opacity(frame.opacity, |cx| {
                cx.backdrop_blur(reach, frost_radius, 1.0);
            });
        }

        frame.apply(cx, self.scale_origin(), |cx| {
            cx.with_overlay(|cx| {
                // Scrim over the whole viewport — the visual half of the blocking
                // layer policy (the event half swallows outside input below).
                if self.blocking {
                    cx.rect(
                        self.scrim_rect(panel),
                        background.with_alpha(scrim_a),
                        None,
                        0.0,
                        None,
                    );
                }

                // Lift the panel, fill it, stamp the shared bracket reticle (same
                // visual language as Pane / DockFrame) — the base layer takes the
                // chrome plain; specializations pass their own accents.
                paint_panel_chrome(cx, panel, PanelChrome::default());

                // The panel's real children on top of the fill. A nested overlay
                // painted in here records a DEEPER scene segment → composites above
                // everything this layer draws (Scene::overlay_segments).
                for child in &self.base.children {
                    paint_child(child.as_ref(), cx);
                }
            })
        });
    }

    /// What the panel did not take. A press here landed on the scrim: it fires
    /// `on_outside_click`, and a **blocking** layer swallows it (and every other pointer event)
    /// so nothing behind the modal is driven through it.
    fn on_event(&mut self, ev: &Event) -> Handled {
        if !self.is_open() {
            return Handled::No;
        }
        match ev {
            Event::PointerDown(p) => {
                if !self.panel_bounds().contains(p.pos)
                    && let Some(f) = &self.on_outside_click
                {
                    f();
                }
                self.swallow()
            }
            Event::PointerUp(_) | Event::PointerMove(_) | Event::Scroll(_) => self.swallow(),
            // **The dismiss key, answered where the keys already arrive.**
            //
            // An open overlay holds the keyboard — `base.focused = open`, set in the constructor —
            // so this key has always been delivered here and was simply dropped. Every surface
            // built on top wrote its own dismissal instead (`Dialog`, `ContextMenu`,
            // `CommandPalette`: three copies of one sentence), and a surface **composed** rather
            // than built had none at all — a plugin's overlay could not be closed from the
            // keyboard (F003/P097/T502).
            //
            // **Unanswered when nothing was declared**, which is what keeps those three exactly as
            // they were: they set no dismissal on their inner overlay, so the key passes up to
            // them untouched. A surface that declares one closes on it, whoever composed it.
            // **Tab moves through the panel, and stays inside it.**
            //
            // Universal, so it is not gated on anything being declared: an overlay is a place the
            // keyboard is contained, and containment is what makes Tab mean something here rather
            // than wandering into the page behind. `Dialog` did this for itself and nothing else
            // did, which is why a composed surface — a plugin's — had no traversal at all.
            Event::Key {
                key: crate::component::GridKey::Tab,
                pressed: true,
            } => {
                let forward = !crate::event::modifiers().shift;
                Component::advance_focus(self, forward);
                Handled::Yes
            }
            Event::Widget(WidgetIntent::Dismiss) => match &self.on_dismiss {
                Some(f) => {
                    f();
                    Handled::Yes
                }
                None => Handled::No,
            },
            // A **non-blocking** layer is not on the path of a press beside it, so the press
            // reaches it as the outside event instead. Same hook, both shapes.
            Event::PointerDownOutside(_) => {
                if let Some(f) = &self.on_outside_click {
                    f();
                }
                Handled::No
            }
            _ => Handled::No,
        }
    }
}

impl Overlay {
    /// A blocking layer consumes what nothing in it wanted; a non-blocking one lets it through.
    fn swallow(&self) -> Handled {
        if self.blocking {
            Handled::Yes
        } else {
            Handled::No
        }
    }
}

impl LayoutExt for Overlay {}

#[cfg(test)]
mod tests;
