//! Builders for ScrollRegion.

use super::*;

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
            follow_bottom: false,
            previous_bottom: 0.0,
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
    /// or `WHEEL_IDLE_END` seconds after the last wheel movement.
    ///
    /// A wheel gesture has no release — the browsers' `scrollend` waits for a pause for the same
    /// reason — so this one is a timer, and the region keeps asking for frames until it fires.
    #[heca_grid_ui_macros::host_only("behaviour crosses as an Intent, never a callback")]
    pub fn on_scroll_end(mut self, f: impl for<'a> Fn(ScrollInfo<'a>) + 'static) -> Self {
        self.on_scroll_end = Some(Box::new(f));
        self
    }
    /// Keep the bottom visible after content grows, only while already at the bottom.
    /// Disabled by default. Scrolling up suspends following; returning to the bottom resumes it.
    #[heca_grid_ui_macros::prop]
    pub fn follow_bottom(mut self, follow: bool) -> Self {
        self.follow_bottom = follow;
        self
    }

    /// Scroll horizontally only (children overflow left↔right).
    #[heca_grid_ui_macros::host_only(
        "carries no value — a property needs one; the equivalent is an explicit setting"
    )]
    pub fn horizontal(mut self) -> Self {
        self.axes = ScrollAxes::Horizontal;
        self
    }
    /// Scroll on both axes.
    #[heca_grid_ui_macros::host_only(
        "carries no value — a property needs one; the equivalent is an explicit setting"
    )]
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
}
