//! [`Overlay`] — the base overlay surface: scrim + panel chrome + viewport
//! positioning, shared by every overlay widget.
//!
//! **Blocking is a property of this layer, not a per-widget reimplementation**
//! (the T009 overlay rework): a *blocking* overlay paints a dimming scrim over
//! the whole viewport and swallows outside input (modal — [`Dialog`](super::Dialog));
//! a non-blocking one lets outside input fall through (light-dismiss popovers).
//! Positioning is a [property](OverlayPosition) of this layer: the default
//! [`Center`](OverlayPosition::Center) fills the viewport (`Pct(1.0)`²) and
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

pub use chrome::{paint_panel_chrome, PanelChrome, PanelElevation};
pub use place::{
    place_anchored, place_anchored_on, place_at_point, place_beside, AnchorSide, BesideSide,
    OverlayPosition, DEFAULT_ANCHOR_GAP,
};

use crate::animation::{Animation, Presence};
use crate::builders::LayoutExt;
use crate::component::{paint_child, shift_subtree, Base, Component, Event, Handled, PaintCx};
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
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
/// [`panel_boxed`](Overlay::panel_boxed) for a mapper-produced `Box<dyn Component>`), and raise it
/// with [`open`](Overlay::open) / [`hide`](Overlay::hide) / [`toggle`](Overlay::toggle) — or bind
/// [`open_signal`](Overlay::open_signal), which a composing widget does.
/// [`blocking`](Overlay::blocking) selects the layer policy: blocking (default) = scrim + swallow
/// outside input; non-blocking = outside input falls through (light dismiss).
/// [`animation`](Overlay::animation) says how it arrives and leaves; without one it simply appears
/// and goes.
pub struct Overlay {
    base: Base,
    open: Signal<bool>,
    /// Blocking layer policy: scrim + swallow outside input (modal). `false` ⇒
    /// no scrim; outside input falls through after the outside-click callback.
    blocking: bool,
    /// Fired when a press lands outside the panel — the standalone dismissal
    /// hook (a composing widget usually implements its own policy instead).
    on_outside_click: Option<Box<dyn Fn()>>,
    /// How the panel is placed: centered (default) or anchored to a trigger rect.
    position: OverlayPosition,
    /// Explicit panel size, applied to the panel child's style. `None` (default)
    /// leaves the panel to size itself from its content.
    panel_size: Option<(Length, Length)>,
    /// Last-seen viewport, cached during paint (scrim rect + anchored placement).
    viewport: Cell<Size>,
    /// **Where this surface is in its arriving and leaving**, and the animation carrying it —
    /// see [`Overlay::animation`]. A host drives it through
    /// [`Component::set_open`](crate::Component::set_open); `open` and this are the same fact
    /// said twice, one as the signal a composing widget binds, one as the gesture in flight.
    presence: Presence,
}

