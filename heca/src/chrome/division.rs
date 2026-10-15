//! **How one window is divided** between the chrome at its edges and the panes in the middle.

use heca_core::layout::types::{Point, Rectangle, Size};

/// **How one window is divided** between the chrome at its edges and the panes in the middle.
///
/// Built for a window, never by hand: the `window` field is private, so the only way to get one is
/// [`ChromeConfig::for_window`] — which is where the sidebars are made to yield, so no caller has
/// to know that rule.
///
/// **The widths in here are what a sidebar ACTUALLY gets**, not what it asked for. Everything reads
/// them — the content rect below, and the sidebar shells `chrome/scene.rs` builds — so the panes and
/// the chrome cannot disagree about where the edge is.
#[derive(Clone, Copy, Debug)]
pub struct ChromeConfig {
    pub tab_bar_height: f32,
    pub status_bar_height: f32,
    /// What the left sidebar gets here — its own width, or its share of what was left over.
    pub left_sidebar_width: f32,
    /// What the right sidebar gets here, on the same rule.
    pub right_sidebar_width: f32,
    pub sidebar_gap: f32,
    /// The window this division was computed for. Private on purpose: it is not a knob, and holding
    /// it is what stops [`content_rect`](Self::content_rect) being asked about a different window
    /// than the sidebars yielded into.
    window: Size,
}

impl ChromeConfig {
    /// **Divide a window** between the bars, the sidebars that want `left`/`right`, and the panes.
    ///
    /// The scrolling area is owed [`MIN_COLUMN_WIDTH`](heca_core::layout::scrolling::MIN_COLUMN_WIDTH) and gets it first; the sidebars share
    /// whatever is left, in proportion to what they asked for, down to nothing. That is the whole
    /// rule, and it lives here because a window too narrow for both sidebars is not the sidebars'
    /// business or the panes' — it is the division's.
    pub fn for_window(
        window: Size,
        tab_bar_height: f32,
        status_bar_height: f32,
        left: f32,
        right: f32,
        sidebar_gap: f32,
    ) -> Self {
        let sidebar_gap = sidebar_gap.max(0.0);
        let (left, right) = yield_to_content(window.w as f32, left, right, sidebar_gap);
        Self {
            tab_bar_height,
            status_bar_height,
            left_sidebar_width: left,
            right_sidebar_width: right,
            sidebar_gap,
            window,
        }
    }

    /// The chrome division for `state`'s current window — **the one way the app builds this.**
    ///
    /// The window size is read here rather than passed in, so no caller divides against one size and
    /// asks about another. Two files had privately reimplemented that conversion
    /// (`inner_size()` ÷ `scale_factor`) alongside their own copy of the struct literal.
    pub(crate) fn of(state: &crate::app_state::AppState) -> Self {
        let phys = state.window.inner_size();
        let scale = state.scale_factor as f32;
        Self::for_window(
            Size::new(
                (phys.width as f32 / scale) as f64,
                (phys.height as f32 / scale) as f64,
            ),
            state.tab_bar_height(),
            state.status_bar_height(),
            state.left_sidebar_width(),
            state.right_sidebar_width(),
            state.appearance.effective_sidebar_gap(&state.theme),
        )
    }

    /// The window this was divided for, in logical pixels.
    pub fn window(&self) -> Size {
        self.window
    }

    /// The rectangle available for pane content. Chrome occupies the outer edges; panes get the
    /// centre, and never less than [`MIN_COLUMN_WIDTH`](heca_core::layout::scrolling::MIN_COLUMN_WIDTH) of it — see [`for_window`](Self::for_window).
    pub fn content_rect(&self) -> Rectangle {
        let (window_width, window_height) = (self.window.w as f32, self.window.h as f32);
        let x = self.left_sidebar_width + self.reserved_gap(self.left_sidebar_width);
        let y = self.tab_bar_height;
        let right_reserved = self.right_sidebar_width + self.reserved_gap(self.right_sidebar_width);
        let w = (window_width - x - right_reserved.min((window_width - x).max(0.0))).max(0.0);
        let h = (window_height - self.tab_bar_height - self.status_bar_height).max(0.0);
        Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(w as f64, h as f64),
        )
    }

    /// The gap a sidebar of this width costs — one gap when it is there, nothing when it is not.
    fn reserved_gap(&self, width: f32) -> f32 {
        if width > 0.0 { self.sidebar_gap } else { 0.0 }
    }
}

