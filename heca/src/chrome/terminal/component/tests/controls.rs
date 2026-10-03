//! The scrollback controls: the chip and the scrollbar.

use super::*;

/// At the live bottom there is nothing to show; scrolled up, the chip says how far.
#[test]
fn a_scrolled_terminal_says_how_far_above_the_live_bottom_it_is() {
    let t = Terminal::new();
    let root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    assert!(
        chip(&painted(root.as_ref())).is_none(),
        "live bottom: no chip"
    );

    assert!(t.show(&scrolled(5)), "showing a new viewport is a change");
    let mut root = root;
    heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
    let scene = painted(root.as_ref());
    let texts: Vec<_> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|x| x == "5 lines above"), "{texts:?}");
    assert!(!t.show(&scrolled(5)), "the same viewport changes nothing");
}

/// The chip stays inside the terminal's box whatever the box is.
#[test]
fn the_chip_never_leaves_the_terminal_it_belongs_to() {
    for (w, h) in [(120.0, 60.0), (300.0, 200.0), (900.0, 700.0)] {
        let t = Terminal::new();
        t.show(&scrolled(40));
        let root = laid_out_in(Box::new(t.clone()), w, h);
        let rect = chip(&painted(root.as_ref())).expect("chip is shown");
        assert!(
            rect.loc.x >= 0.0
                && rect.loc.y >= 0.0
                && rect.loc.x + rect.size.w <= w
                && rect.loc.y + rect.size.h <= h,
            "{rect:?} leaves {w}x{h}"
        );
        assert!(
            rect.loc.x + rect.size.w > w - 80.0,
            "it sits at the right edge"
        );
    }
}

/// Clicking the chip means "back to the live bottom" — said to whoever bound the terminal.
#[test]
fn clicking_the_chip_asks_for_the_live_bottom() {
    let to_bottom = Rc::new(Cell::new(0));
    let t = Terminal::new();
    let seen = to_bottom.clone();
    t.bind(seams(ScrollIntents {
        to_bottom: Box::new(move || seen.set(seen.get() + 1)),
        to_offset: Box::new(|_| {}),
    }));
    t.show(&scrolled(5));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let rect = chip(&painted(root.as_ref())).expect("chip is shown");
    let at = heca_grid_ui::Point::new(
        rect.loc.x + rect.size.w / 2.0,
        rect.loc.y + rect.size.h / 2.0,
    );
    for ev in [
        Event::pointer_pressed(at, PointerButton::Left),
        Event::pointer_released(at, PointerButton::Left),
    ] {
        let _ = heca_grid_ui::dispatch(root.as_mut(), &ev);
    }
    assert_eq!(to_bottom.get(), 1);
}

/// Dragging the thumb asks to scroll to the row it points at.
#[test]
fn pressing_the_scrollbar_asks_to_scroll_to_where_it_points() {
    let asked = Rc::new(RefCell::new(Vec::new()));
    let t = Terminal::new();
    let seen = asked.clone();
    t.bind(seams(ScrollIntents {
        to_bottom: Box::new(|| {}),
        to_offset: Box::new(move |rows| seen.borrow_mut().push(rows)),
    }));
    t.show(&scrolled(50));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    // The bar is on the right edge, under the chip; press in its upper half, below the chip.
    let at = heca_grid_ui::Point::new(296.0, 40.0);
    assert_eq!(
        heca_grid_ui::dispatch(
            root.as_mut(),
            &Event::pointer_pressed(at, PointerButton::Left)
        ),
        Handled::Yes
    );
    let rows = asked.borrow().last().copied().expect("asked to scroll");
    assert!(
        rows > 50,
        "the upper half of the track is far up the history: {rows}"
    );
}

/// A terminal with nothing to scroll shows neither control, so a press lands on the terminal.
#[test]
fn a_terminal_at_the_live_bottom_leaves_the_right_edge_to_the_terminal() {
    let t = Terminal::new();
    t.show(&scrolled(0));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let at = heca_grid_ui::Point::new(296.0, 100.0);
    assert_eq!(
        heca_grid_ui::dispatch(
            root.as_mut(),
            &Event::pointer_pressed(at, PointerButton::Left)
        ),
        Handled::No
    );
}

/// Every node placed for a terminal is shown the same viewport, so a rebuilt tree agrees with
/// the one it replaced.
#[test]
fn every_node_placed_for_a_terminal_shows_the_same_viewport() {
    let t = Terminal::new();
    let first = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let second = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    t.show(&scrolled(9));
    for root in [first, second] {
        let mut root = root;
        heca_grid_ui::LayoutEngine::new().compute(root.as_mut(), Size::new(300.0, 200.0));
        assert!(chip(&painted(root.as_ref())).is_some());
    }
}

/// The scrollback controls are the terminal's children: a press or a move on them is theirs,
/// and the terminal says nothing about it — which is what keeps a hover on the chip from
/// reaching the program underneath.
#[test]
fn a_press_or_move_on_the_chip_is_not_the_terminals() {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&scrolled(5));
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let rect = chip(&painted(root.as_ref())).expect("chip is shown");
    let at = heca_grid_ui::Point::new(
        rect.loc.x + rect.size.w / 2.0,
        rect.loc.y + rect.size.h / 2.0,
    );
    for ev in [
        Event::pointer_moved(at),
        Event::pointer_pressed(at, PointerButton::Left),
        Event::pointer_released(at, PointerButton::Left),
    ] {
        let _ = heca_grid_ui::dispatch(root.as_mut(), &ev);
    }
    let heard = pointer_input(&said);
    assert!(heard.is_empty(), "{heard:?}");
}
