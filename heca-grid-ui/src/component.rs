//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::drag::{DragItemId, DropSide};
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{
    Border, BracketCmd, DrawCommand, FontRole, Glow, RectCmd, ScanlineCmd, Scene, Shadow, TextAlign,
    TextCmd, TextStyle,
};
use crate::style::Style;
use crate::theme::Theme;
use heca_core::layout::{Point, Rectangle, Size};
use std::cell::{Cell, RefCell};

thread_local! {
    /// Host-installed hook the widget tree calls to schedule the next frame. The
    /// event loop wires it to its redraw request; widgets reach it via
    /// [`request_frame`]. Thread-local because the UI runs single-threaded.
    static FRAME_REQUEST: RefCell<Option<Box<dyn Fn()>>> = const { RefCell::new(None) };
}

/// Install the callback the widget tree uses to ask the host for the next frame.
///
/// The host (event loop) wires this to its "schedule a redraw" call once at
/// startup. It is the bridge that lets a widget which invalidates itself (an
/// animation step, a caret move, a reactive state change) wake the renderer —
/// the foundation for repainting only what changed instead of every frame.
pub fn install_frame_request(f: impl Fn() + 'static) {
    FRAME_REQUEST.with(|c| *c.borrow_mut() = Some(Box::new(f)));
}

/// Ask the host to schedule a frame. The host coalesces repeated requests into a
/// single redraw. A no-op until [`install_frame_request`] is set (e.g. in
/// headless tests), so widget code can always call it safely.
pub fn request_frame() {
    FRAME_REQUEST.with(|c| {
        if let Some(f) = c.borrow().as_ref() {
            f();
        }
    });
}

/// Logical-pixel margin added around each damaged widget so glow/shadow halos —
/// which paint outside the widget's rect — are included in the redrawn region.
const DAMAGE_PAD: f64 = 64.0;

/// Walk the tree and union the bounds of every widget flagged
/// [`needs_paint`](Base::needs_paint) (padded for glow/shadow reach), **clearing
/// the flags**. Returns the damage rect to repaint, or `None` if nothing changed.
///
/// The host calls this each frame: on an animation/timed frame the result scissors
/// the render to just the changed pixels; on an input frame the host repaints in
/// full (an event can change unknown things) but still calls this to clear flags.
/// Hidden subtrees are skipped — their bounds are stale.
pub fn collect_damage(root: &dyn Component) -> Option<Rectangle> {
    fn union(a: Rectangle, b: Rectangle) -> Rectangle {
        let x0 = a.loc.x.min(b.loc.x);
        let y0 = a.loc.y.min(b.loc.y);
        let x1 = (a.loc.x + a.size.w).max(b.loc.x + b.size.w);
        let y1 = (a.loc.y + a.size.h).max(b.loc.y + b.size.h);
        Rectangle::new(Point::new(x0, y0), Size::new(x1 - x0, y1 - y0))
    }
    fn walk(c: &dyn Component, acc: &mut Option<Rectangle>) {
        let b = c.base();
        if !b.visible.get_untracked() || b.style.layout.hidden {
            return;
        }
        if b.needs_paint() {
            b.clear_needs_paint();
            // Overlay widgets (tooltip bubble, command palette) paint outside their
            // own `bounds`; `damage_bounds` lets them report the rect that actually
            // changed. Default is `bounds`, so ordinary widgets are unaffected.
            let r = c.damage_bounds();
            let padded = Rectangle::new(
                Point::new(r.loc.x - DAMAGE_PAD, r.loc.y - DAMAGE_PAD),
                Size::new(r.size.w + 2.0 * DAMAGE_PAD, r.size.h + 2.0 * DAMAGE_PAD),
            );
            *acc = Some(acc.map_or(padded, |a| union(a, padded)));
        }
        for ch in &b.children {
            walk(ch.as_ref(), acc);
        }
    }
    let mut acc = None;
    walk(root, &mut acc);
    acc
}

/// Walk the tree and report whether any widget's **overlay surface occludes**
/// `pos` (see [`Component::overlay_occludes`]). The host's gate for synthesizing
/// a page-level action from raw input (e.g. right-click → context menu): if an
/// overlay above the page owns that point — an open modal's scrim, a palette, a
/// toast card — the action must not fire underneath it. Hidden subtrees are
/// skipped (their bounds are stale).
pub fn overlay_occluded_at(root: &dyn Component, pos: Point) -> bool {
    let b = root.base();
    if !b.visible.get_untracked() || b.style.layout.hidden {
        return false;
    }
    if root.overlay_occludes(pos) {
        return true;
    }
    b.children
        .iter()
        .any(|c| overlay_occluded_at(c.as_ref(), pos))
}

