//! A place something can land: hidden until a drag it accepts begins, outlined with its letter
//! while one is in flight, and dropped onto like any target.

mod common;

use common::{paint_via_child, press_at, release_at};
use heca_grid_ui::prelude::*;
use heca_grid_ui::widgets::LandingSlot;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

fn move_to(root: &mut dyn Component, pos: Point) {
    let _ = heca_grid_ui::dispatch(root, &Event::pointer_moved(pos));
}

/// A source of `kind` on the left and a placeholder slot that appears for `"pane"` on the right.
fn window(kind: &str) -> Flex {
    let mut root = Flex::row()
        .width(300.0)
        .height(50.0)
        .child(
            Flex::row()
                .at_rect(0.0, 0.0, 100.0, 50.0)
                .key("src")
                .draggable_as(kind),
        )
        .child(
            LandingSlot::new()
                .label("b")
                .while_dragging("pane")
                .key("slot:1")
                .at_rect(200.0, 0.0, 50.0, 50.0),
        );
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 50.0));
    root
}

fn drawn_letters(root: &Flex) -> Vec<String> {
    paint_via_child(root, &Theme::default())
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) if !t.text.is_empty() => Some(t.text.clone()),
            _ => None,
        })
        .collect()
}

fn pick_up(root: &mut Flex) {
    press_at(root, Point::new(10.0, 10.0), PointerButton::Left);
    move_to(root, Point::new(60.0, 40.0));
}

/// **The slot is not there until a drag of the kind it takes begins, and it goes when that ends** —
/// nothing hands it a flag.
#[test]
fn a_slot_appears_while_a_pane_is_carried_and_goes_when_it_ends() {
    let mut root = window("pane");
    assert!(drawn_letters(&root).is_empty(), "hidden before the drag");
    pick_up(&mut root);
    assert_eq!(drawn_letters(&root), ["b"], "its letter, while a pane is carried");
    release_at(&mut root, Point::new(60.0, 40.0), PointerButton::Left);
    assert!(drawn_letters(&root).is_empty(), "gone when it ends");
}

/// A drag of something else never shows it.
#[test]
fn a_slot_stays_hidden_for_a_drag_of_another_kind() {
    let mut root = window("column");
    pick_up(&mut root);
    assert!(drawn_letters(&root).is_empty());
}

/// Dropping onto the slot says what was dropped on it.
#[test]
fn a_drop_on_the_slot_names_it() {
    use std::cell::RefCell;
    use std::rc::Rc;
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let sink = dropped.clone();
    heca_grid_ui::drag::install_drop_sink(move |d| {
        sink.borrow_mut().push((d.source, d.target));
    });
    let mut root = window("pane");
    pick_up(&mut root);
    let on_slot = Point::new(220.0, 20.0);
    move_to(&mut root, on_slot);
    release_at(&mut root, on_slot, PointerButton::Left);
    assert_eq!(*dropped.borrow(), [("src".to_string(), "slot:1".to_string())]);
}

/// An edge is a **line**, not a place: faint while a carry is on, bright and thicker while a drag is
/// over it, drawn in the theme's colour — and the framework's whole-box wash is not laid over it.
#[test]
fn an_edge_is_faint_at_rest_and_takes_the_insertion_colour_under_a_drag() {
    let build = || {
        let mut root = Flex::row()
            .width(300.0)
            .height(100.0)
            .child(
                Flex::row()
                    .at_rect(0.0, 0.0, 50.0, 50.0)
                    .key("src")
                    .draggable_as("pane"),
            )
            .child(
                LandingSlot::new()
                    .edge(true)
                    .accepting("pane")
                    .key("row:0:1")
                    .at_rect(100.0, 40.0, 150.0, 8.0),
            );
        LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));
        root
    };
    let theme = Theme::default();
    let lines = |root: &Flex| -> Vec<(f64, u8, Color)> {
        paint_via_child(root, &theme)
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) if r.rect.size.w == 150.0 => Some((r.rect.size.h, r.fill.a, r.fill)),
                _ => None,
            })
            .collect()
    };
    // A carry is on but nothing is over it: a faint line at half the width.
    let mut root = build();
    press_at(&mut root, Point::new(10.0, 10.0), PointerButton::Left);
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(20.0, 20.0)));
    let width = f64::from(theme.colors.drag_edge_width);
    let rest = lines(&root);
    assert_eq!(rest.len(), 1, "one line and no wash: {rest:?}");
    assert_eq!(rest[0].0, width / 2.0);
    // Over it: the full width, in the target colour, so it stands apart from the rest.
    let _ = heca_grid_ui::dispatch(&mut root, &Event::pointer_moved(Point::new(150.0, 44.0)));
    let near = lines(&root);
    assert_eq!(near.len(), 1, "still one line and no wash: {near:?}");
    assert_eq!(near[0].0, width);
    assert_eq!(near[0].2, theme.colors.warning.with_alpha(near[0].1));
}

/// **An edge whose box is a whole area takes a drop anywhere in it, and draws its line where it is
/// told** — the space beside the last column, with the border at its left side.
#[test]
fn an_edge_over_an_area_takes_a_drop_anywhere_in_it_and_draws_its_line_at_its_side() {
    use heca_grid_ui::widgets::EdgeLine;
    use std::cell::RefCell;
    use std::rc::Rc;
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let sink = dropped.clone();
    heca_grid_ui::drag::install_drop_sink(move |d| {
        sink.borrow_mut().push(d.target);
    });
    let mut root = Flex::row()
        .width(400.0)
        .height(100.0)
        .child(Flex::row().at_rect(0.0, 0.0, 50.0, 50.0).key("src").draggable_as("pane"))
        .child(
            LandingSlot::new()
                .edge(true)
                .line(EdgeLine::Upright(0.0))
                .accepting("pane")
                .key("slot:end")
                .at_rect(100.0, 0.0, 300.0, 100.0),
        );
    LayoutEngine::new().compute(&mut root, Size::new(400.0, 100.0));
    press_at(&mut root, Point::new(10.0, 10.0), PointerButton::Left);
    move_to(&mut root, Point::new(20.0, 20.0));
    let theme = Theme::default();
    let line_x: Vec<f64> = paint_via_child(&root, &theme)
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Rect(r) if r.rect.size.h == 100.0 => Some(r.rect.loc.x + r.rect.size.w / 2.0),
            _ => None,
        })
        .collect();
    assert_eq!(line_x, [100.0], "the line runs along the left side of the area");
    let far = Point::new(380.0, 70.0);
    move_to(&mut root, far);
    release_at(&mut root, far, PointerButton::Left);
    assert_eq!(*dropped.borrow(), ["slot:end".to_string()], "a drop far from the line still lands");
}
