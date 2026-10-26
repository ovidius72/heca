use super::*;
use crate::event::PointerButton;
use crate::widgets::{Button, Label};
use std::cell::Cell;
use std::rc::Rc;

fn open_dialog() -> Dialog {
    Dialog::new("Delete pane?")
        .body(Label::new("This action cannot be undone."))
        .action(Button::new("Cancel"))
        .action(Button::new("Delete"))
        .default_open(true)
}

/// An open dialog wired with a dismiss flag, for the Esc / scrim tests.
fn open_dialog_with_flag() -> (Dialog, Rc<Cell<bool>>) {
    let flag = Rc::new(Cell::new(false));
    let f = flag.clone();
    let d = open_dialog().on_dismiss(move || f.set(true));
    (d, flag)
}

#[test]
fn structure_is_overlay_holding_panel_with_title_body_and_action_row() {
    let d = open_dialog();
    // One child: the composed base Overlay (the blocking layer).
    assert_eq!(d.base().children.len(), 1);
    let overlay = &d.base().children[0];
    // The overlay's single child is the panel: [title, body, action-row].
    assert_eq!(overlay.base().children.len(), 1, "overlay holds the panel");
    let panel = crate::component::area(&d, LAYOUT_AREA).unwrap();
    assert_eq!(panel.base().children.len(), 3, "title + body + action row");
    // Action row holds the two buttons.
    let row = &panel.base().children[2];
    assert_eq!(row.base().children.len(), 2, "two action buttons");
}

#[test]
fn closed_dialog_ignores_input_and_is_not_overlay() {
    let mut d = Dialog::new("x").action(Button::new("OK")); // not opened
    assert!(!d.overlay_active());
    assert!(!d.focusable());
    assert_eq!(
        crate::component::dispatch(
            &mut d,
            &Event::Key {
                key: GridKey::Escape,
                pressed: true
            }
        ),
        Handled::No,
        "a closed dialog handles nothing",
    );
}

#[test]
fn cancel_nav_fires_dismiss_when_dismissible() {
    // Dismiss arrives as the host-resolved `WidgetIntent::Dismiss` (the app maps `dismiss`,
    // default Esc, to it). The dialog no longer hardcodes the Esc key.
    let (mut d, flag) = open_dialog_with_flag();
    assert_eq!(
        crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::Yes,
    );
    assert!(flag.get(), "Dismiss fired the dismiss callback");
}

/// Buttons in the action row that currently hold focus (drives the focus ring).
fn focused_buttons(d: &Dialog) -> Vec<usize> {
    let row = crate::component::area(d, ACTIONS_AREA).unwrap();
    row.base()
        .children
        .iter()
        .enumerate()
        .filter(|(_, b)| b.base().is_focused())
        .map(|(i, _)| i)
        .collect()
}

/// The whole point of `panel_size`: a sized panel **bounds** its body, so a
/// `ScrollRegion` inside it actually overflows and scrolls. Unsized, the panel
/// grows to fit the content and the region never has anything to scroll.
#[test]
fn a_sized_panel_bounds_a_scrollable_body() {
    use crate::Length;
    use crate::builders::Parent;
    use crate::widgets::ScrollRegion;

    // 12 rows, far taller than the 200px panel we ask for.
    let long_body = || {
        let mut region = ScrollRegion::new();
        for i in 0..12 {
            region = region.child(Label::new(format!("row {i}")));
        }
        region
    };

    let viewport = Size::new(600.0, 400.0);

    let mut sized = Dialog::new("Long list")
        .panel_size(Length::Px(300.0), Length::Px(200.0))
        .body(long_body())
        .default_open(true);
    crate::LayoutEngine::new().compute(&mut sized, viewport);
    let panel = sized.panel_bounds();
    assert!(
        (panel.size.h - 200.0).abs() < 1.0,
        "panel honours the requested height, got {}",
        panel.size.h
    );
    assert!(
        panel.size.h < viewport.h,
        "a bounded panel is smaller than the viewport, so the body can overflow"
    );

    // Unsized, the same body makes the panel grow instead (nothing to scroll).
    let mut unsized_dialog = Dialog::new("Long list")
        .body(long_body())
        .default_open(true);
    crate::LayoutEngine::new().compute(&mut unsized_dialog, viewport);
    assert!(
        unsized_dialog.panel_bounds().size.h > panel.size.h,
        "without panel_size the panel hugs the tall content"
    );
}

