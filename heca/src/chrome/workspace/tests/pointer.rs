use super::*;

/// **The edges between columns and between stacked panes are the workspace's children**: after the
/// columns and before the floats, so a float is over them — and dragging one tells the owner which
/// column or pane to resize and by how much.
#[test]
fn dragging_an_edge_resizes_the_neighbours_it_sits_between() {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};

    let posted = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let mut seams = seams();
    seams.resize_column = {
        let posted = posted.clone();
        Rc::new(move |col, share| posted.borrow_mut().push(format!("col {col} {share:.4}")))
    };
    seams.resize_pane = {
        let posted = posted.clone();
        Rc::new(move |col, pane, px| posted.borrow_mut().push(format!("pane {col}/{pane} {px}")))
    };
    let mut ws: Box<dyn Component> = Box::new(workspace(seams));
    assert!(ws.set_props(&two_columns()));
    let mut window = Flex::column().width(800.0).height(600.0);
    window.base_mut().children.push(ws);
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));

    // Order: columns, then the edges, then the float.
    let order: Vec<String> = window.base().children[0]
        .base()
        .children
        .iter()
        .map(|c| c.base().key.clone().unwrap_or_default())
        .collect();
    assert_eq!(
        order,
        [
            "col:1",
            "col:2",
            "split:col:0",
            "split:pane:0:0",
            "slot:0",
            "slot:1",
            "slot:2"
        ]
    );

    let at = |x: f64, y: f64| Point::new(x, y);
    // The gap between the columns is x 340..350: drag it 10px right.
    dispatch(
        &mut window,
        &Event::pointer_pressed(at(345.0, 400.0), PointerButton::Left),
    );
    dispatch(&mut window, &Event::pointer_moved(at(355.0, 400.0)));
    dispatch(
        &mut window,
        &Event::pointer_released(at(355.0, 400.0), PointerButton::Left),
    );
    // The gap between the stacked panes of the first column is y 270..280: drag it 5px down.
    dispatch(
        &mut window,
        &Event::pointer_pressed(at(60.0, 275.0), PointerButton::Left),
    );
    dispatch(&mut window, &Event::pointer_moved(at(60.0, 280.0)));
    dispatch(
        &mut window,
        &Event::pointer_released(at(60.0, 280.0), PointerButton::Left),
    );

    assert_eq!(*posted.borrow(), ["col 0 0.0143", "pane 0/0 5"]);
}

/// **A press on a pane reaches the pane, with the chrome seated over it as the app seats it.**
///
/// The chrome root covers the whole window to hold the bars and sidebars, and it is later in the
/// tree than the workspace, so it is what the pointer meets first. It must let the pointer through
/// its own box, or every press in the content area lands on it and no pane is ever focused, no
/// header button ever clicked (found driving the first one-tree build).
#[test]
fn a_press_on_a_pane_reaches_the_pane_under_the_chrome() {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};

    let focused = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = focused.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| {
        sink.borrow_mut().push(format!(
            "{}({:?})",
            intent.action,
            intent.args.get("pane_id").cloned()
        ))
    });

    let mut window = crate::chrome::new_window_root();
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(ws.set_props(&two_columns_and_a_float()));
    window.base_mut().children.push(ws);
    crate::chrome::seat_chrome(
        &mut window,
        Flex::column()
            .width(heca_grid_ui::Length::FULL)
            .height(heca_grid_ui::Length::FULL),
    );
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));

    let at = Point::new(100.0, 100.0); // inside pane 10
    dispatch(&mut window, &Event::pointer_pressed(at, PointerButton::Left));
    dispatch(&mut window, &Event::pointer_released(at, PointerButton::Left));
    assert_eq!(*focused.borrow(), ["focus_pane(Some(Int(10)))"]);
}

