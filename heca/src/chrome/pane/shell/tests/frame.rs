//! What the frame shows: its header, colour, size and identity.

use super::*;

/// **A second thing in the body is not mistaken for a header.**
///
/// The pane's parts say which row of its template they are, so the host asks by name. It used
/// to count children — `len() == 2` meant "there is a header" — which is the answer that
/// changes the moment anything else is put in the body.
///
/// ⚠️ Ran red first: with the parts unnamed, a two-child body reports the first child's height
/// as a header that is not there.
#[test]
fn a_second_thing_in_the_body_is_not_mistaken_for_a_header() {
    use heca_grid_ui::widgets::Flex;
    use heca_grid_ui::{LayoutEngine, Size};

    let named = |n: &dyn Component, name: &str| -> bool {
        fn walk(n: &dyn Component, name: &str) -> bool {
            n.base().grid_area.as_deref() == Some(name)
                || n.base().children.iter().any(|c| walk(c.as_ref(), name))
        }
        walk(n, name)
    };

    // A body holding two things and NO header.
    let (cb, _) = recording_callbacks();
    let m = model(9);
    let mut tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: Some(Box::new(
            Flex::column().child([Flex::column().height(20.0), Flex::column().height(20.0)]),
        )),
    }
    .build();
    LayoutEngine::new().compute(&mut tree, Size::new(200.0, 200.0));

    assert!(
        !named(&tree, PANE_HEADER_AREA),
        "no part claims the header row, however many things the body holds",
    );
    assert!(
        named(&tree, PANE_CONTENT_AREA),
        "the body is in the content row"
    );

    // And with a header, it is the one that says so.
    let mut with_header = PaneShell {
        model: &m,
        cb: &cb,
        header: Some(crate::chrome::PaneHeader::showing(|_, _| {})),
        content: Some(Box::new(Flex::column())),
    }
    .build();
    LayoutEngine::new().compute(&mut with_header, Size::new(200.0, 200.0));
    assert!(named(&with_header, PANE_HEADER_AREA));
}

/// **A pane shows the facts it is handed in the header it holds** — the header is the pane's
/// own, so whoever owns the pane never reaches it. A pane with no header takes none, and a value
/// of any other type is refused.
#[test]
fn a_pane_shows_the_facts_it_is_handed_in_its_own_header() {
    use crate::chrome::PaneHeader;
    use crate::chrome::pane_header::PaneHeaderInput;
    use heca_grid_ui::Component;
    let (cb, _) = recording_callbacks();
    let m = model(4);
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut with = PaneShell {
        model: &m,
        cb: &cb,
        header: Some(PaneHeader::showing({
            let seen = seen.clone();
            move |input, _| seen.borrow_mut().push(input.pane())
        })),
        content: None,
    }
    .build();
    assert!(with.set_props(&PaneHeaderInput::for_test(PaneId(4))));
    assert_eq!(*seen.borrow(), vec![PaneId(4)]);
    assert!(!with.set_props(&"not facts"), "another type is refused");

    let mut without = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    assert!(
        !without.set_props(&PaneHeaderInput::for_test(PaneId(4))),
        "a pane with no header takes none",
    );
}

/// It renders what it was given: the pane's own identity, from its id.
#[test]
fn the_pane_declares_its_own_identity() {
    let (cb, _) = recording_callbacks();
    let m = model(7);
    let tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    // **The identity is on the outermost node**, because the pane IS the outermost node.
    // It sat one level in while a `KeyHint` wrapped it, so anything looking a pane up by name
    // found an anonymous wrapper, judged the pane unbuilt, and rebuilt every one of them every
    // frame.
    assert_eq!(tree.base().key.as_deref(), Some("pane:7"));
}

/// **An active pane re-tints what it holds by publishing its hue, not by swapping the theme.**
///
/// The frame colour used to be written into a *copy of the theme* the host painted this pane
/// under — which is why each pane needed its own paint call, and therefore why no container
/// could ever paint one of its children. Published on the shell, the hue reaches the same
/// widgets through the ordinary paint walk and who paints the pane stops mattering.
///
/// Without this a pane's contents fall back to the global accent: header buttons in the active
/// pane stop reading as active, and nothing else notices.
#[test]
fn a_pane_publishes_its_frame_colour_to_everything_it_holds() {
    let (cb, _) = recording_callbacks();
    let m = model(7);
    let mut tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();

    let frame = [1.0, 0.0, 0.0, 1.0];
    focus_state_to(
        &mut tree,
        &PaneShellModel {
            active: true,
            border_color: frame,
            ..m
        },
    );

    assert_eq!(
        tree.base().style.visual.accent,
        Some(to_gui_color(frame)),
        "the pane's contents follow its frame colour, whoever paints them",
    );
}