#[test]
fn clicking_panel_body_keeps_button_focus() {
    // Regression: a press on the panel *body* (not a button) must NOT clear the
    // focused button's ring — focus is trapped inside the modal. Previously the
    // panel dispatch cleared focus on any click that missed every focusable.
    let mut d = open_dialog();
    crate::LayoutEngine::new().compute(&mut d, Size::new(600.0, 400.0));
    // Move focus by nav so a button shows the focus ring.
    let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::ItemNext));
    let before = focused_buttons(&d);
    assert!(!before.is_empty(), "a button is focused after ItemNext");

    // Click the panel body: inside the panel bounds but on the title band (no button).
    let panel = d.panel_bounds();
    let body = Point::new(panel.loc.x + panel.size.w * 0.5, panel.loc.y + 2.0);
    assert!(panel.contains(body), "test point is inside the panel body");
    let _ =
        crate::component::dispatch(&mut d, &Event::pointer_pressed(body, PointerButton::Left));

    assert_eq!(
        focused_buttons(&d),
        before,
        "clicking the panel body keeps the focused button (trapped focus)",
    );
}

#[test]
fn cancel_dismisses_even_when_forced_but_scrim_does_not() {
    // `WidgetIntent::Dismiss` is a universal modal cancel: it fires `on_dismiss` even on a
    // `dismissible(false)` (forced-choice) dialog. Only the scrim / outside-click is gated by
    // `dismissible`.
    let flag = Rc::new(Cell::new(false));
    let f = flag.clone();
    let mut d = open_dialog()
        .dismissible(false)
        .on_dismiss(move || f.set(true));
    assert_eq!(
        crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Dismiss)),
        Handled::Yes,
    );
    assert!(flag.get(), "Dismiss dismisses even a forced dialog");

    // A scrim click (press outside the panel) on a forced dialog must NOT dismiss.
    flag.set(false);
    let _ = crate::component::dispatch(
        &mut d,
        &Event::pointer_pressed(Point::new(-100.0, -100.0), PointerButton::Left),
    );
    assert!(!flag.get(), "forced dialog ignores the scrim/outside click");
}

#[test]
fn dialog_nav_moves_focus() {
    // Focus nav arrives as the semantic horizontal `Item*` intents (the host maps the
    // configurable `item_next`/`item_previous` bindings). Each is consumed and lands focus.
    for intent in [WidgetIntent::ItemNext, WidgetIntent::ItemPrevious] {
        let mut d = open_dialog();
        assert_eq!(
            crate::component::dispatch(&mut d, &Event::Widget(intent)),
            Handled::Yes
        );
        assert!(
            !focused_buttons(&d).is_empty(),
            "{intent:?} focuses a button"
        );
    }
}

#[test]
fn text_input_body_types_and_submit_nav_fires_primary() {
    use crate::widgets::Input;
    let fired = Rc::new(Cell::new(false));
    let f = fired.clone();
    // A form dialog: an Input body + OK (primary) / Cancel. `open` focuses the Input first.
    let mut d = Dialog::new("Rename pane")
        .body(Input::new().value("term"))
        .action(Button::new("OK").on_click(move || f.set(true)))
        .action(Button::new("Cancel"))
        .default_open(true);
    // Typed text is delivered field-first to (and consumed by) the focused input. Text, not a
    // key: `Event::TextInput` is what the user actually committed.
    assert_eq!(
        crate::component::dispatch(&mut d, &Event::TextInput("x".to_string())),
        Handled::Yes,
        "the focused input receives typed characters",
    );
    // `WidgetIntent::Activate` (host maps `activate`, default Enter) fires the PRIMARY action.
    assert!(!fired.get());
    let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::Activate));
    assert!(fired.get(), "Activate fires the primary action (OK)");
}

