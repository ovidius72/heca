use super::*;

/// **An open column pick marks the workspace — an empty place with its letter at each gap that can
/// be picked, an outline round each column that can — and the marks leave with the pick**, without
/// rebuilding the panes beside them. A press goes through them to what is under.
#[test]
fn an_open_pick_marks_the_workspace_and_the_marks_leave_with_it() {
    use super::PickMark;
    let keys = |ws: &dyn Component| -> Vec<String> {
        ws.base()
            .children
            .iter()
            .filter_map(|c| c.base().key.clone())
            .collect()
    };
    let mut open = two_columns();
    open.places = open_places();
    open.pick = vec![
        PickMark::Gap { at: 1, letter: 'b' },
        PickMark::Column(ColumnId(2)),
        PickMark::Row {
            col: 0,
            row: 1,
            letter: 'c',
        },
    ];
    let (_window, mut ws) = laid_out(&open);
    let marked = keys(ws.as_ref());
    assert!(
        marked.ends_with(&[
            "pick:gap:1".to_string(),
            "pick:col:2".to_string(),
            "pick:row:0:1".to_string()
        ]),
        "over the columns and the places: {marked:?}"
    );
    let mark = |key: &str| named(ws.as_ref(), key);
    // The place is where the layout put it; the outline is the column's own box.
    assert_eq!(bounds(mark("pick:gap:1")), (318.0, 30.0, 54.0, 500.0));
    assert_eq!(bounds(mark("pick:col:2")), (350.0, 30.0, 300.0, 500.0));
    assert_eq!(bounds(mark("pick:row:0:1")), (40.0, 270.0, 300.0, 0.0), "a border is a line of no thickness; the slot widens its own zone");
    assert!(mark("pick:gap:1").base().pointer_transparent);
    let letters: Vec<String> = painted(ws.as_ref())
        .iter()
        .filter_map(|c| match c {
            heca_grid_ui::DrawCommand::Text(t) if t.text == "b" => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(letters, ["b"], "the letter is in the place");
    let rows: Vec<String> = painted(ws.as_ref())
        .iter()
        .filter_map(|c| match c {
            heca_grid_ui::DrawCommand::Text(t) if t.text == "c" => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(rows, ["c"], "and in the row place");

    assert!(ws.set_props(&two_columns()));
    assert_eq!(
        keys(ws.as_ref()),
        ["col:1", "col:2", "split:col:0", "split:pane:0:0"],
        "the marks left with the pick, and nothing else was rebuilt"
    );
}

/// The window as the app seats it — the workspace under a bare chrome — with every drop the
/// framework hands back recorded as `(source, target, side)`.
/// What the framework said was dropped where: source, target, and the side of the target.
type Drops = Rc<std::cell::RefCell<Vec<(String, String, heca_grid_ui::drag::DropSide)>>>;

fn carrying_window() -> (Flex, Drops) {
    carrying_window_of(two_columns())
}

/// The same, for a model of the caller's.
fn carrying_window_of(model: WorkspaceModel) -> (Flex, Drops) {
    let dropped = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = dropped.clone();
    heca_grid_ui::drag::install_drop_sink(move |d| {
        sink.borrow_mut().push((d.source, d.target, d.side));
    });
    let mut window = crate::chrome::new_window_root();
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(ws.set_props(&model));
    window.base_mut().children.push(ws);
    crate::chrome::seat_chrome(
        &mut window,
        Flex::column()
            .width(heca_grid_ui::Length::FULL)
            .height(heca_grid_ui::Length::FULL),
    );
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));
    (window, dropped)
}

/// Press at `from`, drag to `to` in steps, release — with the window's key held or not.
fn drag(window: &mut Flex, from: Point, to: Point, key_held: bool) {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};
    let held = heca_grid_ui::Modifiers {
        meta: key_held,
        ..heca_grid_ui::Modifiers::default()
    };
    dispatch(window, &Event::ModifiersChanged(held));
    dispatch(window, &Event::pointer_pressed(from, PointerButton::Left));
    for step in 1..=4 {
        let t = f64::from(step) / 4.0;
        let at = Point::new(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        dispatch(window, &Event::pointer_moved(at));
    }
    dispatch(window, &Event::pointer_released(to, PointerButton::Left));
}

/// **Carry a pane with the window's key and drop it on a place the layout opened**: a gap between
/// two columns, or a row between two panes. Nothing is there to land on while no place is open.
#[test]
fn a_pane_carried_with_the_window_key_lands_on_a_place_the_layout_opened() {
    let gap = Point::new(345.0, 100.0);
    let row = Point::new(200.0, 270.0);
    let from = Point::new(100.0, 100.0);

    let (mut closed, dropped) = carrying_window();
    drag(&mut closed, from, gap, true);
    assert!(dropped.borrow().is_empty(), "no place is open: {:?}", dropped.borrow());

    let mut model = two_columns();
    model.places = open_places();
    let (mut window, dropped) = carrying_window_of(model);
    drag(&mut window, from, gap, true);
    drag(&mut window, from, row, true);
    // A pointer a little off the border still counts as on it — the zone is wider than the line.
    drag(&mut window, from, Point::new(200.0, 272.5), true);
    assert_eq!(
        dropped.borrow().iter().map(|d| (d.0.as_str(), d.1.as_str())).collect::<Vec<_>>(),
        [("pane:10", "slot:1"), ("pane:10", "row:0:1"), ("pane:10", "row:0:1")]
    );
}

/// Without the key the press is the terminal's: nothing is picked up, so nothing is dropped.
#[test]
fn a_drag_without_the_window_key_picks_nothing_up() {
    let (mut window, dropped) = carrying_window();
    drag(&mut window, Point::new(100.0, 100.0), Point::new(345.0, 100.0), false);
    assert!(dropped.borrow().is_empty(), "{:?}", dropped.borrow());
}

/// Dropped on another pane, the lower half means after it — and the swap key makes it an exchange,
/// which the drop carries for the host to act on.
#[test]
fn a_pane_carried_onto_another_names_it_and_the_half_it_landed_in() {
    let (mut window, dropped) = carrying_window();
    drag(&mut window, Point::new(100.0, 100.0), Point::new(400.0, 400.0), true);
    let landed = dropped.borrow().clone();
    assert_eq!(landed.len(), 1, "{landed:?}");
    assert_eq!(
        (landed[0].0.as_str(), landed[0].1.as_str(), landed[0].2),
        ("pane:10", "pane:20", heca_grid_ui::drag::DropSide::After)
    );
}

/// **A place is seated at the box the layout gave it** — the workspace computes none of it.
#[test]
fn a_place_is_seated_at_the_box_the_layout_gave_it() {
    let mut model = two_columns();
    model.places = open_places();
    let (_window, ws) = laid_out(&model);
    assert_eq!(bounds(named(ws.as_ref(), "slot:1")), (318.0, 30.0, 54.0, 500.0));
    assert_eq!(bounds(named(ws.as_ref(), "row:0:1")), (40.0, 270.0, 300.0, 0.0));
}

/// **One key, two meanings, told apart by how far the pointer travels**: on a pane showing a link,
/// the window's key and a click opens the link, and the same key and a drag carries the pane — the
/// terminal under the pointer is told neither.
#[test]
fn the_window_key_opens_a_link_on_a_click_and_carries_the_pane_on_a_drag() {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};

    // Every cell of pane 10's terminal is a link, so the click lands on one wherever it is.
    let mut model = two_columns();
    model.places = open_places();
    let terminal = model.panes[&PaneId(10)].content.clone();
    terminal.show(&crate::chrome::terminal::Viewport {
        rows: 12,
        scrollback_rows: 12,
        offset: 0,
        scrollbar: heca_config::appearance::ScrollbarVisibility::Never,
        badge: false,
        cell: (10.0, 20.0),
        top_stable_row: 0,
        match_alpha: 64,
        current_match_alpha: 150,
        nominal_cell: (10.0, 20.0),
        open_link_modifier: heca_config::theme::ModifierKey::Super,
    });
    let links: Vec<_> = (0..12)
        .map(|row| heca_core::backend::HyperlinkSpan {
            row,
            start_col: 0,
            end_col: 30,
            uri: "https://example.com".into(),
        })
        .collect();
    terminal.show_links(&links);

    let (mut window, dropped) = carrying_window_of(model);
    let _ = painted(&window); // the terminal counts its cells from where it was painted
    let opened = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = opened.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| sink.borrow_mut().push(intent.action));

    let at = Point::new(100.0, 100.0);
    let cmd = heca_grid_ui::Modifiers {
        meta: true,
        ..heca_grid_ui::Modifiers::default()
    };
    dispatch(&mut window, &Event::ModifiersChanged(cmd));
    dispatch(&mut window, &Event::pointer_pressed(at, PointerButton::Left));
    dispatch(&mut window, &Event::pointer_released(at, PointerButton::Left));
    assert_eq!(*opened.borrow(), ["open_link"], "a click opens the link");
    assert!(dropped.borrow().is_empty(), "and carries nothing");

    opened.borrow_mut().clear();
    drag(&mut window, at, Point::new(345.0, 100.0), true);
    assert!(opened.borrow().is_empty(), "a drag opens nothing: {:?}", opened.borrow());
    assert_eq!(
        dropped.borrow().iter().map(|d| (d.0.as_str(), d.1.as_str())).collect::<Vec<_>>(),
        [("pane:10", "slot:1")],
        "it carries the pane to the place"
    );
}

/// **A fresh tree laid out once already has a zone to hit**: the border's box has no thickness, and
/// the slot's own reach — resolved at layout — makes it a target a step either side.
#[test]
fn a_border_has_a_reacting_zone_after_the_first_layout() {
    let mut model = two_columns();
    model.places = open_places();
    let (_window, ws) = laid_out(&model);
    let zone = named(ws.as_ref(), "row:0:1").hit_bounds().expect("it reacts");
    assert!(zone.size.h > 0.0, "{zone:?}");
    assert_eq!(zone.loc.y + zone.size.h / 2.0, 270.0, "centred on the border");
}
