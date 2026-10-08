use super::*;

use crate::chrome::pane::testing::{model, recording_callbacks};
use heca_grid_ui::Component;
use heca_grid_ui::{LayoutEngine, Size};

/// Give a standalone pane the box a workspace would place it in.
fn size_to(root: &mut dyn heca_grid_ui::Component, w: f32, h: f32) {
    let style = &mut root.base_mut().style.layout;
    style.width = heca_grid_ui::Length::Px(w);
    style.height = heca_grid_ui::Length::Px(h);
}

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

/// **A pick says what it IS, not only what it runs** (F003/P082/T432).
///
/// This is what lets a policy refuse a candidate *before* a letter is spent on it: with a
/// floating pane active, `prefix+/` lettered every pane and pressing one did nothing, because
/// `ActionPolicy` correctly refuses a `FocusPane` that does not target the active float. A
/// closure gives the policy nothing to ask about — so if this declaration is ever reduced back
/// to one, every pane silently gets its useless letter again and no other test would notice.
#[test]
fn the_pane_says_what_picking_it_would_do() {
    let (cb, _) = recording_callbacks();
    let m = model(7);
    let tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    let intent = tree
        .base()
        .hint
        .as_ref()
        .expect("the pane declares a pick")
        .intent
        .as_ref()
        .expect("…and says what that pick is");
    assert_eq!(
        intent.action, "focus_pane",
        "the built-in, by the name every surface uses"
    );
    assert_eq!(
        intent.args.get("pane_id"),
        Some(&heca_view::PropValue::Int(7)),
        "aimed at this pane, so the policy can judge THIS candidate rather than panes in general"
    );
}

/// It answers what it binds: picking it focuses that pane, and nothing else.
#[test]
fn picking_the_pane_focuses_that_pane() {
    let (cb, picked) = recording_callbacks();
    let m = model(3);
    let tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    let hint = tree.base().hint.as_ref().expect("the pane declares a pick");
    hint.run();
    assert_eq!(*picked.borrow(), vec![heca_core::layout::PaneId(3)]);
}