#[heca_grid_ui_macros::props]
impl Overlay {
    /// A new (closed) blocking overlay with an empty panel slot.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Fill the viewport and center the panel on both axes — real taffy
        // centering, so every descendant gets true bounds.
        base.style.layout.width = Length::Pct(1.0);
        base.style.layout.height = Length::Pct(1.0);
        base.style.layout.justify = Justify::Center;
        base.style.layout.align = Align::Center;
        // Breathing room between the panel and the window edge. It doubles as the
        // inset for the viewport cap in `apply_panel_size`: the panel's `Pct(1.0)`
        // max resolves against this padded content box, so even a huge panel keeps
        // this margin and its border/glow is never shaved by the window edge.
        base.style.layout.padding = VIEWPORT_MARGIN;
        let open = signal(false);
        // **An open layer holds the keyboard.** Binding `open` to `Base::focused` is the whole of
        // how keys and intents reach a panel: the framework delivers them down the focus owner's
        // ancestor chain, so an open overlay is on that path and a closed one is not. It replaces
        // this widget forwarding every key and intent into its panel by hand — the forwarding that
        // had to exist because the overlay had taken over its own subtree's walk in the first place.
        base.focused = open;
        Self {
            base,
            open,
            blocking: true,
            on_outside_click: None,
            position: OverlayPosition::Center,
            panel_size: None,
            viewport: Cell::new(Size::new(f64::INFINITY, f64::INFINITY)),
            // No animation until one is declared: a surface that never asked for one is a cut,
            // costs nothing, and needs no special case anywhere.
            presence: Presence::new(),
        }
    }

    /// Set the **panel** — the single child this layer centers and decorates.
    /// The caller owns the panel's internal layout (padding, gaps, children);
    /// the overlay owns the chrome around it. Replaces any previous panel.
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn panel(mut self, panel: impl Component + 'static) -> Self {
        self.base.children.clear();
        self.base.children.push(Box::new(panel));
        self.apply_panel_size();
        self
    }

    /// Like [`panel`](Overlay::panel) but takes an already-boxed component —
    /// for a panel produced by a mapper returning `Box<dyn Component>` (e.g.
    /// `heca`'s `realize(ViewNode)`).
    #[heca_grid_ui_macros::host_only("composed content — a description uses `children`")]
    pub fn panel_boxed(mut self, panel: Box<dyn Component>) -> Self {
        self.base.children.clear();
        self.base.children.push(panel);
        self.apply_panel_size();
        self
    }

    /// Give the panel an explicit size instead of letting it hug its content.
    ///
    /// The main reason to want this: **a [`ScrollRegion`](super::ScrollRegion)
    /// only scrolls when its parent bounds it.** A panel that sizes to its content
    /// simply grows with a long body, so nothing ever overflows and no scrollbar
    /// appears. Give the panel a height and the body can scroll inside it.
    ///
    /// `Length::Auto` on an axis means "as before" (hug the content). A
    /// [`Pct`](Length::Pct) resolves against the **viewport**, because the
    /// `Overlay` itself fills it — so `Pct(0.8)` is 80% of the viewport, not 80%
    /// of anything the caller laid out.
    ///
    /// Call order does not matter: this is stored on the overlay and re-applied
    /// whenever the panel is (re)set by [`panel`](Overlay::panel) /
    /// [`panel_boxed`](Overlay::panel_boxed).
    ///
    /// ```ignore
    /// // A modal that is 60% of the viewport wide and 70% tall, whose body scrolls.
    /// Overlay::new()
    ///     .panel_size(Length::Pct(0.6), Length::Pct(0.7))
    ///     .panel(Flex::column().child(ScrollRegion::new().child(long_content)))
    /// ```
    #[heca_grid_ui_macros::host_only("takes more than one value, which a single property cannot carry")]
    pub fn panel_size(mut self, width: Length, height: Length) -> Self {
        self.panel_size = Some((width, height));
        self.apply_panel_size();
        self
    }

    /// Push the configured panel size onto the panel child's style, and **always**
    /// cap the panel at the viewport.
    ///
    /// The cap is unconditional, not part of `panel_size`: the `Overlay` fills the
    /// viewport, so `Pct(1.0)` here *is* the window. Without it a fixed `Px` panel
    /// (or a big content-sized one) draws larger than the window and gets cut off
    /// by the screen edge on both sides — a dialog must never be bigger than the
    /// thing it is centered in.
    fn apply_panel_size(&mut self) {
        let Some(panel) = self.base.children.first_mut() else {
            return;
        };
        let style = &mut panel.base_mut().style.layout;
        style.max_width = Some(Length::Pct(1.0));
        style.max_height = Some(Length::Pct(1.0));
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

    /// The **initial** state — a surface *born* open is already there and plays no arrival.
    /// The verbs are [`open`](Overlay::open) / [`hide`](Overlay::hide) / [`toggle`](Overlay::toggle).
    ///
    ///
    /// Order-independent: `.opened(true).animation(..)` and `.animation(..).opened(true)` are the
    /// same surface.
    #[heca_grid_ui_macros::prop]
    pub fn opened(mut self, open: bool) -> Self {
        self.open.set(open);
        self.presence.assume_open(open);
        self
    }

    /// **Put it on screen.** If an [`animation`](Overlay::animation) was declared it plays;
    /// if not, it is simply up. Already up, or still on its way out, and nothing happens.
    pub fn open(&mut self) {
        if self.presence.enter() {
            self.open.set(true);
        }
    }

    /// **Dismiss it.** With an animation this begins the exit and the surface stays on screen,
    /// inert, until the gesture has played out; with none, it is gone now.
    pub fn hide(&mut self) {
        self.presence.leave();
        self.open.set(false);
    }

    /// Open it if it is closed, dismiss it if it is open.
    pub fn toggle(&mut self) {
        match self.presence.is_open() {
            true => self.hide(),
            false => self.open(),
        }
    }

    /// The open-state signal — the host (or composing widget) binds this.
    pub fn open_signal(&self) -> Signal<bool> {
        self.open
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
        self.presence.set_animation(animation.build());
        self
    }

    /// Called when a press lands **outside** the panel (standalone use; a
    /// composing widget usually intercepts the press and applies its own
    /// dismissal policy instead).
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
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

    fn is_open(&self) -> bool {
        self.open.get_untracked()
    }

    /// **Is there still something to draw?** Open, or dismissed and still playing its exit.
    ///
    /// The distinction that makes an animated dismissal possible at all: input follows
    /// [`is_open`](Self::is_open) — a dissolving surface holds nothing, or the map would refuse
    /// every act on the pane it exists to let you choose — while *painting* follows this.
    fn is_present(&self) -> bool {
        self.is_open() || self.presence.is_leaving()
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
    fn presence(&self) -> Option<&Presence> {
        Some(&self.presence)
    }

    fn presence_mut(&mut self) -> Option<&mut Presence> {
        Some(&mut self.presence)
    }

    fn open(&mut self) {
        Overlay::open(self);
    }

    fn hide(&mut self) {
        Overlay::hide(self);
    }

    /// Advance the arrival or exit, then the subtree.
    ///
    /// The sync is here as well as in [`set_open`](Component::set_open) because a **composing**
    /// widget drives the same surface through [`open_signal`](Overlay::open_signal) — one flip of
    /// that signal is an arrival or a dismissal exactly as a host's call is, and neither may be the
    /// only way an animation starts.
    fn tick(&mut self, dt: f32) -> bool {
        self.presence.follow(self.is_open());
        let mut animating = self.presence.tick(dt);
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
        let (background, scrim_a) = {
            let t = cx.theme();
            (t.colors.background, t.colors.interaction.scrim)
        };
        let panel = self.panel_bounds();

        let frame = self.presence.frame();
        frame.apply(cx, self.scale_origin(), |cx| cx.with_overlay(|cx| {
            // Scrim over the whole viewport — the visual half of the blocking
            // layer policy (the event half swallows outside input below).
            if self.blocking {
                let vp = self.viewport.get();
                let scrim = if vp.w.is_finite() {
                    Rectangle::new(Point::new(0.0, 0.0), vp)
                } else {
                    panel
                };
                cx.rect(scrim, background.with_alpha(scrim_a), None, 0.0, None);
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
        }));
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
