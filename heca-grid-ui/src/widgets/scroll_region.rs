//! [`ScrollRegion`] — an embeddable vertical scroll viewport.
//!
//! A scrollable column: children are laid out top-to-bottom at their natural
//! height (the layout engine never flex-shrinks them, so the column overflows),
//! and the visible window is the [`ScrollRegion`]'s own bounds. Content beyond
//! the viewport is clipped (renderer `PushClip`/`PopClip`).
//!
//! **Mechanism — same as the whole-page scroll.** The page scroll shifts the
//! tree's bounds by the scroll delta and lets the framebuffer clip the overflow.
//! This widget reuses that pattern for a sub-region: it bakes `-scroll_offset`
//! into its children's bounds (so paint, hit-testing, and DnD all see the
//! *visual* position — bounds === what's drawn) and clips to its own rect via
//! `PushClip` (a sub-region has no framebuffer, so it needs an explicit clip).
//! Because bounds always match the visual, pointer routing and the drag
//! framework's `source_at`/`resolve_at` (which hit-test against bounds) just
//! work while scrolled — no separate translation layer for DnD.
//!
//! **Layout reset.** Shifting bounds is destructive, so a fresh layout pass
//! (resize / font / content change) would compound the shift. The layout engine
//! calls [`Component::on_layout`] post-order after re-computing bounds; the
//! region resets its `applied_offset` there (children are back at natural), so
//! the next paint re-applies the shift from scratch instead of compounding.
//!
//! Interaction: the wheel (`Event::Scroll`) advances the offset (clamped to
//! `[0, max_offset]`), and the auto-shown scrollbar thumb is draggable. The
//! offset is also exposed as a reactive [`Signal<f32>`] the host can read or
//! drive directly.
//!
//! Scrolls **vertically by default**; opt into horizontal (or both) with
//! [`ScrollRegion::horizontal`] / [`both`](ScrollRegion::both) — plain wheel scrolls
//! vertically, `Shift`+wheel horizontally, and each overflowing axis grows its own
//! thumb (the vertical one on the right edge, the horizontal one on the bottom). Each
//! thumb is a theme-**accent** grip that brightens on hover/drag (mirroring
//! [`MarkerGroup`](crate::widgets::MarkerGroup)'s grip bar) and sits in a wider
//! invisible grab lane so a thin thumb is easy to click. Implements
//! [`StyleExt`](crate::builders::StyleExt), so it can double as a **scrollable
//! surface** (`.background(..).border(..)`). Nested regions compose: the wheel is
//! offered to children first, so the innermost hovered scrollable wins. A distinct
//! scrollbar color token and `PageUp`/`PageDown` (app actions) are future work.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{
    paint_child, shift_subtree, Base, Component, Event, Handled, PaintCx, WidgetIntent,
};
use crate::effects::Eased;
use crate::reactive::{signal, Signal, SignalGet, SignalUpdate};
use crate::style::{Direction, Length, Spacing};
use heca_core::layout::{Point, Rectangle, Size};

/// Visible scrollbar thumb width (logical px).
///
/// The *visible* thickness only. The space the region reserves beside its content is the **grab
/// lane** ([`THUMB_HIT_W`]), which is wider so a slim bar stays easy to hit — so this is the number
/// to turn to make the bar look thinner, and `THUMB_HIT_W` the one to turn if it is claiming too
/// much room. The clearance between bar and content is whatever is left over (11px today).
const SCROLLBAR_W: f64 = 5.0;
/// Gap between the scrollbar and the region's own **outer edge**.
///
/// Zero: the bar sits hard against the edge, and the whole gutter it reserves becomes clearance on
/// the content side. `SCROLLBAR_PAD` used to be applied on both sides, which pushed the bar inward
/// over the content while leaving a sliver of unused space outside it — the wrong way round, since
/// the only thing the bar needs distance from is what it might obscure.
const SCROLLBAR_EDGE_INSET: f64 = 0.0;
/// Grab lane width (logical px): the visible thumb is `SCROLLBAR_W`, but the
/// click/hover target is this wide (centered on the right edge) so a thin thumb
/// is still easy to grab — mirrors [`MarkerGroup`](crate::widgets::MarkerGroup)'s
/// `GRIP_W` invisible grab padding around its thin bar. Without it the 8px thumb
/// misses clicks too often.
const THUMB_HIT_W: f64 = 16.0;
/// Minimum thumb height so a very long list still has a grabbable thumb.
const MIN_THUMB: f64 = 24.0;
/// Space reserved along an edge for a visible scrollbar: **the whole grab lane**. Content is laid
/// out and clipped short of it, and the *perpendicular* track stops before it, so a bar never
/// overlaps content or the other bar in the corner.
///
/// It is [`THUMB_HIT_W`], not the visible thickness, because **a widget's hit area must stay inside
/// the box it reserved**. Reserving only the visible bar plus a 2px pad (7px) while grabbing across
/// 16px left 9px where one pixel belonged to two widgets: the lane, and the row it was painted over.
/// A host cannot arbitrate that — asking "who owns this pixel?" would couple every press path to
/// this widget's internals — and the consequence was concrete: dragging the scrollbar dragged the
/// row behind it, and the press order changed to avoid it stopped rows being draggable at all
/// (F003/P085/T368).
///
/// The cost is that content is 9px narrower beside a *visible* bar. Nothing is reserved when the
/// region does not overflow, so a list that fits keeps its full width.
const SCROLLBAR_GUTTER: f64 = THUMB_HIT_W;
/// Wheel step as a fraction of the viewport height per "line" of delta. The
/// winit wheel delta is already in lines, so one notch (delta ≈ 1) scrolls ~10%
/// of the viewport — gentle in a small sidebar, scales up for a tall one. (The
/// previous build multiplied by a fixed line count × font, which made each notch
/// jump ~75% of a small viewport and overshoot.)
const WHEEL_STEP_FRAC: f64 = 0.1;
/// How far one **keyboard page** moves, as a fraction of the viewport (F003/P011/T012).
///
/// Not a whole viewport: a sliver of overlap means the line you were reading is still on screen
/// after the jump, which is what every pager does and what makes paging through a list readable.
const PAGE_STEP_FRAC: f64 = 0.9;
/// Rest-glow spread radius (px) for a STYLED region (a scrollable panel) — its
/// share of the theme rest halo; a frameless region has no surface and no glow.
const SURFACE_GLOW_RADIUS: f32 = 12.0;
/// How far the content clip is widened on an axis this region does **not** scroll,
/// so a child's rest-glow halo (drawn outside the child's own bounds) isn't
/// scissored flat against the edge and read as a cut-off row. Sized to the surface
/// glow spread above.
const GLOW_BLEED: f64 = SURFACE_GLOW_RADIUS as f64;
/// Delay (seconds) before a held track-press starts repeating its paging.
const TRACK_REPEAT_DELAY: f32 = 0.35;
/// Interval (seconds) between repeated pages while the track press stays held.
const TRACK_REPEAT_INTERVAL: f32 = 0.1;
/// How long the wheel must be quiet before the gesture counts as finished (seconds).
///
/// A wheel reports movement and never reports stopping, so "it ended" can only be inferred from a
/// pause. Browsers settle `scrollend` the same way and land on roughly this long — long enough that a
/// slow scroll is one gesture, short enough to feel immediate.
const WHEEL_IDLE_END: f32 = 0.12;

/// A held press in a scrollbar **track** (off the thumb): the standard
/// press-and-hold affordance — page once immediately, then keep paging toward
/// the cursor until released (pausing when the thumb reaches it).
struct TrackRepeat {
    /// Which axis's track is held (`true` = the horizontal bar).
    horizontal: bool,
    /// Latest cursor position (updated by `PointerMoved` while held).
    pos: Point,
    /// Seconds until the next repeat fires (counts down in `tick`).
    next_in: f32,
}

/// Which axes a [`ScrollRegion`] scrolls. Default [`Vertical`](ScrollAxes::Vertical)
/// keeps every existing caller unchanged; opt into horizontal with
/// [`horizontal`](ScrollRegion::horizontal) / [`both`](ScrollRegion::both).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum ScrollAxes {
    /// Vertical only (the historical default).
    #[default]
    Vertical,
    /// Horizontal only.
    Horizontal,
    /// Both axes.
    Both,
}

impl ScrollAxes {
    /// Does this configuration scroll vertically?
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
    /// Does this configuration scroll horizontally?
    pub fn is_horizontal(self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }
}

/// Where a [`ScrollRegion`] puts the descendant it is following.
///
/// The default [`Minimal`](RevealAlign::Minimal) is scroll-into-view: move as little as possible,
/// and not at all when the target is already on screen. [`Center`](RevealAlign::Center) is the
/// typewriter behaviour — the thing you are on stays at the middle of the viewport and the
/// neighbours move past it — which is what a map or an overview wants, where the point is to see
/// what is *around* the current item rather than merely to keep it visible.
///
/// It applies to the automatic follow only. [`ensure_visible`](ScrollRegion::ensure_visible) is
/// minimal by definition and [`center_on`](ScrollRegion::center_on) is centring by definition; a
/// caller asking for one of those by name gets it whatever this says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, heca_grid_ui_macros::PropName)]
pub enum RevealAlign {
    /// Scroll the least that makes the target fully visible (the historical behaviour).
    #[default]
    Minimal,
    /// Keep the target at the centre of the viewport.
    Center,
}

/// Where a [`ScrollRegion`] is, and what put it there — the payload of
/// [`on_scroll_start`](ScrollRegion::on_scroll_start), [`on_scroll`](ScrollRegion::on_scroll) and
/// [`on_scroll_end`](ScrollRegion::on_scroll_end).
///
/// Everything a listener could ask the region is already here, so it never has to ask:
/// `offset_y == max_y` is "at the bottom", `content.h > viewport.h` is "scrollable at all". The
/// names line up with the ones the web uses for the same numbers — `offset_*` is `scrollTop` /
/// `scrollLeft`, `content` is `scrollHeight` / `scrollWidth`, `viewport` is `clientHeight` /
/// `clientWidth`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollInfo<'a> {
    /// Horizontal offset in content px, from the left.
    pub offset_x: f32,
    /// Vertical offset in content px, from the top.
    pub offset_y: f32,
    /// The largest horizontal offset this content allows (`0.0` when it fits).
    pub max_x: f32,
    /// The largest vertical offset this content allows (`0.0` when it fits).
    pub max_y: f32,
    /// The full size of the content being scrolled.
    pub content: Size,
    /// The visible window onto it.
    pub viewport: Size,
    /// The input that moved it — a wheel, a press, a drag — or **`None` when the host moved it
    /// itself** through [`scroll_to`](ScrollRegion::scroll_to) /
    /// [`ensure_visible`](ScrollRegion::ensure_visible).
    ///
    /// That distinction is the reason the real event is carried rather than a name for it: a
    /// listener that scrolls the region (following a cursor, syncing a second pane) reacts to
    /// `Some` and ignores `None`, and so cannot end up fighting its own call.
    pub event: Option<&'a Event>,
}

/// An embeddable scroll viewport hosting children that may overflow it.
///
/// **It drives itself.** The wheel, a thumb drag, a click in the track and a held press that keeps
/// paging are all handled inside the widget — a host mounts it and it behaves like a scroll area
/// anywhere else, with nothing to wire. What a host owes it is only what it owes any widget: the
/// pointer events it receives, and a `tick`. To *observe* the position, attach
/// [`on_scroll`](ScrollRegion::on_scroll); you never have to implement scrolling to watch it.
///
/// Build with [`ScrollRegion::new`], append children via [`Parent::child`], and
/// read/drive the position via [`ScrollRegion::scroll_offset`] /
/// [`ScrollRegion::scroll_to`] (vertical) and
/// [`scroll_offset_x`](ScrollRegion::scroll_offset_x) /
/// [`scroll_to_x`](ScrollRegion::scroll_to_x) (horizontal). A scrollbar appears
/// automatically on each axis whose content overflows the viewport.
///
/// **Axes.** Vertical by default (back-compat). [`horizontal`](Self::horizontal)
/// or [`both`](Self::both) opt into horizontal scrolling; the wheel scrolls the
/// vertical axis, `Shift`+wheel the horizontal one, and a 2-D trackpad delta
/// drives both. Only enabled axes shift/clip/scrollbar.
///
/// **Scrollable surface.** `ScrollRegion` implements [`StyleExt`], so a plain one
/// is frameless while `.background(..).border(..).radius(..)` makes it a framed,
/// scrollable panel — all values from the [`Theme`](crate::Theme), none hardcoded.
///
/// **Wheel gating.** `Event::Scroll` carries no position, so the default
/// broadcast router (`route_event`) can't hit-test it — an inline scroll region
/// would swallow *every* wheel event in the tree. To avoid that, the region
/// tracks whether the cursor is over it via `PointerMoved` and only consumes a
/// scroll when hovered (and scrollable). The wheel is offered to **children
/// first**, so with nested regions the *innermost* hovered scrollable wins (each
/// gates on its own hover) and an outer whole-page region only scrolls when no
/// descendant consumed the event.
pub struct ScrollRegion {
    base: Base,
    /// Which axes scroll (default [`ScrollAxes::Vertical`]).
    axes: ScrollAxes,
    /// Where the followed descendant is put (default [`RevealAlign::Minimal`]).
    reveal_align: RevealAlign,
    /// May the view move **past the ends of its content**? See [`overscroll`](Self::overscroll).
    overscroll: bool,
    /// Whether a scrollbar may appear at all. See [`scrollbars`](Self::scrollbars).
    scrollbars: bool,
    /// The gliding shift, one [`Eased`] per axis, or `None` for an instant jump (the default, and
    /// every caller's behaviour before this existed). See [`smooth_scroll`](Self::smooth_scroll).
    ///
    /// Two independent values rather than one 2-D one: they share a time constant but not a
    /// distance, so a long horizontal move must not drag the short vertical one out with it.
    eased: Option<(Eased, Eased)>,
    /// Whether the pair above has been seeded from the first real layout yet. The first sync
    /// **snaps** — a surface opens where it belongs rather than gliding in from the corner.
    eased_seeded: bool,
    /// Vertical scroll offset (content px shifted up). 0 = top.
    scroll_offset: Signal<f32>,
    /// Horizontal scroll offset (content px shifted left). 0 = left. Inert unless
    /// [`axes`](Self::axes) includes horizontal.
    scroll_offset_x: Signal<f32>,
    /// The vertical shift currently baked into the children's bounds (=
    /// `scroll_offset` at the last [`sync_shift`](Self::sync_shift)). Bounds hold
    /// `natural - applied_offset`; geometry helpers recover natural as `bounds +
    /// applied_offset`. Reset to 0 by [`on_layout`](Component::on_layout) when
    /// layout re-computes bounds to natural.
    applied_offset: f64,
    /// Horizontal counterpart of [`applied_offset`](Self::applied_offset).
    applied_offset_x: f64,
    /// While dragging the vertical thumb: the y-offset (content px) from the
    /// thumb's top where the grab landed, so the grab point stays under the
    /// cursor. `None` when not dragging.
    thumb_grab: Option<f64>,
    /// While dragging the horizontal thumb: the x-offset from the thumb's left.
    h_thumb_grab: Option<f64>,
    /// Host-owned: whether this region takes keyboard scroll intents. See
    /// [`keyboard_target`](ScrollRegion::keyboard_target). `None` ⇒ it does.
    keyboard_target: Option<Signal<bool>>,
    /// Whether the cursor is over the vertical scrollbar thumb's grab lane. Drives
    /// the hover affordance (the thumb brightens, like [`MarkerGroup`](crate::widgets::MarkerGroup)'s
    /// grip bar).
    thumb_hovered: bool,
    /// Whether the cursor is over the horizontal scrollbar thumb's grab lane.
    h_thumb_hovered: bool,
    /// A held track-press currently auto-repeating its paging (see [`TrackRepeat`]);
    /// `None` when no track press is held.
    track_repeat: Option<TrackRepeat>,
    /// Optional listeners. Purely observers: the region scrolls itself whether or not any is set.
    on_scroll_start: Option<ScrollListener>,
    on_scroll: Option<ScrollListener>,
    on_scroll_end: Option<ScrollListener>,
    /// Whether a scroll gesture is currently in flight (between start and end).
    scrolling: bool,
    /// The **natural** (unscrolled) top-left of the descendant last brought into view.
    ///
    /// Natural rather than on-screen on purpose: it changes when the *selection moves* and stays
    /// put when the *user scrolls*, which is exactly the difference between "follow the cursor" and
    /// "fight the wheel".
    ///
    /// Both coordinates, because a horizontal region's cursor moves **sideways**: remembering only
    /// the top, a strip of cards would decide nothing had changed and never follow at all.
    last_revealed: Option<(f64, f64)>,
    /// The **natural rect** of whatever last asked to be revealed — what [`center_pad`] keeps
    /// centring while nothing is asking.
    ///
    /// A subtree can stop asking without anything having moved: a cursor-following grid withholds
    /// the reveal while the pointer is driving it (`Component::reveals_subtree`), because pointing
    /// at a card must not scroll it. Without this the pad would read "nothing to follow" and fall
    /// back to centring the *content as a whole* — so hovering would slide the map anyway, just to
    /// a different place. Holding the last target means the picture simply stays where the keyboard
    /// left it (F003/P082/T418).
    last_reveal_rect: Option<Rectangle>,
    /// Seconds left before an idle wheel gesture is declared over, or `None` when nothing is
    /// waiting. A wheel has no release, so its end is a **silence**, not an event — see
    /// [`WHEEL_IDLE_END`].
    wheel_idle: Option<f32>,
}

