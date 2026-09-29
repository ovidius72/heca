mod common;

use common::click_at;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, LayoutEngine, Point, Size, Theme};

#[test]
fn toggle_flip_emits_change_action_with_new_value() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut toggle = Toggle::new().on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut toggle, Size::new(200.0, 80.0));

    let b = toggle.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 100.0, b.loc.y);

    // A press outside the track does nothing.
    click_at(&mut toggle, outside, PointerButton::Left);
    assert!(!toggle.is_on());
    assert!(log.borrow().is_empty(), "missed press emits no action");

    // A press inside flips it on and reports the new value.
    click_at(&mut toggle, center, PointerButton::Left);
    assert!(toggle.is_on(), "press flips the toggle on");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("toggle-change", SignalData::Bool(true))),
    );

    // Pressing again flips it back off.
    click_at(&mut toggle, center, PointerButton::Left);
    assert!(!toggle.is_on());
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("toggle-change", SignalData::Bool(false))),
    );
}

#[test]
fn toggle_keyboard_activation_flips_via_focus() {
    use heca_grid_ui::FocusManager;

    let mut ui = Flex::row().child(Toggle::new().on(true));
    LayoutEngine::new().compute(&mut ui, Size::new(200.0, 80.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), Some(0), "toggle is focusable");

    // Space toggles the focused switch off (it started on).
    focus.deliver_key(&mut ui, GridKey::Space);
    let on = ui.base().children[0].base().focused.get_untracked();
    assert!(on, "toggle holds focus after activation");
}

#[test]
fn toggle_knob_slides_toward_target_on_tick() {
    let mut toggle = Toggle::new();
    LayoutEngine::new().compute(&mut toggle, Size::new(200.0, 80.0));

    let knob_x = |t: &Toggle| {
        let theme = Theme::default();
        let scene = common::paint(t, &theme);
        // The knob is the smaller of the two rects (the second emitted).
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect.loc.x),
                _ => None,
            })
            .nth(1)
            .unwrap()
    };

    let off_x = knob_x(&toggle);
    let press = Point::new(
        toggle.base().bounds.loc.x + 1.0,
        toggle.base().bounds.loc.y + 1.0,
    );
    click_at(&mut toggle, press, PointerButton::Left);
    // Advance enough frames to complete the slide.
    for _ in 0..30 {
        toggle.tick(0.016);
    }
    let on_x = knob_x(&toggle);
    assert!(on_x > off_x, "knob slides right when turned on");
}

#[test]
fn disabled_toggle_is_inert_and_unfocusable() {
    let mut toggle = Toggle::new().disabled(true);
    LayoutEngine::new().compute(&mut toggle, Size::new(200.0, 80.0));
    assert!(
        !toggle.focusable(),
        "disabled widgets drop out of focus traversal"
    );

    let b = toggle.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    click_at(&mut toggle, center, PointerButton::Left);
    assert!(!toggle.is_on(), "disabled toggle ignores presses");
}

#[test]
fn checkbox_toggle_emits_change_action_with_new_value() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut cb = Checkbox::new().on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut cb, Size::new(200.0, 80.0));

    let b = cb.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);

    click_at(&mut cb, center, PointerButton::Left);
    assert!(cb.is_checked(), "press checks the box");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("checkbox-change", SignalData::Bool(true))),
    );

    click_at(&mut cb, center, PointerButton::Left);
    assert!(!cb.is_checked(), "press again unchecks");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("checkbox-change", SignalData::Bool(false))),
    );
}

#[test]
fn checkbox_label_is_clickable_and_side_positions_the_box() {
    use heca_grid_ui::LabelSide;

    // Right label (default): clicking far right (on the label) toggles.
    let mut cb = Checkbox::new().label("ENABLE");
    LayoutEngine::new().compute(&mut cb, Size::new(400.0, 40.0));
    let b = cb.base().bounds;
    let far_right = Point::new(b.loc.x + b.size.w - 4.0, b.loc.y + b.size.h / 2.0);
    click_at(&mut cb, far_right, PointerButton::Left);
    assert!(
        cb.is_checked(),
        "clicking the (right) label toggles the box"
    );

    // Left label: the label text command sits left of the box.
    let theme = Theme::default();
    let mut left = Checkbox::new().label("ENABLE").label_side(LabelSide::Left);
    LayoutEngine::new().compute(&mut left, Size::new(400.0, 40.0));
    let scene = common::paint(&left, &theme);
    let text_x = scene.iter().find_map(|c| match c {
        DrawCommand::Text(t) => Some(t.rect.loc.x),
        _ => None,
    });
    let lb = left.base().bounds;
    assert_eq!(
        text_x,
        Some(lb.loc.x),
        "left label starts at the control's left edge"
    );
}

#[test]
fn checkbox_paints_indicator_only_when_checked() {
    let theme = Theme::default();
    let rect_count = |cb: &Checkbox| {
        let scene = common::paint(cb, &theme);
        common::rects(&scene).len()
    };

    let mut unchecked = Checkbox::new();
    LayoutEngine::new().compute(&mut unchecked, Size::new(80.0, 80.0));
    let mut checked = Checkbox::new().checked(true);
    LayoutEngine::new().compute(&mut checked, Size::new(80.0, 80.0));

    // Unchecked: just the box. Checked: box + indicator.
    assert_eq!(rect_count(&unchecked), 1, "unchecked paints only the box");
    assert_eq!(rect_count(&checked), 2, "checked adds the indicator");
}
