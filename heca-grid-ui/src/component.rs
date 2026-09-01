//! The component model: the [`Component`] trait, the shared [`Base`] struct
//! every widget embeds, and the [`PaintCx`] painting context.
//!
//! "Extends a base" is expressed in idiomatic Rust as **composition**: a widget
//! embeds a [`Base`] (style, bounds, children, visibility signal) and implements
//! [`Component`]. Shared chrome (background, border, glow, corner brackets) lives
//! once on [`PaintCx`], so every component reuses it (DRY).

use crate::color::Color;
use crate::drag::DropSide;
use crate::hint::DeclaredAction;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
use crate::scene::{
    Border, BracketCmd, DrawCommand, FontRole, Glow, HostCmd, HostDraw, RectCmd, ScanlineCmd, Scene,
    Shadow, TextAlign, TextCmd, TextStyle,
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

/// **Does any widget in this tree need laying out again?** Clears the flags as it walks.
///
/// The layout twin of [`collect_damage`], and the host calls it the same way — once a frame,
/// before deciding whether to run [`LayoutEngine::compute`](crate::LayoutEngine::compute). It
/// answers a question a repaint cannot: a widget that changed the *shape* of the tree has moved
/// its siblings, and only a layout pass can put them right.
///
/// Unlike damage there is nothing to union — layout is a whole-tree pass, so the answer is a bool.
/// Hidden subtrees are **not** skipped: a subtree that just became hidden is exactly the case that
/// needs the pass, and its stale bounds are what the pass is about to fix.
///
/// ```ignore
/// // In the host's frame, beside the existing damage call:
/// if heca_grid_ui::needs_layout(&ui) {
///     self.layout_dirty = true;
/// }
/// ```
pub fn needs_layout(root: &dyn Component) -> bool {
    fn walk(c: &dyn Component, found: &mut bool) {
        let b = c.base();
        if b.needs_layout() {
            b.clear_needs_layout();
            *found = true;
        }
        for child in &b.children {
            walk(child.as_ref(), found);
        }
    }
    let mut found = false;
    walk(root, &mut found);
    found
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
    /// **The pointer passes straight through this widget** (CSS `pointer-events: none`).
    ///
    /// For content that is *decoration standing in for something else*: a `Select`'s chosen option
    /// is echoed inside the closed trigger, and while it stands there it is not an option you can
    /// pick — the trigger is the control. Left as a target it hovered on its own, so the text lit
    /// up and the chevron beside it did not: one control wearing two highlights.
    ///
    /// It is about **input, not paint** — the widget still draws — and it applies to the whole
    /// subtree, because a decoration's children are decoration too.
    pub pointer_transparent: bool,
    /// Explicit Tab-order index (like HTML `tabindex`). Focusables with an index
    /// are visited first in ascending order; those without (`None`) follow in
    /// tree position order. Set via [`LayoutExt::tab_index`](crate::builders::LayoutExt::tab_index).
    pub tab_index: Option<i32>,
    /// Child components, laid out by this component's flex container.
    pub children: Vec<Box<dyn Component>>,
    /// This widget **can be dragged**, and what is dragged is [`key`](Self::key) — the identity it
    /// already declares about itself. Universal opt-in via
    /// [`ComponentExt::draggable`](crate::builders::ComponentExt::draggable); resolved generically
    /// by [`drag::source_at`](crate::drag::source_at).
    ///
    /// It used to carry an opaque id handed out by a host registry, and a closed list of which
    /// surfaces were allowed to drag at all. Both are gone: a row said who it was twice, and a
    /// plugin's row could say it neither time — it could not be added to a list that is an enum in
    /// our source, so it could never be dragged (Antonio, 2026-09-01: *"hardcode smell?? what i
    /// hate"*). A widget with no `key` is not a drag source, because there would be nothing to
    /// name what was picked up.
    pub draggable: bool,
    /// **What this widget is, when it is dragged** — an opaque word its component chose
    /// (`"pane"`, `"column"`, `"docker.container"`). Set with
    /// [`ComponentExt::draggable_as`](crate::builders::ComponentExt::draggable_as); `None` means it
    /// says nothing about itself and every target takes it.
    ///
    /// It is deliberately a free string and not a list the library knows: a closed set is one a
    /// plugin cannot join, and the same shape a row already uses to say what its right-click menu
    /// is about.
    pub drag_kind: Option<String>,
    /// This widget **accepts drops**, identified the same way — by its [`key`](Self::key).
    /// Universal opt-in via [`ComponentExt::drop_target`](crate::builders::ComponentExt::drop_target);
    /// resolved generically by [`drag::resolve_at`](crate::drag::resolve_at).
    pub drop_target: bool,
    /// **What this widget takes**, by the same words. Empty means it takes anything, which is what
    /// a target that says nothing gets. Set with
    /// [`ComponentExt::accepts`](crate::builders::ComponentExt::accepts).
    ///
    /// A target that refuses is not offered: the walk skips it and keeps looking outward, so
    /// nothing is drawn over something that would then do nothing. That mismatch is what this
    /// exists to end — the line said yes and the release said no, because the drawing and the rule
    /// lived in different places (Antonio, driving, 2026-09-01).
    pub accepts: Vec<String>,
    /// When set, this widget is a **navigable row** carrying its own identity: the keyboard cursor,
    /// the right-click target and the drag are three readers of this one declaration.
    ///
    /// A string the component chose about *itself*, so it
    /// survives a tree rebuild. Universal opt-in via [`ComponentExt::key`](crate::builders::ComponentExt::key);
    /// enumerated by [`nav::collect_keys`](crate::nav::collect_keys) and hit-tested by
    /// [`nav::key_at`](crate::nav::key_at). Opaque here — nothing in this library parses it.
    pub key: Option<String>,

    /// **This node was seated as a surface** — placed above the page rather than laid out in it.
    /// A host sets it when it seats one; no author ever writes it and no widget behaves
    /// differently for having it.
    ///
    /// It buys one rule, and the rule is the browser's: a surface **passes the pointer through
    /// where it covers nothing**, exactly as `pointer-events: none` on a positioned wrapper does,
    /// while its children keep taking what lands on them. Right-click still opens the menu of
    /// whatever is under the cursor.
    ///
    /// It exists because a surface's *box* is routinely much bigger than what it draws — a
    /// notification stack spans the window so a corner can mean the screen's corner, and a
    /// decorator wrapped around one hugs it and spans the window too. Left to the box, an empty
    /// invisible surface swallows every press in the application and nothing anywhere fails
    /// (Antonio, driving, 2026-09-01). Fixing the widgets one at a time does not end it: the next
    /// surface, or the next decorator over one, brings it back, and its author had no way to know
    /// they were meant to think about it.
    ///
    /// A surface that *wants* to swallow says so where it already says it —
    /// [`overlay_occludes`](Component::overlay_occludes), which `Overlay::blocking(true)` answers
    /// for the whole viewport. So the cost to an author is nothing, and the cost of forgetting is
    /// nothing.
    pub surface: bool,
    /// **Which enclosing region this subtree belongs to** — a panel, a dock, a tab group, whatever
    /// the host calls the thing that holds rows. Universal opt-in via
    /// [`ComponentExt::scope_key`](crate::builders::ComponentExt::scope_key); hit-tested by
    /// [`nav::scope_at`](crate::nav::scope_at). Opaque here, exactly like `key`.
    ///
    /// A *separate* field rather than a flavour of `key` because the two answer different
    /// questions about the same point: `key` says which **row**, this says which **region
    /// containing rows**, and a host commonly wants both from one press. Folding them together would
    /// also make a region turn up in `collect_keys` as a steppable row, which it is not.
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
    /// A universal slot like [`key`](Self::key) and [`drag_source`](Self::drag_source), so
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
    /// **What a leader-key pick does to this widget, when that differs from acting on it** —
    /// written with [`on_hint`](crate::builders::ComponentExt::on_hint), on any widget.
    ///
    /// It is the **override**, not the switch. Being pickable is not opt-in: anything actionable is
    /// offered a letter and picking it does what clicking it does (F003/P082/T441). This says a pick
    /// does something *else* — heca's sidebar row activates the pane and leaves the sidebar on a
    /// click, and stays in the sidebar on a pick.
    ///
    /// There is no id to register and no registry to reach; the declaration lives on the widget and
    /// the collector reads it out of the laid-out tree.
    ///
    /// **The builder is on every widget** (F003/P082/T432). It used to be on
    /// [`KeyHint`](crate::widgets::KeyHint) alone, which put the two facts about a target — who it
    /// is and what picking it does — on two different nodes, and which of them was on top depended
    /// on how the tree was built. That is why the matching code had to search both up and down, and
    /// why sidebar letters kept failing with no error. `KeyHint` stays as a decorator for a
    /// **region that is not a widget you can put a builder on**, and nothing else.
    pub hint: Option<crate::hint::Hint>,
    /// **May this widget be offered a letter at all?** `true` for everything, until a caller says
    /// otherwise with [`hintable(false)`](crate::builders::ComponentExt::hintable).
    ///
    /// It exists because being pickable is **not** opt-in: anything actionable gets a letter, so the
    /// only thing left to say is "not me". Setting it `true` does nothing — a widget nobody can act
    /// on has nothing for a letter to run (F003/P082/T441).
    pub hintable: bool,
    /// **Can the user act on this widget — click it, or activate it from the keyboard?**
    ///
    /// Set by whichever builder wires the action up, whatever it is called: the generic
    /// [`on_click`](crate::builders::ComponentExt::on_click) /
    /// [`on_double_click`](crate::builders::ComponentExt::on_double_click) /
    /// [`on_key_down`](crate::builders::ComponentExt::on_key_down) /
    /// [`on_key_up`](crate::builders::ComponentExt::on_key_up), and the widgets that keep their own
    /// callback instead — `Button::on_click`, `Row::on_activate`, and the six like them.
    ///
    /// **It is a field and not a question asked of the handler list**, because the answer is not in
    /// the handler list. Eight widgets store their action in a private field of their own, so
    /// `Handlers::has(Click)` is `false` for a `Button` — the single case that matters most. And the
    /// two spellings (`on_click`, `on_activate`) mean the same thing, so no set of `EventKind`s
    /// names it either. One flag, set where the action is wired, is the only thing a walk over the
    /// tree can read (F003/P082/T441).
    pub activatable: bool,
    /// **The letter currently offered for [`hint`](Self::hint)** — `Some("a")` while a picker is
    /// open, `None` otherwise. Set by the host through
    /// [`offer_hint`](crate::hint::offer_hint); drawn by the widget that declared the hint.
    ///
    /// **It lives beside the declaration on purpose, and this is load-bearing.** The letter has to
    /// be drawn *by the widget*, in the widget's own paint, because that is the only way it lands
    /// in the same place on screen as the thing it labels. A host that walks the trees and paints
    /// the caps itself has to guess which scene — and which **half** of it — the declaring widget
    /// ended up in, and it will guess wrong: a `Scene` defers overlay segments to a frame-final
    /// band ordered by nesting depth, so caps painted into the base draw *under* any overlay, and
    /// caps painted at depth 1 draw under anything nested deeper. Both failures are invisible in
    /// every test and look exactly like "the picker does nothing".
    ///
    /// A plugin's surface therefore gets the picker right by construction: it declares a hint, the
    /// framework offers it a letter, and its own paint puts that letter wherever the widget is.
    /// There is nothing host-side to teach about the plugin's layering (F003/P082/T427).
    pub hint_label: Signal<Option<String>>,
    /// **How this widget's letter is drawn** — where it sits, its size, its colour.
    ///
    /// These were private fields on [`KeyHint`](crate::widgets::KeyHint), which is why a letter
    /// could only appear by wrapping a widget in one. Here, the framework draws the cap for any
    /// widget carrying a letter and each one places its own (F003/P082/T431).
    pub hint_style: crate::widgets::HintStyle,
    /// **Actions this widget declares by name** — what a binding, a menu entry or a script can ask
    /// it to do (F003/P082/T427).
    ///
    /// The counterpart of [`hint`](Self::hint): a hint says what a *pick* does to this region, an
    /// action says what a *named verb* does to it. Written with
    /// [`on_action`](crate::builders::ComponentExt::on_action).
    ///
    /// **It exists so a surface can own a verb without being a `Provider`.** A dock declares its
    /// actions through the provider trait; a *layer* — an overlay, a plugin's panel — had no such
    /// seam at all, so it could only bind verbs the app had already compiled in. That is why the
    /// exposé's picker had to borrow the built-in `hint_pick`, and why a plugin could contribute
    /// targets to heca's picker but never open one of its own.
    ///
    /// The name is the whole address: the host finds the declaring widget by walking the retained
    /// trees, exactly as it finds a hint. Nothing is registered, so nothing has to be
    /// un-registered when a tree is rebuilt.
    pub actions: Vec<DeclaredAction>,
    /// Whether [`Event::Mount`] has been delivered. Set by the first layout pass that sees this
    /// widget — the first moment it is both in a live tree and laid out.
    pub(crate) mounted: Cell<bool>,
    /// Repaint flag for the retained renderer: set when this widget's visuals
    /// changed and cleared once it's repainted. Starts `true` (everything paints
    /// on the first frame). The renderer repaints only widgets whose flag is set,
    /// and unions their bounds into the frame's damage region.
    needs_paint: Cell<bool>,
    /// **Relayout flag: this widget's TREE changed, not just its pixels.**
    ///
    /// A widget that adds or removes children between frames — one that reconciles a host-owned
    /// list, like [`ToastStack`](crate::widgets::ToastStack) — has changed the shape of the tree,
    /// and a repaint cannot fix that: the siblings around it are still laid out where they were.
    /// The host reads this through [`needs_layout`] and re-runs the layout pass.
    ///
    /// It is separate from [`needs_paint`](Base::needs_paint) because they cost different things:
    /// a repaint is per-frame and cheap, a layout pass walks and re-measures the whole tree. A
    /// widget that merely changed colour must not trigger one.
    needs_layout: Cell<bool>}

impl Base {
    /// **The name this widget declares itself by** — its [`key`](Self::key), or its
    /// [`scope_key`](Self::scope_key) when it names a region rather than a row.
    ///
    /// The two fields stay separate for the reasons given on each: one says *which row*, the other
    /// *which region containing rows*, and only the first is a steppable cursor stop. But both are a
    /// name the widget chose about itself, so **everything that names a widget asks here** and they
    /// cannot disagree.
    ///
    /// This exists because they did disagree. [`nav::identity_of`](crate::nav::identity_of) read
    /// only `key`, so a dock — which declares itself with `scope_key` — contributed nothing to its
    /// children's names. Two docks holding the same rows produced two sets of identical identities,
    /// and anything keyed on identity silently addressed the wrong one: a remembered hint letter
    /// bounced between the two copies on every opening, and each dock's own name fell back to its
    /// decorative drag grip and shifted whenever a row was added.
    pub fn identity(&self) -> Option<&str> {
        self.key.as_deref().or(self.scope_key.as_deref())
    }

    /// **Does this widget answer to `name`?** Either declaration counts, so a container can be
    /// addressed by the region name it published without also inventing a row key.
    ///
    /// The lookup twin of [`identity`](Self::identity): that one asks what a widget is *called*,
    /// this one asks whether a given name reaches it. Both live here so no caller writes the
    /// `key`-or-`scope_key` test by hand and drifts from the other.
    pub fn answers_to(&self, name: &str) -> bool {
        self.key.as_deref() == Some(name) || self.scope_key.as_deref() == Some(name)
    }

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
            pointer_transparent: false,
            tab_index: None,
            children: Vec::new(),
            draggable: false,
            drag_kind: None,
            drop_target: false,
            accepts: Vec::new(),
            key: None,
            scope_key: None,
            font: 15.0,
            viewport: Size::new(f64::MAX, f64::MAX),
            pointer: crate::pointer::PointerState::new(),
            handlers: None,
            context_menu: None,
            surface: false,
            hint: None,
            hintable: true,
            activatable: false,
            hint_label: crate::reactive::signal(None),
            hint_style: crate::widgets::HintStyle::default(),
            actions: Vec::new(),
            mounted: Cell::new(false),
            needs_paint: Cell::new(true),
            // A fresh widget is laid out by the pass that mounts it; it has nothing to re-request.
            needs_layout: Cell::new(false),
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

    /// **Show or hide this widget — the one way to do it.**
    ///
    /// `hidden` is the engine's `display: none`: the widget leaves the layout entirely, and
    /// everything after it moves. So flipping it is a **layout** change, not a paint one, and this
    /// asks for the pass that re-places the siblings — which is why nothing should ever write
    /// `style.layout.hidden` directly. Writing it by hand is how a dock's rows ended up painted on
    /// top of each other and a sidebar row stayed collapsed with its content already arrived: the
    /// value was right and nobody had moved anything.
    ///
    /// **It only asks when the value actually changes.** That is what makes it safe to call from
    /// `remeasure`, which runs *inside* the layout pass: re-applying the same state there marks
    /// nothing, so a widget that syncs itself every pass cannot request one every pass.
    pub fn set_hidden(&mut self, hidden: bool) {
        if self.style.layout.hidden != hidden {
            self.style.layout.hidden = hidden;
            self.mark_needs_layout();
        }
    }

    /// Whether this widget is out of the layout entirely (`display: none`).
    pub fn is_hidden(&self) -> bool {
        self.style.layout.hidden
    }

    /// **Say that this widget's tree changed and must be laid out again.**
    ///
    /// Call it when you add or remove children outside the layout pass — reconciling a host-owned
    /// list, revealing a subtree, growing a row. A repaint is not enough: the widget's siblings are
    /// still laid out around the shape the tree used to have.
    ///
    /// It also requests a frame, so a host that is otherwise idle wakes up to run the pass.
    ///
    /// **Why this exists.** A stacked notification removes its card only once the card's exit has
    /// played — several frames after the click that dismissed it. Nothing re-laid-out at that
    /// moment, so the cards below it kept their old positions until some unrelated click happened
    /// to trigger a layout, and the gap where the card had been simply sat there
    /// (Antonio, driving the showcase, F003/P096).
    pub fn mark_needs_layout(&self) {
        self.needs_layout.set(true);
        request_frame();
    }

    /// Whether this widget's tree changed and wants a layout pass.
    pub fn needs_layout(&self) -> bool {
        self.needs_layout.get()
    }

    /// Clear the relayout flag — [`needs_layout`] does this as it walks.
    pub fn clear_needs_layout(&self) {
        self.needs_layout.set(false);
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
        self.focused_by_keyboard()
    }

    /// **Did this widget's focus arrive from the keyboard?** — `focused && focus_visible`, CSS
    /// `:focus-visible` semantics, and the one definition of that question.
    ///
    /// Two things read it, for the same reason, and neither should re-derive it:
    ///
    /// - [`shows_focus_ring`](Self::shows_focus_ring) — a click focuses without ringing.
    /// - [`wants_visible`](Component::wants_visible) — **pointing at something never scrolls it.**
    ///   A reveal exists to bring into view what the user cannot see, which is the keyboard's
    ///   case; what the mouse is on is visible by definition, and scrolling it moves it out from
    ///   under the pointer that asked. Clicking a row inside a scrolled region used to focus it,
    ///   which asked for a reveal, which centred it — so the first click only scrolled and the
    ///   second one did what you meant.
    pub fn focused_by_keyboard(&self) -> bool {
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

    /// **This surface's arrival and exit, if it has one** — the seam a host drives.
    ///
    /// A widget that can appear and disappear embeds a [`Presence`](crate::animation::Presence)
    /// and returns it here; everything else keeps the default `None` and is a *cut*, which costs
    /// nothing and needs no special case anywhere.
    ///
    /// It exists beside [`base`](Self::base) because a host that **mounts** surfaces — the layer
    /// stack, a plugin panel host — has to do three things to a surface without knowing which
    /// widget it is: keep it mounted while its exit is still playing
    /// ([`Presence::is_leaving`](crate::animation::Presence::is_leaving)), paint it as its
    /// animation says ([`Presence::frame`](crate::animation::Presence::frame)), and — when the
    /// surface is **rebuilt** with fresh content — carry the gesture across to the new tree by
    /// swapping this value, so an arrival already played does not play again.
    ///
    /// It is deliberately **not** recursive: a surface is the root of what was mounted, not
    /// something to be hunted for in a subtree. A widget that *composes* an
    /// [`Overlay`](crate::widgets::Overlay) (a [`Dialog`](crate::widgets::Dialog)) forwards this to
    /// the overlay it composes if it wants a host to drive it.
    fn presence(&self) -> Option<&crate::animation::Presence> {
        None
    }

    /// The mutable half of [`presence`](Self::presence) — see there. Both, for the same reason
    /// [`base`](Self::base) and [`base_mut`](Self::base_mut) are both there.
    fn presence_mut(&mut self) -> Option<&mut crate::animation::Presence> {
        None
    }

    /// **Put this surface on screen.**
    ///
    /// Safe to call at any time: a surface already up is not re-arrived, and one already on its way
    /// out is not resurrected. Both rules live in [`Presence`](crate::animation::Presence), so no
    /// caller repeats them. With no animation declared it is simply up.
    ///
    /// Default: nothing to open.
    fn open(&mut self) {}

    /// **Dismiss this surface.**
    ///
    /// With an animation declared this *begins* the exit — the surface is gone when
    /// [`Presence::is_leaving`](crate::animation::Presence::is_leaving) says the gesture has played
    /// out, which is what lets a host keep painting it while it goes. With none, it is simply gone.
    ///
    /// Default: nothing to hide.
    fn hide(&mut self) {}

    /// Open it if it is closed, dismiss it if it is open. The default reads
    /// [`presence`](Self::presence), so a surface gets it for free.
    fn toggle(&mut self) {
        match self.presence().is_some_and(crate::animation::Presence::is_open) {
            true => self.hide(),
            false => self.open(),
        }
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
        self.base().focused_by_keyboard()
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
    ///
    /// **Three walks ask it now**, and each one that forgets shows the same defect from a different
    /// angle: paint (the clip itself), input ([`hit_test`](crate::pointer::hit_test) — a row past
    /// the fold is not clickable) and the **picker** ([`hint`](crate::hint) — a row past the fold
    /// gets no letter). The picker was the one that did not, and its symptom was keycaps for
    /// scrolled-away sidebar rows painted over the top bar and the status bar, because a cap goes
    /// into the overlay band and an overlay segment starts unclipped on purpose (F003/P082/T438).
    ///
    /// If you are adding a fourth walk over the tree, it asks this too.
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

    /// **Can this widget be dragged?** Default reads [`draggable`](Base::draggable); a widget whose
    /// draggability changes may override.
    ///
    /// It answers only *whether*, never *what*: what gets dragged is the widget's identity, and an
    /// identity is [`key`](Base::key) **when it declared one and derived from its content when it
    /// did not** — the same string the keyboard cursor, the right-click target and a remembered
    /// hint letter are filed under ([`nav::identity_of`](crate::nav::identity_of)). Asking for the
    /// key here instead would make `.draggable()` silently do nothing on every widget that never
    /// needed a name, and put an internal rule in front of the author (Antonio, 2026-09-01).
    fn is_drag_source(&self) -> bool {
        self.base().draggable
    }

    /// **Does this widget accept drops?** The same shape, reading
    /// [`drop_target`](Base::drop_target). Walked by
    /// [`drag::resolve_at`](crate::drag::resolve_at).
    fn is_drop_target(&self) -> bool {
        self.base().drop_target
    }

    /// **Would this widget take what is being dragged?** A target that declared no
    /// [`accepts`](Base::accepts) takes anything; one that did takes only what it named, and a
    /// dragged widget that named nothing is taken by anyone.
    ///
    /// Asked during resolution, so a target that would refuse is never offered and never drawn on.
    fn accepts_drag(&self, kind: Option<&str>) -> bool {
        let want = &self.base().accepts;
        match kind {
            _ if want.is_empty() => true,
            Some(k) => want.iter().any(|w| w == k),
            None => true,
        }
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
pub(crate) fn deliver_to_path(node: &mut dyn Component, path: &[usize], ev: &Event) -> Handled {
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
    } else if matches!(ev, Event::Hint(_)) {
        // **A pick acts on the widget it named, and on nothing else** (F003/P082/T432). This is the
        // target phase, and it is the only place a widget's own declaration runs — what continues
        // up the chain is the *event*, which is the delegation seam.
        //
        // The divergence from a click is deliberate: clicking a child of a clickable box **is**
        // clicking the box, because the pointer is over both, while picking a row is **not** picking
        // the pane that holds it. A pick is nominal, not spatial. Without this, a pickable pane
        // holding pickable rows fires both and you land on the pane — and every such container would
        // hand-write the DOM's `e.target !== e.currentTarget` guard, which is N copies of a rule
        // that belongs here.
        run_pick(node);
    }
    if node.base_mut().run_handlers(ev) == Handled::Yes {
        return Handled::Yes;
    }
    node.on_event(ev)
}

/// **Do to `node` what picking it means**, in two cases and in this order (F003/P082/T441):
///
/// 1. it **declared** what a pick does ([`Base::hint`]) — run that. The override, for a region that
///    answers a pick differently from a click: heca's sidebar row activates the pane and leaves on a
///    click, and stays in the sidebar on a pick.
/// 2. otherwise **act on it as a click would**, because that is what a letter over an ordinary
///    button promises. Delivered as a real [`Event::Click`] at the widget's centre, through the
///    handlers it already registered — never a second path that could drift from what the mouse
///    does.
fn run_pick(node: &mut dyn Component) {
    if let Some(hint) = &node.base().hint {
        hint.run();
        return;
    }
    if !crate::hint::is_target(node) {
        return;
    }
    // The centre, so a handler reading the position lands inside the widget it was aimed at.
    let b = node.base().bounds;
    let at = heca_core::layout::Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let click = Event::Click(crate::event::PointerEvent::at(at).with_target_bounds(b));
    node.base_mut().run_handlers(&click);
    node.on_event(&click);
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
///
/// **It also draws the child's hint letter**, if it is carrying one (F003/P082/T431). That is here,
/// and not in each widget, for the reason the hidden check is: it is the one place every container
/// already funnels its children through, so a letter appears over *any* widget — a plugin's
/// included — with nothing to opt into and no wrapper to remember. Before this, only
/// [`KeyHint`](crate::widgets::KeyHint) could draw one, so being pickable meant being wrapped.
///
/// **No host pass may paint a keycap.** One that walks the trees itself has to guess which scene,
/// and which half of it, the declaring widget ended up in — and it guesses wrong invisibly. See
/// [`key_hint::paint_hint_label`](crate::widgets::key_hint::paint_hint_label).
pub fn paint_child(c: &dyn Component, cx: &mut PaintCx) {
    if c.base().style.layout.hidden {
        return;
    }
    c.paint(cx);
    crate::widgets::key_hint::paint_hint_label(c, cx);
    paint_drag_feedback(c, cx);
}

/// **What a drag looks like, drawn by the widgets taking part in it** — the target marks where the
/// thing would land, and the source carries a picture of itself under the cursor.
///
/// It is here, beside the hint letter, because both are the same kind of thing: something the
/// framework draws *over* any widget from state the framework already keeps, so no widget opts in
/// and no host paints on their behalf. That is what every other drag-and-drop library settled on —
/// Flutter's target rebuilds with what is over it, SwiftUI and the browser show a picture of the
/// dragged view by default, and the thing that follows the cursor is drawn in a layer above
/// everything so no scroll area clips it.
///
/// heca drew all of this centrally instead: the app read its own drag phases and painted the
/// indicator and the chip for the whole window, so a new surface — a plugin's especially — got a
/// drag that looked like nothing at all until the app was taught about it.
///
/// **What a widget may still decide** is what it *is*: `Onto` versus an insertion line comes from
/// where the pointer sits in the target, and the modifiers ride along for a host convention (heca
/// reads Shift as "swap these two"). The library has no opinion on what a modifier means; it only
/// makes sure the widget can answer.
fn paint_drag_feedback(c: &dyn Component, cx: &mut PaintCx) {
    let b = c.base();
    if b.pointer.is_drag_over() {
        let bounds = b.bounds;
        match b.pointer.drag_modifiers().shift {
            true => cx.swap_indicator(bounds),
            false => cx.drop_indicator(bounds, b.pointer.drag_side()),
        }
    }
    if b.pointer.is_dragging() {
        // A picture of what was picked up, offset off the cursor and vertically centred on it, in
        // the overlay band so nothing this widget sits inside can clip it.
        let at = b.pointer.drag_pos();
        let text = c.text_summary().unwrap_or_default();
        let h = b.bounds.size.h.clamp(18.0, 28.0);
        let w = (text.chars().count() as f64 * 7.5 + 20.0).clamp(48.0, 220.0);
        let rect = Rectangle::new(
            Point::new(at.x + 10.0, at.y - h / 2.0),
            heca_core::layout::Size::new(w, h),
        );
        cx.drag_ghost(rect, &text, b.pointer.drag_modifiers().shift);
    }
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
    /// The inherited **control tone** for the subtree currently being painted — see
    /// [`with_control_tone`](Self::with_control_tone). `None` at the root, and `None` almost
    /// everywhere: a container has to say it wants the controls inside it to follow its hue.
    control_tone: Option<Color>,
    content_glow: Option<Glow>,
    /// Translation applied to every draw emitted through this context — see
    /// [`with_translate`](Self::with_translate). `(0, 0)` normally: a widget paints at its bounds.
    offset: (f64, f64),
    /// Alpha multiplier applied to every draw emitted through this context — see
    /// [`with_opacity`](Self::with_opacity). `1.0` normally: a widget paints at its own colours.
    opacity: f32,
    /// The active [scale](PaintCx::with_scale) — multiplicative, like `opacity`.
    scale: f32,
    /// The fixed point the scale shrinks toward, already in scene coordinates.
    scale_origin: Point,
    /// The clip currently in force — every open [`with_clip`](Self::with_clip) intersected, in the
    /// same space the callers passed (a widget's own layout coordinates, before
    /// [`placed`](Self::placed)), so a value read off `Base::bounds` can be compared with it
    /// directly. `None` is *"nothing clips this"*.
    ///
    /// Read by [`clip`](Self::clip) — a widget that draws into the **overlay band** needs it,
    /// because that band starts unclipped and geometry alone would let a keycap land outside the
    /// dock it belongs to (F003/P082/T438).
    clip: Option<Rectangle>,
}

/// **Make a wrapper transparent to layout** — the sizing half of "transparent".
///
/// A wrapper that decorates without changing the picture ([`KeyHint`](crate::widgets::KeyHint),
/// [`Visibility`](crate::widgets::Visibility), [`FocusScope`](crate::widgets::FocusScope),
/// [`KeyHintGroup`](crate::widgets::KeyHintGroup)) hugs its child, so its bounds are the child's —
/// which is what the decoration is positioned off. Hugging alone is not transparency:
///
/// - a child sized as a **share** (`Length::Pct`) resolves that percentage against its parent, and
///   its parent is now the wrapper. A hugged wrapper is `Auto`, so the share resolves against
///   nothing and silently falls back to the child's **content** size — the widget stops being a
///   share and becomes as wide as its text;
/// - the same for a `max_width` a child sets to keep itself inside its container.
///
/// That is why the exposé's cards would not shrink with the window: each card asked for 100% of a
/// wrapper that asked for 100% of nothing, so a card stayed as wide as the path inside it and every
/// card's text ran across its neighbours (Antonio, driving, 2026-08-24). `expose/mod.rs` already
/// carried a hand-written workaround — "the room the panel gives it has to be passed on
/// deliberately" — which is one call site fixing a rule that belongs here.
///
/// Adopting whatever the child declares keeps the chain unbroken, and a child that hugs still hugs,
/// because then there is nothing to adopt.
pub fn wrap_transparently(base: &mut Base, child: &dyn Component) {
    let child = child.base().style.layout;
    if !matches!(child.width, crate::style::Length::Auto) {
        base.style.layout.width = child.width;
    }
    if !matches!(child.height, crate::style::Length::Auto) {
        base.style.layout.height = child.height;
    }
    base.style.layout.max_width = base.style.layout.max_width.or(child.max_width);
    base.style.layout.max_height = base.style.layout.max_height.or(child.max_height);
    // **Whether it may be squeezed is the content's answer too.** Everything gives way by default,
    // so a wrapper around something that refuses — a `StatusDot`, whose circle has no narrower
    // version — gave way in its place, and the dot was drawn as a 3px sliver inside a box that had
    // shrunk around it. The wrapper has no opinion of its own; it *is* the widget inside it
    // (F003/P096/T483).
    base.style.layout.flex_shrink = base.style.layout.flex_shrink.or(child.flex_shrink);
}

/// Scale every colour in a draw command by `a`, leaving its geometry alone.
///
/// Written out per command rather than as a blanket "multiply anything colour-shaped", because the
/// commands carry colours in several roles — a fill, a border, the light of a glow, the dark of a
/// shadow — and each has to be dimmed on its own or a fading panel loses its outline a frame before
/// its body, or keeps a halo around nothing.
/// Scale one command: its geometry through `place`, and **everything measured in pixels with it** —
/// the font size, the corner radius, the border width, the glow and shadow falloff and offsets.
///
/// That second half is the whole difference between a zoom and a mistake. A rect that halves while
/// its text stays 14px, its radius stays 6px and its 1px border stays 1px is not the same picture
/// further away; it is a different, wronger picture. `fade_command` beside it is the same idea for
/// colour.
fn scale_command(cmd: DrawCommand, k: f32, place: impl Fn(Rectangle) -> Rectangle) -> DrawCommand {
    let glow = |g: Glow| Glow { radius: g.radius * k, ..g };
    let shadow = |s: Shadow| Shadow {
        radius: s.radius * k,
        dx: s.dx * k,
        dy: s.dy * k,
        ..s
    };
    match cmd {
        DrawCommand::Host(h) => DrawCommand::Host(HostCmd {
            rect: place(h.rect),
            // A blur radius is a distance, so it scales with everything else; a surface id and an
            // alpha are not distances.
            draw: match h.draw {
                HostDraw::Backdrop { radius } => HostDraw::Backdrop { radius: radius * k },
                other => other,
            },
            ..h
        }),
        DrawCommand::Rect(r) => DrawCommand::Rect(RectCmd {
            rect: place(r.rect),
            radius: r.radius * k,
            border: r.border.map(|b| Border { width: b.width * k, ..b }),
            glow: r.glow.map(glow),
            shadow: r.shadow.map(shadow),
            ..r
        }),
        DrawCommand::Text(t) => DrawCommand::Text(TextCmd {
            rect: place(t.rect),
            size: t.size * k,
            glow: t.glow.map(glow),
            ..t
        }),
        DrawCommand::Brackets(b) => DrawCommand::Brackets(BracketCmd {
            rect: place(b.rect),
            len: b.len * k,
            thickness: b.thickness * k,
            glow: b.glow.map(glow),
            ..b
        }),
        DrawCommand::Scanline(s) => DrawCommand::Scanline(ScanlineCmd {
            rect: place(s.rect),
            ..s
        }),
        // A clip is geometry too: it must follow the picture it clips, or a scaled subtree is cut
        // to the rect it had at life size.
        DrawCommand::PushClip(r) => DrawCommand::PushClip(place(r)),
        DrawCommand::PopClip => DrawCommand::PopClip,
    }
}

fn fade_command(cmd: DrawCommand, a: f32) -> DrawCommand {
    let dim = |c: Color| c.with_alpha((c.a as f32 * a).round().clamp(0.0, 255.0) as u8);
    match cmd {
        // Host work composites at its own strength times the context's opacity, so a surface or a
        // frost inside a fading overlay fades with it.
        DrawCommand::Host(h) => DrawCommand::Host(HostCmd { alpha: h.alpha * a, ..h }),
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
            control_tone: None,
            content_glow: None,
            offset: (0.0, 0.0),
            opacity: 1.0,
            scale: 1.0,
            scale_origin: Point::new(0.0, 0.0),
            clip: None,
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

    /// Paint `f`'s subtree **scaled** by `factor` about `origin` — the whole surface smaller or
    /// larger, not a different layout.
    ///
    /// This is what an overview/exposé opens and closes with, and what a fade cannot express: a map
    /// that appears by *zooming out from life size* says "this is the same thing, further away",
    /// where a fade says "a different picture". niri animates its overview exactly this way.
    ///
    /// **It scales the picture, not the layout.** Nothing is measured again, no widget is told, and
    /// no bounds change — which is the point: the tree is laid out once, at life size, and a scale
    /// is something done *to* it, exactly as [`with_opacity`](Self::with_opacity) is. It therefore
    /// does not change hit-testing either; a surface mid-zoom is still where its bounds say.
    ///
    /// Multiplicative and nesting, like opacity, so a scaled panel inside a scaled layer draws at
    /// the product. Every command is scaled on its way out through [`emit`](Self::emit) — position
    /// **and** size, and with them the font size, the corner radius, the border width, the glow and
    /// the shadow. Scaling geometry while leaving those alone is what makes a naive zoom look
    /// wrong: text that stays huge in a shrinking box, hairlines that turn into slabs.
    pub fn with_scale(&mut self, factor: f32, origin: Point, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = (self.scale, self.scale_origin);
        // Compose about the outer transform's own frame, so nesting behaves.
        self.scale_origin = self.scaled_point(origin);
        self.scale = previous.0 * factor.max(0.0);
        f(self);
        (self.scale, self.scale_origin) = previous;
    }

    /// A point under the active scale.
    fn scaled_point(&self, p: Point) -> Point {
        if self.scale == 1.0 {
            return p;
        }
        let o = self.scale_origin;
        Point::new(
            o.x + (p.x - o.x) * self.scale as f64,
            o.y + (p.y - o.y) * self.scale as f64,
        )
    }

    /// A rect under the active scale: its position moves toward the origin and its size shrinks.
    fn scaled_rect(&self, r: Rectangle) -> Rectangle {
        if self.scale == 1.0 {
            return r;
        }
        Rectangle::new(
            self.scaled_point(r.loc),
            Size::new(
                r.size.w * self.scale as f64,
                r.size.h * self.scale as f64,
            ),
        )
    }

    /// Emit a command, scaled by the active [opacity](Self::with_opacity) and
    /// [scale](Self::with_scale). The one door to the scene, so a new draw helper cannot forget to
    /// honour either one — it never hears about them.
    fn emit(&mut self, cmd: DrawCommand) {
        let cmd = if self.scale == 1.0 {
            cmd
        } else {
            scale_command(cmd, self.scale, |r| self.scaled_rect(r))
        };
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
        // Remembered as well as emitted: the renderer's scissor cannot help a draw that goes into
        // the **overlay band**, which starts unclipped on purpose (`Scene::begin_overlay`). A
        // widget drawing there asks `clip()` and keeps itself inside by hand.
        let outer = self.clip.replace(match self.clip {
            Some(outer) => outer.intersection(rect).unwrap_or(Rectangle::new(
                rect.loc,
                Size::new(0.0, 0.0),
            )),
            None => rect,
        });
        f(self);
        self.clip = outer;
        self.scene.push(DrawCommand::PopClip);
    }

    /// **What is currently clipping this paint** — every open [`with_clip`](Self::with_clip)
    /// intersected, in the widget's own layout coordinates. `None` means nothing does.
    ///
    /// For a widget that draws into the overlay band, where the renderer's scissor does not reach:
    /// see [`paint_hint_label`](crate::widgets::paint_hint_label), which keeps a keycap inside the
    /// dock its row lives in rather than letting it fall on the frame.
    pub fn clip(&self) -> Option<Rectangle> {
        self.clip
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

    /// Paint the closure's subtree with `tone` as the **inherited control tone** — the *chrome*
    /// counterpart of [`with_content_color`](Self::with_content_color).
    ///
    /// Content colour is ink: it reaches text and glyphs. This reaches a control's own
    /// **chrome** — a [`Button`](crate::widgets::Button)'s border and hover fill, an
    /// [`IconButton`](crate::widgets::IconButton)'s hover tint — which is otherwise the theme
    /// accent and nothing else. A composition that has a hue of its own needs both: a
    /// notification card is severity-toned, and a `Retry` inside a *danger* card cannot be blue.
    ///
    /// **Why a second channel rather than widening the first**: `Item`, `Row`, `Choice`,
    /// `ContextMenu`, `CommandPalette` and `Select` all publish a content colour today, and every
    /// button composed inside one would have silently restyled. This one starts empty — nothing
    /// publishes it — so a control looks exactly as it did unless a container asks otherwise
    /// (F003/P096/T484).
    ///
    /// The tone is a **theme token resolved by the publisher at paint**, never a colour stored at
    /// build time, so a theme reload re-tones what is already on screen. Nesting restores the
    /// outer value on exit.
    pub fn with_control_tone(&mut self, tone: Color, f: impl FnOnce(&mut PaintCx<'a>)) {
        let previous = self.control_tone.replace(tone);
        f(self);
        self.control_tone = previous;
    }

    /// The inherited control tone, if a container published one via
    /// [`with_control_tone`](Self::with_control_tone). A control resolves its hue as: **its own
    /// explicit tone → this → the theme accent.**
    pub fn control_tone(&self) -> Option<Color> {
        self.control_tone
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
    /// **Place content something else rasterised** — a terminal, an image, a video, a plugin's own
    /// canvas — in `rect`.
    ///
    /// This widget says where; the host owns the texture and does the drawing. `id` is opaque here:
    /// nothing about textures, formats or devices crosses into this library. See
    /// [`HostDraw`](crate::scene::HostDraw).
    pub fn surface(&mut self, rect: Rectangle, id: u64) {
        if self.culled(rect) {
            return;
        }
        let rect = self.placed(rect);
        self.emit(DrawCommand::Host(HostCmd {
            draw: HostDraw::Surface { id },
            rect,
            alpha: 1.0,
        }));
    }

    /// **Blur whatever is already drawn behind this widget**, within `rect`.
    ///
    /// Recorded in scene order, so it blurs exactly what came before it and nothing of what comes
    /// after. `alpha` fades the blurred copy — a surface arriving fades its backdrop in with itself,
    /// rather than holding the session out of focus and snapping sharp in one frame at the end.
    pub fn backdrop_blur(&mut self, rect: Rectangle, radius: f32, alpha: f32) {
        if radius <= 0.0 || alpha <= 0.0 || self.culled(rect) {
            return;
        }
        let rect = self.placed(rect);
        self.emit(DrawCommand::Host(HostCmd {
            draw: HostDraw::Backdrop { radius },
            rect,
            alpha,
        }));
    }

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