/// A position listener. The higher-ranked bound is what lets [`ScrollInfo`] borrow the event that
/// caused the move instead of copying a description of it.
type ScrollListener = Box<dyn for<'a> Fn(ScrollInfo<'a>)>;

#[heca_grid_ui_macros::props]
impl ScrollRegion {
    /// A new vertical scroll region.
    pub fn new() -> Self {
        let mut base = Base::new();
        // Deliberately NOT focusable / not a tab-stop and it binds NO keys: heca is
        // tmux-style, so plain keys belong to the underlying app (terminal/editor).
        // Keyboard scrolling is driven by the HOST through prefix-gated, configurable
        // scroll actions that call `scroll_to`/`scroll_by`/`ensure_visible` — never by
        // the widget swallowing raw keys. See docs/overlay-design.md (scroll actions).
        base.style.layout.direction = Direction::Column;
        // A scroll viewport must be allowed to be SMALLER than its content — that
        // is the whole point of it. Flexbox defaults fight this twice: a flex item's
        // `min-height` is `auto` (= its content size) and this crate sets
        // `flex_shrink: 0` so explicit widget sizes are never squished. Left at the
        // defaults, a region inside a bounded parent (a sized `Dialog` panel) grows
        // to its content and **overflows the panel instead of scrolling** — no
        // overflow, so no scrollbar. Opting out of both here means a region scrolls
        // wherever it is put, without every caller having to know this.
        base.style.layout.min_width = Some(Length::Px(0.0));
        base.style.layout.min_height = Some(Length::Px(0.0));
        base.style.layout.flex_shrink = Some(1.0);
        // Breathing room so the first and last rows don't sit flush against the
        // clip edge, and a real gap between children — rows that touch are hard to
        // scan. Both are theme SPACING TOKENS, not literals, so they scale with the
        // font, the size variant and UI zoom (a px value tuned at one font size is
        // wrong at every other). A caller can still override either.
        base.style.layout.padding_y = Some(Spacing::Sm.into());
        base.style.layout.gap = Spacing::Md.into();
        Self {
            base,
            axes: ScrollAxes::default(),
            reveal_align: RevealAlign::default(),
            overscroll: false,
            scrollbars: true,
            eased: None,
            eased_seeded: false,
            scroll_offset: signal(0.0),
            scroll_offset_x: signal(0.0),
            applied_offset: 0.0,
            applied_offset_x: 0.0,
            thumb_grab: None,
            h_thumb_grab: None,
            keyboard_target: None,
            thumb_hovered: false,
            h_thumb_hovered: false,
            track_repeat: None,
            on_scroll_start: None,
            on_scroll: None,
            on_scroll_end: None,
            scrolling: false,
            last_revealed: None,
            last_reveal_rect: None,
            wheel_idle: None,
        }
    }

