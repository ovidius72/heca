//! **What every widget holds**: the [`Base`] each one embeds.

use super::*;

mod methods;

/// State shared by every component. Concrete widgets embed this.
pub struct Base {
    /// Layout + visual style.
    pub style: Style,
    /// This component's node in the layout tree (set during layout).
    pub node: Option<taffy::NodeId>,
    /// Which retained layout tree [`node`](Self::node) belongs to, so a component moved in from
    /// another tree is not taken for a node of this one.
    pub(crate) layout_tree: Cell<u64>,
    /// What this tree remembers of its last layout — only a root holds one.
    pub(crate) layout_cache: RefCell<Option<Box<crate::layout::RetainedLayout>>>,
    /// Absolute bounds in logical pixels, filled in after layout.
    pub bounds: Rectangle,
    /// Whether this component is rendered.
    pub visible: Signal<bool>,
    /// Whether this component is disabled — dimmed, non-interactive, and skipped
    /// by focus traversal. A common, base-level property every widget inherits.
    pub disabled: Signal<bool>,
    /// Whether this component currently holds keyboard focus.
    ///
    /// Written only through the focus door ([`Base::focus`] / [`Base::blur`] and the signal a
    /// widget follows, [`Base::follow_focus`]); read it, never set it.
    pub focused: Signal<bool>,
    /// Who focused this widget last, and the host signal it follows, if any. See
    /// [`FocusDoor`](crate::focus::FocusDoor).
    pub(crate) focus_door: crate::focus::FocusDoor,
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
    /// Set by **controls that compose their content from children** — a
    /// [`Button`](crate::widgets::Button) is one click target and one Tab stop no matter what it
    /// holds. Without this, a focusable descendant (say a `Toggle` used as decoration) would take
    /// its own Tab stop while being click-dead, because the control consumes the press in its own
    /// `event` and never routes it to children. That mismatch — focusable but inert — is the bug
    /// this prevents.
    ///
    /// Honoured by [`focus`](crate::focus) traversal. Orthogonal to
    /// [`Layout::hidden`](crate::style::Layout::hidden), which removes a subtree from layout *and*
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
    /// tree position order. Set via
    /// [`LayoutExt::tab_index`](crate::builders::LayoutExt::tab_index).
    pub tab_index: Option<i32>,
    /// Child components, laid out by this component's flex container.
    pub children: Vec<Box<dyn Component>>,
    /// This widget **can be dragged**, and what is dragged is [`key`](Self::key) — the identity it
    /// already declares about itself. Universal opt-in via
    /// [`DragExt::draggable`](crate::builders::DragExt::draggable); resolved generically
    /// by [`drag::source_at`](crate::drag::source_at).
    ///
    /// It used to carry an opaque id handed out by a host registry, and a closed list of which
    /// surfaces were allowed to drag at all. Both are gone: a row said who it was twice, and a
    /// plugin's row could say it neither time — it could not be added to a list that is an enum in
    /// our source, so it could never be dragged. A widget with no `key` is not a drag source,
    /// because there would be nothing to name what was picked up.
    pub draggable: bool,
    /// **When a press may pick this widget up** — `None` is always. Set with
    /// [`DragExt::draggable_when`](crate::builders::DragExt::draggable_when). Asked when
    /// a drag would *start* only: one already in flight is never cancelled by the rule going false.
    pub drag_gate: Option<std::rc::Rc<dyn Fn(Modifiers) -> bool>>,
    /// **What this widget is, when it is dragged** — an opaque word its component chose
    /// (`"pane"`, `"column"`, `"docker.container"`). Set with
    /// [`DragExt::draggable_as`](crate::builders::DragExt::draggable_as); `None` means it
    /// says nothing about itself and every target takes it.
    ///
    /// It is deliberately a free string and not a list the library knows: a closed set is one a
    /// plugin cannot join, and the same shape a row already uses to say what its right-click menu
    /// is about.
    pub drag_kind: Option<String>,
    /// **Show the small chip under the pointer while it is carried**, instead of a picture of this
    /// widget. See [`DragExt::drag_chip`](crate::builders::DragExt::drag_chip).
    pub drag_chip: bool,
    /// This widget **accepts drops**, identified the same way — by its [`key`](Self::key).
    /// Universal opt-in via
    /// [`DragExt::drop_target`](crate::builders::DragExt::drop_target); resolved
    /// generically by [`drag::resolve_at`](crate::drag::resolve_at).
    pub drop_target: bool,
    /// **What this widget takes**, by the same words. Empty means it takes anything, which is what
    /// a target that says nothing gets. Set with
    /// [`DragExt::accepts`](crate::builders::DragExt::accepts).
    ///
    /// A target that refuses is not offered: the walk skips it and keeps looking outward, so
    /// nothing is drawn over something that would then do nothing. That mismatch is what this
    /// exists to end — the line said yes and the release said no, because the drawing and the rule
    /// lived in different places.
    pub accepts: Vec<String>,
    /// **Can a thing be dropped *onto* this widget, or only beside it?**
    ///
    /// `true` — the default — means the middle of the target is real: dropping a pane onto a pane
    /// swaps or moves it there, so the target reads as three bands (before it, onto it, after it).
    /// `false` says the target is a **sibling in an ordered list**, where "onto" means nothing: a
    /// column dropped on a column is a reorder and can only land before or after, so the target
    /// reads as two halves and the insertion line flips at the midpoint.
    ///
    /// Set by [`accepts`](crate::builders::DragExt::accepts) and
    /// [`accepts_beside`](crate::builders::DragExt::accepts_beside). It exists because the
    /// two are genuinely different and only the component knows which it is.
    pub accepts_onto: bool,
    /// When set, this widget is a **navigable row** carrying its own identity: the keyboard cursor,
    /// the right-click target and the drag are three readers of this one declaration.
    ///
    /// A string the component chose about *itself*, so it
    /// survives a tree rebuild. Universal opt-in via
    /// [`IdentityExt::key`](crate::builders::IdentityExt::key); enumerated by
    /// [`nav::collect_keys`](crate::nav::collect_keys) and hit-tested by
    /// [`nav::key_at`](crate::nav::key_at). Opaque here — nothing in this library parses it.
    pub key: Option<String>,