/// **A zoom resizes the pane without rebuilding it** — the regression that shipped.
///
/// The retained tree is deliberately NOT rebuilt when the rect changes (a rebuild mid-drag
/// throws away the widget's signals), so the size must be a per-frame input. When it was baked
/// into `build` instead, the frame kept its first width forever: traced live as asked 648 wide,
/// got 380, every frame, while the terminal content moved to the new rect.
#[test]
fn resizing_an_already_built_shell_moves_the_frame() {
    let (cb, _) = recording_callbacks();
    let m = model(1);
    let mut tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();

    size_to(&mut tree, 320.0, 728.0);
    LayoutEngine::new().compute(&mut tree, Size::new(320.0, 728.0));
    assert!((tree.base().bounds.size.w - 320.0).abs() < 0.5);

    // …now zoom it. Same tree, no rebuild.
    size_to(&mut tree, 648.0, 728.0);
    LayoutEngine::new().compute(&mut tree, Size::new(648.0, 728.0));
    assert!(
        (tree.base().bounds.size.w - 648.0).abs() < 0.5,
        "the frame stayed at {} after the pane grew to 648",
        tree.base().bounds.size.w
    );
}

/// The box test: laid out at the rect it was given, the shell never exceeds it. This is the one
/// assertion that catches a component computing geometry of its own.
#[test]
fn the_shell_never_exceeds_the_box_it_is_given() {
    let (cb, _) = recording_callbacks();
    for (w, h) in [(400.0, 300.0), (120.0, 80.0), (1600.0, 900.0)] {
        let mut m = model(1);
        m.w = w;
        m.h = h;
        let mut tree = PaneShell {
            model: &m,
            cb: &cb,
            header: None,
            content: None,
        }
        .build();
        size_to(&mut tree, w, h);
        LayoutEngine::new().compute(&mut tree, Size::new(w as f64, h as f64));
        let b = tree.base().bounds;
        // EXACT, not "within": the pane's rect is given by the WM layout engine, so the frame
        // must land on it to the pixel. `<=` would pass while the border was drawn short —
        // which is exactly the regression this assertion was added for.
        assert!(
            (b.size.w - w as f64).abs() < 0.5 && (b.size.h - h as f64).abs() < 0.5,
            "shell laid out {}x{} for a {w}x{h} pane",
            b.size.w,
            b.size.h
        );
    }
}

/// **And focus still shows**, because it is written on instead. Guarded because the tempting
/// way to pass the test above is to stop the glow appearing at all.
#[test]
fn a_focused_pane_still_glows() {
    use heca_grid_ui::Component;

    let (cb, _) = crate::chrome::pane::testing::recording_callbacks();
    let mut pane = PaneShell {
        model: &model(3),
        cb: &cb,
        header: None,
        content: None,
    }
    .build();

    focus_state_to(
        &mut pane,
        &PaneShellModel {
            active: false,
            ..model(3)
        },
    );
    assert!(
        pane.base().style.visual.glow.is_none(),
        "an unfocused pane has no halo"
    );

    focus_state_to(
        &mut pane,
        &PaneShellModel {
            active: true,
            ..model(3)
        },
    );
    assert!(
        pane.base().style.visual.glow.is_some(),
        "and a focused one does, without the tree being rebuilt to get it",
    );
}

/// **A header that changes shape is built again, and nothing else is.** A chip appearing — a working
/// folder arriving, a button shown — needs a new header tree; the terminal in the pane's body keeps
/// the node it had, so its scrollback controls, its search and anything in flight survive.
#[test]
fn a_header_that_changes_shape_leaves_what_the_pane_holds_alone() {
    use crate::chrome::PaneHeader;
    use crate::chrome::pane_header::PaneHeaderInput;
    use heca_grid_ui::scene::{DrawCommand, Scene};
    use heca_grid_ui::widgets::Label;

    fn content(n: &mut dyn Component) -> Option<&mut dyn Component> {
        if n.base().grid_area.as_deref() == Some(PANE_CONTENT_AREA) {
            return Some(n);
        }
        n.base_mut()
            .children
            .iter_mut()
            .find_map(|c| content(c.as_mut()))
    }
    fn header_says(n: &dyn Component, words: &str) -> bool {
        let theme = heca_grid_ui::theme::Theme::default();
        let mut scene = Scene::new();
        heca_grid_ui::paint_child(n, &mut heca_grid_ui::PaintCx::new(&mut scene, &theme));
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == words))
    }
    fn lay_out(n: &mut dyn Component) {
        size_to(n, 400.0, 300.0);
        LayoutEngine::new().compute(n, Size::new(400.0, 300.0));
    }

    let (cb, _) = recording_callbacks();
    let m = model(4);
    // The header's shape here is "the chips pane N has", so new facts are a new shape.
    let mut pane = PaneShell {
        model: &m,
        cb: &cb,
        header: Some(PaneHeader::showing(|input, node| {
            let shape = format!("chips of {}", input.pane().0);
            node.show(shape.clone(), move || {
                Some(Box::new(Label::new(shape)) as Box<dyn Component>)
            });
        })),
        content: Some(Box::new(crate::chrome::terminal::Terminal::new())),
    }
    .build();
    assert!(pane.set_props(&PaneHeaderInput::for_test(PaneId(1))));
    lay_out(&mut pane);
    assert!(header_says(&pane, "chips of 1"));
    // A mark only this node of the content carries.
    content(&mut pane).expect("a body").base_mut().key = Some("placed once".into());

    assert!(pane.set_props(&PaneHeaderInput::for_test(PaneId(2))));
    lay_out(&mut pane);

    assert!(
        header_says(&pane, "chips of 2"),
        "the header took its new shape"
    );
    assert_eq!(
        content(&mut pane).expect("a body").base().key.as_deref(),
        Some("placed once"),
        "the terminal is the node it was"
    );
}
