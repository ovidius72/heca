use super::*;

#[test]
fn toast_stack_is_overlay_active_only_when_it_has_toasts() {
    use heca_grid_ui::{Component, ToastSpec, ToastStack};

    let items = signal(Vec::<ToastSpec>::new());
    let mut stack = ToastStack::new(items);
    stack.tick(0.0); // reconcile (empty)
    assert!(!stack.overlay_active(), "empty stack doesn't grab input");

    items.set(vec![ToastSpec::new(1, "Saved"), ToastSpec::new(2, "Done")]);
    stack.tick(0.0); // reconcile (now 2)
    assert!(
        stack.overlay_active(),
        "a non-empty stack is overlay-active"
    );
}

#[test]
fn toast_stack_dismiss_reports_the_clicked_id() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};
    use std::cell::Cell;
    use std::rc::Rc;

    let dismissed = Rc::new(Cell::new(0u64));
    let d = dismissed.clone();
    let items = signal(vec![ToastSpec::new(7, "Connection lost").body("Retrying")]);
    let mut stack = ToastStack::new(items)
        .position(ToastPosition::TopLeft)
        .on_dismiss(move |id| d.set(id));
    settled_stack(&mut stack, Size::new(800.0, 600.0));

    // The × is the card's last child. Ask the ENGINE where it landed rather than guessing a
    // gutter: the card composes its content, so only the layout pass knows.
    let card = stack.base().children[0].base();
    let x_bounds = card.children[2].base().bounds;
    let at = Point::new(
        x_bounds.loc.x + x_bounds.size.w / 2.0,
        x_bounds.loc.y + x_bounds.size.h / 2.0,
    );
    let _ = heca_grid_ui::dispatch(&mut stack, &Event::pointer_pressed(at, PointerButton::Left));
    let hit = heca_grid_ui::dispatch(
        &mut stack,
        &Event::pointer_released(at, PointerButton::Left),
    );
    assert!(
        matches!(hit, Handled::Yes),
        "a click on a toast's × is consumed"
    );
    assert_eq!(
        dismissed.get(),
        7,
        "the dismissed toast's id is reported to the host"
    );
}

#[test]
fn toast_stack_passes_through_clicks_that_miss_every_toast() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};

    let items = signal(vec![ToastSpec::new(1, "Hi")]);
    let mut stack = ToastStack::new(items).position(ToastPosition::TopLeft);
    settled_stack(&mut stack, Size::new(800.0, 600.0));
    // Far from the top-left toast → not consumed, so the UI behind still gets it.
    let hit = heca_grid_ui::dispatch(
        &mut stack,
        &Event::pointer_pressed(Point::new(700.0, 500.0), PointerButton::Left),
    );
    assert!(
        matches!(hit, Handled::No),
        "clicks that miss every toast pass through"
    );
}

/// **A stacked card's action is reachable by keyboard** — the whole reason the cards had to be in
/// the tree. The picker walks the laid-out tree, so a card kept in a private list beside it could
/// never be lettered, and a notification's action was a mouse-only gesture (F003/P096/T486).
#[test]
fn a_stacked_cards_action_is_a_pick_target() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};

    let items = signal(vec![
        ToastSpec::new(1, "Build failed").action("retry", "Retry"),
    ]);
    let mut stack = ToastStack::new(items).position(ToastPosition::TopRight);
    settled_stack(&mut stack, Size::new(800.0, 600.0));

    let targets = heca_grid_ui::hint::collect_hints(&stack);
    assert!(
        !targets.is_empty(),
        "nothing inside a stacked card can be picked — the cards are not in the tree",
    );
    // The action and the × are both real controls, so both are offered a letter, and both are
    // inside the card rather than at the stack's own origin.
    let card = stack.base().children[0].base().bounds;
    assert!(
        targets.iter().all(|(_, r)| card.contains(r.loc)),
        "a pick target landed outside the card that holds it: {targets:?}",
    );
}

