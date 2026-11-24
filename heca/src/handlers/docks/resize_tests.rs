use super::{current_heights, resized_shares};

/// The shares a drag step leaves, and the heights they give the same total.
fn step(heights: &[f64], by: f64) -> Vec<f64> {
    let shares = resized_shares(heights, 0, by, 40.0).expect("an edge to move");
    current_heights(Some(shares), heights.to_vec())
}

/// **Steps of a drag add up, though no layout runs between them** — the edge ends where the
/// pointer is. A drag sends one step per pointer move and several run before the next layout; each
/// starts from the shares the step before it wrote, so 30 then 20 moves the edge 50, as one step of
/// 50 does. (Read from the screen, the second step started from the old heights and the edge moved
/// only 20 — it fell behind the pointer.)
#[test]
fn steps_of_a_drag_add_up_without_a_layout_between_them() {
    let on_screen = vec![400.0, 400.0];
    let shares = resized_shares(&on_screen, 0, 30.0, 40.0).unwrap();
    // No layout: the screen still says 400 / 400, the shares say otherwise.
    let now = current_heights(Some(shares), on_screen.clone());
    let after_two = step(&now, 20.0);
    let after_one = step(&on_screen, 50.0);
    for (a, b) in after_two.iter().zip(&after_one) {
        assert!(
            (a - b).abs() < 0.01,
            "30 + 20 is 50: {after_two:?} vs {after_one:?}"
        );
    }
    assert!((after_two[0] - 450.0).abs() < 0.01, "{after_two:?}");
}

/// **Before the first step there are no shares** — the docks start from what is on screen, so a
/// dock whose own body asked for its size does not jump when the drag begins.
#[test]
fn the_first_step_starts_from_the_screen() {
    let on_screen = vec![300.0, 500.0];
    assert_eq!(current_heights(None, on_screen.clone()), on_screen);
}

/// **Neither neighbour goes below the floor**, and the last dock moves the edge above it.
#[test]
fn a_drag_keeps_both_docks_above_the_floor() {
    let heights = step(&[400.0, 400.0], 10_000.0);
    assert!(
        (heights[1] - 40.0).abs() < 0.01,
        "the one below keeps the floor: {heights:?}"
    );
    let last = resized_shares(&[400.0, 400.0], 1, 30.0, 40.0).unwrap();
    let last = current_heights(Some(last), vec![400.0, 400.0]);
    assert!(
        (last[0] - 430.0).abs() < 0.01,
        "the last dock moves the edge above it down: {last:?}"
    );
}