/// The letter sits centred over the pane — the large-target placement, which is where it is
/// drawn today and what the maintainer expects.
#[test]
fn the_letter_is_centred_over_the_pane() {
    let (cb, _) = recording_callbacks();
    let m = model(1);
    let tree = PaneShell {
        model: &m,
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    assert_eq!(tree.base().hint_style.placement, HintPlacement::Center);
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

/// **What a click on a pane means, asserted through the real path.** ⚠️ Ran red first.
///
/// The pane names an action; the framework routes it. So this installs the sink a host installs
/// and reads back what the pane asked for — no callbacks, because the pane has none any more.
/// It carried a struct of host functions until now, cloned at the call site, which a plugin
/// could not have built.
fn dispatched(build: impl FnOnce(&mut UiPane)) -> Vec<String> {
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = seen.clone();
    heca_grid_ui::intent::install_intent_sink(move |i| {
        sink.borrow_mut().push(format!(
            "{}({:?})",
            i.action,
            i.args.get("pane_id").cloned()
        ))
    });

    let cb = crate::chrome::pane::testing::recording_callbacks().0;
    let mut pane = PaneShell {
        model: &model(3),
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    size_to(&mut pane, 400.0, 300.0);
    LayoutEngine::new().compute(&mut pane, Size::new(400.0, 300.0));
    build(&mut pane);
    seen.borrow().clone()
}

/// **Clicking a pane focuses it**, by naming the action rather than calling a function.
#[test]
fn a_click_on_a_pane_asks_for_that_pane() {
    use heca_grid_ui::event::{Event, PointerEvent};
    let at = heca_core::layout::Point::new(50.0, 50.0);
    let fired = dispatched(|pane| {
        heca_grid_ui::dispatch(pane, &Event::Click(PointerEvent::at(at)));
    });
    assert_eq!(
        fired,
        vec!["focus_pane(Some(Int(3)))".to_string()],
        "the pane named the action and the framework carried it",
    );
}

/// **A right-click focuses it too, and deliberately does NOT claim the click.**
///
/// A declared menu opens only for a right-click nobody took, so claiming it here would switch
/// this pane's own menu off — two declarations on one widget cancelling each other, with
/// nothing failing anywhere. Guarded because `stop_propagation` reads like the tidy thing to
/// add and would break the menu silently.
#[test]
fn a_right_click_focuses_the_pane_without_claiming_the_click() {
    use heca_grid_ui::event::{Event, PointerEvent};
    let at = heca_core::layout::Point::new(50.0, 50.0);
    let mut handled = heca_grid_ui::Handled::Yes;
    let fired = dispatched(|pane| {
        handled = heca_grid_ui::dispatch(pane, &Event::RightClick(PointerEvent::at(at)));
    });
    assert_eq!(fired, vec!["focus_pane(Some(Int(3)))".to_string()]);
    assert_eq!(
        handled,
        heca_grid_ui::Handled::No,
        "left unclaimed, so the framework goes on to open the menu the pane declared",
    );
}

/// **And it has a menu it does not own.** Declared, so the framework opens it at the pointer
/// and takes it down when a press lands outside — a pane has no business knowing what a menu
/// is.
#[test]
fn a_pane_declares_a_menu_rather_than_opening_one() {
    use heca_grid_ui::Component;
    let (cb, _) = crate::chrome::pane::testing::recording_callbacks();
    let pane = PaneShell {
        model: &model(3),
        cb: &cb,
        header: None,
        content: None,
    }
    .build();
    // The declaration sits on the pane itself, inside the picker wrapper the shell returns.
    fn declares_a_menu(n: &dyn Component) -> bool {
        n.base().context_menu.is_some()
            || n.base()
                .children
                .iter()
                .any(|c| declares_a_menu(c.as_ref()))
    }
    assert!(
        declares_a_menu(&pane),
        "the pane says it has one; opening and closing it is nobody else's business here",
    );
}

/// **Focusing a pane must not rebuild it.** ⚠️ Ran red against its own bug.
///
/// A right-click is made from a press and a release on the *same* widget. Whether a pane is
/// active used to be part of its identity, so focusing it — which is what the press itself
/// asks for — threw the tree away and built a new one. The release then landed on a different
/// widget, no click was ever completed, and the first right-click on an unfocused pane focused
/// it and opened nothing. Only a second one, with nothing left to rebuild, showed the menu
///.
///
/// The same trap caught the pane's rect and its header's words before this. Anything that
/// changes while a gesture is in flight belongs on the tree, not in the key.
#[test]
fn focusing_a_pane_does_not_change_its_identity() {
    let mut inactive = model(3);
    inactive.active = false;
    let mut active = model(3);
    active.active = true;
    active.border_color = [1.0, 0.0, 0.0, 1.0];
    active.accent = [1.0, 0.0, 0.0, 1.0];

    assert_eq!(
        inactive.key(),
        active.key(),
        "the same pane, focused or not, is the same pane — so focusing it keeps its tree and \
         the press already recorded on it",
    );
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

/// **A tiled pane is carried by the window's key, as a picture of itself; a float is not carried at
/// all** — it is over everything and moves where it was put.
#[test]
fn a_tiled_pane_is_carried_as_a_picture_and_a_float_is_not_carried() {
    let (cb, _) = recording_callbacks();
    let build = |m: &PaneShellModel| {
        PaneShell {
            model: m,
            cb: &cb,
            header: None,
            content: None,
        }
        .build()
    };
    let tiled = build(&model(7));
    assert!(tiled.base().draggable && tiled.base().drag_image);
    assert_eq!(tiled.base().drag_kind.as_deref(), Some(crate::chrome::PANE_DRAG_KIND));
    assert!(tiled.base().drag_gate.is_some(), "only while the window's key is held");

    let float = PaneShellModel {
        float: Some(crate::chrome::pane::FloatLook {
            background: [0.0, 0.0, 0.0, 1.0],
            frost: 0.0,
        }),
        ..model(8)
    };
    let float = build(&float);
    assert!(!float.base().draggable && !float.base().drag_image);
}
