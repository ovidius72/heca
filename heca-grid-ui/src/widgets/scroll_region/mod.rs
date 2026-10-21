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
//! [`StyleExt`], so it can double as a **scrollable
//! surface** (`.background(..).border(..)`). Nested regions compose: the wheel is
//! offered to children first, so the innermost hovered scrollable wins. A distinct
//! scrollbar color token and `PageUp`/`PageDown` (app actions) are future work.

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{
    Base, Component, Event, Handled, PaintCx, WidgetIntent, paint_child, shift_subtree,
};
use crate::effects::Eased;
use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
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
/// row behind it, and the press order changed to avoid it stopped rows being draggable at all.
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
/// How far one **keyboard page** moves, as a fraction of the viewport.
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
    /// Follow new content only when the viewport was at its previous bottom.
    follow_bottom: bool,
    /// Previous layout's maximum vertical offset, compared in signal precision.
    previous_bottom: f32,
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
    /// The viewport size is part of the key: shrinking must keep the same cursor in view too.
    last_revealed: Option<(f64, f64, Size)>,
    /// The **natural rect** of whatever last asked to be revealed — what [`center_pad`](Self::center_pad) keeps
    /// centring while nothing is asking.
    ///
    /// A subtree can stop asking without anything having moved: a cursor-following grid withholds
    /// the reveal while the pointer is driving it (`Component::reveals_subtree`), because pointing
    /// at a card must not scroll it. Without this the pad would read "nothing to follow" and fall
    /// back to centring the *content as a whole* — so hovering would slide the map anyway, just to
    /// a different place. Holding the last target means the picture simply stays where the keyboard
    /// left it.
    last_reveal_rect: Option<Rectangle>,
    /// Seconds left before an idle wheel gesture is declared over, or `None` when nothing is
    /// waiting. A wheel has no release, so its end is a **silence**, not an event — see
    /// [`WHEEL_IDLE_END`].
    wheel_idle: Option<f32>,
}

/// A position listener. The higher-ranked bound is what lets [`ScrollInfo`] borrow the event that
/// caused the move instead of copying a description of it.
type ScrollListener = Box<dyn for<'a> Fn(ScrollInfo<'a>)>;


mod builders;
mod component;
mod geometry;
mod motion;
mod reveal;
mod scroll;

impl Default for ScrollRegion { fn default() -> Self { Self::new() } }
impl LayoutExt for ScrollRegion {}
impl StyleExt for ScrollRegion {}
impl Parent for ScrollRegion {}

#[cfg(test)]
mod tests;