#[test]
fn input_edit_reaches_focused_field_nav_moves_to_button() {
    use crate::widgets::Input;
    // Field-first + host-resolved editing: an `Edit*` intent is forwarded to the focused
    // input (consumed there, focus stays on it), while an `Item*` intent moves focus.
    let mut d = Dialog::new("Rename")
        .body(Input::new().value("ab"))
        .action(Button::new("OK"))
        .action(Button::new("Cancel"))
        .default_open(true);
    // Editing shortcut → forwarded to the input; it is consumed and focus stays on the field.
    assert_eq!(
        crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::EditDeleteBack)),
        Handled::Yes,
        "EditDeleteBack reaches the focused input",
    );
    assert!(
        focused_buttons(&d).is_empty(),
        "editing keeps focus in the input"
    );
    // Nav moves focus off the input onto a button.
    let _ = crate::component::dispatch(&mut d, &Event::Widget(WidgetIntent::ItemNext));
    assert!(
        !focused_buttons(&d).is_empty(),
        "ItemNext navigates to a button"
    );
}

// ── A dialog answers as a surface ──────────────────────────────────────

/// **A host can raise a dialog, and until now it could not.** ⚠️ Ran red against its own bug.
///
/// A `Dialog` is composed on an `Overlay` but forwarded only its keyboard, so asking the
/// dialog itself to open reached the `Component` trait's empty default and did nothing at all.
/// The single thing that had ever raised one was `default_open(true)` at construction — which
/// is why a dialog could not be shown by name, and why a handle pointing at one moved a flag
/// no surface was reading.
#[test]
fn asking_a_dialog_to_show_actually_opens_it() {
    let mut d = Dialog::new("Delete pane?")
        .body(Label::new("This action cannot be undone."))
        .action(Button::new("Cancel"));
    assert!(!d.is_open(), "built closed, as every surface is");

    Component::show(&mut d);
    assert!(
        d.is_open(),
        "the dialog forwards opening to the overlay it is built on, so a host that holds a \
         surface can raise it without knowing what kind it is",
    );

    Component::close(&mut d);
    assert!(!d.is_open(), "and the same for dismissing it");
}

/// **A dialog reports its own arrival and exit**, so a host can tell a surface that is going
/// away from one that has gone.
///
/// It answered `None` before, which reads as "not a surface at all": a dismissed dialog
/// counted as finished the instant it was asked to leave, rather than when its gesture had
/// played out.
#[test]
fn a_dialog_reports_whether_it_is_up() {
    use crate::animation::Presence;

    let mut d = open_dialog();
    assert!(
        d.presence().is_some_and(Presence::is_open),
        "an open dialog says it is up",
    );

    Component::close(&mut d);
    assert!(
        d.presence().is_some_and(|p| !p.is_open()),
        "and a dismissed one says it is not",
    );
}

/// **A dialog raised by its handle starts on the control it named.**
///
/// The same rule the overlay guards prove, asserted through the composed widget, because that
/// is the shape a caller actually holds — and because forwarding a subset of a surface's
/// behaviour is what broke the keyboard the last time it was done by halves.
#[test]
fn a_dialog_raised_by_its_handle_starts_where_it_said() {
    let mut d = Dialog::new("Delete pane?")
        .body(Label::new("This action cannot be undone."))
        .action(Button::new("Cancel"))
        .action(Button::new("Delete"))
        .default_action("Delete");

    let handle = d.handle();
    let opener = move || handle.show();
    opener();
    d.tick(0.016);

    fn focused(n: &dyn Component, out: &mut Vec<String>) {
        if n.base().is_focused()
            && let Some(name) = n.text_summary()
        {
            out.push(name);
        }
        for c in &n.base().children {
            focused(c.as_ref(), out);
        }
    }
    let mut names = Vec::new();
    focused(&d, &mut names);
    assert!(
        names.iter().any(|n| n == "Delete"),
        "opened from a closure holding nothing but the handle, and the keyboard landed on the \
         control the dialog named: {names:?}",
    );
}

#[test]
fn interleaved_body_and_actions_keep_field_focus_and_primary_activation() {
    use crate::widgets::Input;

    let primary = Rc::new(Cell::new(false));
    let canceled = Rc::new(Cell::new(false));
    let submit = primary.clone();
    let cancel = canceled.clone();
    let mut dialog = Dialog::new("Rename")
        .action(Button::new("Save").on_click(move || submit.set(true)))
        .child(Input::new().value("name"))
        .child(Label::new("A second body component"))
        .action(Button::new("Cancel").on_click(move || cancel.set(true)))
        .default_open(true);

    assert_eq!(
        crate::component::dispatch(&mut dialog, &Event::TextInput("x".into())),
        Handled::Yes,
        "the field receives focus even when an action was declared before the body",
    );
    crate::component::dispatch(&mut dialog, &Event::Widget(WidgetIntent::Activate));
    assert!(primary.get(), "the first declared action remains primary");
    assert!(!canceled.get(), "body content cannot become the action row");
}