    /// Called when scrolling **begins** — the first movement of a gesture, or of a run of
    /// programmatic scrolls.
    ///
    /// All three listeners are optional and none of them drive anything: a host that just wants a
    /// working scroll area attaches nothing at all and gets the wheel, the thumb, the track click
    /// and the click-and-hold regardless.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_scroll_start(mut self, f: impl for<'a> Fn(ScrollInfo<'a>) + 'static) -> Self {
        self.on_scroll_start = Some(Box::new(f));
        self
    }

    /// Called on **every** position change, with the full [`ScrollInfo`].
    ///
    /// This fires as fast as the input arrives — once per wheel notch, once per pointer move during
    /// a thumb drag. Do cheap things here and put anything expensive in
    /// [`on_scroll_end`](Self::on_scroll_end).
    ///
    /// ```ignore
    /// ScrollRegion::new().both()
    ///     .on_scroll(|s| gutter.set(s.offset_y / s.max_y))          // cheap, every frame
    ///     .on_scroll_end(|s| {
    ///         // `event` is None when our own `scroll_to` moved it — don't react to ourselves.
    ///         if s.event.is_some() && s.offset_y == s.max_y { load_more(); }
    ///     })
    /// ```
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_scroll(mut self, f: impl for<'a> Fn(ScrollInfo<'a>) + 'static) -> Self {
        self.on_scroll = Some(Box::new(f));
        self
    }

    /// Called when scrolling **settles**: the release that ends a thumb drag or a held track press,
    /// or [`WHEEL_IDLE_END`] seconds after the last wheel movement.
    ///
    /// A wheel gesture has no release — the browsers' `scrollend` waits for a pause for the same
    /// reason — so this one is a timer, and the region keeps asking for frames until it fires.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_scroll_end(mut self, f: impl for<'a> Fn(ScrollInfo<'a>) + 'static) -> Self {
        self.on_scroll_end = Some(Box::new(f));
        self
    }

    /// Scroll horizontally only (children overflow left↔right).
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn horizontal(mut self) -> Self {
        self.axes = ScrollAxes::Horizontal;
        self
    }

    /// Scroll on both axes.
    #[heca_grid_ui_macros::host_only("carries no value — a property needs one; the equivalent is an explicit setting")]
    pub fn both(mut self) -> Self {
        self.axes = ScrollAxes::Both;
        self
    }

    /// Which axes this region scrolls.
    pub fn clone_axes(&self) -> ScrollAxes {
        self.axes
    }

    /// Set the scrolling axes explicitly (default [`ScrollAxes::Vertical`]).
    #[heca_grid_ui_macros::prop]
    pub fn axes(mut self, axes: ScrollAxes) -> Self {
        self.axes = axes;
        self
    }

    /// Where the followed descendant is put (default [`RevealAlign::Minimal`]).
    #[heca_grid_ui_macros::prop]
    pub fn reveal_align(mut self, align: RevealAlign) -> Self {
        self.reveal_align = align;
        self
    }

    /// How this region places the descendant it follows.
    pub fn clone_reveal_align(&self) -> RevealAlign {
        self.reveal_align
    }

    /// Let the view move **past the ends of its content**, so a centred target reaches the middle
    /// even when there is nothing on one side of it to fill the space.
    ///
    /// This is the difference between an axis that has ends and one that does not. A stack of
    /// workspaces has a top and a bottom: the first one rests against the top edge, because half a
    /// screen of nothing above it is not information. A strip of columns is a ribbon that runs off
    /// both sides: the leftmost column still comes to the centre when you focus it, with empty
    /// space to its left, because "the focused one is in the middle" is the whole grammar of the
    /// surface and an exception at the ends breaks it.
    ///
    /// Off by default — every list keeps stopping at its content, which is what a list should do.
    /// Only meaningful together with [`RevealAlign::Center`].
    #[heca_grid_ui_macros::prop]
    pub fn overscroll(mut self, on: bool) -> Self {
        self.overscroll = on;
        self
    }

    /// Whether the view may move past the ends of its content.
    pub fn clone_overscroll(&self) -> bool {
        self.overscroll
    }

    /// Whether a scrollbar may appear (default `true`).
    ///
    /// Turning it off removes the thumb **and the gutter it reserves**, so the content gets the
    /// full width back rather than a bar-shaped strip of nothing. Both, because reserving room for
    /// a bar that never draws is the same bug as drawing one nobody wants.
    ///
    /// For a surface where the bar carries no information: a map positioned by where the cursor is
    /// has no "how far down am I" to report — the picture already says it — and a bar across it
    /// reads as a divider between things that are not divided.
    #[heca_grid_ui_macros::prop]
    pub fn scrollbars(mut self, on: bool) -> Self {
        self.scrollbars = on;
        self
    }

    /// Whether a scrollbar may appear.
    pub fn clone_scrollbars(&self) -> bool {
        self.scrollbars
    }

    /// Ease the view to where it is going over `seconds`, instead of jumping there.
    ///
    /// It smooths **where the content sits**, which is the scroll offset *and* the centring pad
    /// together — animating only the offset would leave the pad snapping underneath it, and the
    /// two would visibly disagree on a surface using both.
    ///
    /// A wheel notch and a thumb drag are **never** eased: they have to track the input one to one
    /// or the region feels like it is lagging behind the finger. What eases is the view moving on
    /// its own — following a cursor, a programmatic `scroll_to`.
    ///
    /// `0.0` or less turns it off, so a config value of zero means "instant" rather than "divide
    /// by zero". Off by default: every existing list keeps jumping exactly as it did.
    #[heca_grid_ui_macros::prop]
    pub fn smooth_scroll(mut self, seconds: f32) -> Self {
        self.eased = (seconds > 0.0).then(|| (Eased::new(seconds), Eased::new(seconds)));
        self.eased_seeded = false;
        self
    }

    /// Whether the view glides to where it is going rather than jumping.
    pub fn clone_smooth_scroll(&self) -> bool {
        self.eased.is_some()
    }

    /// The reactive vertical scroll offset (content px). Read or drive it from the
    /// host: `region.scroll_offset().get_untracked()` / `.set(v)`. Use
    /// [`scroll_to`](Self::scroll_to) to set with clamping.
    pub fn scroll_offset(&self) -> Signal<f32> {
        self.scroll_offset
    }

    /// The reactive **horizontal** scroll offset (content px). Only meaningful when
    /// [`axes`](Self::axes) includes horizontal. Use [`scroll_to_x`](Self::scroll_to_x)
    /// to set with clamping.
    pub fn scroll_offset_x(&self) -> Signal<f32> {
        self.scroll_offset_x
    }

    /// Set the horizontal scroll offset, clamped to `[0, max_offset_x]`, bake it
    /// into the children's bounds, and request a repaint. Returns the clamped value.
    pub fn scroll_to_x(&mut self, offset: f32) -> f32 {
        self.set_offset_x(offset, None)
    }

    /// Set the scroll offset, clamped to `[0, max_offset]`, bake it into the
    /// children's bounds immediately, and request a repaint. Returns the clamped
    /// value actually applied. Prefer this over raw `scroll_offset().set()` —
    /// it keeps the shifted bounds (used for paint, hit-testing, and DnD) in
    /// sync with the offset in the same call.
    pub fn scroll_to(&mut self, offset: f32) -> f32 {
        self.set_offset_y(offset, None)
    }

    /// The **only** way the vertical offset moves: clamp, bake the shift, report.
    ///
    /// Every path funnels through here — wheel, thumb drag, held track press, scroll-into-view and
    /// the public API — so a listener cannot miss a movement, and a new way to scroll cannot forget
    /// to announce itself. `cause` is the input that did it, or `None` when the host did.
    fn set_offset_y(&mut self, offset: f32, cause: Option<&Event>) -> f32 {
        let (min, max) = self.offset_bounds_y();
        let v = offset.clamp(min as f32, max as f32);
        let changed = v != self.scroll_offset.get_untracked();
        self.scroll_offset.set(v);
        self.snap_if_driven(cause);
        self.sync_shift();
        if changed {
            self.report_scroll(cause);
        }
        v
    }

    /// The horizontal counterpart of [`set_offset_y`](Self::set_offset_y).
    fn set_offset_x(&mut self, offset: f32, cause: Option<&Event>) -> f32 {
        let (min, max) = self.offset_bounds_x();
        let v = offset.clamp(min as f32, max as f32);
        let changed = v != self.scroll_offset_x.get_untracked();
        self.scroll_offset_x.set(v);
        self.snap_if_driven(cause);
        self.sync_shift();
        if changed {
            self.report_scroll(cause);
        }
        v
    }

    /// A movement the user is driving **with the pointer** lands immediately: a thumb drag has to
    /// track the finger one to one, and a bar that lags it by even a few frames feels broken.
    ///
    /// A **wheel notch is not that**. It is a discrete step — a tenth of a viewport at a time — so
    /// applying it instantly is a jump, and a run of them is a run of jumps. Eased, the same notches
    /// read as one continuous movement. So the wheel is deliberately *not* snapped here even though
    /// it is user input: what has to track the input is the thing under the finger, and a wheel has
    /// nothing under it.
    fn snap_if_driven(&mut self, cause: Option<&Event>) {
        if !matches!(
            cause,
            Some(Event::PointerMove(_) | Event::PointerDown(_))
        ) {
            return;
        }
        let (x, y) = self.desired_shift();
        if let Some((ex, ey)) = self.eased.as_mut() {
            ex.snap(x);
            ey.snap(y);
        }
    }

    /// Report a movement: `scroll_start` if this began one, then `scroll` — and arm the idle timer
    /// that ends a wheel gesture, which has no release to end it.
    fn report_scroll(&mut self, cause: Option<&Event>) {
        if !self.scrolling {
            self.scrolling = true;
            self.fire(self.on_scroll_start.as_deref(), cause);
        }
        // A gesture with a release ends on that release; a wheel ends on silence.
        self.wheel_idle = matches!(cause, Some(Event::Scroll(_)) | None)
            .then_some(WHEEL_IDLE_END);
        self.fire(self.on_scroll.as_deref(), cause);
    }

    /// Declare the gesture over and report it once. No-op when nothing was in flight.
    /// Whether this region should take keyboard scroll intents right now.
    ///
    /// Host-owned, because *which* surface has the keyboard is the host's business and changes
    /// without the tree being rebuilt. `None` means "yes" — a region with no host wiring behaves as
    /// it always did, which is what a single-region app or an example wants. A host with several
    /// regions in one tree binds the flag on **each** of them, so nothing depends on that default.
    ///
    /// This is what makes "scroll the focused surface" work without the host having to know where
    /// the region sits in the tree (F003/P011/T012): the intent is dispatched into the whole tree
    /// and every region that is not the target declines.
    /// It binds [`Base::focused`] too, which is how the intent gets here at all: keyboard events
    /// are delivered to the focus owner and the region it encloses, so a region that says the
    /// keyboard is aimed at it *is* the owner, and one that says otherwise is not on the path.
    #[heca_grid_ui_macros::host_only("bound to a live host signal, which static data cannot drive")]
    pub fn keyboard_target(mut self, focused: Signal<bool>) -> Self {
        self.keyboard_target = Some(focused);
        self.base.focused = focused;
        self
    }

    /// Act on a keyboard scroll intent, or decline it.
    ///
    /// Declines (`Handled::No`) when the intent is not a scroll one, or when this region cannot
    /// scroll that axis at all — either the axis is disabled or the content fits. Declining is what
    /// lets a nested region, or the host, get a turn instead of the key dying here.
    fn scroll_intent(&mut self, intent: WidgetIntent, cause: &Event) -> Handled {
        // Not the keyboard's target → not ours, so the event carries on to whichever region is.
        if let Some(focused) = self.keyboard_target
            && !focused.get_untracked()
        {
            return Handled::No;
        }
        let vp = self.base.bounds;
        let (vertical, target) = match intent {
            WidgetIntent::ScrollPageUp => (
                true,
                self.scroll_offset.get_untracked() as f64 - PAGE_STEP_FRAC * vp.size.h,
            ),
            WidgetIntent::ScrollPageDown => (
                true,
                self.scroll_offset.get_untracked() as f64 + PAGE_STEP_FRAC * vp.size.h,
            ),
            WidgetIntent::ScrollToTop => (true, 0.0),
            // Past the end on purpose: `set_offset_y` clamps, so this lands exactly on the last
            // scrollable pixel without this code having to know where that is.
            WidgetIntent::ScrollToBottom => (true, f64::MAX),
            WidgetIntent::ScrollPageLeft => (
                false,
                self.scroll_offset_x.get_untracked() as f64 - PAGE_STEP_FRAC * vp.size.w,
            ),
            WidgetIntent::ScrollPageRight => (
                false,
                self.scroll_offset_x.get_untracked() as f64 + PAGE_STEP_FRAC * vp.size.w,
            ),
            WidgetIntent::ScrollToLeftEdge => (false, 0.0),
            WidgetIntent::ScrollToRightEdge => (false, f64::MAX),
            _ => return Handled::No,
        };
        let room = if vertical {
            self.max_offset()
        } else {
            self.max_offset_x()
        };
        if room <= 0.0 {
            return Handled::No;
        }
        let target = target.clamp(0.0, room) as f32;
        if vertical {
            self.set_offset_y(target, Some(cause));
        } else {
            self.set_offset_x(target, Some(cause));
        }
        self.end_scroll(Some(cause));
        Handled::Yes
    }

    fn end_scroll(&mut self, cause: Option<&Event>) {
        if !self.scrolling {
            return;
        }
        self.scrolling = false;
        self.wheel_idle = None;
        self.fire(self.on_scroll_end.as_deref(), cause);
    }

    /// Hand one listener the current position. Silent when nobody is listening — the region drives
    /// itself either way, which is the whole point of these being optional.
    fn fire(&self, listener: Option<&dyn for<'a> Fn(ScrollInfo<'a>)>, cause: Option<&Event>) {
        let Some(listener) = listener else { return };
        listener(ScrollInfo {
            offset_x: self.scroll_offset_x.get_untracked(),
            offset_y: self.scroll_offset.get_untracked(),
            max_x: self.max_offset_x() as f32,
            max_y: self.max_offset() as f32,
            content: Size::new(self.content_extent_x(), self.content_extent()),
            viewport: self.base.bounds.size,
            event: cause,
        });
    }

    /// Scroll by `delta` content px (signed: positive = down), clamped to
    /// `[0, max_offset]`. The wheel and click-track paging go through here; host
    /// scroll actions may call it too.
    fn scroll_by(&mut self, delta: f64, cause: Option<&Event>) {
        let next = self.scroll_offset.get_untracked() as f64 + delta;
        self.set_offset_y(next as f32, cause);
    }

    /// End any in-flight thumb drag or held track press. `true` if one was in flight.
    ///
    /// The single definition of "the gesture is over", so the release path and the stale-grab
    /// recovery on the next press cannot drift apart.
    fn release_grabs(&mut self) -> bool {
        let ended = self.thumb_grab.take().is_some()
            | self.h_thumb_grab.take().is_some()
            | self.track_repeat.take().is_some();
        if ended {
            // The thumb may go from bright to rest if the cursor is no longer over the lane.
            self.base.mark_needs_paint();
        }
        ended
    }

    /// One track-paging step **toward `pos`** on the given axis: a screenful in
    /// the cursor's direction relative to the thumb, and a no-op once the thumb
    /// has reached the cursor (the classic pause-under-the-pointer behavior).
    /// Shared by the initial track press and each held-press repeat.
    fn page_toward(&mut self, horizontal: bool, pos: Point, cause: Option<&Event>) {
        if horizontal {
            if let Some(thumb) = self.h_thumb_rect() {
                let dir = if pos.x < thumb.loc.x {
                    -1.0
                } else if pos.x > thumb.loc.x + thumb.size.w {
                    1.0
                } else {
                    return; // thumb reached the cursor — pause
                };
                let next = self.scroll_offset_x.get_untracked() as f64
                    + dir * self.base.bounds.size.w;
                self.set_offset_x(next as f32, cause);
            }
        } else if let Some(thumb) = self.thumb_rect() {
            let dir = if pos.y < thumb.loc.y {
                -1.0
            } else if pos.y > thumb.loc.y + thumb.size.h {
                1.0
            } else {
                return; // thumb reached the cursor — pause
            };
            self.scroll_by(dir * self.base.bounds.size.h, cause);
        }
    }

    /// Scroll minimally so the given rect — read from a descendant's current
    /// `bounds` (visual/on-screen space) — is fully inside the viewport. If the
    /// rect is already visible, nothing happens; if it sits above the viewport,
    /// the view scrolls to put its top at the viewport top; if below, its
    /// bottom at the viewport bottom. This is the **scroll-into-view** a host
    /// container (e.g. the sidebar) uses to keep the keyboard cursor in view when
    /// the selection moves: pass the selected descendant's `base().bounds`.
    ///
    /// The rect is in *visual* space (what you read from a component's bounds at
    /// the current scroll position); the widget recovers the natural position
    /// internally via its baked shift (`applied_offset`), so the host never has
    /// to track the scroll offset or do offset math itself. Minimal movement —
    /// it won't jump if the item is already on screen.
    pub fn ensure_visible(&mut self, visual_rect: Rectangle) {
        self.sync_shift();
        let vp = self.base.bounds;
        let off = self.applied_offset;
        let natural_top = visual_rect.loc.y + off;
        let natural_bot = natural_top + visual_rect.size.h;
        let cur = self.scroll_offset.get_untracked() as f64;
        let vp_top = vp.loc.y;
        let vp_bot = vp.loc.y + vp.size.h;
        if cur > natural_top - vp_top {
            // Item's top is above the viewport — scroll up to align tops.
            self.scroll_to((natural_top - vp_top) as f32);
        } else if cur < natural_bot - vp_bot {
            // Item's bottom is below the viewport — scroll down to align bottoms.
            self.scroll_to((natural_bot - vp_bot) as f32);
        }
        // else already fully visible — no scroll.
    }

    /// Scroll so the given rect — read from a descendant's current `bounds`, exactly as
    /// [`ensure_visible`](Self::ensure_visible) takes it — sits at the **centre** of the viewport.
    ///
    /// Unconditional, unlike `ensure_visible`: a target already on screen but off-centre is brought
    /// back to the middle, because "keep it centred" is the whole behaviour. The clamp inside
    /// [`scroll_to`](Self::scroll_to) does the rest, so at the ends of the content the target simply
    /// stops short of the centre rather than the view scrolling into empty space. A caller that
    /// wants it centred at the ends too gives the content slack of its own — half a viewport of
    /// padding at each end is the usual way, and it is the caller's because only the caller knows
    /// whether that empty space belongs in its surface.
    /// **Both axes**, unlike [`ensure_visible`](Self::ensure_visible), which is vertical only: a
    /// horizontal region centres horizontally, and a two-axis one does both. A strip of cards is
    /// the case that needs it — "the one you are on sits in the middle" is the same sentence
    /// whichever way the cards are laid out.
    pub fn center_on(&mut self, visual_rect: Rectangle) {
        self.sync_shift();
        let vp = self.base.bounds;
        if self.axes.is_vertical() {
            let natural_top = visual_rect.loc.y + self.applied_offset;
            let mid = natural_top + visual_rect.size.h / 2.0;
            self.scroll_to((mid - vp.loc.y - vp.size.h / 2.0) as f32);
        }
        if self.axes.is_horizontal() {
            let natural_left = visual_rect.loc.x + self.applied_offset_x;
            let mid = natural_left + visual_rect.size.w / 2.0;
            self.scroll_to_x((mid - vp.loc.x - vp.size.w / 2.0) as f32);
        }
    }

    /// Convenience: scroll so the direct child at `index` is fully visible. Use
    /// this for a list whose selectable units are direct children (e.g. a flat
    /// list of `Item`s). For a nested selectable unit (a sidebar row inside a
    /// `DockFrame`/group), use [`ensure_visible`](Self::ensure_visible) with the
    /// descendant's `bounds` instead. Out-of-range index is a no-op.
    pub fn scroll_to_child(&mut self, index: usize) {
        if let Some(child) = self.base.children.get(index) {
            let rect = child.base().bounds;
            self.ensure_visible(rect);
        }
    }

    /// Total content extent along the scroll axis (max child **natural** bottom
    /// relative to this region's top, never less than the viewport height).
    /// Uses `+ applied_offset` to recover natural positions from the shifted
    /// bounds.
    fn content_extent(&self) -> f64 {
        let vp = self.base.bounds;
        let off = self.applied_offset;
        let mut max_bottom = vp.loc.y + vp.size.h;
        for c in &self.base.children {
            let b = c.base().bounds;
            // natural bottom = shifted bottom + applied shift.
            let bottom = b.loc.y + b.size.h + off;
            if bottom > max_bottom {
                max_bottom = bottom;
            }
        }
        (max_bottom - vp.loc.y).max(vp.size.h)
    }

    /// The **raw** content extent along each axis — what the children actually occupy, with no
    /// floor at the viewport. [`content_extent`](Self::content_extent) clamps to the viewport
    /// because that is what the scroll maths wants; centring needs the unclamped truth, since the
    /// whole question is *how much smaller than the viewport the content is*.
    fn raw_extent(&self) -> (f64, f64) {
        let vp = self.base.bounds;
        let (mut right, mut bottom) = (vp.loc.x, vp.loc.y);
        for c in &self.base.children {
            let b = c.base().bounds;
            right = right.max(b.loc.x + b.size.w + self.applied_offset_x);
            bottom = bottom.max(b.loc.y + b.size.h + self.applied_offset);
        }
        (right - vp.loc.x, bottom - vp.loc.y)
    }

    /// How far to push the content down and right so it sits in the **middle of a region it does
    /// not fill** — `(0, 0)` unless [`reveal_align`](Self::reveal_align) is
    /// [`Center`](RevealAlign::Center) and there is leftover room on that axis.
    ///
    /// This is the other half of what "centred" means, and the half that has nothing to do with
    /// scrolling. `center_on` can only ever put the target in the middle by *moving the content
    /// past it*; when the content is smaller than the viewport there is nothing to move — the
    /// offset clamps to `0` and everything lands hard against the top-left corner. A map of one
    /// workspace, or a strip of three columns on a wide screen, is exactly that case, and pinning
    /// it to the corner is what an overview must never look like.
    ///
    /// **It centres the cursor, not the box.** When something in the subtree asks to be revealed,
    /// the pad puts *that* in the middle, so focusing the leftmost of three columns brings it to
    /// the centre and pushes its neighbours off the right edge — the same picture scrolling gives
    /// when the content is larger. Only with nothing to follow does it centre the content as a
    /// whole, which is the honest answer for a surface with no cursor in it.
    ///
    /// Deliberately **not** `justify_content: center` on the region: flexbox centring overflows
    /// equally in both directions once the content is larger, which puts the first row above the
    /// scrollable area and makes it permanently unreachable. This applies only while there is
    /// slack, so it can never do that — CSS's `safe center`, which taffy has no spelling for.
    fn center_pad(&self) -> (f64, f64) {
        if self.reveal_align != RevealAlign::Center {
            return (0.0, 0.0);
        }
        let vp = self.base.bounds;
        let (content_w, content_h) = self.raw_extent();
        // Natural (pre-shift) rect of whatever asked to be seen — or, while nothing is asking, the
        // last thing that did. See `last_reveal_rect`: a grid stops asking while the pointer drives
        // its cursor, and the pad must hold rather than re-centre on the whole strip.
        let target = crate::component::reveal_target_in(&self.base.children)
            .map(|r| {
                Rectangle::new(
                    Point::new(r.loc.x + self.applied_offset_x, r.loc.y + self.applied_offset),
                    r.size,
                )
            })
            .or(self.last_reveal_rect);
        let over = self.overscroll;
        let pad = |on: bool, vp_start: f64, vp_len: f64, content: f64, tgt: Option<(f64, f64)>| {
            if !on || (content > vp_len && !over) {
                // Overflowing an axis with ends: the scroll offset does the centring, and it
                // clamps — so the first and last of anything rest against the edge instead of
                // floating in half a screen of nothing.
                return 0.0;
            }
            // **What is centred depends on whether there is anything you cannot see.**
            //
            // When the content **fits**, centre the *content*: everything is on screen, so pulling
            // one card to the middle can only push its neighbours off the edges — which is exactly
            // what an exposé must never do. The map showed a screen of empty space on the left with
            // the last column clipped on the right, because the cursor card was being centred in a
            // strip that already fitted twice over (Antonio, with a screenshot, 2026-08-12).
            //
            // When it **overflows** — only reachable here with `overscroll`, which has no ends to
            // clamp against — centre the cursor, because then there really is something off-screen
            // and following it is the whole point.
            match tgt.filter(|_| content > vp_len) {
                Some((start, len)) => vp_len / 2.0 - (start - vp_start) - len / 2.0,
                None => (vp_len - content) / 2.0,
            }
        };
        // Temporary instrumentation (F003/P082/T420): the exposé's content measured 727 in a 730
        // viewport — fitting — while this pushed it 185px down, which is the "it overflows, follow
        // the cursor" branch. Six analytic diagnoses in a row have been wrong; this prints what the
        // decision is actually made on.
        #[cfg(debug_assertions)]
        if std::env::var_os("HECA_TRACE_PAD").is_some() {
            eprintln!(
                "[heca] center_pad: vp={:.0}x{:.0} content={content_w:.0}x{content_h:.0} \
                 over={over} target={:?} align={:?}",
                vp.size.w,
                vp.size.h,
                target.map(|r| (r.loc.y, r.size.h)),
                self.reveal_align,
            );
        }
        (
            pad(
                self.axes.is_horizontal(),
                vp.loc.x,
                vp.size.w,
                content_w,
                target.map(|r| (r.loc.x, r.size.w)),
            ),
            pad(
                self.axes.is_vertical(),
                vp.loc.y,
                vp.size.h,
                content_h,
                target.map(|r| (r.loc.y, r.size.h)),
            ),
        )
    }

    /// The range the vertical offset may take: `[0, max]` normally.
    ///
    /// **With [`overscroll`](Self::overscroll) it gains a viewport of room at each end.** Without
    /// that the wheel is dead on an overscrolling surface: the position is carried by the centring
    /// pad, the content usually fits, so `max_offset` is `0`, the range is a single point and every
    /// notch is a no-op. A viewport either side is enough to look past anything and bounded enough
    /// that the view cannot be flicked off into empty space.
    fn offset_bounds_y(&self) -> (f64, f64) {
        let max = self.max_offset();
        // **Overscroll is a cushion past the ends, and content that fits has no ends.** Applied
        // unconditionally it handed a full viewport of travel in each direction to a map with
        // nothing off screen, so the wheel carried the whole picture out of the window and there
        // was no way back but the keyboard (Antonio, driving, 2026-08-12).
        match self.overscroll && self.axes.is_vertical() && max > 0.0 {
            true => (-self.base.bounds.size.h, max + self.base.bounds.size.h),
            false => (0.0, max),
        }
    }

    /// The horizontal counterpart of [`offset_bounds_y`](Self::offset_bounds_y).
    fn offset_bounds_x(&self) -> (f64, f64) {
        let max = self.max_offset_x();
        match self.overscroll && self.axes.is_horizontal() && max > 0.0 {
            true => (-self.base.bounds.size.w, max + self.base.bounds.size.w),
            false => (0.0, max),
        }
    }

    /// Largest valid vertical offset (≥ 0). Always 0 when the vertical axis is
    /// disabled. When a horizontal bar shows it reserves that bar's gutter, so the
    /// user can scroll the last row fully **above** the bar instead of leaving it
    /// half-hidden in the gutter — the effective viewport is `viewport_h − gutter`.
    fn max_offset(&self) -> f64 {
        if !self.axes.is_vertical() {
            return 0.0;
        }
        let reserve = if self.scrollbars && self.h_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        (self.content_extent() + reserve - self.base.bounds.size.h).max(0.0)
    }

    /// Total content extent along the **horizontal** axis (max child natural right
    /// relative to this region's left, never less than the viewport width). Uses
    /// `+ applied_offset_x` to recover natural positions from the shifted bounds.
    fn content_extent_x(&self) -> f64 {
        let vp = self.base.bounds;
        let off = self.applied_offset_x;
        let mut max_right = vp.loc.x + vp.size.w;
        for c in &self.base.children {
            let b = c.base().bounds;
            let right = b.loc.x + b.size.w + off;
            if right > max_right {
                max_right = right;
            }
        }
        (max_right - vp.loc.x).max(vp.size.w)
    }

    /// Largest valid horizontal offset (≥ 0). Always 0 when the horizontal axis is
    /// disabled. Reserves the vertical bar's gutter (when it shows) so the last
    /// column can clear the bar.
    fn max_offset_x(&self) -> f64 {
        if !self.axes.is_horizontal() {
            return 0.0;
        }
        let reserve = if self.scrollbars && self.v_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        (self.content_extent_x() + reserve - self.base.bounds.size.w).max(0.0)
    }

    /// Whether the vertical axis currently overflows (→ a vertical scrollbar shows).
    /// Independent of the horizontal bar, so each bar can size the other's track
    /// (leaving the corner empty) without recursing through `thumb_rect`.
    fn v_overflow(&self) -> bool {
        self.axes.is_vertical() && self.content_extent() > self.base.bounds.size.h + 0.5
    }

    /// Whether the horizontal axis currently overflows (→ a horizontal scrollbar shows).
    fn h_overflow(&self) -> bool {
        self.axes.is_horizontal() && self.content_extent_x() > self.base.bounds.size.w + 0.5
    }

    /// The vertical scrollbar thumb rect (in viewport space), or `None` when the
    /// content fits. The track stops short of the horizontal bar's gutter when both
    /// show, so the two never overlap in the bottom-right corner.
    fn thumb_rect(&self) -> Option<Rectangle> {
        // `scrollbars(false)` means no bar: no thumb drawn, none to grab, and no gutter reserved.
        // Gating only the gutter left the bar painted — visible, draggable, and taking no space.
        if !self.scrollbars || !self.v_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_h = self.content_extent();
        let track_h = vp.size.h - if self.h_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        let thumb_h = ((vp.size.h / content_h) * track_h)
            .max(MIN_THUMB)
            .min(track_h);
        let max_off = self.max_offset();
        let frac = if max_off > 0.0 {
            (self.scroll_offset.get_untracked() as f64 / max_off).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_y = vp.loc.y + (track_h - thumb_h) * frac;
        let thumb_x = vp.loc.x + vp.size.w - SCROLLBAR_W - SCROLLBAR_EDGE_INSET;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(SCROLLBAR_W, thumb_h),
        ))
    }

    /// The thumb's **grab lane** — the visible 8px thumb centered inside a wider
    /// `THUMB_HIT_W` click/hover target at the right edge (at the thumb's y), so a
    /// thin thumb is still easy to grab. `None` when not scrollable. Used for
    /// press/hover hit-testing; [`thumb_rect`](Self::thumb_rect) is the painted
    /// (thin) thumb.
    fn thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.thumb_rect()?;
        let lane_x = self.base.bounds.loc.x + self.base.bounds.size.w - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(lane_x, t.loc.y),
            Size::new(THUMB_HIT_W, t.size.h),
        ))
    }

    /// The **horizontal** scrollbar thumb rect (bottom edge, viewport space), or
    /// `None` when the content fits horizontally. Mirrors [`thumb_rect`](Self::thumb_rect).
    fn h_thumb_rect(&self) -> Option<Rectangle> {
        if !self.scrollbars || !self.h_overflow() {
            return None;
        }
        let vp = self.base.bounds;
        let content_w = self.content_extent_x();
        let track_w = vp.size.w - if self.v_overflow() { SCROLLBAR_GUTTER } else { 0.0 };
        let thumb_w = ((vp.size.w / content_w) * track_w)
            .max(MIN_THUMB)
            .min(track_w);
        let max_off = self.max_offset_x();
        let frac = if max_off > 0.0 {
            (self.scroll_offset_x.get_untracked() as f64 / max_off).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb_x = vp.loc.x + (track_w - thumb_w) * frac;
        let thumb_y = vp.loc.y + vp.size.h - SCROLLBAR_W - SCROLLBAR_EDGE_INSET;
        Some(Rectangle::new(
            Point::new(thumb_x, thumb_y),
            Size::new(thumb_w, SCROLLBAR_W),
        ))
    }

    /// The horizontal thumb's **grab lane** (a wider `THUMB_HIT_W`-tall target at
    /// the bottom edge). `None` when not scrollable horizontally.
    fn h_thumb_hit_rect(&self) -> Option<Rectangle> {
        let t = self.h_thumb_rect()?;
        let lane_y = self.base.bounds.loc.y + self.base.bounds.size.h - THUMB_HIT_W;
        Some(Rectangle::new(
            Point::new(t.loc.x, lane_y),
            Size::new(t.size.w, THUMB_HIT_W),
        ))
    }

    /// Bake the current `scroll_offset` into the children's bounds. Shifts each
    /// direct child's subtree by `applied_offset − scroll_offset` so the bounds
    /// end at `natural − scroll_offset` (the visual position). Idempotent when
    /// already in sync. Called from `paint` and `event` so bounds are always
    /// current for drawing, hit-testing, and DnD.
    /// Where the content **wants** to sit: the scroll offset, less the centring pad.
    ///
    /// The pad rides the same shift as the offset, and subtracts: a positive pad moves the content
    /// **down and right**, into the middle of the room it does not fill. The two never fight —
    /// where there is slack the offset is pinned at 0, and where there is none the pad is.
    fn desired_shift(&self) -> (f64, f64) {
        let (pad_x, pad_y) = self.center_pad();
        (
            self.scroll_offset_x.get_untracked() as f64 - pad_x,
            self.scroll_offset.get_untracked() as f64 - pad_y,
        )
    }

    /// Clamp a shift so the content can **never leave the viewport**.
    ///
    /// The position of an overscrolling surface is two numbers added together — the centring pad
    /// and the scroll offset — and bounding each of them separately does not bound their sum. Twice
    /// now that sum has carried the whole map off screen, leaving an empty window and no way back
    /// but the keyboard. This is the invariant that cannot be violated by any combination of them:
    /// a quarter of the viewport's worth of content stays on screen, whatever the two parts say.
    fn clamp_visible(&self, shift: (f64, f64)) -> (f64, f64) {
        let vp = self.base.bounds.size;
        let (content_w, content_h) = self.raw_extent();
        let clamp = |s: f64, viewport: f64, content: f64| {
            if viewport <= 0.0 || content <= 0.0 {
                return s;
            }
            let keep = (viewport * 0.25).min(content);
            s.clamp(keep - viewport, content - keep)
        };
        (
            clamp(shift.0, vp.w, content_w),
            clamp(shift.1, vp.h, content_h),
        )
    }

    fn sync_shift(&mut self) {
        let desired = self.clamp_visible(self.desired_shift());
        // Smoothing puts the *eased* value on screen and lets `tick` walk it to `desired`; without
        // it the desired value goes straight on. The first sync snaps either way.
        let (target_x, target_y) = match self.eased.as_mut() {
            None => desired,
            Some((ex, ey)) => {
                if !self.eased_seeded {
                    self.eased_seeded = true;
                    ex.snap(desired.0);
                    ey.snap(desired.1);
                }
                ex.set(desired.0);
                ey.set(desired.1);
                (ex.value(), ey.value())
            }
        };
        let dy = self.applied_offset - target_y;
        let dx = self.applied_offset_x - target_x;
        if dx != 0.0 || dy != 0.0 {
            for child in self.base.children.iter_mut() {
                shift_subtree(child.as_mut(), dx, dy);
            }
            self.applied_offset = target_y;
            self.applied_offset_x = target_x;
            self.base.mark_needs_paint();
        }
    }
}