/// **Whatever a plugin puts anywhere is clicked where it is drawn, and a pane is clicked where
/// nothing else is — with nobody setting anything.**
///
/// The chrome is built the way the app builds it: a window-sized column holding a top bar and a
/// middle row with a painted dock on the right and an unpainted spacer that covers the panes. A
/// plugin widget sits in the dock, another in an overlay seated over everything, each with only a
/// click handler.
#[test]
fn a_plugin_widget_in_a_dock_or_an_overlay_and_a_pane_each_get_their_own_press() {
    use heca_grid_ui::builders::{ComponentExt, Parent};
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};
    use heca_grid_ui::{Color, Length};

    let clicks = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let focused = clicks.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| {
        focused.borrow_mut().push(intent.action.to_string())
    });
    let dock_clicks = clicks.clone();
    let overlay_clicks = clicks.clone();

    let mut window = crate::chrome::new_window_root();
    let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
    assert!(ws.set_props(&two_columns_and_a_float()));
    window.base_mut().children.push(ws);

    let mut dock = Flex::column()
        .width(100.0)
        .height(Length::FULL)
        .child(Flex::row().at_rect(0.0, 70.0, 100.0, 40.0).on_click(move |_| dock_clicks.borrow_mut().push("dock".into())));
    dock.base_mut().style.visual.fill = Some(Color::new(20, 20, 20, 255));
    crate::chrome::seat_chrome(
        &mut window,
        Flex::column()
            .width(Length::FULL)
            .height(Length::FULL)
            .child(Flex::row().width(Length::FULL).height(30.0))
            .child(
                Flex::row()
                    .width(Length::FULL)
                    .height(Length::FULL)
                    .child(Flex::row().width(Length::FULL).height(Length::FULL))
                    .child(dock),
            ),
    );
    crate::chrome::place_surface(
        &mut window,
        "plugin.overlay",
        Box::new(
            Flex::row()
                .width(Length::FULL)
                .height(Length::FULL)
                .child(Flex::row().at_rect(680.0, 300.0, 100.0, 40.0).on_click(move |_| overlay_clicks.borrow_mut().push("overlay".into()))),
        ),
    );
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));

    let press = |window: &mut Flex, x: f64, y: f64| {
        let at = Point::new(x, y);
        dispatch(window, &Event::pointer_pressed(at, PointerButton::Left));
        dispatch(window, &Event::pointer_released(at, PointerButton::Left));
    };
    press(&mut window, 740.0, 120.0);
    press(&mut window, 720.0, 320.0);
    press(&mut window, 100.0, 100.0);
    assert_eq!(*clicks.borrow(), ["dock", "overlay", "focus_pane"]);
}

/// **Every layout container lets a press through where it covers nothing** — a plugin wraps its
/// dock in a `Grid`, a `Surface`, and gets the same answer as with a `Flex`: the widget
/// inside is clicked, and a pane behind the wrapper is clicked where the wrapper holds nothing.
#[test]
fn every_layout_container_over_the_panes_leaves_them_their_presses() {
    use heca_grid_ui::builders::{ComponentExt, Parent};
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};
    use heca_grid_ui::widgets::{Grid, KeyHintGroup, Surface, Visibility};
    use heca_grid_ui::Length;

    type Wrap = fn(Flex) -> Box<dyn Component>;
    let wrappers: [(&str, Wrap); 5] = [
        ("Flex", |inner| Box::new(inner)),
        ("Grid", |inner| Box::new(Grid::new().width(Length::FULL).height(Length::FULL).child(inner))),
        ("Surface", |inner| {
            Box::new(Surface::column().width(Length::FULL).height(Length::FULL).child(inner))
        }),
        ("Visibility", |inner| Box::new(Visibility::new(inner, true))),
        ("KeyHintGroup", |inner| Box::new(KeyHintGroup::new(inner))),
    ];

    for (name, wrap) in wrappers {
        let clicks = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
        let sink = clicks.clone();
        heca_grid_ui::intent::install_intent_sink(move |intent| {
            sink.borrow_mut().push(intent.action.to_string())
        });
        let item_clicks = clicks.clone();

        let mut window = crate::chrome::new_window_root();
        let mut ws: Box<dyn Component> = Box::new(workspace(seams()));
        assert!(ws.set_props(&two_columns_and_a_float()));
        window.base_mut().children.push(ws);
        let inner = Flex::row()
            .width(Length::FULL)
            .height(Length::FULL)
            .child(
                Flex::row()
                    .at_rect(680.0, 300.0, 100.0, 40.0)
                    .on_click(move |_| item_clicks.borrow_mut().push("item".into())),
            );
        crate::chrome::seat_chrome(
            &mut window,
            Flex::column()
                .width(Length::FULL)
                .height(Length::FULL)
                .child(wrap(inner)),
        );
        LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));

        for (x, y) in [(720.0, 320.0), (100.0, 100.0)] {
            let at = Point::new(x, y);
            dispatch(&mut window, &Event::pointer_pressed(at, PointerButton::Left));
            dispatch(&mut window, &Event::pointer_released(at, PointerButton::Left));
        }
        assert_eq!(*clicks.borrow(), ["item", "focus_pane"], "inside a {name}");
    }
}