    /// **The grid area this node belongs in** — CSS `grid-area: title`, said by the child about
    /// itself rather than written into it by its parent.
    ///
    /// It lives here beside [`key`](Self::key) rather than in [`Style`] because a name is a
    /// `String` and `Layout` is `Copy`. Resolved during layout against the template of whichever
    /// grid turns out to hold it, so a child can be built before its parent exists — and a template
    /// change replaces no children.
    pub grid_area: Option<String>,

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
    /// Fixing the widgets one at a time does not end it: the next surface, or the next decorator
    /// over one, brings it back, and its author had no way to know they were meant to think about
    /// it.
    ///
    /// A surface that *wants* to swallow says so where it already says it —
    /// [`overlay_occludes`](Component::overlay_occludes), which `Overlay::blocking(true)` answers
    /// for the whole viewport. So the cost to an author is nothing, and the cost of forgetting is
    /// nothing.
    pub surface: bool,

    /// **How this component arrives and leaves** — every component, not only the panel-shaped ones.
    ///
    /// [`visible`](Base::visible) says whether it is on screen. This says whether it is *coming or
    /// going*, and it is the pair that makes [`show`](Component::show) /
    /// [`close`](Component::close) / [`toggle`](Component::toggle) mean something on a `Select`, a
    /// `Button`, a pane, a dock — anything — rather than only on the two widgets that happened to
    /// embed it.
    ///
    /// It lived on `Overlay` and on the toast card, which meant showing and closing was a
    /// *surface* capability: a `Dialog` had to hand-write five methods forwarding to the overlay it
    /// composed, a `ContextMenu` and a `CommandPalette` could not forward at all because they are
    /// their own panels, and anything anyone composed later had to discover the same thing and copy
    /// the same code. A rule a caller has to remember is the bug (AGENTS.md § 0 rule 2), so it is
    /// here, once, and nobody forwards anything.
    ///
    /// A component that declares no animation cuts — costing nothing and needing no special case.
    pub presence: crate::animation::Presence,

