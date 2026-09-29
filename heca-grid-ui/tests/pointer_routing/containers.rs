use super::*;

// ── containers on the way past ────────────────────────────────────────────────────────────────

/// **Bubbling survives every container that used to own its own walk.** Each of these once
/// forwarded events to its children by hand, one event kind at a time; a widget inside one has to
/// receive the whole vocabulary, and the container above it has to see what its subtree did.
#[test]
fn every_routing_container_passes_the_whole_vocabulary_through() {
    fn probe_in(wrap: impl FnOnce(Probe) -> Box<dyn Component>) -> Vec<String> {
        let (probe, seen) = Probe::new(100.0, 50.0);
        let mut root = wrap(probe);
        LayoutEngine::new().compute(root.as_mut(), Size::new(400.0, 300.0));
        // Aim at the probe wherever layout put it.
        fn find(c: &dyn Component, w: f32, h: f32) -> Option<Rectangle> {
            let b = c.base().bounds;
            if (b.size.w - w as f64).abs() < 0.5 && (b.size.h - h as f64).abs() < 0.5 {
                return Some(b);
            }
            c.base()
                .children
                .iter()
                .find_map(|c| find(c.as_ref(), w, h))
        }
        let b = find(root.as_ref(), 100.0, 50.0).expect("the probe was laid out");
        let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
        mv(root.as_mut(), at);
        click_at(root.as_mut(), at, PointerButton::Left);
        let _ = heca_grid_ui::dispatch(root.as_mut(), &Event::wheel(at, 0.0, 1.0));
        kinds(&seen)
    }

    let cases: Vec<(&str, Vec<String>)> = vec![
        ("FocusScope", probe_in(|p| Box::new(FocusScope::new(p)))),
        (
            "DockFrame",
            probe_in(|p| Box::new(DockFrame::new("DOCK").child(p))),
        ),
        (
            "Overlay",
            probe_in(|p| {
                Box::new(
                    Overlay::new()
                        .panel(Flex::column().child(p))
                        .default_open(true),
                )
            }),
        ),
        (
            "Dialog",
            probe_in(|p| Box::new(Dialog::new("T").body(p).default_open(true))),
        ),
    ];
    for (name, seen) in cases {
        for want in [
            "PointerEnter",
            "PointerDown",
            "PointerUp",
            "Click",
            "Scroll",
        ] {
            assert!(
                seen.contains(&want.to_string()),
                "{name} never let {want} through to its child: {seen:?}",
            );
        }
    }
}

/// A widget whose overlay is drawn **on top** takes the click, whatever its place in the child
/// order — what is drawn over a thing is what is clicked.
#[test]
fn an_open_dropdown_takes_a_click_over_a_sibling_drawn_under_it() {
    let (probe, seen) = Probe::new(200.0, 200.0);
    let mut root = Flex::column()
        .child(Select::new(["ONE", "TWO", "THREE"]))
        .child(probe);
    LayoutEngine::new().compute(&mut root, Size::new(300.0, 300.0));

    // Open the list: it drops over the probe below it.
    let trigger = root.base().children[0].base().bounds;
    click_at(
        &mut root,
        Point::new(trigger.loc.x + 5.0, trigger.loc.y + trigger.size.h / 2.0),
        PointerButton::Left,
    );
    let before = kinds(&seen).len();
    // A click just under the trigger is on the open list, not on the widget it covers.
    click_at(
        &mut root,
        Point::new(trigger.loc.x + 5.0, trigger.loc.y + trigger.size.h + 5.0),
        PointerButton::Left,
    );
    let after = kinds(&seen);
    assert!(
        !after[before..].contains(&"Click".to_string()),
        "the widget under the panel was not clicked through: {after:?}",
    );
}