/// State shared by every component. Concrete widgets embed this.
pub struct Base {
    /// Layout + visual style.
    pub style: Style,
    /// This component's node in the layout tree (set during layout).
    pub node: Option<taffy::NodeId>,
    /// Absolute bounds in logical pixels, filled in after layout.
    pub bounds: Rectangle,
    /// Whether this component is rendered.
    pub visible: Signal<bool>,
    /// Whether this component is disabled — dimmed, non-interactive, and skipped
    /// by focus traversal. A common, base-level property every widget inherits.
    pub disabled: Signal<bool>,
    /// Whether this component currently holds keyboard focus.
    pub focused: Signal<bool>,
    /// Whether the focus ring should show — true for keyboard focus, false for
    /// mouse focus (focus-visible behavior).
    pub focus_visible: Signal<bool>,
    /// Whether this component **opts into keyboard focus**. Defaults to `false`;
    /// interactive widgets set it `true` — in their constructor (always-focusable
    /// controls) or when a click/activate callback is wired (conditionally
    /// interactive rows/buttons). The [`Component::focusable`] trait default
    /// combines it with [`disabled`](Self::disabled): a widget is focusable iff
    /// `focusable && !disabled`, so widgets no longer re-implement that boilerplate.
    /// A genuinely dynamic widget (e.g. an overlay focusable only while open) still
    /// overrides [`Component::focusable`] instead of setting this flag.
    pub focusable: bool,
    /// Whether this widget is the **only** focus target in its subtree: focus traversal does not
    /// descend past it, so its children can never be Tab-focused.
    ///
    /// Set by **controls that compose their content from children** — a [`Button`](crate::widgets::Button)
    /// is one click target and one Tab stop no matter what it holds. Without this, a focusable
    /// descendant (say a `Toggle` used as decoration) would take its own Tab stop while being
    /// click-dead, because the control consumes the press in its own `event` and never routes it
    /// to children. That mismatch — focusable but inert — is the bug this prevents.
    ///
    /// Honoured by [`focus`](crate::focus) traversal. Orthogonal to
    /// [`Style::hidden`](crate::style::Style::hidden), which removes a subtree from layout *and*
    /// focus; a barrier keeps the subtree visible and laid out, and only stops focus descent.
    pub focus_barrier: bool,
    /// Whether this widget is **one click target**: the primary (left) press lands on *it*, not on
    /// the content it composes, and the click that press turns into is its own.
    ///
    /// The pointer twin of [`focus_barrier`](Self::focus_barrier), and set by the same widgets for
    /// the same reason — a control is one thing to click and one thing to Tab to, whatever it
    /// holds. The router applies it during capture, so a control declares it instead of writing
    /// the claim out: nine widgets had written `PointerDown => Handled::Yes` by hand, and every
    /// one of them claimed **any** button, which is how a right-click died on a sidebar row
    /// instead of reaching the menu behind it.
    ///
    /// Only the primary button, and only while enabled. Other buttons pass straight through to
    /// whatever wants them.
    pub one_click_target: bool,
    /// Explicit Tab-order index (like HTML `tabindex`). Focusables with an index
    /// are visited first in ascending order; those without (`None`) follow in
    /// tree position order. Set via [`LayoutExt::tab_index`](crate::builders::LayoutExt::tab_index).
    pub tab_index: Option<i32>,
    /// Child components, laid out by this component's flex container.
    pub children: Vec<Box<dyn Component>>,
    /// If set, this widget is a **drag source**: a press inside its bounds can
    /// begin a drag carrying this opaque id (the app maps it back to a pane /
    /// column / etc.). Universal opt-in via [`ComponentExt::draggable`](crate::builders::ComponentExt::draggable);
    /// resolved generically by [`drag::source_at`](crate::drag::source_at).
    pub drag_source: Option<DragItemId>,
    /// When set, this widget is a **drop target**: a drag released over its bounds
    /// drops onto this opaque id. Universal opt-in via
    /// [`ComponentExt::drop_target`](crate::builders::ComponentExt::drop_target); resolved
    /// generically by [`drag::resolve_at`](crate::drag::resolve_at).
    pub drop_target: Option<DragItemId>,
    /// When set, this widget is a **navigable row** carrying its own identity: the keyboard cursor,
    /// the right-click target and (later) drag are three readers of this one declaration.
    ///
    /// Unlike [`drag_source`](Self::drag_source) — a registry slot the app hands out — this is a
    /// string the component chose about *itself*, so it
    /// survives a tree rebuild. Universal opt-in via [`ComponentExt::nav_key`](crate::builders::ComponentExt::nav_key);
    /// enumerated by [`nav::collect_nav_keys`](crate::nav::collect_nav_keys) and hit-tested by
    /// [`nav::nav_key_at`](crate::nav::nav_key_at). Opaque here — nothing in this library parses it.
    pub nav_key: Option<String>,
    /// **Which enclosing region this subtree belongs to** — a panel, a dock, a tab group, whatever
    /// the host calls the thing that holds rows. Universal opt-in via
    /// [`ComponentExt::scope_key`](crate::builders::ComponentExt::scope_key); hit-tested by
    /// [`nav::scope_at`](crate::nav::scope_at). Opaque here, exactly like `nav_key`.
    ///
    /// A *separate* field rather than a flavour of `nav_key` because the two answer different
    /// questions about the same point: `nav_key` says which **row**, this says which **region
    /// containing rows**, and a host commonly wants both from one press. Folding them together would
    /// also make a region turn up in `collect_nav_keys` as a steppable row, which it is not.
    pub scope_key: Option<String>,
    /// Resolved font size in logical px, written by the layout pass: the widget's
    /// own `style.font_size` if it set one (> 0), otherwise the theme's base font.
    /// Widgets read **this** for text + size, so a global font flows in for free.
    pub font: f32,
    /// The **viewport the tree was laid out against**, written by the layout pass.
    ///
    /// A widget that draws a floating panel has to clamp it on screen, and it used to learn the
    /// viewport from `PaintCx` — one pass *after* the layout that placed the panel. So the first
    /// frame placed it against a stale size and the next one corrected it, and a context menu
    /// visibly jumped after it appeared (Antonio, 2026-08-10). The engine already knows the size it
    /// was told to compute against; this is that size, available at the moment placement happens.
    pub viewport: Size,
    /// Hover, capture, click-run and drag state, kept by the pointer router (see
    /// [`crate::pointer`]). A widget reads `pointer.hovered`; nothing else here writes it.
    ///
    /// It lives on the widget rather than in a router the host owns, so several mounted trees
    /// never share a hover or a capture, and a widget dropped mid-gesture takes its own state with
    /// it instead of stranding a press somewhere.
    pub pointer: crate::pointer::PointerState,
    /// This widget's registered event handlers, keyed by [`EventKind`] — what
    /// [`ComponentExt`](crate::builders::ComponentExt) writes and [`dispatch`] runs. `None` until the
    /// first one is registered, which is the common case and costs a null pointer.
    pub handlers: Option<Box<Handlers>>,
    /// **The context menu this widget carries**, built fresh each time it is triggered.
    ///
    /// A universal slot like [`nav_key`](Self::nav_key) and [`drag_source`](Self::drag_source), so
    /// an `Icon`, a `Label` and a plugin's own widget carry one on the same terms as a `Row`. A
    /// right-click, or the host's `open_context_menu` action, walks **outwards** to the nearest
    /// widget that has one — see [`crate::menu`] for the whole model. Written with
    /// [`ComponentExt::context_menu`](crate::builders::ComponentExt::context_menu).
    ///
    /// Produces the [`ContextMenu`](crate::widgets::ContextMenu) to show. A factory, so a menu
    /// whose rows depend on state the tree is not rebuilt on is current when it opens, and so a
    /// composed row's subtree can be built again on the second right-click. A plain value is
    /// wrapped in one — see
    /// [`IntoContextMenu`](crate::builders::IntoContextMenu).
    ///
    /// The **menu inside it is content**: the same [`Menu`](crate::widgets::Menu) value could be
    /// shown by a menu bar instead. What makes it a *context* menu is being here — attached to a
    /// widget, opened by a right-click or the keyboard action.
    pub context_menu: Option<Box<dyn Fn() -> crate::widgets::ContextMenu>>,
    /// **What a leader-key pick does to this region** — written with
    /// [`KeyHint::on_peek`](crate::widgets::KeyHint::on_peek), the wrapper that draws the letter.
    ///
    /// The whole of the capability: a wrapped region carrying one is offered a letter by the
    /// picker, and picking that letter runs it. There is no id to register, no registry to reach,
    /// and no host type in the closure.
    ///
    /// The slot lives here so the collector stays one uniform walk, but **the builder is on the
    /// wrapper, not on every widget**: being pickable is something you opt a region into, so
    /// `Label::on_peek` is a method that never has to exist.
    pub peek: Option<Box<dyn Fn()>>,
    /// Whether [`Event::Mount`] has been delivered. Set by the first layout pass that sees this
    /// widget — the first moment it is both in a live tree and laid out.
    pub(crate) mounted: Cell<bool>,
    /// Repaint flag for the retained renderer: set when this widget's visuals
    /// changed and cleared once it's repainted. Starts `true` (everything paints
    /// on the first frame). The renderer repaints only widgets whose flag is set,
    /// and unions their bounds into the frame's damage region.
    needs_paint: Cell<bool>}

impl Base {
    /// A new base with default style and an empty child list.
    pub fn new() -> Self {
        Self {
            style: Style::default(),
            node: None,
            bounds: Rectangle::from_size(Size::new(0.0, 0.0)),
            visible: signal(true),
            disabled: signal(false),
            focused: signal(false),
            focus_visible: signal(false),
            focusable: false,
            focus_barrier: false,
            one_click_target: false,
            tab_index: None,
            children: Vec::new(),
            drag_source: None,
            drop_target: None,
            nav_key: None,
            scope_key: None,
            font: 15.0,
            viewport: Size::new(f64::MAX, f64::MAX),
            pointer: crate::pointer::PointerState::new(),
            handlers: None,
            context_menu: None,
            peek: None,
            mounted: Cell::new(false),
            needs_paint: Cell::new(true),
        }
    }

    /// Whether the pointer is over this widget **or a descendant** — the CSS `:hover` rule.
    ///
    /// Read this instead of testing `bounds.contains(pos)`: the router already resolved which
    /// widget the pointer is over, including which one is on top and what is clipped away, and a
    /// private copy of the test cannot know either.
    pub fn hovered(&self) -> bool {
        self.pointer.is_hovered()
    }

    /// Mark this widget as needing a repaint and ask the host for a frame. Call on
    /// a visual change the host wouldn't otherwise know about — an animation step,
    /// a caret move, an imperative state edit. (Reactive state changes route here
    /// too, at their mutation site.)
    pub fn mark_needs_paint(&self) {
        self.needs_paint.set(true);
        request_frame();
    }

    /// Whether this widget needs repainting.
    pub fn needs_paint(&self) -> bool {
        self.needs_paint.get()
    }

    /// Clear the repaint flag — the renderer calls this once the widget is painted.
    pub fn clear_needs_paint(&self) {
        self.needs_paint.set(false);
    }

    /// The widget's size-variant **padding/dimension** multiplier — what widgets
    /// multiply their intrinsic padding / fixed dims by in `remeasure`. (The font is
    /// scaled separately, by [`WidgetSize::font_scale`], during layout.) At `Small`
    /// this is tighter than the font so controls get compact, not just smaller. See
    /// [`WidgetSize`](crate::style::WidgetSize).
    pub fn size_scale(&self) -> f32 {
        self.style.layout.size.pad_scale()
    }

    /// Whether the keyboard **focus ring** should draw: focused AND the focus is
    /// keyboard-driven ([`focus_visible`](Self::focus_visible)) — CSS
    /// `:focus-visible` semantics. A mouse click focuses a widget (so Enter/Space
    /// work, the caret shows, …) but sets `focus_visible = false`, so pointer
    /// users are not ringed; Tab/arrow navigation sets it `true` and the ring
    /// appears. Every widget gates its ring paint on this single definition
    /// (plus the theme's `show_focus_border` and its own disabled check).
    pub fn shows_focus_ring(&self) -> bool {
        self.focused.get_untracked() && self.focus_visible.get_untracked()
    }

    /// The rect an **inset state highlight** should cover — a hover tint, an active pill:
    /// [`bounds`](Self::bounds) pulled in by `inset` on each side, but **never further than this
    /// widget's own padding** on that side.
    ///
    /// The inset exists so a pill's rounded corners never contend with a rounded container's, and
    /// the space padding already reserves is enough for that. Past the padding it would be cutting
    /// into the content box, which is a highlight crossing the very thing it highlights: a row with
    /// no padding used to draw a pill *shorter than its own content*, leaving a badge sticking out
    /// above and below it.
    ///
    /// Every widget that draws an inset highlight asks here, so the rule holds for the next one too.
    pub fn highlight_rect(&self, inset: f64) -> Rectangle {
        let b = self.bounds;
        let l = &self.style.layout;
        let (left, right) = (
            inset.min(l.pad_left() as f64),
            inset.min(l.pad_right() as f64),
        );
        let (top, bottom) = (
            inset.min(l.pad_top() as f64),
            inset.min(l.pad_bottom() as f64),
        );
        Rectangle::new(
            Point::new(b.loc.x + left, b.loc.y + top),
            Size::new(
                (b.size.w - left - right).max(0.0),
                (b.size.h - top - bottom).max(0.0),
            ),
        )
    }
}

impl Default for Base {
    fn default() -> Self {
        Self::new()
    }
}