impl ScrollRegion {
    /// Walk the on-screen shift toward [`desired_shift`](Self::desired_shift), returning `true`
    /// while it still has ground to cover (which is what keeps frames coming).
    ///
    /// The motion itself is [`Eased`]'s, in `effects.rs` beside `Flash` and `Attention` — the same
    /// value-plus-`tick` shape every animated widget here embeds.
    fn advance_eased(&mut self, dt: f32) -> bool {
        let (dx, dy) = self.clamp_visible(self.desired_shift());
        let Some((ex, ey)) = self.eased.as_mut() else { return false };
        ex.set(dx);
        ey.set(dy);
        let moving = ex.tick(dt) | ey.tick(dt);
        self.sync_shift();
        moving
    }
}

impl Default for ScrollRegion {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ScrollRegion {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Layout just re-computed every bound to its natural position — clear the
    /// baked shift so the next `sync_shift` re-applies it from scratch instead
    /// of compounding. (Post-order: children already assigned.)
    ///
    /// The offset is also **re-clamped** here: a relayout can GROW the viewport
    /// (window resize, zoom-out), leaving the stored offset beyond the new max —
    /// re-applying it raw would shift the content past the edge with no
    /// scrollbar left to bring it back. `scroll_to`/`scroll_to_x` clamp against
    /// the freshly-assigned natural bounds and re-bake the shift.
    fn on_layout(&mut self) {
        self.applied_offset = 0.0;
        self.applied_offset_x = 0.0;
        let y = self.scroll_offset.get_untracked();
        let x = self.scroll_offset_x.get_untracked();
        self.scroll_to(y);
        self.scroll_to_x(x);
        self.reserve_scrollbar_gutters();
        self.follow_revealed();
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        let vp = self.base.bounds;
        // Styled-surface decoration (background/border/glow/radius from the theme
        // via `StyleExt`) painted in viewport space, before the clipped content — a
        // plain region sets none of these and stays frameless (and, having no
        // surface, carries no glow). A STYLED region (a scrollable panel) without an
        // explicit `.glow(..)` falls back to the theme rest glow, like every surface.
        let s = &self.base.style;
        if (s.visual.fill.is_some() || s.visual.border.is_some()) && s.visual.glow.is_none() {
            cx.rect(
                vp,
                s.visual.fill.unwrap_or(crate::color::Color::TRANSPARENT),
                s.visual.border,
                s.visual.radius,
                cx.rest_glow(SURFACE_GLOW_RADIUS),
            );
        } else {
            cx.paint_base(&self.base);
        }
        // Reserve a gutter for each visible scrollbar so content is never drawn
        // *under* the thumb: clip the content short of the lane on the right (when
        // the vertical bar shows) and/or the bottom (horizontal bar). The thumbs are
        // then painted below in the full viewport rect, sitting in the clear gutter.
        let mut content_clip = vp;
        if self.v_overflow() {
            content_clip.size.w = (content_clip.size.w - SCROLLBAR_GUTTER).max(0.0);
        }
        if self.h_overflow() {
            content_clip.size.h = (content_clip.size.h - SCROLLBAR_GUTTER).max(0.0);
        }
        // Widen the clip on an axis the region does NOT scroll, so a child's GLOW
        // (which paints outside its own bounds — every bordered surface carries a
        // rest halo) isn't scissored flat against the edge, which reads as the row
        // being "cut off". Only the scrolling axis needs a tight clip — that is the
        // one where content genuinely moves through the viewport and must be cut at
        // the boundary. A non-scrolling axis has nothing to hide, so the few px of
        // bleed is free.
        //
        // Safe to widen on both sides now that `reserve_scrollbar_gutters` keeps content out of the
        // lane by LAYOUT: the clip is no longer the only thing standing between a card and the
        // scrollbar, so letting a halo bleed a few px costs nothing. It was not always so — when
        // the gutter existed only as a clip, this widening handed it straight back and content was
        // drawn under the bar.
        if !self.axes.is_horizontal() {
            content_clip.loc.x -= GLOW_BLEED;
            content_clip.size.w += 2.0 * GLOW_BLEED;
        }
        if !self.axes.is_vertical() {
            content_clip.loc.y -= GLOW_BLEED;
            content_clip.size.h += 2.0 * GLOW_BLEED;
        }
        // Keep the baked shift current (paint takes `&self`, so sync via the
        // signal value; the shift was already applied by the last `event`/layout
        // reset — `applied_offset` matches `scroll_offset` here in steady state).
        cx.with_clip(content_clip, |cx| {
            for child in &self.base.children {
                paint_child(child.as_ref(), cx);
            }
        });
        // Scrollbar thumb on top, in viewport space (not scrolled with content).
        // Theme-driven hover affordance (mirrors MarkerGroup's grip bar): the
        // thumb is a dim accent at rest, brightens when its grab lane is hovered,
        // and is full-bright while dragged — reading as "grab here". Radius and
        // color both come from the theme (no hardcoded radius/Color).
        if let Some(t) = self.thumb_rect() {
            let alpha = if self.thumb_grab.is_some() || self.thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = cx.accent().with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
        // Horizontal scrollbar thumb (bottom edge), same theme-driven affordance.
        if let Some(t) = self.h_thumb_rect() {
            let alpha = if self.h_thumb_grab.is_some() || self.h_thumb_hovered {
                cx.theme().colors.interaction.thumb_hover
            } else {
                cx.theme().colors.interaction.thumb_rest
            };
            let theme = cx.theme();
            let color = cx.accent().with_alpha(alpha);
            cx.rect(t, color, None, theme.colors.control_radius(), None);
        }
    }

    /// The region **clips** its content, so a press or a move outside its bounds must not reach
    /// content that has been scrolled out of sight. `dispatch` applies that; the widget only
    /// declares it.
    fn clips_children(&self) -> bool {
        true
    }

    /// Before the children: the offset has to be baked into bounds so everything below hit-tests
    /// against what is actually drawn, and a scrollbar gesture beats whatever sits under it.
    fn on_event_capture(&mut self, ev: &Event) -> Handled {
        // Bake the current scroll offset into bounds first, so hit-testing and
        // DnD see the visual position (bounds === what's drawn).
        self.sync_shift();
        let vp = self.base.bounds;

        match ev {
            Event::PointerDown(p) => self.press_capture(p.pos, vp, ev),
            Event::PointerMove(p) => self.move_capture(p.pos, vp, ev),
            // Scroll and the release deliberately fall through to the children first — see `on_event`.
            _ => Handled::No,
        }
    }

    /// After the children have declined: the wheel (so the innermost hovered region wins, which is
    /// what makes nesting work) and the end of a gesture.
    fn on_event(&mut self, ev: &Event) -> Handled {
        let vp = self.base.bounds;
        match ev {
            Event::Scroll(p) => {
                // **The wheel carries a position now**, and the router only delivers it to what is
                // under the cursor — so being here *is* the hover test this arm used to do by
                // hand against a flag it maintained from moves. What is left is the honest
                // question: can this region scroll the axis it was asked for? If not it declines,
                // and the event carries on to the region outside it.
                //
                // The host already mapped modifiers to axes (plain wheel → `delta_y`,
                // `Shift`+wheel → `delta_x`), so each axis is simply consumed if it can move.
                let mut handled = false;
                let (min_y, max_y) = self.offset_bounds_y();
                let (min_x, max_x) = self.offset_bounds_x();
                if p.delta_y != 0.0 && max_y > min_y {
                    let next = self.scroll_offset.get_untracked() as f64
                        + (p.delta_y as f64) * WHEEL_STEP_FRAC * vp.size.h;
                    self.set_offset_y(next as f32, Some(ev));
                    handled = true;
                }
                if p.delta_x != 0.0 && max_x > min_x {
                    let next = self.scroll_offset_x.get_untracked() as f64
                        + (p.delta_x as f64) * WHEEL_STEP_FRAC * vp.size.w;
                    self.set_offset_x(next as f32, Some(ev));
                    handled = true;
                }
                if handled { Handled::Yes } else { Handled::No }
            }
            // Keyboard scrolling (F003/P011/T012). Semantic, so the arithmetic lives here rather
            // than in the app: the widget is what knows its viewport and its content.
            //
            // After the children, like the wheel, so the innermost scrollable region wins when
            // regions nest. A region that cannot scroll the axis asked for returns `No` and the
            // event carries on — a vertical-only region must not swallow a horizontal page.
            Event::Widget(intent) => self.scroll_intent(*intent, ev),
            Event::PointerUp(_) => {
                // No bounds check: a drag that started here ends here, wherever the cursor drifted
                // to — the press captured the pointer, so the release is delivered here whatever
                // it is over. Gating a release on position is exactly how a thumb gets stuck.
                let was_dragging = self.release_grabs();
                if was_dragging {
                    self.end_scroll(Some(ev));
                    Handled::Yes
                } else {
                    Handled::No
                }
            }
            _ => Handled::No,
        }
    }
    /// Children first (the default recursion), then the held-track-press repeat:
    /// after [`TRACK_REPEAT_DELAY`] a held press in a scrollbar track keeps paging
    /// toward the cursor every [`TRACK_REPEAT_INTERVAL`] until released. Returns
    /// `true` while a press is held so the host keeps ticking.
    fn tick(&mut self, dt: f32) -> bool {
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating |= self.advance_eased(dt);
        let mut fire: Option<(bool, Point)> = None;
        if let Some(tr) = &mut self.track_repeat {
            tr.next_in -= dt;
            if tr.next_in <= 0.0 {
                tr.next_in = TRACK_REPEAT_INTERVAL;
                fire = Some((tr.horizontal, tr.pos));
            }
            animating = true;
        }
        if let Some((horizontal, pos)) = fire {
            self.page_toward(horizontal, pos, None);
        }
        // A wheel gesture reports every movement and never reports stopping, so the end is a
        // pause. Keep asking for frames while one is pending, or the timer would only advance when
        // something else happened to redraw — and the end would land late, or never.
        if let Some(left) = self.wheel_idle {
            let left = left - dt;
            if left <= 0.0 {
                self.wheel_idle = None;
                self.end_scroll(None);
            } else {
                self.wheel_idle = Some(left);
                animating = true;
            }
        }
        animating
    }
}

impl ScrollRegion {
    /// Reserve layout space for each visible scrollbar, so content is laid out **beside** the bar
    /// rather than under it.
    ///
    /// Clipping alone is not enough, and the difference is visible: a card laid out full width and
    /// then trimmed at the lane loses its rounded corner and ends on a hard vertical cut, because
    /// it still *is* wider than the space it has. Reserving the gutter as padding on that one side
    /// makes the card the width it appears to be.
    ///
    /// Applied after layout, so it takes effect on the next pass — which is how a classic
    /// scrollbar behaves everywhere. It cannot oscillate: narrowing content can only make it taller,
    /// never shorter, so a region that overflows vertically still overflows once it has reserved
    /// the space.
    fn reserve_scrollbar_gutters(&mut self) {
        // Read the overflow flags before borrowing the layout mutably.
        let (v_overflow, h_overflow) = (self.v_overflow(), self.h_overflow());
        // A step is a fraction of the font, and this compares against a fixed gutter — so the
        // authored padding has to become pixels before the two can be weighed against each other.
        let font = self.base.font;
        let layout = &mut self.base.style.layout;
        // What the other side of each axis already insets by. The bar takes the LARGER of that and
        // the gutter, never the sum: where the padding is already roomy enough the bar simply sits
        // in it, both sides stay equal, and nothing looks lopsided. Space is only added when the
        // bar genuinely needs more than is already there.
        let base_x = layout.padding_x.unwrap_or(layout.padding).resolve(font);
        let base_y = layout.padding_y.unwrap_or(layout.padding).resolve(font);
        let gutter = SCROLLBAR_GUTTER as f32;
        // The reservation is a measured number, not an authored step — it is what the bar needs.
        let want_x = v_overflow.then(|| crate::style::Space::Px(base_x.max(gutter)));
        let want_y = h_overflow.then(|| crate::style::Space::Px(base_y.max(gutter)));
        if layout.padding_right != want_x || layout.padding_bottom != want_y {
            layout.padding_right = want_x;
            layout.padding_bottom = want_y;
            self.base.mark_needs_paint();
        }
    }