/// **Hover reaches inside a stacked card**, and stops there. The framework marks hover along the
/// hit-test target's ancestor chain; while the cards were hand-routed nothing was ever marked, so
/// a stacked card's action and × stayed dark under the pointer (reported twice).
#[test]
fn hovering_a_stacked_cards_action_lights_it_and_claims_the_move() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};

    let items = signal(vec![
        ToastSpec::new(1, "Build failed").action("retry", "Retry"),
    ]);
    let mut stack = ToastStack::new(items).position(ToastPosition::TopLeft);
    settled_stack(&mut stack, Size::new(800.0, 600.0));

    // The action lives in the card's text column, in the action row under the body.
    let action = stack.base().children[0].base().children[1].base().children[2]
        .base()
        .children[0]
        .base()
        .bounds;
    assert!(action.size.w > 0.0, "the action was never laid out");
    let at = Point::new(
        action.loc.x + action.size.w / 2.0,
        action.loc.y + action.size.h / 2.0,
    );

    let hit = heca_grid_ui::dispatch(&mut stack, &Event::pointer_moved(at));
    assert!(
        matches!(hit, Handled::Yes),
        "a move over a card is claimed, or the page behind lights up under a toast",
    );
    let action_widget = &stack.base().children[0].base().children[1].base().children[2]
        .base()
        .children[0];
    assert!(
        action_widget.base().hovered(),
        "the action under the pointer is not lit"
    );

    // …and a move that misses every card is not claimed, so the page keeps hovering normally.
    let miss = heca_grid_ui::dispatch(&mut stack, &Event::pointer_moved(Point::new(700.0, 560.0)));
    assert!(
        matches!(miss, Handled::No),
        "a move between the cards must fall through"
    );
}

/// **A widget that changes the shape of its own tree asks the host to lay it out again.**
///
/// A repaint cannot fix a structural change: the siblings are still laid out around the shape the
/// tree used to have. The stack drops a card only once its exit has played — several frames after
/// the click — and nothing re-laid-out at that moment, so the cards below kept their old positions
/// until an unrelated click happened to trigger a pass (Antonio, driving the showcase, F003/P096).
#[test]
fn a_dismissed_card_asks_for_the_relayout_that_moves_the_others_up() {
    use heca_grid_ui::{ToastSpec, ToastStack};

    let items = signal(vec![
        ToastSpec::new(1, "one"),
        ToastSpec::new(2, "two"),
        ToastSpec::new(3, "three"),
    ]);
    let mut stack = ToastStack::new(items);
    settled_stack(&mut stack, Size::new(800.0, 600.0));
    // Clear whatever the arrivals asked for; we are testing the dismissal.
    let _ = heca_grid_ui::needs_layout(&stack);
    assert!(
        !heca_grid_ui::needs_layout(&stack),
        "a settled stack keeps asking for layout"
    );

    // Drop the MIDDLE one — the case where the others must move.
    items.set(vec![ToastSpec::new(1, "one"), ToastSpec::new(3, "three")]);
    stack.tick(0.0);
    assert_eq!(stack.base().children.len(), 3, "it should still be leaving");
    assert!(
        !heca_grid_ui::needs_layout(&stack),
        "nothing structural has happened yet — the card is still on screen, playing its exit",
    );

    // Play the exit out. The frame the card actually leaves the tree is the frame that must ask.
    while stack.tick(1.0 / 60.0) {}
    assert_eq!(stack.base().children.len(), 2, "the card should be gone");
    assert!(
        heca_grid_ui::needs_layout(&stack),
        "the card left the tree and nobody asked for a layout — the cards below it stay put",
    );
    assert!(
        !heca_grid_ui::needs_layout(&stack),
        "the flag is cleared as it is read"
    );
}

