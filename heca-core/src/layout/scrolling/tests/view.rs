//! What the window's view does as the active column changes.

use super::*;


#[test]
fn scroll_view_pans_and_clamps_to_content_bounds() {
    // Three ~half-viewport columns overflow the 1000px viewport, so the view
    // can pan — but only within the content (never scrolls the layout away).
    let mut space = space_with_columns(3);
    let vw = space.view.area.size.w;

    // Pan hard left, then again → second is a no-op (already at the left bound).
    space.m().scroll_view(-vw * 10.0);
    let left_bound = space.r().view_pos();
    space.m().scroll_view(-vw * 10.0);
    assert!(
        (space.r().view_pos() - left_bound).abs() < 1.0,
        "clamped at the left content bound"
    );

    // Pan hard right, then again → clamped at the right bound.
    space.m().scroll_view(vw * 10.0);
    let right_bound = space.r().view_pos();
    space.m().scroll_view(vw * 10.0);
    assert!(
        (space.r().view_pos() - right_bound).abs() < 1.0,
        "clamped at the right content bound"
    );
    assert!(
        right_bound > left_bound,
        "the right bound is further right than the left bound"
    );
}


#[test]
fn scroll_view_is_noop_when_all_columns_fit() {
    let mut space = test_scrolling_space();
    space.m().add_column(None, test_column(1, ColumnWidth::Proportion(0.5)), true);
    let before = space.r().view_pos();
    space.m().scroll_view(500.0);
    assert!(
        (space.r().view_pos() - before).abs() < f64::EPSILON,
        "a single column that fits the viewport does not scroll"
    );
}


#[test]
fn refocusing_active_column_refits_a_scrolled_view() {
    // After panning the view away, re-activating the already-active column must
    // scroll it back into view (#3: focusing a stranded column reveals it).
    let mut space = space_with_columns(3);
    let active = space.view.active_column;
    let fitted = space.r().view_pos();
    // Pan far away so the active column is off-screen.
    let pan = -space.view.area.size.w * 10.0;
    space.m().scroll_view(pan);
    assert!(
        (space.r().view_pos() - fitted).abs() > 1.0,
        "precondition: the view has moved away from the active column"
    );
    // Re-activating the same column re-fits it.
    space.m().activate_column(active);
    assert!(
        space.view.offset.is_static(),
        "ensure-visible snaps the view statically"
    );
}


/// **Changing focus does not make the view jump — it animates.** Moving the active column changes
/// the origin `view_pos` is measured from, so the offset has to shift by exactly that much or the
/// strip would snap on the first frame and only then slide.
///
/// The columns are wider than the window, so the target is far away and the move really animates
/// (a target within a pixel is applied at once, which hides the shift).
#[test]
fn changing_the_active_column_leaves_the_view_where_it_was_until_it_animates() {
    let mut space = test_scrolling_space();
    for id in 1..=3 {
        space
            .m()
            .add_column(None, test_column(id, ColumnWidth::Fixed(800.0)), true);
    }
    space.m().activate_column(0);
    let before = space.r().view_pos();

    space.m().activate_column(2);

    assert!(
        !space.view.offset.is_static(),
        "precondition: the far column is reached by an animation",
    );
    assert!(
        (space.r().view_pos() - before).abs() < 1.0,
        "the view started from {before}, and first drew at {}",
        space.r().view_pos(),
    );
}