/// [`Event::Unmount`] fires from here, because this is the only moment the library can be sure a
/// widget is leaving: trees are values, and a rebuilt one drops the old one wholesale.
///
/// Only [`Base`] handlers see it — the widget around this base is already coming apart, so there
/// is nothing left to call `on_event` on. That is enough for what unmount is for: releasing what
/// the widget registered with the host (a drag id, a hint slot) at the moment the tree that
/// registered it goes away, instead of a range the host has to remember to prune.
impl Drop for Base {
    fn drop(&mut self) {
        if let Some(h) = self.handlers.as_mut()
            && h.has(EventKind::Unmount)
        {
            let _ = h.run(&Event::Unmount);
        }
    }
}

/// The event vocabulary lives in [`crate::event`]; it is re-exported here because every widget
/// already imports its events from `component`, and moving a type is not a reason to touch fifty
/// files.
pub use crate::event::{
    DragEvent, Event, EventCx, EventKind, GridKey, Handled, Handlers, Modifiers, PointerButton,
    PointerEvent, RawPointer, RawPointerKind, WidgetIntent,
};

/// Behavior shared by all components. Implementors provide access to their
/// [`Base`]; `paint`/`event` have sensible container defaults.
pub trait Component {
    /// Borrow this component's base.
    fn base(&self) -> &Base;
    /// Mutably borrow this component's base.
    fn base_mut(&mut self) -> &mut Base;

    /// Whether this component participates in keyboard focus traversal
    /// (Tab/Shift+Tab). The default reads the declared [`Base::focusable`] flag and
    /// excludes disabled widgets — so an interactive widget just sets
    /// `base.focusable = true` (in its constructor, or when a callback is wired)
    /// rather than re-implementing this. Override only for genuinely dynamic
    /// focusability (e.g. an overlay focusable only while open).
    fn focusable(&self) -> bool {
        self.base().focusable && !self.base().disabled.get_untracked()
    }

    /// A single-letter **accelerator** for this component, if any (e.g. a confirm button's
    /// `y` / `n`). The widget renders it (`Label (x)`); a host that owns the keypress activates
    /// the matching component — a confirm [`Dialog`](crate::widgets::Dialog) fires the button
    /// whose `shortcut()` matches a typed letter, but only when it is a confirm (no text field to
    /// steal the key). Default `None`.
    fn shortcut(&self) -> Option<char> {
        None
    }

    /// Whether this component currently has an **open overlay** (e.g. a `Select`
    /// dropdown). The host routes pointer/key events to an overlay-active widget
    /// first, so it can capture clicks/keys outside its layout bounds. Default
    /// `false`; see [`FocusManager`](crate::focus::FocusManager).
    fn overlay_active(&self) -> bool {
        false
    }

    /// Whether this component's **overlay surface geometrically occludes** `pos`
    /// (logical px). A host asks this before synthesizing a page-level action from
    /// a raw input — e.g. right-click → "open the context menu": if the point is
    /// covered by an overlay drawn above the page, the action must not fire
    /// underneath it. Distinct from [`overlay_active`](Self::overlay_active)
    /// (input **grab**): a non-grabbing overlay like a toast card still occludes
    /// the points it covers, while a **modal** overlay (an open `Dialog` scrim)
    /// occludes the whole viewport. A widget whose open overlay deliberately
    /// yields to a fresh trigger (a `ContextMenu`, where a second right-click
    /// re-anchors) keeps the default. Scan a tree with
    /// [`overlay_occluded_at`]. Default `false` (plain widgets never occlude).
    fn overlay_occludes(&self, pos: Point) -> bool {
        let _ = pos;
        false
    }

    /// The **plain-text form of this component's content** — its accessible name.
    ///
    /// A control whose content is *composed* cannot read that content's text: children are
    /// `impl Component`, so their types are erased. But a container sometimes needs that text — to
    /// **report** its value as a string, the way
    /// [`Select::selected_label`](crate::widgets::Select::selected_label) reports the chosen option.
    /// This is the one value it can pull out of an otherwise opaque subtree.
    ///
    /// The default computes the name **from the contents**, like the web's accessible-name
    /// algorithm: the first child that has one wins. So a
    /// [`Choice`](crate::widgets::Choice) composed of an `Icon` + `Label("HIGH")` summarizes to
    /// `"HIGH"` with no wiring — the `Icon` has no text, the `Label` does.
    /// [`Label`](crate::widgets::Label) is the leaf that supplies it; widgets that render text
    /// they own (not via a child) should override this too.
    fn text_summary(&self) -> Option<String> {
        self.base().children.iter().find_map(|c| c.text_summary())
    }

    /// Emit draw commands. Default: paint the base chrome, then children.
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base().visible.get_untracked() {
            return;
        }
        cx.paint_base(self.base());
        for child in &self.base().children {
            paint_child(child.as_ref(), cx);
        }
    }

    /// Handle an event **before** this widget's children see it. Default: ignore it.
    ///
    /// This is where a widget takes something away from its subtree: a modal swallowing input
    /// aimed at the page behind it, a control that owns a whole gesture (a scrollbar thumb grab
    /// beats whatever is under the cursor), or state that must be current before anything below is
    /// hit-tested. Returning [`Handled::Yes`] stops the walk — the children never see it.
    ///
    /// Use it only for those. Anything a widget does with an event its children *declined* belongs
    /// in [`on_event`](Component::on_event).
    fn on_event_capture(&mut self, _ev: &Event) -> Handled {
        Handled::No
    }

    /// Handle an event this widget's children did not take. Default: ignore it.
    ///
    /// **A widget never routes to its children.** [`dispatch`] does that, always, for every event
    /// — which is the point: forwarding is not a line anyone writes, so it is not a line anyone can
    /// forget. A container that only cared about presses used to be free to handle the press and
    /// return, quietly stranding every child that needed the wheel or the release. That is how a
    /// scroll region ends up mounted, painted, and dead.
    fn on_event(&mut self, _ev: &Event) -> Handled {
        Handled::No
    }

    /// Called on this widget **after its subtree has seen `ev`**, whether or not something in
    /// there consumed it, with `handled` reporting which.
    ///
    /// The hook for a container that **watches what its own subtree did** rather than acting on
    /// the event itself: an [`ItemGroup`](crate::widgets::ItemGroup) reports a toggle when the
    /// header row inside it flips `expanded`, and the header consuming the click is the normal
    /// case, not the exception. [`on_event`](Self::on_event) cannot express that — it runs only
    /// when nothing below wanted the event — and taking over the walk to get it (which is what
    /// these containers used to do) means every event kind now depends on that container
    /// forwarding it correctly forever.
    ///
    /// Observation only: it returns nothing, so it can neither consume the event nor let a
    /// consumed one continue.
    fn after_subtree(&mut self, ev: &Event, handled: Handled) {
        let _ = (ev, handled);
    }

    /// `true` when this widget wants an enclosing scroll region to **keep it in view**.
    ///
    /// The default is keyboard focus, which is what browsers do: focus something off-screen and the
    /// view comes to it. A widget that is "current" by some other measure — a list's navigation
    /// cursor, a search hit — overrides this to say so, and then every
    /// [`ScrollRegion`](crate::widgets::ScrollRegion) it is ever placed inside follows it, with
    /// nothing wired at the call site.
    /// **Run this widget's primary action** — what a click or Enter on it would do — and report
    /// whether it had one. `false` by default: most widgets do nothing on their own.
    ///
    /// The equivalent of `button.click()` in the DOM, and it exists because there was no way to say
    /// "activate this child" through `dyn Component`. Without it a caller has only one lever: build
    /// a fake `Event::Key { Enter }` and dispatch it, hoping the target claims raw keys — which is
    /// what [`Dialog::submit`](crate::widgets::Dialog) did to fire its primary button. That worked
    /// only because seven widgets claimed `Enter` without checking whether they owned the keyboard,
    /// and it broke the moment they stopped. Synthesising input to reach behaviour is a symptom of
    /// a missing call, not a technique.
    fn activate(&mut self) -> bool {
        false
    }

    fn wants_visible(&self) -> bool {
        self.base().focused.get_untracked()
    }

    /// The rect this widget occupies **for input**, or `None` when it takes none at all.
    ///
    /// The default is [`bounds`](Base::bounds), which is right for every widget drawn where it is
    /// laid out. Two kinds are not: one that paints a **floating panel** (a menu, a palette) reports
    /// the panel, and one that is **inert right now** (a closed [`Overlay`](crate::widgets::Overlay))
    /// reports `None`, which takes its whole subtree out of the pointer's reach while leaving it
    /// laid out.
    fn hit_bounds(&self) -> Option<Rectangle> {
        Some(self.base().bounds)
    }

    /// The rect (logical px) to repaint when this widget is flagged
    /// [`needs_paint`](Base::needs_paint) — used by [`collect_damage`] in place of `bounds`.
    /// Overlay widgets that paint **outside** their own bounds (a tooltip bubble, a command-palette
    /// panel) override it so a redraw covers what they actually drew.
    fn damage_bounds(&self) -> Rectangle {
        self.base().bounds
    }

    /// `true` when this widget **clips** its children to its own bounds, so a press or a move
    /// outside those bounds must not reach them.
    ///
    /// Without it, content scrolled out of a [`ScrollRegion`](crate::widgets::ScrollRegion) stays
    /// clickable where it *would* have been: its bounds are real, it simply isn't drawn. Paint
    /// already honours the clip; this is the same rule for input, in the one place that walks the
    /// tree rather than in each clipping widget's own gate.
    fn clips_children(&self) -> bool {
        false
    }

    /// Called when this component gains keyboard focus. Default: set the focus
    /// flag (which drives the focus ring). Override to add behaviour.
    /// `visible` is true for keyboard focus (show the ring), false for mouse.
    fn on_focus(&mut self, visible: bool) {
        self.base_mut().focused.set(true);
        self.base_mut().focus_visible.set(visible);
    }

    /// Called when this component loses keyboard focus. Default: clear it.
    fn on_blur(&mut self) {
        self.base_mut().focused.set(false);
        self.base_mut().focus_visible.set(false);
    }

    /// The taffy style for this component's layout node. Default: the base
    /// style mapped via [`Style::to_taffy`]. Layout containers that need extra
    /// taffy config (e.g. [`Grid`](crate::widgets::Grid) injecting `display:
    /// grid` + track templates) override this.
    fn taffy_style(&self) -> taffy::Style {
        self.base().style.layout.to_taffy()
    }

    /// Recompute size from the resolved font ([`Base::font`]). Widgets whose
    /// dimensions depend on font size override this; the layout pass calls it on
    /// every node after resolving the font, so a global font reflows the tree
    /// without per-widget wiring. Default: no-op.
    fn remeasure(&mut self) {}

    /// Text whose **height depends on the width the engine resolves** — return `Some` and the
    /// layout pass hands this node to taffy's measure path instead of a fixed size.
    ///
    /// [`remeasure`](Self::remeasure) cannot express this: it runs *before* layout, so it has no
    /// width to wrap into and can only report a size it already knows. A wrapping
    /// [`Label`](crate::widgets::Label) is the one widget that needs the difference; everything
    /// else measures itself and returns `None` (the default).
    ///
    /// Only consulted for **leaves** — a widget with children is sized by them.
    fn measure_text(&self) -> Option<crate::layout::TextMeasure> {
        None
    }

    /// Called by the layout engine **after** this node's bounds (and all its
    /// descendants' bounds) have been (re)computed and written to `Base::bounds`.
    /// Post-order: children fire before the parent. Default: no-op. Override to
    /// react to a fresh layout — e.g. a scroll viewport resets any shift it had
    /// baked into its children's bounds (they are now back at their natural
    /// positions), so the next paint re-applies the shift from scratch instead
    /// of compounding. This is what lets an embeddable scroll viewport reuse the
    /// whole-page scroll pattern (shift subtree bounds + clip) without owning the
    /// layout/scroll cycle.
    fn on_layout(&mut self) {}

    /// Advance time-based animations by `dt` seconds. Returns `true` if a
    /// **continuous** animation is still running (eases, slides, spinners), so the
    /// host schedules another frame at its frame cap. Default: recurse.
    ///
    /// A widget whose visual changes only at sparse, known moments (e.g. a blinking
    /// caret toggling ~twice a second) should return `false` here and instead report
    /// the time to its next change via [`next_redraw`](Self::next_redraw), so the
    /// host can sleep until then rather than redrawing every frame.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base_mut().children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }

    /// Seconds until this subtree next needs a **timed** redraw (independent of the
    /// continuous-animation signal from [`tick`](Self::tick)) — e.g. a focused
    /// `Input`'s caret returns the time to its next blink toggle. The host wakes at
    /// the soonest such time across the tree instead of redrawing continuously.
    /// `None` = no timed redraw pending. Default: the soonest across children.
    fn next_redraw(&self) -> Option<f32> {
        let mut soonest = None;
        for child in self.base().children.iter() {
            soonest = soonest_redraw(soonest, child.next_redraw());
        }
        soonest
    }

    /// The opaque drag-source id if this widget is draggable (see
    /// [`Base::drag_source`]). Default reads the base; widgets needing dynamic
    /// behavior may override. Walked by [`drag::source_at`](crate::drag::source_at).
    fn as_drag_source(&self) -> Option<DragItemId> {
        self.base().drag_source
    }

    /// The opaque drop-target id if this widget accepts drops (see
    /// [`Base::drop_target`]). Default reads the base. Walked by
    /// [`drag::resolve_at`](crate::drag::resolve_at).
    fn as_drop_target(&self) -> Option<DragItemId> {
        self.base().drop_target
    }
}