    /// Is `pos` over a scrollbar lane — thumb or track — of an axis that actually scrolls?
    ///
    /// One definition, used by hover here and by the paging press, so the two can never disagree
    /// about where the scrollbar is.
    fn in_scrollbar_lane(&self, pos: Point, vp: Rectangle) -> bool {
        if !vp.contains(pos) {
            return false;
        }
        let vertical = self.thumb_rect().is_some() && pos.x >= vp.loc.x + vp.size.w - THUMB_HIT_W;
        let horizontal =
            self.h_thumb_rect().is_some() && pos.y >= vp.loc.y + vp.size.h - THUMB_HIT_W;
        vertical || horizontal
    }

    /// Keep the descendant that asked to be visible in view — the keyboard's half of scrolling.
    ///
    /// Any widget reporting [`Component::wants_visible`] (focus by default; a list's navigation
    /// cursor by override) is scrolled to, in **every** region it is ever placed inside. That is
    /// the point of it living here: "make the view follow the cursor" is not something each host
    /// wires for each list, forgets in the next one, and rediscovers as a bug.
    ///
    /// It moves only when the *target* moves. The remembered position is the target's natural one,
    /// so scrolling the region by hand does not change it and the view does not snap back — the
    /// user can always look somewhere else.
    ///
    /// [`reveal_align`](Self::reveal_align) decides *where* it lands: minimally in view, or centred.
    fn follow_revealed(&mut self) {
        let target = crate::component::reveal_target_in(&self.base.children);
        let Some(rect) = target else {
            self.last_revealed = None;
            return;
        };
        // Natural = what layout produced, before this region's shift was baked in.
        let natural = (
            rect.loc.x + self.applied_offset_x,
            rect.loc.y + self.applied_offset,
        );
        self.last_reveal_rect = Some(Rectangle::new(
            Point::new(natural.0, natural.1),
            rect.size,
        ));
        if self.last_revealed == Some(natural) {
            return;
        }
        self.last_revealed = Some(natural);
        match self.reveal_align {
            RevealAlign::Minimal => self.ensure_visible(rect),
            // With `overscroll` the centring pad already tracks the target on every sync, and a
            // scroll on top of it would centre the same thing twice and overshoot by that much.
            // What the offset carries there is **where the user has wheeled to**, and moving the
            // cursor is a new instruction that supersedes it — so it goes back to zero and the pad
            // puts the target dead centre, instead of centring it half a screen off for good.
            RevealAlign::Center if self.overscroll => {
                self.scroll_to(0.0);
                self.scroll_to_x(0.0);
            }
            RevealAlign::Center => self.center_on(rect),
        }
    }