#[test]
fn nested_and_multiple_children_stay_between_the_title_and_actions() {
    use crate::builders::{Parent, PlaceExt};
    use crate::widgets::{Flex, Grid};

    for (width, height) in [(240.0, 220.0), (700.0, 500.0)] {
        let mut dialog = Dialog::new("Question")
            .panel_size(crate::Length::Percent(0.8), crate::Length::Percent(0.8))
            .action(Button::new("OK"))
            .child(Grid::new().template_row("auto auto")
                .child(Label::new("First")).child(Label::new("Second")))
            .child(Flex::column().child([Label::new("Third"), Label::new("Fourth")]))
            .default_open(true);
        crate::LayoutEngine::new().compute(&mut dialog, Size::new(width, height));

        let panel = dialog.panel_bounds();
        let body = dialog.part_mut(BODY_AREA).unwrap().base().bounds;
        let actions = dialog.part_mut(ACTIONS_AREA).unwrap().base().bounds;
        assert!(body.loc.y + body.size.h <= actions.loc.y + 0.01);
        assert!(actions.loc.y + actions.size.h <= panel.loc.y + panel.size.h + 0.01);
        assert!(body.loc.x >= panel.loc.x && body.loc.x + body.size.w <= panel.loc.x + panel.size.w);
        let panel_base = dialog.base.children[0].base().children[0].base();
        let inner_width = panel.size.w
            - panel_base.style.layout.pad_left(panel_base.font) as f64
            - panel_base.style.layout.pad_right(panel_base.font) as f64;
        assert!((body.size.w - inner_width).abs() < 0.01,
            "the body fills the panel width rather than hugging its text");
        assert!((actions.size.w - inner_width).abs() < 0.01,
            "the action row can align its buttons against the panel's right edge");
        // Adding unrelated named areas inside a body cannot shadow the panel's own parts.
        let nested = Flex::column().area(ACTIONS_AREA).child(Label::new("Not an action"));
        let dialog = Parent::child(dialog, nested);
        assert_eq!(crate::component::area(&dialog, LAYOUT_AREA).unwrap().base().children.len(), 3);
    }
}

#[test]
fn missing_parts_leave_no_header_or_action_gap() {
    use crate::builders::Parent;
    use crate::widgets::Flex;

    let mut dialog = Parent::child(Dialog::new(""), [Flex::column().height(30.0)])
        .default_open(true);
    crate::LayoutEngine::new().compute(&mut dialog, Size::new(600.0, 400.0));
    let panel = dialog.panel_bounds();
    let body = dialog.part_mut(BODY_AREA).unwrap().base().bounds;
    let panel_base = dialog.base.children[0].base().children[0].base();
    let padding = panel_base.style.layout.pad_top(panel_base.font) as f64;
    assert!((panel.size.h - body.size.h - padding * 2.0).abs() < 0.01,
        "only body and padding remain, without empty header/action tracks or gaps");
}

#[test]
fn resizing_a_panel_gives_extra_height_to_its_body() {
    let mut previous: Option<(f32, f64, f64, f64)> = None;
    for height in [180.0, 360.0] {
        let mut dialog = Dialog::new("Question")
            .panel_size(crate::Length::Px(300.0), crate::Length::Px(height))
            .child(Label::new("Body"))
            .action(Button::new("OK"))
            .default_open(true);
        crate::LayoutEngine::new().compute(&mut dialog, Size::new(800.0, 600.0));
        let body = dialog.part_mut(BODY_AREA).unwrap().base().bounds.size.h;
        let title = dialog.part_mut(TITLE_AREA).unwrap().base().bounds.size.h;
        let actions = dialog.part_mut(ACTIONS_AREA).unwrap().base().bounds.size.h;
        if let Some((old_height, old_body, old_title, old_actions)) = previous {
            assert!((body - old_body - (height - old_height) as f64).abs() < 0.01,
                "the body receives the added space, rather than stretching title/actions");
            assert_eq!(title, old_title);
            assert_eq!(actions, old_actions);
        }
        previous = Some((height, body, title, actions));
    }
}