/// **While a pane floats, a press reaches the float and the edges between the columns do nothing.**
///
/// The host used to drop every press while a float was up, so the float itself could not be
/// clicked. Presses go to the tree now: the float answers because it is in it, and no edge is
/// seated, so the gap beside it neither resizes nor shows a resize cursor.
#[test]
fn a_float_answers_its_press_and_the_edges_beside_it_do_nothing() {
    use heca_grid_ui::component::dispatch;
    use heca_grid_ui::event::{Event, PointerButton};

    let seen = Rc::new(std::cell::RefCell::new(Vec::<String>::new()));
    let sink = seen.clone();
    heca_grid_ui::intent::install_intent_sink(move |intent| {
        sink.borrow_mut().push(format!(
            "{}({:?})",
            intent.action,
            intent.args.get("pane_id").cloned()
        ))
    });
    let mut seams = seams();
    seams.resize_column = {
        let seen = seen.clone();
        Rc::new(move |col, _| seen.borrow_mut().push(format!("resize col {col}")))
    };
    let mut window = crate::chrome::new_window_root();
    let mut ws: Box<dyn Component> = Box::new(workspace(seams));
    assert!(ws.set_props(&two_columns_and_a_float()));
    window.base_mut().children.push(ws);
    crate::chrome::seat_chrome(
        &mut window,
        Flex::column()
            .width(heca_grid_ui::Length::FULL)
            .height(heca_grid_ui::Length::FULL),
    );
    LayoutEngine::new().compute(&mut window, heca_grid_ui::Size::new(800.0, 600.0));

    // The gap between the columns (x 340..350) at a height the float does not cover.
    let gap = Point::new(345.0, 400.0);
    assert_eq!(
        heca_grid_ui::cursor_at(&window, gap),
        heca_grid_ui::Cursor::Default,
        "no resize cursor while a pane floats"
    );
    dispatch(&mut window, &Event::pointer_pressed(gap, PointerButton::Left));
    dispatch(&mut window, &Event::pointer_moved(Point::new(355.0, 400.0)));
    dispatch(
        &mut window,
        &Event::pointer_released(Point::new(355.0, 400.0), PointerButton::Left),
    );
    assert!(
        !seen.borrow().iter().any(|a| a.starts_with("resize")),
        "{:?}",
        seen.borrow()
    );

    // A press on the float itself (pane 30, x 200..450, y 100..300) reaches it.
    seen.borrow_mut().clear();
    let on_float = Point::new(300.0, 200.0);
    dispatch(&mut window, &Event::pointer_pressed(on_float, PointerButton::Left));
    dispatch(&mut window, &Event::pointer_released(on_float, PointerButton::Left));
    assert_eq!(*seen.borrow(), ["focus_pane(Some(Int(30)))"]);
}
