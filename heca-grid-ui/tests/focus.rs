mod common;

use common::type_text;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{Event, LayoutEngine, Point, Size};

#[test]
fn focus_traversal_and_keyboard_activation() {
    use heca_grid_ui::FocusManager;
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(0u32));
    let (a, b) = (clicked.clone(), clicked.clone());
    let mut ui = Flex::row()
        .child(Button::primary("A").on_click(move || a.set(a.get() + 1)))
        .child(Button::secondary("B").on_click(move || b.set(b.get() + 1)));

    let mut focus = FocusManager::new();
    assert_eq!(focus.focused(&mut ui), None);

    // Tab → first focusable; Space activates it.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(&mut ui), Some(0));
    focus.deliver_key(&mut ui, GridKey::Space);
    assert_eq!(clicked.get(), 1);

    // Tab → second; Enter activates it.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(&mut ui), Some(1));
    focus.deliver_key(&mut ui, GridKey::Enter);
    assert_eq!(clicked.get(), 2);

    // Forward wraps to first; backward wraps to last.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(&mut ui), Some(0));
    focus.advance(&mut ui, false);
    assert_eq!(focus.focused(&mut ui), Some(1));
}

#[test]
fn a_press_focuses_the_focusable_under_it_and_a_miss_keeps_focus() {
    use heca_grid_ui::FocusManager;

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    // Center of the second button (focus index 1).
    let b = ui.base().children[1].base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);

    let focus = FocusManager::new();
    common::press_at(&mut ui, center, PointerButton::Left);
    assert_eq!(
        focus.focused(&mut ui),
        Some(1),
        "a press focuses the focusable under it"
    );
    assert!(
        ui.base().children[1].base().focused.get_untracked(),
        "hit button shows focus"
    );
    assert!(
        !ui.base().children[1].base().focused_by_keyboard(),
        "a press is the mouse: no ring"
    );

    // A press that lands on nothing focusable changes nothing.
    common::press_at(&mut ui, Point::new(9999.0, 9999.0), PointerButton::Left);
    assert_eq!(
        focus.focused(&mut ui),
        Some(1),
        "a press on nothing focusable keeps focus"
    );

    // A press on the other button moves focus to it, and the first lets go.
    let a = ui.base().children[0].base().bounds;
    common::press_at(
        &mut ui,
        Point::new(a.loc.x + a.size.w / 2.0, a.loc.y + a.size.h / 2.0),
        PointerButton::Left,
    );
    assert_eq!(
        focus.focused(&mut ui),
        Some(0),
        "a press elsewhere moves focus"
    );
    assert!(
        !ui.base().children[1].base().focused.get_untracked(),
        "the previous holder let go"
    );
}

#[test]
fn dispatch_focuses_on_press_and_falls_through_when_unconsumed() {
    use heca_grid_ui::FocusManager;

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    let b = ui.base().children[1].base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);

    let mut focus = FocusManager::new();
    // No overlay open → nothing to offer.
    assert_eq!(
        focus.offer_to_overlay(&mut ui, &Event::wheel(center, 0.0, 1.0)),
        Handled::No,
        "no open overlay → nothing consumes the offer"
    );

    // A press dispatches with focus-on-press semantics: the clicked widget focuses.
    focus.dispatch(
        &mut ui,
        &Event::pointer_pressed(center, PointerButton::Left),
    );
    assert_eq!(
        focus.focused(&mut ui),
        Some(1),
        "dispatch focuses the pressed widget"
    );

    // A press that misses every focusable keeps focus where it was.
    focus.dispatch(
        &mut ui,
        &Event::pointer_pressed(Point::new(9999.0, 9999.0), PointerButton::Left),
    );
    assert_eq!(
        focus.focused(&mut ui),
        Some(1),
        "dispatch keeps focus on a miss"
    );

    // No widget consumes a scroll → dispatch reports No so the host can page-scroll.
    assert_eq!(
        focus.dispatch(&mut ui, &Event::wheel(center, 0.0, 1.0)),
        Handled::No,
        "unconsumed scroll falls through to the host"
    );
}

