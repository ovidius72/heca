use super::*;
use heca_core::layout::types::{Point, Size};

/// The default bar heights, from the one place that says them: the config's appearance.
fn bars() -> heca_config::appearance::AppearanceConfig {
    heca_config::appearance::AppearanceConfig::default()
}

/// **With a sidebar hidden, the panes own that edge right up to the window** — there is no strip
/// along it that belongs to nobody.
///
/// The hit test used to answer this itself, and got it wrong twice: it fell back to a hardcoded
/// `40.0` on the left, the width of a rail that no longer exists, so a hidden left sidebar left a
/// 40px band where clicking a pane did nothing; and it guarded the right edge with an **x**
/// compared against half the window's **height**. It asks `content_rect` now, so both edges are
/// whatever the division says they are — which is what this pins.
#[test]
fn a_hidden_sidebar_leaves_no_strip_the_panes_do_not_own() {
    let r = ChromeConfig::for_window(
        Size::new(1280.0, 800.0),
        bars().effective_top_bar_height(),
        bars().effective_bottom_bar_height(),
        0.0,
        0.0,
        0.0,
    )
    .content_rect();

    assert!(
        r.contains(Point::new(1.0, 100.0)),
        "the left edge is the window's"
    );
    assert!(
        r.contains(Point::new(1279.0, 100.0)),
        "and so is the right edge"
    );
    assert!(
        r.contains(Point::new(700.0, 100.0)),
        "a point past half the window's height is still in the panes"
    );
}

#[test]
fn test_chrome_content_rect_left_sidebar() {
    let r = ChromeConfig::for_window(
        heca_core::layout::types::Size::new(1280.0, 800.0),
        bars().effective_top_bar_height(),
        bars().effective_bottom_bar_height(),
        200.0,
        0.0,
        0.0,
    )
    .content_rect();
    assert_eq!(r.loc.x, 200.0);
    assert_eq!(r.loc.y, 32.0);
    assert_eq!(r.size.w, 1080.0);
    assert_eq!(r.size.h, 744.0);
}

#[test]
fn test_chrome_content_rect_no_sidebars() {
    let r = ChromeConfig::for_window(
        heca_core::layout::types::Size::new(1280.0, 800.0),
        bars().effective_top_bar_height(),
        bars().effective_bottom_bar_height(),
        40.0,
        0.0,
        0.0,
    )
    .content_rect();
    assert_eq!(r.loc.x, 40.0);
    assert_eq!(r.loc.y, 32.0);
    assert_eq!(r.size.w, 1240.0);
    assert_eq!(r.size.h, 744.0);
}

/// **The columns are pushed only with the pointer over them, near an edge**: hardest at the edge,
/// nothing in the middle, and nothing over a sidebar — so a row moved inside a sidebar leaves them
/// where they are.
#[test]
fn the_columns_are_pushed_only_near_their_own_edges() {
    use heca_core::layout::{Point, Rectangle, Size};
    let area = Rectangle::new(Point::new(200.0, 40.0), Size::new(800.0, 600.0));
    let push = |x: f32| super::edge_push((x, 300.0), area, 100.0);
    assert_eq!(push(200.0), -1.0, "hardest at the left edge");
    assert_eq!(push(250.0), -0.5, "half way into the trigger");
    assert_eq!(push(600.0), 0.0, "nothing in the middle");
    assert_eq!(push(950.0), 0.5, "half way into the right trigger");
    assert_eq!(push(150.0), 0.0, "nothing over the left sidebar");
    assert_eq!(push(1050.0), 0.0, "nothing over the right sidebar");
    assert_eq!(
        super::edge_push((250.0, 10.0), area, 100.0),
        0.0,
        "nothing above the columns either"
    );
}
