//! What the pointer does to a pane: picking, clicking, its menu and carrying it.

use super::*;

/// **A pick says what it IS, not only what it runs**.
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
    let (cb, _) = crate::chrome::panes::pane::testing::recording_callbacks();
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
/// it and opened nothing. Only a second one, with nothing left to rebuild, showed the menu.
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
    assert!(tiled.base().draggable && !tiled.base().drag_chip);
    assert_eq!(
        tiled.base().drag_kind.as_deref(),
        Some(crate::chrome::PANE_DRAG_KIND)
    );
    assert!(
        tiled.base().drag_gate.is_some(),
        "only while the window's key is held"
    );

    let float = PaneShellModel {
        float: Some(crate::chrome::panes::pane::FloatLook {
            background: [0.0, 0.0, 0.0, 1.0],
            frost: 0.0,
        }),
        ..model(8)
    };
    let float = build(&float);
    assert!(!float.base().draggable);
}

/// **Whatever the pane holds, however deep its typing area sits**, the pane wears the one letter
/// and picking it types there — a browser or an editor composes its view, so what takes the
/// keyboard is rarely the content's outermost part.
#[test]
fn a_pane_types_into_what_it_holds_however_deep() {
    use heca_grid_ui::builders::Parent;
    let (cb, picked) = recording_callbacks();
    let held = crate::chrome::panes::terminal::Terminal::unbound();
    let mut pane = PaneShell {
        model: &model(5),
        cb: &cb,
        header: None,
        content: Some(heca_grid_ui::WidgetModel::new(
            heca_grid_ui::widgets::Flex::column()
                .child(heca_grid_ui::widgets::Flex::row().child(held)),
        )),
    }
    .build();
    let lettered: Vec<Vec<usize>> = heca_grid_ui::collect_hints(&pane)
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(lettered, vec![Vec::<usize>::new()], "one letter, the pane's");

    assert!(heca_grid_ui::fire_hint(&mut pane, &[]));
    assert_eq!(*picked.borrow(), vec![heca_core::layout::PaneId(5)]);
    let deep = heca_grid_ui::area(&pane, PANE_CONTENT_AREA)
        .and_then(|part| part.base().children.first())
        .and_then(|row| row.base().children.first())
        .expect("the typing area")
        .base()
        .focus_id();
    assert_eq!(heca_grid_ui::keyboard_holder(&pane), Some(deep));
}