#[test]
fn dispatch_gives_an_open_overlay_first_dibs() {
    use heca_grid_ui::FocusManager;

    // A Select is overlay-capable: while open it grabs input outside its bounds.
    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Select::new(["LOW", "MEDIUM", "HIGH"]));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 200.0));

    let mut focus = FocusManager::new();
    let sb = ui.base().children[1].base().bounds;

    // Press on the Select trigger opens its dropdown (no overlay yet → normal route).
    focus.dispatch(
        &mut ui,
        &Event::pointer_pressed(
            Point::new(sb.loc.x + 5.0, sb.loc.y + 5.0),
            PointerButton::Left,
        ),
    );
    assert!(
        focus.overlay_active(&mut ui),
        "pressing the trigger opens the dropdown overlay"
    );

    // With the dropdown open, a press on a row (outside the trigger's layout bounds)
    // is grabbed by the overlay first — it commits the selection and closes — rather
    // than being treated as a fresh focus/click on the tree behind it. The row is
    // found by its **bounds**: the options are real children, placed in the panel.
    let row2 = ui.base().children[1].base().children[2].base().bounds;
    let handled = focus.dispatch(
        &mut ui,
        &Event::pointer_pressed(
            Point::new(row2.loc.x + 10.0, row2.loc.y + row2.size.h / 2.0),
            PointerButton::Left,
        ),
    );
    assert_eq!(handled, Handled::Yes, "the open overlay consumes the press");
    assert!(
        !focus.overlay_active(&mut ui),
        "committing a row closes the dropdown"
    );
}

#[test]
fn focusable_is_driven_by_the_base_flag_and_disabled() {
    // Non-interactive widgets stay unfocusable (default `Base.focusable == false`).
    assert!(
        !Label::new("x").focusable(),
        "a plain label is not focusable"
    );

    // Always-focusable controls opt in from their constructor…
    assert!(Input::new().focusable(), "an input is focusable");
    assert!(Button::new("OK").focusable(), "a button is focusable");
    // …and the centralized default excludes disabled widgets.
    assert!(
        !Button::new("OK").disabled(true).focusable(),
        "a disabled button is not focusable",
    );

    // Conditionally-interactive rows opt in only once a callback is wired.
    assert!(
        !Row::new().focusable(),
        "a row with no on_activate is not focusable",
    );
    assert!(
        Row::new().on_activate(|| {}).focusable(),
        "a row becomes focusable once on_activate is wired",
    );
    assert!(
        !Row::new().on_activate(|| {}).disabled(true).focusable(),
        "a disabled interactive row is not focusable",
    );
}

#[test]
fn tab_index_orders_traversal_before_position() {
    // Visual order A, B, C — but B has tab_index 1, A has 2, C is unindexed.
    // Tab order: indexed ascending (B, A) then unindexed by position (C).
    let mut ui = Flex::row()
        .child(Button::primary("A").tab_index(2))
        .child(Button::primary("B").tab_index(1))
        .child(Button::primary("C"));
    LayoutEngine::new().compute(&mut ui, Size::new(600.0, 100.0));

    let focused_child = |ui: &Flex| -> Option<usize> {
        // Returns which child (by position) holds focus.
        (0..3).find(|&i| ui.base().children[i].base().focused.get_untracked())
    };

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    assert_eq!(focused_child(&ui), Some(1), "first Tab → B (tab_index 1)");
    focus.advance(&mut ui, true);
    assert_eq!(focused_child(&ui), Some(0), "next → A (tab_index 2)");
    focus.advance(&mut ui, true);
    assert_eq!(focused_child(&ui), Some(2), "then unindexed C by position");
    focus.advance(&mut ui, true);
    assert_eq!(focused_child(&ui), Some(1), "wraps back to B");
}

#[test]
fn disabled_widget_skipped_by_focus_traversal() {
    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Toggle::new().disabled(true))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(&mut ui), Some(0), "first button focuses");
    // The disabled toggle is not focusable, so Tab lands on the second button.
    focus.advance(&mut ui, true);
    assert_eq!(
        focus.focused(&mut ui),
        Some(1),
        "disabled toggle is skipped"
    );
}

