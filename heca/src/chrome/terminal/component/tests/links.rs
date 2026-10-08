//! Where the links are, and what a click and a hover on one mean — all through the tree.

use super::*;
use heca_core::backend::HyperlinkSpan;
use heca_grid_ui::Modifiers;

/// A link on row 3, cells 4 up to (not including) 9 — under the point (45, 61) of the 10 × 20 grid.
fn span(row: usize, start_col: usize, end_col: usize, uri: &str) -> HyperlinkSpan {
    HyperlinkSpan {
        row,
        start_col,
        end_col,
        uri: uri.to_string(),
    }
}

fn on_link() -> heca_grid_ui::Point {
    heca_grid_ui::Point::new(45.0, 61.0)
}

fn off_link() -> heca_grid_ui::Point {
    heca_grid_ui::Point::new(245.0, 161.0)
}

type Said = Rc<RefCell<Vec<TerminalInput>>>;
type Opened = Rc<RefCell<Vec<String>>>;

/// A terminal with one link, an owner listening, and the intents the tree posts.
fn with_a_link() -> (Terminal, Box<dyn heca_grid_ui::Component>, Said, Opened) {
    let (seams, said) = recording();
    let t = Terminal::new();
    t.bind(seams);
    t.show(&scrolled(0));
    t.show_links(&[span(3, 4, 9, "https://example.com")]);
    let mut root = laid_out_in(Box::new(t.clone()), 300.0, 200.0);
    let _ = painted(root.as_ref());
    t.place(Some(Rectangle::new(
        heca_grid_ui::Point::new(0.0, 0.0),
        heca_grid_ui::Size::new(300.0, 200.0),
    )));
    let opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| {
        sink.borrow_mut()
            .push(format!("{} {:?}", intent.action, intent.args.get("url")));
    });
    said.borrow_mut().clear();
    // Nothing is held until a test says so.
    hold(root.as_mut(), Modifiers::default());
    (t, root, said, opened)
}

fn hold(root: &mut dyn heca_grid_ui::Component, m: Modifiers) {
    heca_grid_ui::dispatch(root, &Event::ModifiersChanged(m));
}

fn cmd() -> Modifiers {
    Modifiers {
        meta: true,
        ..Modifiers::default()
    }
}

fn click(root: &mut dyn heca_grid_ui::Component, at: heca_grid_ui::Point) {
    heca_grid_ui::dispatch(root, &Event::pointer_pressed(at, PointerButton::Left));
    heca_grid_ui::dispatch(root, &Event::pointer_released(at, PointerButton::Left));
}

/// A span covers its first cell and not the one at its end; the row matters.
#[test]
fn a_link_covers_its_cells_from_the_start_up_to_the_end() {
    let (t, ..) = with_a_link();
    let at = |col: f32, row: f32| (col * 10.0 + 5.0, row * 20.0 + 1.0);
    assert_eq!(t.link_at(at(4.0, 3.0)).as_deref(), Some("https://example.com"));
    assert_eq!(t.link_at(at(8.0, 3.0)).as_deref(), Some("https://example.com"));
    assert_eq!(t.link_at(at(9.0, 3.0)), None, "the end is outside");
    assert_eq!(t.link_at(at(3.0, 3.0)), None, "before the start");
    assert_eq!(t.link_at(at(5.0, 2.0)), None, "another row");
}

/// A link on a row the selection keeps is found by its stable row, and only while on screen.
#[test]
fn a_link_is_found_by_the_stable_row_a_selection_keeps() {
    let (t, ..) = with_a_link();
    assert_eq!(
        t.link_at_stable(3, 5).as_deref(),
        Some("https://example.com")
    );
    assert_eq!(t.link_at_stable(-1, 5), None, "scrolled off above");
    assert_eq!(t.link_at_stable(24, 5), None, "below the screen");
}

/// **Clicking a link with the window's key held opens it, and the program is told nothing.**
#[test]
fn a_click_on_a_link_with_the_window_key_held_opens_it() {
    let (_t, mut root, said, opened) = with_a_link();
    hold(root.as_mut(), cmd());
    click(root.as_mut(), on_link());
    assert_eq!(
        *opened.borrow(),
        ["open_link Some(Text(\"https://example.com\"))"]
    );
    assert!(pointer_input(&said).is_empty(), "{:?}", said.borrow());
}

/// Without the key a click is the program's, and nothing opens.
#[test]
fn a_plain_click_on_a_link_is_the_programs() {
    let (_t, mut root, said, opened) = with_a_link();
    click(root.as_mut(), on_link());
    assert!(opened.borrow().is_empty());
    assert_eq!(pointer_input(&said).len(), 2, "a press and a release");
}

/// With the key held but off any link, nothing opens, and the press is still not the program's.
#[test]
fn a_click_off_a_link_with_the_window_key_held_opens_nothing() {
    let (_t, mut root, said, opened) = with_a_link();
    hold(root.as_mut(), cmd());
    click(root.as_mut(), off_link());
    assert!(opened.borrow().is_empty());
    assert!(pointer_input(&said).is_empty());
}

/// The other buttons never open a link.
#[test]
fn a_right_click_on_a_link_opens_nothing() {
    let (_t, mut root, _said, opened) = with_a_link();
    hold(root.as_mut(), cmd());
    for ev in [
        Event::pointer_pressed(on_link(), PointerButton::Right),
        Event::pointer_released(on_link(), PointerButton::Right),
    ] {
        heca_grid_ui::dispatch(root.as_mut(), &ev);
    }
    assert!(opened.borrow().is_empty());
}

/// **The hand shows over a link exactly when a click would open it.**
#[test]
fn the_cursor_is_a_hand_over_a_link_only_while_the_window_key_is_held() {
    use heca_grid_ui::Cursor;
    let (_t, mut root, ..) = with_a_link();
    let over = |root: &dyn heca_grid_ui::Component, p| heca_grid_ui::cursor_at(root, p);
    assert_ne!(over(root.as_ref(), on_link()), Cursor::Pointer, "key up");
    hold(root.as_mut(), cmd());
    assert_eq!(over(root.as_ref(), on_link()), Cursor::Pointer);
    assert_ne!(over(root.as_ref(), off_link()), Cursor::Pointer, "off a link");
}