/// **The sidebars' share of a window too narrow for everyone** — pure, so the rule is testable
/// without a window.
///
/// The panes are owed [`MIN_COLUMN_WIDTH`](heca_core::layout::scrolling::MIN_COLUMN_WIDTH). What is left over is the sidebars', split in proportion
/// to what each asked for, so neither is starved for the other's benefit and a wide sidebar gives up
/// more than a narrow one. Widening the window restores both exactly: nothing here is written back
/// to the region's stored size, which is the user's own choice and stays theirs.
fn yield_to_content(window_width: f32, left: f32, right: f32, sidebar_gap: f32) -> (f32, f32) {
    let (left, right) = (left.max(0.0), right.max(0.0));
    let gaps =
        if left > 0.0 { sidebar_gap } else { 0.0 } + if right > 0.0 { sidebar_gap } else { 0.0 };
    let floor = heca_core::layout::scrolling::MIN_COLUMN_WIDTH as f32;
    let room = (window_width - floor - gaps).max(0.0);
    let wanted = left + right;
    if wanted <= room || wanted <= 0.0 {
        return (left, right);
    }
    let share = room / wanted;
    (left * share, right * share)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window of `w`x`h` divided between two sidebars that want `left`/`right`.
    fn divided(w: f32, h: f32, left: f32, right: f32, gap: f32) -> ChromeConfig {
        ChromeConfig::for_window(Size::new(w as f64, h as f64), 32.0, 24.0, left, right, gap)
    }

    #[test]
    fn test_content_rect_full() {
        let c = ChromeConfig::for_window(Size::new(1280.0, 800.0), 32.0, 24.0, 200.0, 200.0, 0.0);
        let r = c.content_rect();
        assert_eq!(r.loc.x, 200.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 880.0);
        assert_eq!(r.size.h, 744.0);
    }

    /// **The panes keep a column's width; the sidebars give it up.** Two 300px sidebars in a 500px
    /// window must not leave the scrolling area zero wide: the content is owed `MIN_COLUMN_WIDTH`
    /// and takes it first.
    #[test]
    fn the_sidebars_yield_before_the_scrolling_area_is_squeezed_away() {
        let min = heca_core::layout::scrolling::MIN_COLUMN_WIDTH;
        let r = divided(500.0, 600.0, 300.0, 300.0, 0.0).content_rect();
        assert_eq!(
            r.size.w, min,
            "the panes keep exactly the floor they are owed",
        );
    }

    /// Both give up the same *proportion*, not the same number of pixels — so a wide sidebar pays
    /// more than a narrow one and neither is starved for the other's benefit.
    #[test]
    fn a_wide_sidebar_gives_up_more_than_a_narrow_one() {
        let c = divided(500.0, 600.0, 300.0, 100.0, 0.0);
        assert!(
            c.left_sidebar_width > c.right_sidebar_width * 2.9,
            "left asked for 3x as much, so it keeps ~3x as much: {} vs {}",
            c.left_sidebar_width,
            c.right_sidebar_width,
        );
        assert_eq!(
            c.content_rect().size.w,
            heca_core::layout::scrolling::MIN_COLUMN_WIDTH
        );
    }

    /// **Widening gives it all back.** The yield is derived per window and never written to the
    /// region's stored size, which is the user's own choice.
    #[test]
    fn widening_the_window_restores_the_width_the_user_chose() {
        assert!(divided(500.0, 600.0, 300.0, 300.0, 0.0).left_sidebar_width < 300.0);
        assert_eq!(
            divided(1600.0, 600.0, 300.0, 300.0, 0.0).left_sidebar_width,
            300.0
        );
    }

    /// A window with no room for either sidebar hands the whole of it to the panes, rather than
    /// leaving a sliver of chrome nobody can use.
    #[test]
    fn a_window_narrower_than_the_floor_keeps_no_sidebar_at_all() {
        let c = divided(120.0, 600.0, 300.0, 300.0, 8.0);
        assert_eq!(c.left_sidebar_width, 0.0);
        assert_eq!(c.right_sidebar_width, 0.0);
        assert_eq!(
            c.content_rect().size.w,
            120.0,
            "all of it goes to the panes"
        );
    }

    #[test]
    fn test_content_rect_no_sidebars() {
        let r = divided(1024.0, 768.0, 0.0, 0.0, 0.0).content_rect();
        assert_eq!(r.loc.x, 0.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 1024.0);
        assert_eq!(r.size.h, 712.0);
    }

    #[test]
    fn test_content_rect_reserves_sidebar_gap_between_sidebars_and_content() {
        let r = divided(1280.0, 800.0, 200.0, 200.0, 12.0).content_rect();
        assert_eq!(r.loc.x, 212.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 856.0);
        assert_eq!(r.size.h, 744.0);
    }
}