#[test]
fn a_raw_char_key_is_a_shortcut_not_text() {
    use heca_grid_ui::{Input, Modifiers};
    // **Text is not a key.** A field types from `Event::TextInput` — the character the user
    // actually committed, whatever produced it — so a raw `Char` key is only ever a shortcut and
    // the field leaves it alone, modified or not. That is what the host's "deliver the real
    // character, not the lowercased combo key" fixup used to work around.
    let mut inp = Input::new().value("hi");
    inp.base().focus(false);
    heca_grid_ui::dispatch(
        &mut inp,
        &Event::ModifiersChanged(Modifiers {
            ctrl: true,
            ..Default::default()
        }),
    );
    assert_eq!(
        heca_grid_ui::dispatch(
            &mut inp,
            &Event::Key {
                key: GridKey::Char('h'),
                pressed: true
            }
        ),
        heca_grid_ui::Handled::No,
        "a raw char key is left for whoever resolves shortcuts",
    );
    assert_eq!(inp.value_str(), "hi", "and nothing was typed");

    // The text channel is what types.
    type_text(&mut inp, "!");
    assert_eq!(inp.value_str(), "hi!");
}

/// **Pointing at something never scrolls it.** A reveal brings into view what the user cannot see
/// — the keyboard's case. What the mouse is on is visible by definition, and scrolling it moves it
/// out from under the pointer that asked: clicking a row in a scrolled region focused it, the
/// focus asked for a reveal, the region centred it, and the click was spent — only the second one
/// did what you meant.
#[test]
fn a_click_does_not_ask_to_be_scrolled_into_view_but_the_keyboard_does() {
    use heca_grid_ui::{Component, FocusManager, Label, Parent as _, PointerButton};

    let mut row = heca_grid_ui::Row::new()
        .child(Label::new("pane-1"))
        .on_activate(|| {});
    row.base_mut().style.layout.width = Length::Px(200.0);
    LayoutEngine::new()
        .base_font(14.0)
        .compute(&mut row, Size::new(200.0, 40.0));
    assert!(!row.wants_visible(), "an untouched row asks for nothing");

    // Clicking focuses it — but the mouse is already looking at it.
    let b = row.base().bounds;
    let at = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let mut focus = FocusManager::new();
    focus.dispatch(&mut row, &Event::pointer_pressed(at, PointerButton::Left));
    focus.dispatch(&mut row, &Event::pointer_released(at, PointerButton::Left));
    assert!(
        row.base().focused.get_untracked(),
        "the click should still focus it"
    );
    assert!(
        !row.wants_visible(),
        "a clicked row asked to be scrolled into view — the region centres it and eats the click",
    );

    // Tab is the case a reveal exists for: the cursor can go somewhere you cannot see. This is
    // the hook `FocusManager::advance` calls — `visible: true` is what "the keyboard did it" means.
    row.on_focus(true);
    assert!(
        row.wants_visible(),
        "keyboard focus must still bring the row into view"
    );
}

/// Focus-visible semantics (T014): a mouse click focuses a widget (Enter/Space
/// work) but draws NO ring; keyboard navigation (advance) shows it. Widgets gate
/// their ring paint on `Base::shows_focus_ring()`, which is exactly this.
#[test]
fn focus_ring_shows_on_keyboard_focus_not_on_mouse_click() {
    use heca_grid_ui::FocusManager;

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));
    let b = ui.base().children[0].base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);

    let mut focus = FocusManager::new();
    // Click-focus: focused (activation works) but the ring must NOT draw.
    focus.dispatch(
        &mut ui,
        &Event::pointer_pressed(center, PointerButton::Left),
    );
    let a = &ui.base().children[0];
    assert!(a.base().focused.get_untracked(), "click focuses the widget");
    assert!(
        !a.base().shows_focus_ring(),
        "mouse focus is not focus-visible — no ring"
    );

    // Keyboard navigation: the newly-focused widget rings.
    focus.advance(&mut ui, true);
    assert!(
        ui.base()
            .children
            .iter()
            .any(|c| c.base().shows_focus_ring()),
        "keyboard focus (advance) shows the ring"
    );
}