    /// A press, before the children: end any stale gesture, then try the scrollbar lanes.
    fn press_capture(&mut self, pos: Point, vp: Rectangle, cause: &Event) -> Handled {
        {
            let pos = &pos;
                // A fresh press means any previous gesture is over — so a grab can never survive
                // one, even if this region never saw the release that should have ended it.
                //
                // It should always see that release: `PointerReleased` is part of the pointer set
                // a host owes any tree it mounts, and `release_grabs` below does not care where
                // the release landed. But a host that forwards a subset produces a thumb welded to
                // the cursor with no way back, and that has now happened in three different
                // surfaces. The widget stops being the thing that pays for it: the worst a missing
                // release can do here is survive until the next click.
                self.release_grabs();
                // Grab the thumb via its wider hit lane (the thin visible thumb is
                // easy to miss); `thumb_grab` stores the grab point relative to the
                // *visible* thumb top so the cursor stays pinned to it.
                if let Some(hit) = self.thumb_hit_rect()
                    && hit.contains(*pos)
                {
                    let t = self.thumb_rect().expect("scrollable: thumb exists");
                    self.thumb_grab = Some(pos.y - t.loc.y);
                    self.thumb_hovered = true;
                    self.base.mark_needs_paint();
                    return Handled::Yes;
                }
                if let Some(hit) = self.h_thumb_hit_rect()
                    && hit.contains(*pos)
                {
                    let t = self.h_thumb_rect().expect("scrollable-x: thumb exists");
                    self.h_thumb_grab = Some(pos.x - t.loc.x);
                    self.h_thumb_hovered = true;
                    self.base.mark_needs_paint();
                    return Handled::Yes;
                }
                // Click in the scrollbar TRACK but off the thumb → page toward the
                // click (a screenful in that direction), the standard scrollbar
                // affordance — and ARM the press-and-hold repeat: holding the press
                // keeps paging (see `tick`) until release. The thumb-grab checks
                // above already returned for a hit on the thumb itself, so reaching
                // here means the empty track.
                if self.thumb_rect().is_some() {
                    let lane_x = vp.loc.x + vp.size.w - THUMB_HIT_W;
                    if pos.x >= lane_x && vp.contains(*pos) {
                        self.page_toward(false, *pos, Some(cause));
                        self.track_repeat = Some(TrackRepeat {
                            horizontal: false,
                            pos: *pos,
                            next_in: TRACK_REPEAT_DELAY,
                        });
                        return Handled::Yes;
                    }
                }
                if self.h_thumb_rect().is_some() {
                    let lane_y = vp.loc.y + vp.size.h - THUMB_HIT_W;
                    if pos.y >= lane_y && vp.contains(*pos) {
                        self.page_toward(true, *pos, Some(cause));
                        self.track_repeat = Some(TrackRepeat {
                            horizontal: true,
                            pos: *pos,
                            next_in: TRACK_REPEAT_DELAY,
                        });
                        return Handled::Yes;
                    }
                }
                // Not a scrollbar gesture: let it through. The children are walked by `dispatch`,
                // and `clips_children` already stops a press outside the viewport reaching content
                // that has been scrolled out of sight.
                Handled::No
        }
    }

    /// A move, before the children: hover state and the scrollbar lanes, then any drag in flight.
    fn move_capture(&mut self, pos: Point, vp: Rectangle, cause: &Event) -> Handled {
        {
            let pos = &pos;
                // Track thumb-lane hover for the highlight affordance. A drag in
                // progress keeps handling moves even after the cursor leaves.
                let lane_hit = self.thumb_hit_rect().is_some_and(|h| h.contains(*pos));
                if lane_hit != self.thumb_hovered {
                    self.thumb_hovered = lane_hit;
                    self.base.mark_needs_paint();
                }
                let h_lane_hit = self.h_thumb_hit_rect().is_some_and(|h| h.contains(*pos));
                if h_lane_hit != self.h_thumb_hovered {
                    self.h_thumb_hovered = h_lane_hit;
                    self.base.mark_needs_paint();
                }
                if let Some(grab) = self.thumb_grab {
                    let content_h = self.content_extent();
                    let max_off = (content_h - vp.size.h).max(0.0);
                    let track_h = vp.size.h;
                    let thumb_h = ((vp.size.h / content_h) * track_h).max(MIN_THUMB);
                    let thumb_top = pos.y - grab;
                    let frac = if track_h - thumb_h > 0.0 {
                        ((thumb_top - vp.loc.y) / (track_h - thumb_h)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    self.set_offset_y((frac * max_off) as f32, Some(cause));
                    Handled::Yes
                } else if let Some(grab) = self.h_thumb_grab {
                    let content_w = self.content_extent_x();
                    let max_off = (content_w - vp.size.w).max(0.0);
                    let track_w = vp.size.w;
                    let thumb_w = ((vp.size.w / content_w) * track_w).max(MIN_THUMB);
                    let thumb_left = pos.x - grab;
                    let frac = if track_w - thumb_w > 0.0 {
                        ((thumb_left - vp.loc.x) / (track_w - thumb_w)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    self.set_offset_x((frac * max_off) as f32, Some(cause));
                    Handled::Yes
                } else if let Some(tr) = &mut self.track_repeat {
                    // A held track press is a grab: track the cursor (the repeat
                    // pages toward wherever it is now) and consume the move.
                    tr.pos = *pos;
                    Handled::Yes
                } else if self.in_scrollbar_lane(*pos, vp) {
                    // The cursor is over a scrollbar lane, so the lane owns it: without this the
                    // move falls through and the row *underneath* the scrollbar lights up as
                    // hovered, which reads as pointing at content the user is not pointing at.
                    // The whole lane, not just the thumb — the track is part of the control (a
                    // press there pages), so it is not content either.
                    Handled::Yes
                } else {
                    // No drag in flight: the children get it (walked by `dispatch`, and skipped
                    // entirely when the cursor is outside this region — see `clips_children`).
                    Handled::No
                }
        }
    }

}

impl LayoutExt for ScrollRegion {}
impl StyleExt for ScrollRegion {}
impl Parent for ScrollRegion {}

#[cfg(test)]
mod tests {
    use crate::event::PointerButton;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// A resolved wheel event over the region under test. The router puts a real one here only
    /// when the pointer is over the region; these tests call the widget directly, so they say so.
    fn wheel(delta_x: f32, delta_y: f32) -> Event {
        Event::Scroll(crate::event::PointerEvent {
            delta_x,
            delta_y,
            ..crate::event::PointerEvent::at(Point::new(10.0, 10.0))
        })
    }

    /// A **raw** wheel at a point — for the tests that go through the router, which is what
    /// decides whose wheel it is.
    fn wheel_at(pos: Point, delta_x: f32, delta_y: f32) -> Event {
        Event::wheel(pos, delta_x, delta_y)
    }

    /// **A wheel over content that fits does nothing.** `overscroll` exists so a map larger than
    /// the window can be pushed past its ends; applied to content with no ends it gave a full
    /// viewport of travel in each direction, and the wheel carried the whole exposé off screen
    /// with no way back but the keyboard (Antonio, driving, 2026-08-12).
    #[test]
    fn overscroll_gives_no_travel_to_content_that_fits() {
        let mut r = region_with_children(&[40.0, 40.0]); // content 80, viewport 100
        r.overscroll = true;
        assert_eq!(r.max_offset(), 0.0, "the fixture must actually fit");
        assert_eq!(r.offset_bounds_y(), (0.0, 0.0), "so there is nowhere to scroll");

        r.scroll_by(500.0, None);
        assert_eq!(r.scroll_offset.get_untracked(), 0.0, "and the wheel moves nothing");
    }

    /// The counterpart: real overflow still gets its cushion, so the map can be pushed past its
    /// last row rather than stopping dead against it.
    #[test]
    fn overscroll_still_cushions_content_that_overflows() {
        let mut r = region_with_children(&[80.0, 80.0]); // content 160, viewport 100
        r.overscroll = true;
        assert!(r.max_offset() > 0.0, "the fixture must actually overflow");
        let (min, max) = r.offset_bounds_y();
        assert!(min < 0.0 && max > r.max_offset(), "a viewport of cushion each way: {min}..{max}");
    }

    fn region_with_children(child_heights: &[f64]) -> ScrollRegion {
        // Children are real components only for layout; here we just need bounds
        // set on them. We build a ScrollRegion and manually stamp child bounds
        // (as the layout engine would) so the geometry helpers are testable in
        // isolation. `applied_offset` starts at 0, so bounds are "natural".
        let mut r = ScrollRegion::new();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        r.base.children.clear();
        let mut y = 0.0;
        for &h in child_heights {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds =
                Rectangle::new(Point::new(0.0, y), Size::new(200.0, h));
            r.base.children.push(Box::new(child));
            y += h;
        }
        r
    }

    /// **The grab lane stays inside the space the bar reserved** (F003/P085/T368).
    ///
    /// The reserved strip is what content is laid out and clipped short of; the lane is where a
    /// press counts as grabbing the thumb. While the lane was wider than the strip, 9px of it sat on
    /// the row beside it and one pixel belonged to two widgets — so a press there was ambiguous, and
    /// the host could only pick a winner by knowing this widget's internals. Nothing outside can fix
    /// that; keeping the hit area inside the reserved box is the widget's own job.
    #[test]
    fn the_grab_lane_does_not_reach_over_the_content() {
        // Three 60px children in a 100px viewport ⇒ it overflows, so the bar shows.
        let r = region_with_children(&[60.0, 60.0, 60.0]);
        assert!(r.v_overflow(), "the fixture must overflow for a bar to exist");

        let lane = r.thumb_hit_rect().expect("a scrollable region has a grab lane");
        let content_right = r.base.bounds.loc.x + r.base.bounds.size.w - SCROLLBAR_GUTTER;
        assert!(
            lane.loc.x >= content_right,
            "the lane starts at {} but content runs to {} — {}px belong to both",
            lane.loc.x,
            content_right,
            content_right - lane.loc.x,
        );
        // The painted bar is inside its own lane too, hard against the outer edge.
        let thumb = r.thumb_rect().expect("scrollable ⇒ a thumb");
        assert!(thumb.loc.x >= lane.loc.x);
        assert!(thumb.loc.x + thumb.size.w <= lane.loc.x + lane.size.w);
    }

    /// A keyboard page moves by most of a viewport, and the region clamps its own ends.
    ///
    /// The intents are semantic on purpose (F003/P011/T012): the host says "one page on" and the
    /// widget decides what that means, because it is the only thing that knows its viewport and
    /// content. `ScrollToBottom` asks for `f64::MAX` and lands on the last scrollable pixel without
    /// the caller knowing where that is.
    #[test]
    fn a_keyboard_page_scrolls_by_most_of_a_viewport_and_clamps() {
        let mut r = region_with_children(&[100.0, 100.0, 100.0]); // 300 of content in a 100 viewport
        let page = PAGE_STEP_FRAC * 100.0; // 90

        assert_eq!(r.scroll_intent(WidgetIntent::ScrollPageDown, &Event::Widget(WidgetIntent::ScrollPageDown)), Handled::Yes);
        assert!((r.scroll_offset.get_untracked() as f64 - page).abs() < 0.01, "one page down");

        r.scroll_intent(WidgetIntent::ScrollPageUp, &Event::Widget(WidgetIntent::ScrollPageUp));
        assert_eq!(r.scroll_offset.get_untracked(), 0.0, "back to the top, not past it");

        r.scroll_intent(WidgetIntent::ScrollToBottom, &Event::Widget(WidgetIntent::ScrollToBottom));
        assert_eq!(
            r.scroll_offset.get_untracked() as f64,
            r.max_offset(),
            "the bottom is the last scrollable pixel, clamped by the widget",
        );

        r.scroll_intent(WidgetIntent::ScrollToTop, &Event::Widget(WidgetIntent::ScrollToTop));
        assert_eq!(r.scroll_offset.get_untracked(), 0.0);
    }

    /// A region declines an axis it cannot scroll, so the event carries on.
    ///
    /// This is what makes nesting and the host fallback work: a vertical-only region must not
    /// swallow a horizontal page, and a region whose content fits must not swallow anything. If it
    /// consumed them, an outer region would never see the key and the user would press it to no
    /// effect — silently.
    #[test]
    fn a_region_declines_an_axis_it_cannot_scroll() {
        // Vertical-only (the default), so horizontal intents are not ours.
        let mut r = region_with_children(&[100.0, 100.0, 100.0]);
        assert_eq!(
            r.scroll_intent(WidgetIntent::ScrollPageRight, &Event::Widget(WidgetIntent::ScrollPageRight)),
            Handled::No,
            "a vertical-only region declines a horizontal page",
        );
        assert_eq!(r.scroll_offset_x.get_untracked(), 0.0, "and moves nothing");

        // Content that fits: nothing to scroll on either axis.
        let mut fits = region_with_children(&[40.0]);
        assert_eq!(
            fits.scroll_intent(WidgetIntent::ScrollPageDown, &Event::Widget(WidgetIntent::ScrollPageDown)),
            Handled::No,
            "content that fits declines, so an outer region gets its turn",
        );

        // A non-scroll intent is never ours.
        assert_eq!(
            r.scroll_intent(WidgetIntent::Activate, &Event::Widget(WidgetIntent::Activate)),
            Handled::No,
        );
    }

    /// A region that is not the keyboard's target declines, so the one that is can take it.
    ///
    /// This is how "scroll the focused surface" works without the host knowing where any region sits
    /// in the tree: the intent goes into the whole tree and every region but the target refuses it.
    /// Unset means "yes", so a single-region app or an example needs no wiring; a host with several
    /// regions binds the flag on each of them and depends on no default.
    #[test]
    fn only_the_keyboard_target_takes_a_scroll_intent() {
        let ev = Event::Widget(WidgetIntent::ScrollPageDown);

        let mut unwired = region_with_children(&[100.0, 100.0, 100.0]);
        assert_eq!(unwired.scroll_intent(WidgetIntent::ScrollPageDown, &ev), Handled::Yes);

        let focused = crate::reactive::signal(true);
        let mut target = region_with_children(&[100.0, 100.0, 100.0]).keyboard_target(focused);
        assert_eq!(target.scroll_intent(WidgetIntent::ScrollPageDown, &ev), Handled::Yes);

        // The same region, once the keyboard is somewhere else — it must not move.
        focused.set(false);
        let before = target.scroll_offset.get_untracked();
        assert_eq!(target.scroll_intent(WidgetIntent::ScrollPageDown, &ev), Handled::No);
        assert_eq!(target.scroll_offset.get_untracked(), before, "and it did not scroll");
    }

    /// The intents reach the region through normal dispatch, after the children.
    ///
    /// The unit tests above call the handler directly; this one goes through `dispatch` to prove the
    /// wiring, since `on_event` (not `on_event_capture`) is what gives the innermost region the
    /// first refusal — the same order the wheel uses.
    ///
    /// It says the keyboard is aimed here, because that is now the whole of how a keyboard event
    /// finds anything: an intent is delivered to the focus owner and the region it encloses, so a
    /// tree with nothing focused has nowhere to deliver one.
    #[test]
    fn a_scroll_intent_arrives_through_dispatch() {
        let mut r =
            region_with_children(&[100.0, 100.0, 100.0]).keyboard_target(crate::reactive::signal(true));
        let handled = crate::component::dispatch(&mut r, &Event::Widget(WidgetIntent::ScrollPageDown));
        assert_eq!(handled, Handled::Yes);
        assert!(r.scroll_offset.get_untracked() > 0.0, "it scrolled");
    }

    #[test]
    fn content_extent_sums_children_and_clamps_to_viewport() {
        let r = region_with_children(&[60.0, 60.0]);
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn no_thumb_when_content_fits() {
        let r = region_with_children(&[40.0, 40.0]);
        assert!(r.thumb_rect().is_none());
        assert!(r.max_offset().abs() < f64::EPSILON);
    }

    #[test]
    fn thumb_appears_and_scales_with_visible_fraction() {
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        assert!((t.size.h - (100.0 / 120.0 * 100.0)).abs() < 1e-6);
        assert!(t.loc.y.abs() < 1e-6);
    }

    #[test]
    fn thumb_hit_lane_is_wider_than_the_visible_thumb_and_contains_it() {
        // Regression guard for "thumb misses clicks": the grab lane must be wider
        // than the thin visible thumb and must contain it, so a click near the
        // thumb still grabs.
        let r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        let hit = r.thumb_hit_rect().expect("scrollable → hit lane");
        assert!(hit.size.w > t.size.w, "hit lane is wider than the visible thumb");
        assert_eq!(hit.size.w as i32, THUMB_HIT_W as i32);
        // The visible thumb sits inside the lane horizontally.
        assert!(t.loc.x >= hit.loc.x - 0.001);
        assert!(t.loc.x + t.size.w <= hit.loc.x + hit.size.w + 0.001);
        // Same vertical span.
        assert!((hit.loc.y - t.loc.y).abs() < 1e-6);
        assert!((hit.size.h - t.size.h).abs() < 1e-6);
    }

    #[test]
    fn pressing_in_the_hit_lane_but_off_the_visible_thumb_still_grabs() {
        // A click in the grab padding (left of the thin thumb, inside the wider
        // lane) must still start a drag — the original 8px thumb missed these.
        let mut r = region_with_children(&[60.0, 60.0]);
        let t = r.thumb_rect().expect("scrollable → thumb");
        // A point just left of the visible thumb, inside the hit lane.
        let pos = Point::new(t.loc.x - 4.0, t.loc.y + 4.0);
        assert!(r.thumb_hit_rect().unwrap().contains(pos), "pos is in the hit lane");
        assert!(!t.contains(pos), "pos is NOT on the thin visible thumb");
        let handled = crate::component::dispatch(&mut r, &Event::pointer_pressed(pos, PointerButton::Left));
        assert_eq!(handled, Handled::Yes, "press in the lane grabs the thumb");
        assert!(r.thumb_grab.is_some(), "a drag started");
    }

    #[test]
    fn scroll_to_clamps_to_max_offset() {
        let mut r = region_with_children(&[60.0, 60.0]); // max_offset 20
        assert!((r.scroll_to(50.0) - 20.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(-5.0) - 0.0_f32).abs() < f32::EPSILON);
        assert!((r.scroll_to(10.0) - 10.0_f32).abs() < f32::EPSILON);
    }

    #[test]
    fn sync_shift_bakes_offset_into_children_and_recovers_natural() {
        let mut r = region_with_children(&[60.0, 60.0]); // content 120, vp 100
        r.scroll_offset.set(20.0); // max_offset
        r.sync_shift();
        // applied_offset now equals scroll_offset.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        // Children shifted up by 20 (visual position): first child now at y=-20.
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // Geometry helpers still report the *natural* extent (120) via the
        // applied_offset recovery — scrolling must not change content_extent.
        assert!((r.content_extent() - 120.0).abs() < f64::EPSILON);
        assert!((r.max_offset() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn on_layout_resets_applied_offset_so_shift_does_not_compound() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_offset.set(20.0);
        r.sync_shift();
        assert!(r.applied_offset.abs() > 0.0); // applied = 20
        // Simulate a fresh layout pass re-stamping natural bounds, then on_layout.
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // on_layout re-applies the shift immediately (paint is \u0026self and can't
        // sync, so we must leave the bounds already shifted) — no snap to top, no
        // compounding: applied is back to the scroll offset and child 0 is at -20.
        assert!((r.applied_offset - 20.0).abs() < f64::EPSILON);
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
        // A follow-up sync_shift is a no-op (already in sync) — no double shift.
        r.sync_shift();
        assert!((r.base.children[0].base().bounds.loc.y - (-20.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn wheel_step_is_viewport_fraction_not_fixed_lines() {
        // Regression guard: the step must scale with the viewport, so a small
        // viewport doesn't jump ~75% per notch (the original overshoot bug).
        let r = region_with_children(&[60.0, 60.0]); // vp 100
        let step = WHEEL_STEP_FRAC * r.base.bounds.size.h;
        // 10% of viewport per line of delta — gentle, proportional.
        assert!((step - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ensure_visible_scrolls_down_when_item_is_below_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, max 20
        // child[1] natural 60..120, off 0 → visual 60..120, below vp (0..100).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, 60.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled so the item's bottom (120) aligns with the viewport bottom
        // (100): offset = 120 - 100 = 20 (= max_offset).
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
    }

    #[test]
    fn ensure_visible_scrolls_up_when_item_is_above_viewport() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(20.0); // applied_offset=20; child[0] visual y = 0 - 20 = -20
        // child[0] visual rect {y:-20, h:60} — top is above the viewport (y=0).
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, -20.0),
            Size::new(200.0, 60.0),
        ));
        // Scrolled back so the item's top (natural 0) aligns with viewport top.
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6);
    }

    /// **Centring is unconditional**, which is the difference from `ensure_visible`: a target
    /// already fully on screen but sitting off to one side is still brought back to the middle.
    /// That is what makes an overview read as a map — the current thing stays put and the
    /// neighbours move past it.
    #[test]
    fn center_on_puts_the_target_in_the_middle_even_when_it_is_already_visible() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0, 60.0]); // vp 100, content 240
        // child[1] natural 60..120: its middle is 90, the viewport's is 50.
        r.center_on(Rectangle::new(Point::new(0.0, 60.0), Size::new(200.0, 60.0)));
        assert!(
            (r.scroll_offset.get_untracked() - 40.0_f32).abs() < 1e-6,
            "90 (target middle) - 50 (viewport middle) = 40, not the 0 a minimal reveal would leave",
        );
    }

    /// The clamp is the whole end-of-content story: near the top the target stops short of the
    /// centre rather than the view scrolling into empty space. A surface that wants it centred
    /// there too pads its own content — the widget does not invent space its caller did not ask for.
    #[test]
    fn center_on_clamps_at_the_ends_instead_of_scrolling_into_nothing() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0, 60.0]);
        r.center_on(Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 60.0)));
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6, "the first child cannot go lower");
        r.center_on(Rectangle::new(Point::new(0.0, 180.0), Size::new(200.0, 60.0)));
        assert!(
            (r.scroll_offset.get_untracked() - 140.0_f32).abs() < 1e-6,
            "the last child stops at max_offset (240 content - 100 viewport)",
        );
    }