/// Combine two "seconds until next redraw" requests, keeping the sooner one
/// (`None` means "no request").
pub fn soonest_redraw(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

/// Deliver `ev` to `node` and its subtree. **The only child walk in this library.**
///
/// Three steps, and a widget takes part by implementing at most two of them:
///
/// 1. [`on_event_capture`](Component::on_event_capture) — the widget's chance to take the event
///    away from its own subtree. `Yes` stops here.
/// 2. **the children**, last-added first so the top of the z-order wins. No widget writes this.
/// 3. [`on_event`](Component::on_event) — what the widget does with what nobody below wanted.
///
/// Step 2 is the point. It used to live inside each container's `event`, which meant every
/// container decided, one `match` arm at a time, which kinds of event its children were allowed to
/// see — and a container that only cared about presses silently stranded any child that needed the
/// wheel or a release. That is not a mistake anyone makes on purpose; it is what a hand-written
/// list does over time. Now there is no list.
pub fn dispatch(node: &mut dyn Component, ev: &Event) -> Handled {
    // The raw pointer stream is not delivered: it is resolved, once, and what it *means* is
    // delivered instead — to the widget under the pointer and then up its ancestors.
    if let Event::Raw(raw) = ev {
        return crate::pointer::route(node, raw);
    }
    deliver(node, ev)
}

/// Deliver an event that carries no position: a key, typed text, a semantic intent, a modifier
/// change, a mount.
///
/// **Keyboard events go to the focused widget and bubble**, exactly as they do in a browser: the
/// walk goes down the ancestor chain of whatever holds focus (each ancestor may take it away in
/// capture), reaches the focus owner, and comes back up through the same chain. Nothing is
/// declared, nothing is forwarded, and a widget that does not hold focus — or contain the thing
/// that does — is not offered the key at all.
///
/// That last clause used to be three separate flags. A container said `routes_own_subtree` to take
/// over the walk, `takes_raw_keys` to be offered keys without focus, and `takes_text_input` to be
/// offered typed text; each was a question about the widget asked so the framework could route,
/// and each had to be answered right by every author. Focus already says all three: a
/// [`FocusScope`](crate::widgets::FocusScope) that does not hold the keyboard is simply not on the
/// path, so it neither hears the key nor has to decline it.
///
/// Everything else still broadcasts — a modifier change is an announcement to the whole tree, and
/// a resolved pointer event a caller delivers by hand carries its own target.
pub fn deliver(node: &mut dyn Component, ev: &Event) -> Handled {
    if is_keyboard(ev) {
        // **Nothing focused, nothing delivered.** A keyboard event with no owner belongs to no
        // widget, and handing it to the tree anyway is the whole family of bugs this replaced: the
        // first row in a list answering an Enter meant for the cursor, an unfocused dock answering
        // an intent aimed at its neighbour. A host that wants a key to reach a surface focuses the
        // surface — which it already does, because that is what the focus ring means.
        let Some(path) = focus_path(node) else {
            return Handled::No;
        };
        return deliver_to_path(node, &path, ev);
    }
    broadcast(node, ev)
}

/// Events that follow keyboard focus rather than the whole tree.
///
/// [`Event::Widget`] is one of them: a semantic intent is what a key **resolved to**, so it belongs
/// to whoever the key would have gone to. Delivering it any wider is how one dock answered for
/// another.
fn is_keyboard(ev: &Event) -> bool {
    matches!(
        ev,
        Event::Key { .. } | Event::TextInput(_) | Event::Widget(_)
    )
}

/// The path from `node` to the **deepest** widget in it holding keyboard focus, or `None` when
/// nothing in this tree does.
///
/// Deepest, because focus nests: a host marks a whole dock as the keyboard's target *and* the field
/// inside it is focused, and the key belongs to the field. Hidden and invisible subtrees are
/// skipped — a closed overlay still holds the focus flag its field had when it closed, and that
/// must not pull the keyboard into something nobody can see.
pub(crate) fn focus_path(node: &dyn Component) -> Option<Vec<usize>> {
    // **Last-added first**, the same order hit-testing uses: what is drawn on top owns the input.
    // Several things can carry the focus flag at once — an open layer says it holds the keyboard,
    // and the button the user clicked before opening it still says so too — and the one on top is
    // the one that means it. Walking in document order picked the button and left every keystroke
    // falling through the palette to the page behind it.
    for (i, child) in node.base().children.iter().enumerate().rev() {
        if !child.base().visible.get_untracked() || child.base().style.layout.hidden {
            continue;
        }
        if let Some(mut sub) = focus_path(child.as_ref()) {
            sub.insert(0, i);
            return Some(sub);
        }
    }
    node.base().focused.get_untracked().then(Vec::new)
}

/// Capture down `path`, then handlers and [`on_event`](Component::on_event) back up — the same
/// target-and-bubble walk the pointer uses, for an event whose target is the focus owner.
///
/// An empty path means `node` **is** the target, and the walk turns around there. **Its children
/// are not visited**, for keys, for typed text and for intents alike: the target is the widget that
/// answers, exactly as it is for the pointer. The difference is only in how each one is found —
/// hit-testing walks to the deepest widget under the cursor, and focus is asserted by the deepest
/// widget that holds it.
///
/// The alternative — letting an intent descend into the target's subtree — was tried and taken out.
/// It made a whole region answer for a capability nobody in it had claimed, so two widgets that both
/// handled one intent was not an error, and reading the tree could not tell you where a key landed.
fn deliver_to_path(node: &mut dyn Component, path: &[usize], ev: &Event) -> Handled {
    if node.on_event_capture(ev) == Handled::Yes {
        return Handled::Yes;
    }
    if let Some((head, rest)) = path.split_first() {
        let from_subtree = match node.base_mut().children.get_mut(*head) {
            Some(child) => deliver_to_path(child.as_mut(), rest, ev),
            None => Handled::No,
        };
        node.after_subtree(ev, from_subtree);
        if from_subtree == Handled::Yes {
            return Handled::Yes;
        }
    } else if matches!(ev, Event::Widget(_)) {
        // **An intent enters the focused region.** It is not a key — it is what a key *resolved to*,
        // a capability named out loud (`ScrollPageDown`, `Dismiss`), so it is addressed to the
        // focused region and whichever widget in there owns that capability answers. A raw key
        // stops at the owner, which is what keeps the first row in a list from eating an Enter
        // meant for the cursor.
        let mut from_subtree = Handled::No;
        for child in node.base_mut().children.iter_mut().rev() {
            if broadcast(child.as_mut(), ev) == Handled::Yes {
                from_subtree = Handled::Yes;
                break;
            }
        }
        node.after_subtree(ev, from_subtree);
        if from_subtree == Handled::Yes {
            return Handled::Yes;
        }
    }
    if node.base_mut().run_handlers(ev) == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// Capture down, children last-first, bubble up — for the events that are addressed to everything
/// rather than to one widget.
fn broadcast(node: &mut dyn Component, ev: &Event) -> Handled {
    if node.on_event_capture(ev) == Handled::Yes {
        return Handled::Yes;
    }
    let mut from_subtree = Handled::No;
    for child in node.base_mut().children.iter_mut().rev() {
        if broadcast(child.as_mut(), ev) == Handled::Yes {
            from_subtree = Handled::Yes;
            break;
        }
    }
    node.after_subtree(ev, from_subtree);
    if from_subtree == Handled::Yes {
        return Handled::Yes;
    }
    if node.base_mut().run_handlers(ev) == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// The bounds of the first descendant (or `node` itself) asking to be kept in view, in tree order.
///
/// Depth-first so the innermost claim wins: a focused row inside a marked group is the thing to
/// reveal, not the group.
pub(crate) fn reveal_target(node: &dyn Component) -> Option<Rectangle> {
    for child in &node.base().children {
        if let Some(found) = reveal_target(child.as_ref()) {
            return Some(found);
        }
    }
    node.wants_visible().then(|| node.base().bounds)
}

/// [`reveal_target`] over a child list — what a container scans, since it never reveals *itself*.
pub(crate) fn reveal_target_in(children: &[Box<dyn Component>]) -> Option<Rectangle> {
    children.iter().find_map(|c| reveal_target(c.as_ref()))
}

/// Paint a child, unless it is hidden via `style.hidden` (taffy `display: none`).
/// A `display: none` subtree is collapsed to zero size at the top-left by layout,
/// so painting it would stamp its (stale, overlapping) contents there — every
/// container skips hidden children instead, matching the web. Containers with
/// bespoke paint loops (e.g. [`Pane`](crate::widgets::Pane),
/// [`DockFrame`](crate::widgets::DockFrame)) reuse this so a collapsed body/group
/// never bleeds onto the rest of the tree.
pub(crate) fn paint_child(c: &dyn Component, cx: &mut PaintCx) {
    if c.base().style.layout.hidden {
        return;
    }
    c.paint(cx);
}

/// Translate a component's whole subtree by `(dx, dy)` — bounds only, no re-layout.
///
/// The primitive behind every widget that **places** children the layout engine could not: layout
/// computes each node's natural rect, and the widget then bakes an offset into it so that
/// **bounds === what is drawn === what is clickable**. A
/// [`ScrollRegion`](crate::widgets::ScrollRegion) bakes `-scroll_offset` this way; a
/// [`Select`](crate::widgets::Select) bakes the offset from the trigger's flow to its overlay
/// panel. Because bounds stay truthful, pointer routing, hit-testing and drag resolution — which
/// all read bounds — keep working with no special cases.
///
/// Shifting is destructive, so the widget must re-derive it after every layout pass (the engine
/// calls [`Component::on_layout`] post-order once bounds are natural again).
pub(crate) fn shift_subtree(c: &mut dyn Component, dx: f64, dy: f64) {
    c.base_mut().bounds.loc.x += dx;
    c.base_mut().bounds.loc.y += dy;
    let n = c.base().children.len();
    for i in 0..n {
        let child = &mut c.base_mut().children[i];
        shift_subtree(child.as_mut(), dx, dy);
    }
}

/// Scrim alpha used to dim a disabled widget — applied by [`PaintCx::dim`].
const DISABLED_SCRIM: f32 = 0.55;

/// Logical-px slack around the viewport kept un-culled, so a shape's glow/shadow
/// halo spilling in from just off-screen still draws. See [`PaintCx::culled`].
const CULL_MARGIN: f64 = 96.0;

/// Bright bracket length along each edge of a [`PaintCx::bracket_frame`],
/// measured from the corner (in addition to the rounded arc). The straight
/// midsection between the two brackets on an edge is dimmed back to a line.
const BRACKET_ARM_LEN: f32 = 12.0;
/// Bright corner brackets are drawn a touch thicker than the subtle border for
/// emphasis — an **additive** boost so they don't balloon at large border widths
/// (a multiplier made them far too heavy at e.g. `border_width = 3`).
const BRACKET_WIDTH_BOOST: f32 = 1.0;
/// Alpha of the subtle continuous accent line that traces the whole perimeter of
/// a [`PaintCx::bracket_frame`] — the dimmed "midsection" the bright corners sit
/// on top of. ~0.31 leaves a faint accent line.
const BRACKET_DIM_ALPHA: u8 = 80;

/// Painting context handed to [`Component::paint`]. Wraps the [`Scene`] and the
/// active [`Theme`], and exposes the shared Tron drawing helpers.
pub struct PaintCx<'a> {
    scene: &'a mut Scene,
    theme: &'a Theme,
    /// Visible viewport size in logical px. Widgets that place overlays (e.g. a
    /// `Select` dropdown) use it to flip/cap against the screen. Defaults to
    /// "infinite" so non-host callers (tests) keep the open-below behavior.
    viewport: Size,
    /// The **inherited content color** for the subtree currently being painted — see
    /// [`with_content_color`](Self::with_content_color). `None` at the root.
    content_color: Option<Color>,
    content_glow: Option<Glow>,
    /// Translation applied to every draw emitted through this context — see
    /// [`with_translate`](Self::with_translate). `(0, 0)` normally: a widget paints at its bounds.
    offset: (f64, f64),
    /// Alpha multiplier applied to every draw emitted through this context — see
    /// [`with_opacity`](Self::with_opacity). `1.0` normally: a widget paints at its own colours.
    opacity: f32,
}

/// Scale every colour in a draw command by `a`, leaving its geometry alone.
///
/// Written out per command rather than as a blanket "multiply anything colour-shaped", because the
/// commands carry colours in several roles — a fill, a border, the light of a glow, the dark of a
/// shadow — and each has to be dimmed on its own or a fading panel loses its outline a frame before
/// its body, or keeps a halo around nothing.
fn fade_command(cmd: DrawCommand, a: f32) -> DrawCommand {
    let dim = |c: Color| c.with_alpha((c.a as f32 * a).round().clamp(0.0, 255.0) as u8);
    match cmd {
        DrawCommand::Rect(r) => DrawCommand::Rect(RectCmd {
            fill: dim(r.fill),
            border: r.border.map(|b| Border { color: dim(b.color), ..b }),
            glow: r.glow.map(|g| Glow { color: dim(g.color), ..g }),
            shadow: r.shadow.map(|s| Shadow { color: dim(s.color), ..s }),
            ..r
        }),
        DrawCommand::Text(t) => DrawCommand::Text(TextCmd {
            color: dim(t.color),
            glow: t.glow.map(|g| Glow { color: dim(g.color), ..g }),
            ..t
        }),
        DrawCommand::Brackets(b) => DrawCommand::Brackets(BracketCmd {
            color: dim(b.color),
            glow: b.glow.map(|g| Glow { color: dim(g.color), ..g }),
            ..b
        }),
        DrawCommand::Scanline(s) => DrawCommand::Scanline(ScanlineCmd {
            color: dim(s.color),
            ..s
        }),
        // Clips carry no colour; they are geometry, and a fade must not move anything.
        clip @ (DrawCommand::PushClip(_) | DrawCommand::PopClip) => clip,
    }
}

impl<'a> PaintCx<'a> {
    /// Create a painting context over `scene` using `theme`.
    pub fn new(scene: &'a mut Scene, theme: &'a Theme) -> Self {
        Self {
            scene,
            theme,
            viewport: Size::new(f64::MAX, f64::MAX),
            content_color: None,
            content_glow: None,
            offset: (0.0, 0.0),
            opacity: 1.0,
        }
    }

    /// Paint `f`'s subtree **translated** by `(dx, dy)` — the same components, drawn somewhere else.
    ///
    /// This is deliberately narrow. A component is laid out in exactly one place, and its bounds are
    /// the contract for hit-testing and drawing alike (`bounds === what is drawn === what is
    /// clickable`). But a control occasionally has to render content it *owns but does not hold* —
    /// a [`Select`](crate::widgets::Select) shows the chosen option in its trigger while that option
    /// is away in the open list. Nothing can be in two places, so the trigger draws a **second
    /// image** of it here.
    ///
    /// What is drawn this way is **not interactive**: it has no bounds of its own, so it is not
    /// hit-tested, focusable or hoverable — the control's own bounds are the click target. Use it
    /// only for such an echo, never to move a widget: shifting where a component *lives* is
    /// [`shift_subtree`] + [`Component::on_layout`], which keeps its bounds honest.
    pub fn with_translate(&mut self, dx: f64, dy: f64, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.offset;
        self.offset = (previous.0 + dx, previous.1 + dy);
        f(self);
        self.offset = previous;
    }

    /// Paint `f`'s subtree at `alpha` (`0.0..=1.0`) — a whole surface fading, not a colour choice.
    ///
    /// Multiplicative and nesting: a half-faded panel holding a half-faded row draws it at a
    /// quarter. Every command this context emits is scaled on its way out, so a widget needs to
    /// know nothing about it — which is the point. A fade is something done **to** a surface, and a
    /// surface that had to cooperate with its own fade would mean every widget carrying an alpha.
    ///
    /// It scales colour only, never geometry: the thing stays exactly where it is and stops being
    /// visible, which is what "fade" means. Nor does it change hit-testing — a surface on its way
    /// out is still there until whoever is fading it takes it away.
    pub fn with_opacity(&mut self, alpha: f32, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.opacity;
        self.opacity = previous * alpha.clamp(0.0, 1.0);
        f(self);
        self.opacity = previous;
    }

    /// Emit a command, scaled by the active [opacity](Self::with_opacity). The one door to the
    /// scene, so a new draw helper cannot forget to honour a fade it never heard of.
    fn emit(&mut self, cmd: DrawCommand) {
        let a = self.opacity;
        self.scene.push(if a >= 1.0 { cmd } else { fade_command(cmd, a) });
    }

    /// Apply the active [translation](Self::with_translate) to a rect on its way to the scene.
    fn placed(&self, r: Rectangle) -> Rectangle {
        if self.offset == (0.0, 0.0) {
            return r;
        }
        Rectangle::new(
            Point::new(r.loc.x + self.offset.0, r.loc.y + self.offset.1),
            r.size,
        )
    }

    /// Set the visible viewport size (the host passes the window size).
    pub fn with_viewport(mut self, viewport: Size) -> Self {
        self.viewport = viewport;
        self
    }

    /// The visible viewport size in logical px.
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// Whether `r` lies fully outside the viewport (plus a halo margin for
    /// glow/shadow spill) and can be skipped. Off-screen content emits no draw
    /// command, so scrolling a tall page or maximizing the window doesn't pay to
    /// paint / shape / upload what isn't visible. The default viewport is
    /// "infinite" (headless), where nothing is ever culled; overlays draw
    /// on-screen, so they're never culled either.
    fn culled(&self, r: Rectangle) -> bool {
        let vp = self.viewport;
        let r = self.placed(r);
        r.loc.y + r.size.h < -CULL_MARGIN
            || r.loc.y > vp.h + CULL_MARGIN
            || r.loc.x + r.size.w < -CULL_MARGIN
            || r.loc.x > vp.w + CULL_MARGIN
    }

    /// Run `f` with draws routed to the scene's **overlay layer** (painted on
    /// top of everything). Used by popovers/dropdowns for correct z-order.
    pub fn with_overlay(&mut self, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.begin_overlay();
        f(self);
        self.scene.end_overlay();
    }

    /// Run `f` with all its draws **clipped** to `rect` (logical px). Content that
    /// falls outside is scissored away by the renderer — the primitive a scrolling
    /// viewport uses so partial rows/glyphs are cut at the panel edge instead of
    /// spilling out. Nested clips intersect with their parent.
    pub fn with_clip(&mut self, rect: Rectangle, f: impl FnOnce(&mut PaintCx<'a>)) {
        self.scene.push(DrawCommand::PushClip(self.placed(rect)));
        f(self);
        self.scene.push(DrawCommand::PopClip);
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        self.theme
    }

    /// Paint `f`'s subtree with `color` as the **inherited content color** — the color that
    /// unstyled text and glyphs ([`Label`](crate::widgets::Label), [`Icon`](crate::widgets::Icon))
    /// use when the caller gave them none. This is `color` inheritance in the CSS sense, and it is
    /// how a control tints the content it *composes* rather than draws.
    ///
    /// A control (e.g. [`Button`](crate::widgets::Button)) cannot set its children's colors
    /// directly: children are `impl Component`, so it doesn't know their types, and the
    /// [`Theme`] its state color derives from is only reachable here, in `paint`. So instead of
    /// pushing color *into* the children, it publishes one value they *pull*:
    ///
    /// ```ignore
    /// // In the control's `paint`, after its own chrome:
    /// let tint = accent.lerp(on_accent, hover_progress);          // state-derived, per frame
    /// cx.with_content_color(tint, |cx| {
    ///     for child in &self.base.children { child.paint(cx); }   // children just paint themselves
    /// });
    /// ```
    ///
    /// The control repaints every frame while its hover/press animation runs, so the value changes
    /// each frame and **the children animate without knowing anything about hover, or the parent,
    /// or animation at all**.
    ///
    /// The fallback is deliberately narrow. It applies only where a widget has no color of its own:
    /// an explicit `.color(..)` always wins, and a widget with an *intrinsic semantic* color
    /// (`Badge::danger`, `StatusDot::online`) ignores it entirely — exactly as a `.badge-danger`
    /// stays red inside a colored parent on the web. Nesting restores the outer value on exit.
    pub fn with_content_color(&mut self, color: Color, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.content_color.replace(color);
        f(self);
        self.content_color = previous;
    }

    /// The inherited content color, if a parent published one via
    /// [`with_content_color`](Self::with_content_color). Widgets that render bare text or glyphs
    /// resolve their color as: **own explicit color → this → a theme token** (usually
    /// `theme.colors.foreground`).
    pub fn content_color(&self) -> Option<Color> {
        self.content_color
    }

    /// Paint the closure's subtree with `glow` as the **inherited content glow** — the
    /// halo counterpart of [`with_content_color`](Self::with_content_color), and it
    /// exists for the same reason.
    ///
    /// A control that *composes* its content holds its children as `impl Component`:
    /// it cannot reach in and style them, and it cannot know whether a child is even
    /// a glyph. [`RailCell`](crate::widgets::RailCell) is the case — its resting look
    /// *is* a bare icon, so the only way to give that icon a halo is for the cell to
    /// publish one and let the glyph pull it.
    ///
    /// The published glow is **unscaled**, like the one [`rest_glow`](Self::rest_glow)
    /// returns: `glow_size` is applied once at the drawing chokepoint
    /// ([`icon_glowing`](Self::icon_glowing), [`rect`](Self::rect)), never by the
    /// publisher. Scaling before publishing would apply the setting twice.
    pub fn with_content_glow(&mut self, glow: Option<Glow>, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = std::mem::replace(&mut self.content_glow, glow);
        f(self);
        self.content_glow = previous;
    }

    /// The inherited content glow, if a parent published one via
    /// [`with_content_glow`](Self::with_content_glow).
    pub fn content_glow(&self) -> Option<Glow> {
        self.content_glow
    }

    /// Queue a rounded rectangle with optional border and glow.
    pub fn rect(
        &mut self,
        rect: Rectangle,
        fill: Color,
        border: Option<Border>,
        radius: f32,
        glow: Option<Glow>,
    ) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Rect(RectCmd {
            rect: self.placed(rect),
            fill,
            border,
            radius,
            glow: self.scaled_glow(glow),
            shadow: None,
        }));
    }

    /// Queue a soft **drop shadow** for `rect` (corner `radius`): a dark, blurred,
    /// offset halo drawn *behind* it that lifts the shape off the background. Call
    /// this **before** painting the shape's fill, so the shape occludes the
    /// shadow's center and only its fringe shows. Unlike [`glow`](PaintCx::rect),
    /// the shadow darkens (composites a dark color with alpha), so it reads on
    /// dark themes — and it's independent of the glow/border tokens, so it shows
    /// even when both are off (e.g. a floating Modal at `border_width == 0`).
    pub fn drop_shadow(&mut self, rect: Rectangle, radius: f32, shadow: Shadow) {
        if shadow.color.a == 0 || shadow.radius <= 0.0 {
            return;
        }
        self.emit(DrawCommand::Rect(RectCmd {
            rect: self.placed(rect),
            fill: Color::TRANSPARENT,
            border: None,
            radius,
            glow: None,
            shadow: Some(shadow),
        }));
    }

    /// Build a box border in `color` at the **theme's** [`border_width`](crate::theme::Theme::border_width),
    /// or `None` when borders are off (`border_width == 0`). Widgets should build
    /// their box border with this instead of hardcoding a stroke, so they all
    /// honor the token (and disappear together at width 0). The single chokepoint
    /// that keeps border width theme-driven across the widget set.
    pub fn border(&self, color: Color) -> Option<Border> {
        let w = self.theme.colors.border_width;
        (w > 0.0).then_some(Border { color, width: w })
    }

    /// Queue the **thin-outline focus indicator**: an accent-toned outline drawn *just outside*
    /// `rect` (a CSS-style `outline` with an offset gap), with a restrained halo. An alternative to
    /// the corner-bracket reticle ([`corner_brackets`](Self::corner_brackets)).
    ///
    /// This is **the** keyboard focus indicator for the whole widget set (Button, IconButton,
    /// Toggle, Checkbox, Input, Select, Tabs, Item, Row, RailCell, Toast, BadgeButton, ScrollRegion,
    /// …). Because it sits **outside** the widget box rather than on its edge, it is visible whether
    /// or not the widget draws its own border — borderless variants (e.g. Ghost/Link buttons) get
    /// the same clear ring — and it never merges into the widget's own border. It is purely drawn
    /// (it does not affect layout), so it can overlap neighbouring padding like a real focus outline.
    /// `radius` is the widget's own corner radius; the outline widens it by the offset to stay
    /// concentric. Width uses the [`focus_border_width`](crate::theme::Theme::focus_border_width)
    /// token (its own width, so it stays visible even when decorative borders are off), and the halo
    /// is scaled by `glow_size` inside [`rect`](Self::rect) — dropping to nothing when glow is
    /// `none`. The glow is kept low so it reads as a focus cue, not an alarm. Pair with the theme's
    /// [`effective_focus_ring`](crate::theme::Theme::effective_focus_ring) /
    /// [`focus_ring_tone`](crate::theme::Theme::focus_ring_tone) for the color. (The decorative
    /// corner-bracket reticle is a different primitive — [`bracket_frame`](Self::bracket_frame).)
    pub fn focus_ring(&mut self, rect: Rectangle, color: Color, radius: f32) {
        // Offset gap (logical px) between the widget edge and the outline — like CSS `outline-offset`.
        const OFFSET: f32 = 2.0;
        let o = OFFSET as f64;
        let outset = Rectangle::new(
            Point::new(rect.loc.x - o, rect.loc.y - o),
            Size::new(rect.size.w + 2.0 * o, rect.size.h + 2.0 * o),
        );
        let border = Border {
            color,
            width: self.theme.focus_border_width,
        };
        // `rect` runs the glow through `scaled_glow`, so this halo tracks `glow_size`
        // (and vanishes at `none`) exactly like every other glow in the library.
        let glow = Some(Glow {
            color,
            radius: 3.0,
            intensity: 0.3,
        });
        self.rect(outset, Color::TRANSPARENT, Some(border), radius + OFFSET, glow);
    }

    /// Queue a text run within `rect` at an explicit logical `size`. The renderer
    /// centers the text within `rect` (per `align` horizontally, always centered
    /// vertically) using real glyph metrics.
    ///
    /// `style` is the **font** style ([`TextStyle`]: weight + slant). Decorations — underline,
    /// strikethrough — are not shaped: a line is a rect, and the widget draws it itself from the
    /// theme (see [`Label`](crate::widgets::Label)).
    pub fn text(
        &mut self,
        rect: Rectangle,
        text: &str,
        color: Color,
        size: f32,
        align: TextAlign,
        style: TextStyle,
    ) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: text.to_string(),
            color,
            size,
            align,
            style,
            font: FontRole::Text,
            glow: None,
        }));
    }

    /// Queue a single icon glyph centered in `rect`, shaped with the icon font
    /// ([`FontRole::Icon`]). `glyph` is the codepoint as a string; the renderer
    /// selects the embedded icon family. Used by [`Icon`](crate::widgets::Icon).
    pub fn icon(&mut self, rect: Rectangle, glyph: &str, color: Color, size: f32) {
        self.icon_glowing(rect, glyph, color, size, None);
    }

    /// Draw a **Nerd Font** glyph — the counterpart of [`icon`](PaintCx::icon) for the app's second
    /// glyph set ([`NfIcon`](crate::widgets::NfIcon)), which supplies what Phosphor has none of:
    /// keyboard keys. `glyph` is the codepoint as a string.
    pub fn nf_icon(&mut self, rect: Rectangle, glyph: &str, color: Color, size: f32) {
        if self.culled(rect) {
            return;
        }
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: glyph.to_string(),
            color,
            size,
            align: TextAlign::Center,
            style: TextStyle::REGULAR,
            font: FontRole::NerdFont,
            glow: None,
        }));
    }

    /// [`icon`](PaintCx::icon) with an additive halo behind the glyph.
    ///
    /// This is the glyph counterpart of a surface's glow, and it goes through the
    /// **same chokepoint**: `glow` is scaled by the theme's `glow_size` token before
    /// it reaches the scene, so `GlowLevel::None` drops the halo entirely and every
    /// other level scales it exactly as it scales a rect's. Pass
    /// [`rest_glow`](PaintCx::rest_glow) to give a bare glyph the same faint resting
    /// halo that bordered surfaces carry.
    ///
    /// A halo is deliberately opt-in per call rather than a property of the icon
    /// font: terminal cell glyphs are the hottest path in the app and must never
    /// take it.
    pub fn icon_glowing(
        &mut self,
        rect: Rectangle,
        glyph: &str,
        color: Color,
        size: f32,
        glow: Option<Glow>,
    ) {
        if self.culled(rect) {
            return;
        }
        let glow = self.scaled_glow(glow);
        self.emit(DrawCommand::Text(TextCmd {
            rect: self.placed(rect),
            text: glyph.to_string(),
            color,
            size,
            align: TextAlign::Center,
            style: TextStyle::REGULAR,
            font: FontRole::Icon,
            glow,
        }));
    }

    /// Queue a theme-tinted "press flash" overlay over `rect`. `amount` is the
    /// flash strength in `0.0..=1.0` (see [`Flash`](crate::effects::Flash));
    /// `radius` must match the widget's corner radius so the overlay follows a
    /// rounded shape instead of poking square corners past it.
    pub fn flash(&mut self, rect: Rectangle, amount: f32, radius: f32) {
        if amount <= 0.0 {
            return;
        }
        let a = (amount.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.rect(rect, self.theme.colors.foreground.with_alpha(a), None, radius, None);
    }

    /// Draw the **drag ghost** — the small labelled chip that follows the cursor
    /// during a drag. Theme-driven: an accent-filled rounded rect with the label in
    /// the background color, painted on the **overlay layer** so it sits above all
    /// chrome. `rect` is the chip's bounds (the app positions it at the cursor).
    ///
    /// When `swap` is true the chip gains an inset **double frame** — the same motif
    /// as [`swap_indicator`](Self::swap_indicator) — so the cursor-following chip tells
    /// the user this drag is an **exchange**, not a move (there is no OS "swap" cursor).
    pub fn drag_ghost(&mut self, rect: Rectangle, text: &str, swap: bool) {
        let (accent, bg, radius) = (self.theme.colors.accent, self.theme.colors.background, self.theme.colors.border_radius);
        let font = (rect.size.h as f32 * 0.55).clamp(10.0, 15.0);
        self.with_overlay(|cx| {
            cx.rect(rect, accent.with_alpha(217), None, radius, None);
            cx.rect(rect, Color::TRANSPARENT, Some(Border { color: accent, width: 1.5 }), radius, None);
            if swap {
                let inset = 3.0_f64;
                let inner = Rectangle::new(
                    Point::new(rect.loc.x + inset, rect.loc.y + inset),
                    Size::new((rect.size.w - inset * 2.0).max(0.0), (rect.size.h - inset * 2.0).max(0.0)),
                );
                cx.rect(
                    inner,
                    Color::TRANSPARENT,
                    Some(Border { color: bg.with_alpha(180), width: 1.0 }),
                    (radius - inset as f32).max(0.0),
                    None,
                );
            }
            cx.text(rect, text, bg, font, TextAlign::Center, TextStyle::REGULAR);
        });
    }

    /// Draw a **drop indicator** over a target's `bounds`: an accent insertion line
    /// for [`DropSide::Before`]/[`After`](DropSide), or an accent wash + outline for
    /// [`DropSide::Onto`]. Theme-driven (derives from `accent`); the app calls this
    /// over the bounds returned by [`resolve_at`](crate::drag::resolve_at).
    pub fn drop_indicator(&mut self, bounds: Rectangle, side: DropSide) {
        let accent = self.theme.colors.accent;
        match side {
            DropSide::Onto => {
                self.rect(bounds, accent.with_alpha(45), None, self.theme.colors.border_radius, None);
                self.rect(
                    bounds,
                    Color::TRANSPARENT,
                    Some(Border { color: accent, width: 1.5 }),
                    self.theme.colors.border_radius,
                    None,
                );
            }
            DropSide::Before | DropSide::After => {
                let thickness = 2.0;
                let y = if side == DropSide::Before {
                    bounds.loc.y - thickness / 2.0
                } else {
                    bounds.loc.y + bounds.size.h - thickness / 2.0
                };
                self.rect(
                    Rectangle::new(Point::new(bounds.loc.x, y), Size::new(bounds.size.w, thickness)),
                    accent,
                    None,
                    1.0,
                    None,
                );
            }
        }
    }

    /// Draw a **swap indicator** over a target's whole `bounds` — for an *exchange*
    /// gesture (Shift+drag) where the entire target is swapped with the source. There
    /// is no before/after for a swap, so this deliberately draws a whole-item **double
    /// accent frame** (+ faint wash) instead of [`drop_indicator`](Self::drop_indicator)'s
    /// insertion line or `Onto` wash — a distinct cue that the whole item is the target.
    /// Theme-driven (derives from `accent` / `radius` / `border_width`).
    pub fn swap_indicator(&mut self, bounds: Rectangle) {
        let accent = self.theme.colors.accent;
        let radius = self.theme.colors.border_radius;
        // Faint wash + bold outer frame.
        self.rect(bounds, accent.with_alpha(28), None, radius, None);
        let outer_w = (self.theme.colors.border_width * 2.0).max(2.5);
        self.rect(
            bounds,
            Color::TRANSPARENT,
            Some(Border { color: accent, width: outer_w }),
            radius,
            None,
        );
        // Inset second line → reads as a "double frame" (= swap the whole item),
        // visually separating it from the single-outline `Onto` look.
        let inset = 3.0_f64;
        let inner = Rectangle::new(
            Point::new(bounds.loc.x + inset, bounds.loc.y + inset),
            Size::new(
                (bounds.size.w - inset * 2.0).max(0.0),
                (bounds.size.h - inset * 2.0).max(0.0),
            ),
        );
        self.rect(
            inner,
            Color::TRANSPARENT,
            Some(Border { color: accent.with_alpha(120), width: 1.0 }),
            (radius - inset as f32).max(0.0),
            None,
        );
    }

    /// Dim `rect` with a background-colored scrim — the standard look for a
    /// **disabled** widget. `radius` must match the widget's corner radius so the
    /// scrim follows its rounded shape. DRY: every widget reuses this instead of
    /// dimming each color by hand.
    pub fn dim(&mut self, rect: Rectangle, radius: f32) {
        let a = (DISABLED_SCRIM * 255.0).round() as u8;
        self.rect(
            rect,
            self.theme.colors.background.with_alpha(a),
            None,
            radius,
            None,
        );
    }

    /// Draw the prominent **flat corner-bracket frame** used by container chrome
    /// ([`Pane`](crate::widgets::Pane), [`DockFrame`](crate::widgets::DockFrame)):
    /// a subtle continuous accent line tracing the full rounded perimeter, with
    /// bright thick accent **corners** (rounded arc + a short straight arm along
    /// each edge) layered on top — the Tron reticle. DRY: the frame is defined
    /// once here instead of per-widget.
    ///
    /// Width/radius come from `theme.colors.border_width`/`theme.colors.border_radius`: at
    /// `border_width == 0` the frame draws **nothing** (no border anywhere),
    /// consistent with every other widget's border gate. For a surface that
    /// carries its own width/radius (e.g. a self-themed sidebar shell), use
    /// [`bracket_frame_with`](Self::bracket_frame_with).
    pub fn bracket_frame(&mut self, rect: Rectangle) {
        let (radius, border_width) = (self.theme.colors.border_radius, self.theme.colors.border_width);
        self.bracket_frame_with(rect, border_width, radius);
    }

    /// [`bracket_frame`](Self::bracket_frame) with an explicit `border_width` and
    /// corner `radius`, independent of the theme — so one surface can size its own
    /// reticle (the per-widget `Pane::border_width`/radius overrides). The accent
    /// color still comes from the theme. `border_width <= 0.0` ⇒ no frame.
    pub fn bracket_frame_with(&mut self, rect: Rectangle, border_width: f32, radius: f32) {
        let accent = self.theme.colors.accent;
        // Borders off (`border_width == 0`) ⇒ no frame at all, like every other
        // widget. A container that needs definition without a border should carry
        // a fill, not a forced hairline.
        if border_width <= 0.0 {
            return;
        }
        let b = rect;

        // Subtle continuous accent line tracing the whole rounded perimeter — the
        // dimmed "midsection" that the bright corners sit on top of.
        self.rect(
            b,
            Color::TRANSPARENT,
            Some(Border {
                color: accent.with_alpha(BRACKET_DIM_ALPHA),
                width: border_width,
            }),
            radius,
            None,
        );

        // Bright thick corner brackets. The renderer draws the accent border as a
        // band just OUTSIDE the rect edge, carrying the theme corner radius — so
        // redrawing that same border clipped to a corner-sized box yields a bright
        // *rounded* corner plus a short straight arm along each edge, without
        // faking the rounding. `keep` is the bright span from each corner (rounded
        // arc + `BRACKET_ARM_LEN`); `m` grows the clip box outward so it also
        // captures the outer border band hugging the corner.
        let bracket_width = border_width + BRACKET_WIDTH_BOOST;
        let keep = f64::from(radius + BRACKET_ARM_LEN);
        let m = f64::from(bracket_width);
        let (x, y, w, h) = (b.loc.x, b.loc.y, b.size.w, b.size.h);
        let bright = Border { color: accent, width: bracket_width };

        let corners = [
            Point::new(x - m, y - m),               // top-left
            Point::new(x + w - keep, y - m),        // top-right
            Point::new(x - m, y + h - keep),        // bottom-left
            Point::new(x + w - keep, y + h - keep), // bottom-right
        ];
        for loc in corners {
            self.with_clip(Rectangle::new(loc, Size::new(keep + m, keep + m)), |cx| {
                cx.rect(b, Color::TRANSPARENT, Some(bright), radius, None);
            });
        }
    }

    /// Paint the shared chrome for a component's base (background/border/glow).
    pub fn paint_base(&mut self, base: &Base) {
        let s = &base.style;
        if s.visual.fill.is_none() && s.visual.border.is_none() && s.visual.glow.is_none() {
            return;
        }
        self.rect(
            base.bounds,
            s.visual.fill.unwrap_or(Color::TRANSPARENT),
            s.visual.border,
            s.visual.radius,
            s.visual.glow,
        );
    }

    /// The theme-driven **rest glow** a widget surface carries before any
    /// hover/focus/active state: color from the theme's `glow` token, intensity
    /// from `interaction.control_rest_glow` (`None` when that token is `0` — the
    /// flat look), halo radius supplied by the caller (each widget scales its own).
    /// This is the single definition every widget shares, so the whole library
    /// honors the `glow_size` setting at rest uniformly (the returned glow runs
    /// through [`scaled_glow`](Self::scaled_glow) in `rect` like every other).
    /// Widgets add their own gates on top (a disabled control never halos); a
    /// tone-following widget (e.g. a destructive button) overrides the color.
    pub fn rest_glow(&self, radius: f32) -> Option<Glow> {
        let i = self.theme.colors.interaction.control_rest_glow;
        (i > 0).then_some(Glow {
            color: self.theme.colors.glow,
            radius,
            intensity: i as f32 / 255.0,
        })
    }

    /// Scale a glow by the theme's `glow_size` token (the **sole** owner of glow:
    /// presence + halo radius). `intensity` is intentionally *not* applied here —
    /// it controls only the scanline/CRT overlay — so the two settings no longer
    /// overlap. `GlowLevel::None` drops the glow entirely.
    fn scaled_glow(&self, glow: Option<Glow>) -> Option<Glow> {
        // `GlowLevel` (theme/config `glow_size`) owns BOTH dimensions of glow:
        // `radius_scale` (halo size) and `strength_scale` (alpha). Apply both here —
        // the single chokepoint — so every widget's glow tracks the config uniformly
        // instead of baking a fixed intensity. `Medium` (default) is 1.0×, so this is
        // a no-op for the default theme; `none` drops glow entirely.
        let radius = self.theme.colors.glow_size.radius_scale();
        let strength = self.theme.colors.glow_size.strength_scale();
        glow.filter(|_| radius > 0.0).map(|g| Glow {
            radius: g.radius * radius,
            intensity: g.intensity * strength,
            ..g
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn mark_needs_paint_sets_the_flag_and_requests_a_frame() {
        let frames = Rc::new(Cell::new(0u32));
        let f = frames.clone();
        install_frame_request(move || f.set(f.get() + 1));

        let base = Base::new();
        assert!(base.needs_paint(), "a fresh widget needs its first paint");
        base.clear_needs_paint();
        assert!(!base.needs_paint(), "clearing drops the flag");

        base.mark_needs_paint();
        assert!(base.needs_paint(), "marking re-sets the repaint flag");
        assert_eq!(frames.get(), 1, "marking asks the host for exactly one frame");
    }
}