/// **A card offers as many actions as its spec lists, and says WHICH was pressed.** One action was
/// a widget limitation written into the data: a notification that can be retried *and* inspected
/// needs two, and "which card" alone cannot say what to do (F003/P096/T487).
#[test]
fn a_spec_with_two_actions_renders_two_and_reports_the_pressed_key() {
    use heca_grid_ui::{ButtonVariant, ToastAction, ToastPosition, ToastSpec, ToastStack};
    use std::cell::RefCell;
    use std::rc::Rc;

    let pressed: Rc<RefCell<Vec<(u64, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let p = pressed.clone();
    let items = signal(vec![
        ToastSpec::new(9, "Build failed")
            .action("rebuild", "Retry")
            .action_with(ToastAction::new("open_log", "View log").variant(ButtonVariant::Ghost)),
    ]);
    let mut stack = ToastStack::new(items)
        .position(ToastPosition::TopLeft)
        .on_action(move |id, key| p.borrow_mut().push((id, key.to_string())));
    settled_stack(&mut stack, Size::new(800.0, 600.0));

    // The action row is the third child of the card's text column.
    let row = &stack.base().children[0].base().children[1].base().children[2];
    assert_eq!(
        row.base().children.len(),
        2,
        "two actions in the spec, two controls on the card"
    );

    // Press the SECOND one — the point is that the key distinguishes them.
    let b = row.base().children[1].base().bounds;
    let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let _ = heca_grid_ui::dispatch(&mut stack, &Event::pointer_pressed(at, PointerButton::Left));
    let _ = heca_grid_ui::dispatch(
        &mut stack,
        &Event::pointer_released(at, PointerButton::Left),
    );
    assert_eq!(
        pressed.borrow().as_slice(),
        &[(9, "open_log".to_string())],
        "the pressed action's own key comes back, not just the card id",
    );
}

/// **No actions means no action row at all** — an unfilled slot leaves the layout rather than
/// sitting empty, so a bare notification costs neither space nor a gap.
#[test]
fn a_spec_with_no_actions_renders_no_action_row() {
    use heca_grid_ui::{ToastPosition, ToastSpec, ToastStack};

    let items = signal(vec![ToastSpec::new(1, "Saved")]);
    let mut stack = ToastStack::new(items).position(ToastPosition::TopLeft);
    settled_stack(&mut stack, Size::new(800.0, 600.0));

    let row = &stack.base().children[0].base().children[1].base().children[2];
    assert!(
        row.base().children.is_empty(),
        "an empty action slot built controls"
    );
    assert_eq!(
        row.base().bounds.size.h,
        0.0,
        "an empty action row still takes {}px",
        row.base().bounds.size.h
    );
}

/// **A card the host drops leaves, and is gone once its exit has played** — not the frame its id
/// disappears, which would cut a notification off mid-gesture.
#[test]
fn a_dropped_card_plays_its_exit_before_it_goes() {
    use heca_grid_ui::{ToastSpec, ToastStack};

    let items = signal(vec![ToastSpec::new(1, "Saved"), ToastSpec::new(2, "Done")]);
    let mut stack = ToastStack::new(items);
    settled_stack(&mut stack, Size::new(800.0, 600.0));
    assert_eq!(stack.base().children.len(), 2);

    items.set(vec![ToastSpec::new(2, "Done")]);
    stack.tick(0.0);
    assert_eq!(
        stack.base().children.len(),
        2,
        "the dropped card went the frame its id did — its exit had nothing to play over",
    );
    while stack.tick(1.0 / 60.0) {}
    assert_eq!(
        stack.base().children.len(),
        1,
        "…and once it has played, it is gone"
    );
}

/// **A card arrives and leaves on its own** — the library's verbs, not a mechanism of its own, and
/// a closed one takes no space rather than leaving a hole (F003/P096/T485).
#[test]
fn a_toast_opens_hides_and_takes_no_space_while_closed() {
    use heca_grid_ui::{Component, Flex, LayoutExt, Length, Parent, Toast};

    let mut page = Flex::column()
        .width(Length::Px(400.0))
        .child(Toast::info("Saved").default_open(false))
        .child(Toast::info("Also saved"));
    LayoutEngine::new().compute(&mut page, Size::new(400.0, 300.0));
    let closed = page.base().children[0].base().bounds.size.h;
    assert_eq!(closed, 0.0, "a closed card still occupies {closed}px");

    // Opening puts it back in the flow; hiding takes it out again.
    page.base_mut().children[0].show();
    LayoutEngine::new().compute(&mut page, Size::new(400.0, 300.0));
    assert!(
        page.base().children[0].base().bounds.size.h > 0.0,
        "open, it is laid out"
    );

    // Hiding starts the exit. **It stays laid out while it plays** — that is what the gesture has
    // to play over — and takes no space only once it has finished (the card's default is a slide).
    page.base_mut().children[0].close();
    LayoutEngine::new().compute(&mut page, Size::new(400.0, 300.0));
    assert!(
        page.base().children[0].base().bounds.size.h > 0.0,
        "a leaving card is still laid out, or its exit has nothing to play over",
    );
    while page.base_mut().children[0].tick(1.0 / 60.0) {}
    LayoutEngine::new().compute(&mut page, Size::new(400.0, 300.0));
    assert_eq!(
        page.base().children[0].base().bounds.size.h,
        0.0,
        "gone, it is gone"
    );
}
