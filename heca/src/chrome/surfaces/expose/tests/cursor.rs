//! Where the cursor opens, and where it stays across a delete and a rebuild.

use super::*;

/// **Deleting a card hands the cursor to the next one in the SAME row.** The map is rebuilt
/// when the session changes under it, and the rebuilt tree has to be told where the highlight
/// goes — after the delete the pane is gone and its neighbour can no longer be found from it.
#[test]
fn deleting_a_card_moves_the_cursor_to_its_neighbour_first() {
    use heca_grid_ui::event::Event;
    // Open on the FIRST pane; the row also holds pane 2, which must take the cursor.
    let (mut root, seen) = built(Some(PaneId(1)));
    heca_grid_ui::dispatch(root.as_mut(), &Event::TextInput("x".to_string()));
    let order: Vec<String> = seen
        .borrow()
        .iter()
        .map(|i| match () {
            _ if i.contains("ExposeCursor") => "cursor".to_string(),
            _ if i.contains("close_pane_by_id") => "delete".to_string(),
            _ => "other".to_string(),
        })
        .collect();
    assert_eq!(
        order.first().map(String::as_str),
        Some("cursor"),
        "the cursor is handed on BEFORE the delete, or the rebuilt map has nowhere to put it: \
         {:?}",
        seen.borrow(),
    );
    assert!(
        order.contains(&"delete".to_string()),
        "…and the delete follows: {order:?}"
    );
}

/// **The map opens on the pane the session is focused on** — not where it was last browsed.
///
/// The exposé's cursor and the scrolling area's
/// focus must agree the moment it opens. They did not: the remembered highlight won on a fresh
/// open too, so browsing away and dismissing left the next open showing its cursor on one card
/// and its `active` styling on another.
#[test]
fn opening_the_map_starts_on_the_pane_the_session_is_focused_on() {
    let s = session();
    let focused = s
        .l()
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id)
        .expect("the fixture has a focused pane");
    // A different pane remembered from a previous browse, in the row the map would open on.
    let remembered = vec![Some(PaneId(2))];
    assert_ne!(
        remembered[0],
        Some(focused),
        "the fixture must actually differ"
    );

    assert_eq!(
        open_on(s.l(), false, Some(0), &remembered),
        Some(focused),
        "a fresh open ignores where the map was left",
    );
}

/// **…and a rebuild keeps the cursor exactly where it is.** The layer is rebuilt from scratch
/// whenever the session changes underneath it; a rebuild that moved the highlight would fight
/// the user mid-gesture.
#[test]
fn rebuilding_the_map_keeps_the_cursor_where_it_stands() {
    let s = session();
    assert_eq!(
        open_on(s.l(), true, Some(0), &[Some(PaneId(2))]),
        Some(PaneId(2)),
        "while the map is up, the remembered cursor wins",
    );
}

/// **A rebuild in a row other than the active workspace's stays in that row.** Reading the
/// active workspace's slot here is what made deleting a pane in one row jump the map to
/// another.
#[test]
fn a_rebuild_stays_in_the_row_the_cursor_is_in() {
    let mut s = session();
    // Give the second workspace a pane, while the session stays focused in the first.
    s.show(1);
    s.m()
        .add_pane(heca_core::layout::Pane::new(PaneId(3), "c"), None, false);
    s.show(0);

    assert_eq!(
        open_on(s.l(), true, Some(1), &[Some(PaneId(1)), Some(PaneId(3))]),
        Some(PaneId(3)),
        "the cursor is in row 1, so the rebuild reads row 1 — not the active workspace's",
    );
}

/// A remembered pane that has since been **deleted** is no answer at all: fall through to the
/// session's focus, which is where the cursor was handed on to before the card went.
#[test]
fn a_remembered_pane_that_no_longer_exists_falls_back_to_the_session() {
    let s = session();
    let focused = s
        .l()
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id)
        .expect("the fixture has a focused pane");
    assert_eq!(
        open_on(s.l(), true, Some(0), &[Some(PaneId(404))]),
        Some(focused),
    );
}