    /// **Is this component up?** — the flag [`show`](Component::show) and
    /// [`close`](Component::close) raise and lower, on every component.
    ///
    /// Distinct from [`visible`](Base::visible), and the two are not interchangeable: `visible` is
    /// "render me if my parent renders", while this is "this thing is up". An `Overlay` that is
    /// closed must stay **laid out** — its panel keeps its box so it can be measured, hit-tested
    /// and shown again without a reflow — so it cannot express being closed by not being visible.
    ///
    /// It was a private signal on `Overlay`, which is why showing and closing was a capability only
    /// overlay-shaped widgets had. Here, a `Select`, a `Button`, a pane and a dock are shown and
    /// closed by the same call, and a widget that composes another hands this one flag down instead
    /// of keeping a second copy in step.
    pub open: crate::reactive::Signal<bool>,

    /// **The seat's own name for this node**, written by a host when it places a surface and never
    /// by an author.
    ///
    /// It exists so that seating a surface stops destroying the author's [`key`](Base::key). The
    /// two are different things that were being kept in one field: `key` is what the *component*
    /// chose to call itself and survives a rebuild, while this is the slot the host puts it in so
    /// it can find it again. Stamping the slot over the key meant a surface that named itself
    /// — `Dialog::new("..").key("confirm.close")` — arrived on screen with that name erased, so
    /// nothing could point at it afterwards and `key` was a lie for exactly the widgets a plugin
    /// most needs to address.
    pub surface_slot: Option<String>,
    /// **Is what is behind this surface still reachable?** — `true` locks it.
    ///
    /// While a surface is locked, actions on whatever it stands in front of are refused. A dialog
    /// locks: it is asking a question, so nothing behind it should be touched until you answer. A
    /// map of the working area does **not** lock — it is drawn over everything, but its whole
    /// purpose is to let you choose one of the things behind it, and locking made it refuse every
    /// act on the very thing it exists to pick.
    ///
    /// ⚠️ **It is not about pixels**, which is what the old name (`covers_content`) kept implying.
    /// The map covers every pixel it draws over and still sets this `false`.
    ///
    /// ⚠️ Nor is it [`overlay_occludes`](Component::overlay_occludes), which is the *geometric*
    /// question the pointer asks: did this widget cover that point. Locking is about what may
    /// happen, not what was drawn.
    ///
    /// A named surface sets this for itself — a `Dialog` locks because it is a dialog. Reach for
    /// the builder on a raw [`Overlay`](crate::widgets::Overlay), which is the piece a new kind of
    /// surface is built from.
    pub lock: bool,
    /// **Does this surface take the keyboard while it is up?** — the coarse half of layering.
    ///
    /// **`None` means "ask the tree", and that is the answer you want.** A surface that wants keys
    /// *holds focus*: `Overlay`, `ContextMenu` and `CommandPalette` bind their open signal to
    /// [`focused`](Self::focused), which is the whole of how an open layer takes the keyboard. So
    /// an open overlay derives `true` and an ambient one — a toast stack, which holds no focus
    /// because nothing in it is typing — derives `false`, and **neither author writes anything**.
    ///
    /// `Some(v)` overrides that, for a surface whose keyboard story the framework cannot see. It is
    /// deliberately an override and not the default: an unconditional `true` here would make every
    /// ambient surface the active context, and a toast would suppress every letter behind it — the
    /// failure this file's `lock` note also describes.
    ///
    /// ⚠️ **Do not turn this into a plain `bool`.** `routes_own_subtree`, `takes_raw_keys` and
    /// `takes_text_input` were exactly that and were deleted: a second thing to set is a second
    /// thing to get wrong, and it disagrees silently with where the keys actually go.
    pub captures_keyboard: Option<bool>,
    /// **Which enclosing region this subtree belongs to** — a panel, a dock, a tab group, whatever
    /// the host calls the thing that holds rows. Universal opt-in via
    /// [`IdentityExt::scope_key`](crate::builders::IdentityExt::scope_key); hit-tested by
    /// [`focus::page_scope`](crate::focus::page_scope). Opaque here, exactly like `key`.
    ///
    /// A *separate* field rather than a flavour of `key` because the two answer different
    /// questions about the same point: `key` says which **row**, this says which **region
    /// containing rows**, and a host commonly wants both from one press. Folding them together
    /// would also make a region turn up in `collect_keys` as a steppable row, which it is not.
    pub scope_key: Option<String>,
    /// **This region holds the keyboard for everything inside it** — a list with its own cursor,
    /// where `Enter` means "the row the cursor is on", never "the row that was last clicked".
    ///
    /// A press inside it gives the keyboard to the region, not to the control under the pointer
    /// (the control still gets its click), and taking the keyboard into it never hands it on to a
    /// control inside. Set with [`FocusScope::keeps_keyboard`](crate::widgets::FocusScope::keeps_keyboard).
    pub keeps_keyboard: bool,
    /// Resolved font size in logical px, written by the layout pass: the widget's
    /// own `style.font_size` if it set one (> 0), otherwise the theme's base font.
    /// Widgets read **this** for text + size, so a global font flows in for free.
    pub font: f32,
    /// **The tree's own base font**, written by the layout pass — before any widget's size variant
    /// scales it.
    ///
    /// [`font`](Self::font) is what this widget draws its *content* at, and a size variant is meant
    /// to scale that. What a widget floats *beside* itself is different: a tooltip bubble is a
    /// small panel belonging to the surface, not a part of the control, so an emphasized button was
    /// getting an emphasized bubble — 1.25× the text of an identical tip on the button next to it
    pub root_font: f32,
    /// The **viewport the tree was laid out against**, written by the layout pass.
    ///
    /// A widget that draws a floating panel has to clamp it on screen, and it used to learn the
    /// viewport from `PaintCx` — one pass *after* the layout that placed the panel. So the first
    /// frame placed it against a stale size and the next one corrected it, and a context menu
    /// visibly jumped after it appeared. The engine already knows the size it was told to compute
    /// against; this is that size, available at the moment placement happens.
    pub viewport: Size,
    /// Hover, capture, click-run and drag state, kept by the pointer router (see
    /// [`crate::pointer`]). A widget reads `pointer.hovered`; nothing else here writes it.
    ///
    /// It lives on the widget rather than in a router the host owns, so several mounted trees
    /// never share a hover or a capture, and a widget dropped mid-gesture takes its own state with
    /// it instead of stranding a press somewhere.
    pub pointer: crate::pointer::PointerState,
    /// This widget's registered event handlers, keyed by [`EventKind`] — what
    /// [`EventExt`](crate::builders::EventExt) writes and [`dispatch`](fn@crate::dispatch) runs. `None` until
    /// the first one is registered, which is the common case and costs a null pointer.
    pub handlers: Option<Box<Handlers>>,
    /// What this widget does with the props its owner hands it — see
    /// [`crate::props`](mod@crate::props). `None` for a widget that takes none.
    pub props: Option<crate::props::PropsSlot>,
    /// Whether this widget **clips what it holds** to its own box — CSS `overflow: hidden`. A
    /// child that is placed or grows past the edge is not drawn there and cannot be hit there.
    pub clip_children: bool,
    /// Whether this widget **only arranges its children** — see [`Base::container`]. A layout-only
    /// widget claims the pointer on its own box only where it declares or paints something.
    pub layout_only: bool,
    /// The version of what this child was built from, when a parent that keeps its children
    /// across passes built it from one — see
    /// [`reconcile_keyed`](crate::reconcile::reconcile_keyed).
    pub built_from: Option<String>,
    /// What the cursor looks like while the pointer is over this widget — see
    /// [`cursor_at`](crate::cursor_at). `None` leaves it to what holds this widget.
    pub cursor: Option<crate::cursor::Cursor>,
    /// **The context menu this widget carries**, built fresh each time it is triggered.
    ///
    /// A universal slot like [`key`](Self::key) and [`draggable`](Self::draggable), so
    /// an `Icon`, a `Label` and a plugin's own widget carry one on the same terms as a `Row`. A
    /// right-click, or the host's `open_context_menu` action, walks **outwards** to the nearest
    /// widget that has one — see [`crate::menu`] for the whole model. Written with
    /// [`MenuExt::context_menu`](crate::builders::MenuExt::context_menu).
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
    pub context_menu: Option<Box<dyn Fn(heca_core::layout::Point) -> crate::widgets::ContextMenu>>,
    /// **What a leader-key pick does to this widget, when that differs from acting on it** —
    /// written with [`on_hint`](crate::builders::HintExt::on_hint), on any widget.
    ///
    /// It is the **override**, not the switch. Being pickable is not opt-in: anything actionable is
    /// offered a letter and picking it does what clicking it does. This says a
    /// pick does something *else* — heca's sidebar row activates the pane and leaves the sidebar on
    /// a click, and stays in the sidebar on a pick.
    ///
    /// There is no id to register and no registry to reach; the declaration lives on the widget and
    /// the collector reads it out of the laid-out tree.
    ///
    /// **The builder is on every widget**. It used to be on
    /// [`KeyHint`](crate::widgets::KeyHint) alone, which put the two facts about a target — who it
    /// is and what picking it does — on two different nodes, and which of them was on top depended
    /// on how the tree was built. That is why the matching code had to search both up and down, and
    /// why sidebar letters kept failing with no error. `KeyHint` stays as a decorator for a
    /// **region that is not a widget you can put a builder on**, and nothing else.
    pub hint: Option<crate::hint::Hint>,
    /// **Which pickers this target belongs to.** Empty — the default — means the ordinary one:
    /// `prefix+/`, the picker that letters everything actionable.
    ///
    /// A surface can have more than one verb over the same tree. The exposé's cards mean *go
    /// there* and their ⊠ icons mean *remove that*, and a picker that letters both at once gives
    /// you sixteen letters where you wanted eight — with the wrong half able to delete what you
    /// meant to jump to. So a target names the scopes it answers to and a picker names the one it
    /// letters, and each hands out letters for its own set.
    ///
    /// Written with [`hint_scope`](crate::builders::HintExt::hint_scope), one line on the
    /// widget beside the declaration it scopes:
    ///
    /// ```ignore
    /// IconButton::new(Glyph::X).on_hint(remove(id)).hint_scope(["close"])
    /// ```
    ///
    /// **A scoped target drops out of the ordinary picker**, which is the property that makes this
    /// usable for more than the exposé: a column in the workspaces dock is a *destination* for
    /// "move a pane to a column", not somewhere `prefix+/` should send you, and saying so is one
    /// line rather than a rule the picker has to carry about columns.
    ///
    /// A target may name several scopes and then belongs to each of those pickers.
    pub hint_scopes: Vec<String>,
    /// **The scope this node's own picker letters**, when it is one — set by
    /// [`KeyHintGroup::scope`](crate::widgets::KeyHintGroup::scope).
    ///
    /// It lives here rather than on the widget so a walk can see it without knowing what a
    /// `KeyHintGroup` is: a picker stops descending into a nested picker that letters a different
    /// scope, and that rule belongs to the walk, not to one widget's type.
    pub picker_scope: Option<String>,
    /// **What this widget says on hover.** `None` — the default — means it says nothing and costs
    /// nothing.
    ///
    /// Declared with [`tooltip`](crate::builders::TooltipExt::tooltip), on **every** widget, and
    /// the framework does the rest: the reveal is timed from the hover clock the pointer router
    /// already keeps, and the bubble is drawn in [`paint_child`] beside the hint letter and the
    /// drag feedback.
    ///
    /// It used to be a wrapper placed *around* a widget, so the rule lived in every caller's
    /// discipline — and a widget held in a typed container could not be wrapped at all without
    /// ceasing to be what the container accepts. The same move the pick declaration made when it
    /// left [`KeyHint`](crate::widgets::KeyHint) for every widget.
    pub tooltip: Option<crate::widgets::tooltip::Tip>,
    /// **May this widget be offered a letter at all?** `true` for everything, until a caller says
    /// otherwise with [`hintable(false)`](crate::builders::HintExt::hintable).
    ///
    /// It exists because being pickable is **not** opt-in: anything actionable gets a letter, so
    /// the only thing left to say is "not me". Setting it `true` does nothing — a widget nobody can
    /// act on has nothing for a letter to run.
    pub hintable: bool,
    /// **Can the user act on this widget — click it, or activate it from the keyboard?**
    ///
    /// Set by whichever builder wires the action up, whatever it is called: the generic
    /// [`on_click`](crate::builders::EventExt::on_click) /
    /// [`on_double_click`](crate::builders::EventExt::on_double_click) /
    /// [`on_key_down`](crate::builders::EventExt::on_key_down) /
    /// [`on_key_up`](crate::builders::EventExt::on_key_up), and the widgets that keep their own
    /// callback instead — `Button::on_click`, `Row::on_activate`, and the six like them.
    ///
    /// **It is a field and not a question asked of the handler list**, because the answer is not in
    /// the handler list. Eight widgets store their action in a private field of their own, so
    /// `Handlers::has(Click)` is `false` for a `Button` — the single case that matters most. And
    /// the two spellings (`on_click`, `on_activate`) mean the same thing, so no set of `EventKind`s
    /// names it either. One flag, set where the action is wired, is the only thing a walk over the
    /// tree can read.
    pub activatable: bool,
    /// **This widget exists only to carry the one inside it** — a wrapper, not a thing of its own.
    ///
    /// Set by [`wrap_transparently`], which every such wrapper already calls, so no widget opts in
    /// separately and a new one gets the answer by being written the same way.
    ///
    /// Read by the pick collector: a wrapper and what it wraps must not both spend a letter, so the
    /// chain of "these are layers of one thing" passes through a transparent node and stops at a
    /// real container. It used to be inferred from **child count** — a node holding exactly one
    /// child was assumed to be a wrapper — which made a real container's letter appear and vanish
    /// with the number of things inside it: the workspaces dock wore a letter with two workspaces
    /// and none with one. Counting children to tell what a node IS is the pattern this codebase
    /// forbids, and this is it removed from the one place it was left.
    pub transparent: bool,
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
    /// There is nothing host-side to teach about the plugin's layering.
    pub hint_label: Signal<Option<String>>,
    /// **How this widget's letter is drawn** — where it sits, its size, its colour.
    ///
    /// These were private fields on [`KeyHint`](crate::widgets::KeyHint), which is why a letter
    /// could only appear by wrapping a widget in one. Here, the framework draws the cap for any
    /// widget carrying a letter and each one places its own.
    pub hint_style: crate::widgets::HintStyle,
    /// **Actions this widget declares by name** — what a binding, a menu entry or a script can ask
    /// it to do.
    ///
    /// The counterpart of [`hint`](Self::hint): a hint says what a *pick* does to this region, an
    /// action says what a *named verb* does to it. Written with
    /// [`on_action`](crate::builders::EventExt::on_action).
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
    needs_layout: Cell<bool>,
}