    /// **Content that fits is centred in place.** This is the half of "centred" that has nothing to
    /// do with scrolling, and the half that was missing: with three columns on a wide screen there
    /// is no offset that moves them anywhere, so the map opened hard against the top-left corner —
    /// which is the one thing an overview must never look like.
    #[test]
    fn content_smaller_than_the_region_is_centred_in_place() {
        let mut r = region_with_children(&[40.0]); // viewport 200 × 100, content 200 × 40
        r.axes = ScrollAxes::Both;
        r.reveal_align = RevealAlign::Center;
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(80.0, 40.0));
        r.sync_shift();

        let b = r.base.children[0].base().bounds;
        assert!((b.loc.x - 60.0).abs() < 1e-9, "(200 - 80) / 2 = 60 from the left, got {b:?}");
        assert!((b.loc.y - 30.0).abs() < 1e-9, "(100 - 40) / 2 = 30 from the top, got {b:?}");
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6
                && r.scroll_offset_x.get_untracked().abs() < 1e-6,
            "and it is not a scroll — there is nothing to scroll",
        );
    }

    /// The pad is **only** the slack, so it can never make the start of the content unreachable —
    /// the failure mode of a plain `justify_content: center`, which overflows both ways.
    #[test]
    fn overflowing_content_gets_no_centring_pad() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]); // content 180, viewport 100
        r.reveal_align = RevealAlign::Center;
        r.sync_shift();
        assert!(
            r.base.children[0].base().bounds.loc.y.abs() < 1e-9,
            "the first child stays at the top; centring here is the scroll's job",
        );
    }

    /// **A horizontal region centres horizontally.** A strip of cards is the case that needs it,
    /// and `ensure_visible` — vertical by construction — could never serve one.
    #[test]
    fn center_on_works_on_whichever_axis_the_region_scrolls() {
        let mut r = region_with_children(&[60.0]);
        r.axes = ScrollAxes::Horizontal;
        // One wide child: viewport 200 wide, content 600.
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 60.0));
        // A card at x 300..400 — its middle is 350, the viewport's is 100.
        r.center_on(Rectangle::new(Point::new(300.0, 0.0), Size::new(100.0, 60.0)));
        assert!(
            (r.scroll_offset_x.get_untracked() - 250.0_f32).abs() < 1e-6,
            "350 - 100 = 250 horizontally",
        );
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "and nothing vertically — that axis does not scroll here",
        );
    }

    /// **An eased region walks to its target over several frames**, and keeps asking for them
    /// until it arrives — a view that cut between two positions of the same picture would read as
    /// a different picture.
    #[test]
    fn a_smooth_region_glides_to_its_target_instead_of_jumping() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]).smooth_scroll(0.09); // content 180, vp 100
        r.sync_shift(); // the first sync snaps: a surface opens where it belongs
        assert!(r.applied_offset.abs() < 1e-9, "starts settled at the top");

        r.scroll_to(80.0); // programmatic — the eased path
        assert!(
            r.applied_offset < 80.0,
            "not there yet on the frame the move was asked for: {}",
            r.applied_offset,
        );
        let mut frames = 0;
        while r.tick(1.0 / 60.0) && frames < 120 {
            frames += 1;
        }
        assert!(frames > 1, "it took more than one frame — that is the animation");
        assert!(frames < 120, "and it finished rather than easing forever");
        assert!(
            (r.applied_offset - 80.0).abs() < 0.5,
            "and it arrived exactly: {}",
            r.applied_offset,
        );
    }

    /// **A drag lands at once; a wheel glides.**
    ///
    /// The thing under the finger has to track it one to one — a thumb that lags reads as broken.
    /// A wheel notch has nothing under it: it is a discrete tenth-of-a-viewport step, so applying
    /// it instantly is a jump and a run of them is a run of jumps. Eased, the same notches read as
    /// one movement. Snapping the wheel was what made the map jerky under the mouse.
    #[test]
    fn a_drag_lands_at_once_while_a_wheel_glides() {
        let mut r = region_with_children(&[60.0, 60.0, 60.0]).smooth_scroll(0.09);
        r.sync_shift();

        let wheel = wheel(0.0, 3.0);
        r.set_offset_y(50.0, Some(&wheel));
        assert!(
            r.applied_offset < 50.0,
            "the wheel has somewhere to travel: {}",
            r.applied_offset,
        );
        let mut frames = 0;
        while r.tick(1.0 / 60.0) && frames < 240 {
            frames += 1;
        }
        assert!(frames > 1, "over several frames — that is the glide");

        let drag = Event::PointerMove(crate::event::PointerEvent::at(Point::new(0.0, 0.0)));
        r.set_offset_y(10.0, Some(&drag));
        assert!(
            (r.applied_offset - 10.0).abs() < 1e-9,
            "a pointer drag is there on the same frame: {}",
            r.applied_offset,
        );
    }

    /// **REVERSED 2026-08-12.** This used to assert the opposite — that an overscrolling surface
    /// can be wheeled even when its content fits — on the reasoning that its position is carried by
    /// the centring pad, so `max_offset` is `0` whenever it fits, and clamping the range to
    /// `[0, 0]` left it "frozen under the mouse".
    ///
    /// Antonio, driving, reversed it: *"mouse wheel can still scroll even if there's no need and
    /// put card off the screen"*. Freedom to scroll what is entirely visible is not responsiveness,
    /// it is a way to lose the picture — and in the exposé it did exactly that, wheeling the whole
    /// map out of the window with no way back but the keyboard. Overscroll is a **cushion past the
    /// ends**; content that fits has no ends to cushion.
    #[test]
    fn a_wheel_does_not_move_an_overscrolling_region_whose_content_fits() {
        let mut r = region_with_children(&[40.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        assert_eq!(r.max_offset(), 0.0, "the content fits — there is nothing to scroll *into*");
        r.on_event(&wheel(0.0, 3.0));
        assert_eq!(
            r.scroll_offset.get_untracked(),
            0.0,
            "so the wheel leaves it exactly where it is",
        );
    }

    /// **Shift+wheel reaches the horizontal axis** — the host maps the modifier to `delta_x`, and a
    /// horizontal region consumes it on the same terms.
    #[test]
    fn a_horizontal_delta_scrolls_a_horizontal_overscrolling_region() {
        // Wide enough to actually overflow: a region whose content fits has nowhere to go
        // (see `a_wheel_does_not_move_an_overscrolling_region_whose_content_fits`).
        let mut r = region_with_children(&[40.0]);
        r.axes = ScrollAxes::Horizontal;
        r.base.children[0].base_mut().bounds =
            Rectangle::new(Point::new(0.0, 0.0), Size::new(600.0, 40.0));
        r = r.overscroll(true);
        r.on_event(&wheel(3.0, 0.0));
        assert!(
            r.scroll_offset_x.get_untracked() > 0.0,
            "shift+wheel moves it sideways: {}",
            r.scroll_offset_x.get_untracked(),
        );
    }

    /// Moving the cursor supersedes wherever the user had wheeled to — otherwise the map would
    /// centre every later selection at the same offset from the middle, for good.
    #[test]
    fn moving_the_cursor_cancels_a_wheel_on_an_overscrolling_region() {
        // Tall enough to actually overflow, so there is a wheel position to cancel.
        let mut r = region_with_children(&[160.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        r.on_event(&wheel(0.0, 3.0));
        assert!(r.scroll_offset.get_untracked() > 0.0);

        let mut row = crate::widgets::Row::new();
        row.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(40.0, 40.0));
        row.nav_state().set(true);
        r.base.children.push(Box::new(row));
        r.follow_revealed();
        assert!(
            r.scroll_offset.get_untracked().abs() < 1e-6,
            "back to zero, so the pad centres the new target exactly",
        );
    }

    /// **No combination of pad and wheel can empty the view.** The position of an overscrolling
    /// surface is two numbers added together, and bounding each separately does not bound the sum —
    /// which is how the map twice ended up as an empty window with everything scrolled out of it.
    #[test]
    fn the_content_can_never_be_pushed_off_screen() {
        let mut r = region_with_children(&[40.0]).overscroll(true);
        r.reveal_align = RevealAlign::Center;
        // Wheel as hard as the range allows, repeatedly.
        for _ in 0..50 {
            r.on_event(&wheel(0.0, 20.0));
        }
        for _ in 0..200 {
            r.tick(1.0 / 60.0);
        }
        let child = r.base.children[0].base().bounds;
        let vp = r.base.bounds;
        assert!(
            child.loc.y < vp.loc.y + vp.size.h && child.loc.y + child.size.h > vp.loc.y,
            "the content still overlaps the viewport: {child:?} in {vp:?}",
        );
    }

    /// **`scrollbars(false)` draws no bar at all.** Gating only the reserved gutter left the thumb
    /// painted — visible on screen, grabbable, and taking up no space.
    #[test]
    fn a_region_with_scrollbars_off_paints_no_thumb() {
        let with = region_with_children(&[60.0, 60.0, 60.0]);
        assert!(with.thumb_rect().is_some(), "overflowing, so it would normally show one");
        let without = region_with_children(&[60.0, 60.0, 60.0]).scrollbars(false);
        assert!(without.thumb_rect().is_none(), "and none at all when they are off");
    }

    /// The alignment travels as a property, so a described surface reaches it — the capability
    /// `axes` and `placeholder` both lacked for months.
    #[test]
    fn the_reveal_alignment_is_a_property_and_defaults_to_minimal() {
        use crate::prop::{PropInput, SetProp};
        assert_eq!(ScrollRegion::new().clone_reveal_align(), RevealAlign::Minimal);
        let centred = ScrollRegion::new().set_prop("reveal_align", &PropInput::Text("center".into()));
        assert_eq!(centred.clone_reveal_align(), RevealAlign::Center);
    }

    #[test]
    fn ensure_visible_is_noop_when_item_already_visible() {
        let mut r = region_with_children(&[60.0, 60.0]);
        r.scroll_to(0.0);
        // child[0] natural 0..60 → visual 0..60, fully inside vp 0..100.
        r.ensure_visible(Rectangle::new(
            Point::new(0.0, 0.0),
            Size::new(200.0, 60.0),
        ));
        assert!(r.scroll_offset.get_untracked().abs() < 1e-6, "no scroll when already visible");
    }

    #[test]
    fn scroll_to_child_brings_a_direct_child_into_view() {
        let mut r = region_with_children(&[60.0, 60.0]); // max 20
        r.scroll_to_child(1); // child[1] below → scroll to 20
        assert!((r.scroll_offset.get_untracked() - 20.0_f32).abs() < 1e-6);
        // Out-of-range index is a no-op (no panic, no scroll change).
        let before = r.scroll_offset.get_untracked();
        r.scroll_to_child(99);
        assert!((r.scroll_offset.get_untracked() - before).abs() < f32::EPSILON);
    }

    /// A relayout that GROWS the viewport (resize / zoom-out) must re-clamp a
    /// stale offset — otherwise content stays shifted past the edge with no
    /// scrollbar left to bring it back (the "scrollbar disappears after zoom"
    /// regression).
    #[test]
    fn on_layout_reclamps_a_stale_offset_when_the_viewport_grows() {
        let mut r = region_with_children(&[60.0, 60.0]); // vp 100, content 120
        r.scroll_to(20.0); // at max
        // The viewport grows past the content; layout restamps natural bounds.
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 200.0));
        let mut y = 0.0;
        for c in r.base.children.iter_mut() {
            c.base_mut().bounds = Rectangle::new(Point::new(0.0, y), Size::new(200.0, 60.0));
            y += 60.0;
        }
        r.on_layout();
        // Offset re-clamped to the new max (0): content back at the top, not left
        // shifted off-screen.
        assert!(r.scroll_offset.get_untracked().abs() < f32::EPSILON);
        assert!(r.base.children[0].base().bounds.loc.y.abs() < f64::EPSILON);
    }

    /// A held press in the scrollbar track pages once immediately, then repeats
    /// after the initial delay at the repeat interval, and stops on release.
    #[test]
    fn held_track_press_repeats_paging_until_released() {
        // vp 100 (h), content 600 → max 500; one page = 100.
        let mut r = region_with_children(&[300.0, 300.0]);
        // Press in the vertical track lane, below the thumb.
        let pos = Point::new(195.0, 90.0);
        assert_eq!(crate::component::dispatch(&mut r, &Event::pointer_pressed(pos, PointerButton::Left)), Handled::Yes);
        assert!((r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3, "first page fires on press");
        // Held but before the initial delay: animating, no extra page.
        assert!(r.tick(0.2), "held press keeps the host ticking");
        assert!((r.scroll_offset.get_untracked() - 100.0).abs() < 1e-3);
        // Past the delay: a repeat fires.
        r.tick(0.2);
        assert!((r.scroll_offset.get_untracked() - 200.0).abs() < 1e-3, "repeat after the delay");
        // Steady repeat at the interval.
        r.tick(0.1);
        assert!((r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3, "repeat at the interval");
        // Release disarms it: no further paging however long we tick.
        crate::component::dispatch(&mut r, &Event::pointer_released(pos, PointerButton::Left));
        assert!(!r.tick(1.0));
        assert!((r.scroll_offset.get_untracked() - 300.0).abs() < 1e-3, "stopped on release");
    }

    // ── Horizontal axis ──

    /// A region with the given `axes` and child WIDTHS laid left→right in a 100×100
    /// viewport (mirrors `region_with_children`, which stamps heights).
    fn h_region_with_widths(axes: ScrollAxes, child_widths: &[f64]) -> ScrollRegion {
        let mut r = ScrollRegion::new().axes(axes);
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        r.base.children.clear();
        let mut x = 0.0;
        for &w in child_widths {
            let mut child = crate::widgets::Flex::column();
            child.base_mut().bounds = Rectangle::new(Point::new(x, 0.0), Size::new(w, 100.0));
            r.base.children.push(Box::new(child));
            x += w;
        }
        r
    }

    #[test]
    fn horizontal_extent_and_max_offset() {
        let r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // content 160, vp 100
        assert!((r.content_extent_x() - 160.0).abs() < f64::EPSILON);
        assert!((r.max_offset_x() - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn horizontal_axis_off_means_no_horizontal_scroll() {
        // A vertical-only region ignores horizontal overflow entirely.
        let r = h_region_with_widths(ScrollAxes::Vertical, &[80.0, 80.0]);
        assert_eq!(r.max_offset_x(), 0.0);
        assert!(r.h_thumb_rect().is_none());
    }

    #[test]
    fn horizontal_thumb_appears_only_when_content_overflows() {
        let fits = h_region_with_widths(ScrollAxes::Horizontal, &[40.0, 40.0]); // 80 ≤ 100
        assert!(fits.h_thumb_rect().is_none());
        let over = h_region_with_widths(ScrollAxes::Horizontal, &[80.0, 80.0]); // 160 > 100
        assert!(over.h_thumb_rect().is_some());
    }

    #[test]
    fn scroll_to_x_clamps_to_bounds() {
        let mut r = h_region_with_widths(ScrollAxes::Both, &[80.0, 80.0]); // max_x 60
        assert!((r.scroll_to_x(1000.0) - 60.0).abs() < f32::EPSILON, "clamped to max");
        assert!((r.scroll_to_x(-5.0)).abs() < f32::EPSILON, "clamped to 0");
    }

    /// Nested regions: the wheel goes to children FIRST, so an inner hovered
    /// scrollable consumes it and the outer region stays put (innermost wins).
    #[test]
    fn nested_region_consumes_the_wheel_before_the_outer_one() {
        // Outer: 200×100 viewport, vertical content 300 (scrollable).
        let mut outer = ScrollRegion::new();
        outer.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(200.0, 100.0));
        // Inner region at the top of the outer content: 100×50 viewport, content 200.
        let mut inner = ScrollRegion::new();
        inner.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 50.0));
        let mut tall = crate::widgets::Flex::column();
        tall.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 200.0));
        inner.base.children.push(Box::new(tall));
        let inner_offset = inner.scroll_offset();
        outer.base.children.push(Box::new(inner));
        // Filler that makes the OUTER content overflow too.
        let mut filler = crate::widgets::Flex::column();
        filler.base_mut().bounds =
            Rectangle::new(Point::new(0.0, 50.0), Size::new(200.0, 250.0));
        outer.base.children.push(Box::new(filler));

        // The wheel is routed by position now: over the inner region, it is the inner region's.
        let pos = Point::new(10.0, 10.0);
        let handled = crate::component::dispatch(&mut outer, &wheel_at(pos, 0.0, 1.0));
        assert_eq!(handled, Handled::Yes);
        assert!(
            inner_offset.get_untracked() > 0.0,
            "inner (hovered) region scrolled"
        );
        assert!(
            outer.scroll_offset.get_untracked().abs() < f32::EPSILON,
            "outer region did not scroll while the inner one consumed the wheel"
        );

        // Over the outer region but OFF the inner one: now the outer scrolls.
        let pos = Point::new(150.0, 80.0);
        let handled = crate::component::dispatch(&mut outer, &wheel_at(pos, 0.0, 1.0));
        assert_eq!(handled, Handled::Yes);
        assert!(
            outer.scroll_offset.get_untracked() > 0.0,
            "outer region scrolls when no child consumed the wheel"
        );
    }

    /// A horizontal-only region shows NO vertical thumb even with tall content, and a
    /// horizontal thumb when it overflows — the axes gate both scrollbars.
    #[test]
    fn horizontal_only_region_gates_both_scrollbars() {
        let mut r = ScrollRegion::new().horizontal();
        r.base.bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(100.0, 100.0));
        let mut child = crate::widgets::Flex::column();
        child.base_mut().bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(300.0, 500.0));
        r.base.children.push(Box::new(child));
        assert!(r.thumb_rect().is_none(), "vertical axis disabled → no vertical thumb");
        assert_eq!(r.max_offset(), 0.0);
        assert!(r.h_thumb_rect().is_some(), "horizontal overflow → horizontal thumb");
    }

    // ── Reporting the position (F004/P011/T3) ──────────────────────────────────────────────
    //
    // Watching a scroll area must not require implementing one. These pin the three listeners:
    // what fires, when, and how a listener tells the user's gesture from its own `scroll_to`.

    /// What each callback recorded: its label, and whether an input caused it.
    type Log = Rc<RefCell<Vec<(&'static str, bool)>>>;

    /// A region wired to all three callbacks, plus the log they write to.
    fn recorder() -> (Log, ScrollRegion) {
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let (a, b, c) = (log.clone(), log.clone(), log.clone());
        let r = region_with_children(&[400.0])
            .on_scroll_start(move |s| a.borrow_mut().push(("start", s.event.is_some())))
            .on_scroll(move |s| b.borrow_mut().push(("scroll", s.event.is_some())))
            .on_scroll_end(move |s| c.borrow_mut().push(("end", s.event.is_some())));
        (log, r)
    }

    /// The host's own `scroll_to` reports too — with **no event**, which is exactly how a listener
    /// avoids reacting to its own call and fighting the user.
    #[test]
    fn a_programmatic_scroll_reports_with_no_event() {
        let (log, mut r) = recorder();
        r.scroll_to(40.0);
        assert_eq!(
            log.borrow().as_slice(),
            &[("start", false), ("scroll", false)],
            "start then scroll, neither caused by input",
        );
    }

    /// Nothing is reported when nothing moved — a clamped no-op at either end is not a scroll.
    #[test]
    fn a_clamped_no_op_reports_nothing() {
        let (log, mut r) = recorder();
        r.scroll_to(0.0);
        assert!(log.borrow().is_empty(), "already at the top; nothing moved");
        r.scroll_to(1e9);
        let n = log.borrow().len();
        r.scroll_to(1e9);
        assert_eq!(log.borrow().len(), n, "already at the bottom the second time");
    }

    /// The wheel carries its event through, and `on_scroll` fires per movement.
    #[test]
    fn the_wheel_reports_with_the_event_that_caused_it() {
        let (log, mut r) = recorder();
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(50.0, 50.0)));
        crate::component::dispatch(&mut r, &wheel(0.0, 1.0));
        assert_eq!(log.borrow().as_slice(), &[("start", true), ("scroll", true)]);
    }

    /// A wheel gesture has no release, so its end is a **pause**: `tick` must reach the idle
    /// timeout before `scroll_end` fires, and must keep asking for frames until it does.
    #[test]
    fn a_wheel_gesture_ends_after_the_idle_timeout_and_not_before() {
        let (log, mut r) = recorder();
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(50.0, 50.0)));
        crate::component::dispatch(&mut r, &wheel(0.0, 1.0));

        assert!(r.tick(WHEEL_IDLE_END / 2.0), "still pending: keep the frames coming");
        assert!(
            !log.borrow().iter().any(|(k, _)| *k == "end"),
            "half the idle time is not the end of a gesture",
        );
        r.tick(WHEEL_IDLE_END);
        assert_eq!(log.borrow().last(), Some(&("end", false)), "settled");
        assert!(!r.tick(1.0), "and it stops asking for frames once it has settled");
    }

    /// A thumb drag ends on its release — no timer involved, and the release is the event reported.
    #[test]
    fn a_thumb_drag_ends_on_the_release() {
        let (log, mut r) = recorder();
        let lane_x = r.base.bounds.loc.x + r.base.bounds.size.w - 2.0;
        crate::component::dispatch(&mut r, &Event::pointer_pressed(Point::new(lane_x, 5.0), PointerButton::Left));
        crate::component::dispatch(&mut r, &Event::pointer_moved(Point::new(lane_x, 40.0)));
        assert!(log.borrow().iter().any(|(k, _)| *k == "scroll"), "the drag scrolled it");
        assert!(!log.borrow().iter().any(|(k, _)| *k == "end"), "not while the button is down");

        crate::component::dispatch(&mut r, &Event::pointer_released(Point::new(900.0, 900.0), PointerButton::Left));
        assert_eq!(log.borrow().last(), Some(&("end", true)), "the release ended it");
    }

    /// The payload answers the questions a listener would otherwise have to ask the region.
    #[test]
    fn the_payload_carries_the_whole_picture() {
        let seen = Rc::new(RefCell::new(None));
        let sink = seen.clone();
        let mut r = region_with_children(&[400.0]).on_scroll(move |s| {
            sink.replace(Some((s.offset_y, s.max_y, s.content.h, s.viewport.h)));
        });
        r.scroll_to(1e9);
        let (offset_y, max_y, content_h, viewport_h) = seen.borrow().expect("reported");
        assert_eq!(offset_y, max_y, "scrolled to the bottom, and it says so without asking");
        assert_eq!(content_h, 400.0);
        assert_eq!(viewport_h, 100.0);
        assert_eq!(max_y, (content_h - viewport_h) as f32);
    }
}
