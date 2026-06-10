//! Phase A integration tests: the reactive + layout + component model, headless.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Point, Rectangle, Scene, Size, Theme};

/// A leaf box with a fixed size, for deterministic layout assertions.
fn fixed_box(w: f32, h: f32) -> Flex {
    Flex::column().width(Length::Px(w)).height(Length::Px(h))
}

#[test]
fn row_lays_children_left_to_right_with_gap() {
    let mut root = Flex::row()
        .gap(10.0)
        .width(Length::Px(300.0))
        .height(Length::Px(100.0))
        .child(fixed_box(50.0, 40.0))
        .child(fixed_box(50.0, 40.0));

    LayoutEngine::new().compute(&mut root, Size::new(300.0, 100.0));

    assert_eq!(root.base().bounds.size.w, 300.0);
    let first = &root.base().children[0];
    let second = &root.base().children[1];
    assert_eq!(first.base().bounds.loc.x, 0.0);
    // second sits after the first (50px) plus the 10px gap.
    assert_eq!(second.base().bounds.loc.x, 60.0);
}

#[test]
fn padding_offsets_child_origin() {
    let mut root = Flex::column()
        .padding(10.0)
        .width(Length::Px(200.0))
        .height(Length::Px(200.0))
        .child(fixed_box(50.0, 50.0));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 200.0));

    let child = &root.base().children[0];
    assert_eq!(child.base().bounds.loc.x, 10.0);
    assert_eq!(child.base().bounds.loc.y, 10.0);
}

#[test]
fn flex_grow_absorbs_remaining_space() {
    let mut root = Flex::row()
        .width(Length::Px(200.0))
        .height(Length::Px(50.0))
        .child(fixed_box(40.0, 50.0))
        .child(Flex::row().grow(1.0).height(Length::Px(50.0)));

    LayoutEngine::new().compute(&mut root, Size::new(200.0, 50.0));

    let grower = &root.base().children[1];
    // 200 total - 40 fixed = 160 absorbed by the growing child.
    assert_eq!(grower.base().bounds.size.w, 160.0);
    assert_eq!(grower.base().bounds.loc.x, 40.0);
}

#[test]
fn signal_set_updates_value() {
    let count = signal(0);
    assert_eq!(count.get_untracked(), 0);
    count.set(7);
    assert_eq!(count.get_untracked(), 7);
}

#[test]
fn label_signal_drives_text() {
    let label = Label::new("ONLINE");
    let sig = label.text_signal();
    assert_eq!(sig.get_untracked(), "ONLINE");
    sig.set("OFFLINE".to_string());
    assert_eq!(sig.get_untracked(), "OFFLINE");
}

#[test]
fn paint_emits_background_rect_and_label_text() {
    let root = Surface::new()
        .background(Color::rgb(10, 10, 10))
        .child(Label::new("HI"));

    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        root.paint(&mut cx);
    }

    let rects = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect(_)))
        .count();
    let texts = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text(_)))
        .count();
    assert_eq!(rects, 1, "container background should emit one rect");
    assert_eq!(texts, 1, "label should emit one text run");
}

// ── Phase C: surface components ──

#[test]
fn surface_paints_styled_rect_with_border() {
    let theme = Theme::grid_tron();
    let surface = Surface::new()
        .background(Color::rgb(12, 18, 24))
        .border(theme.accent, 1.5)
        .glow(theme.glow);

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        surface.paint(&mut cx);
    }
    let has_bordered = scene
        .iter()
        .any(|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some() && r.glow.is_some()));
    assert!(has_bordered, "surface should emit a bordered, glowing rect");
}

#[test]
fn card_carries_title_label() {
    let theme = Theme::grid_tron();
    let card = Card::new("UPLINK").child(Label::new("ONLINE"));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        card.paint(&mut cx);
    }
    let texts: Vec<&str> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"UPLINK"), "card title should render");
    assert!(texts.contains(&"ONLINE"), "card body should render");
}

#[test]
fn button_click_fires_within_bounds() {
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(false));
    let flag = clicked.clone();
    let mut button = Button::new("DEREZ").on_click(move || flag.set(true));
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 100.0, b.loc.y);

    button.event(&Event::PointerPressed { pos: outside });
    assert!(!clicked.get(), "click outside bounds must not fire");
    button.event(&Event::PointerPressed { pos: center });
    assert!(clicked.get(), "click inside bounds must fire");
}

#[test]
fn button_hover_tracks_pointer() {
    let mut button = Button::new("HOVER");
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let hovered = button.hovered();
    let b = button.base().bounds;

    button.event(&Event::PointerMoved {
        pos: Point::new(b.loc.x + 2.0, b.loc.y + 2.0),
    });
    assert!(hovered.get_untracked(), "entering bounds sets hover");
    button.event(&Event::PointerMoved {
        pos: Point::new(b.loc.x + b.size.w + 50.0, b.loc.y),
    });
    assert!(!hovered.get_untracked(), "leaving bounds clears hover");
}

#[test]
fn button_variants_paint_distinct_fills() {
    let theme = Theme::grid_tron();
    let fill_of = |button: Button| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            button.paint(&mut cx);
        }
        scene.iter().find_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.fill),
            _ => None,
        })
    };
    // At rest: primary paints an opaque dark surface (neon border, not a bright
    // fill); ghost is invisible (alpha 0).
    let primary = fill_of(Button::primary("X"));
    let ghost = fill_of(Button::ghost("X"));
    assert_eq!(
        primary,
        Some(theme.surface),
        "primary rests on a dark surface"
    );
    assert_eq!(ghost.map(|c| c.a), Some(0), "ghost is invisible at rest");
    assert_ne!(primary, ghost);
}

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
    assert_eq!(focus.focused(), None);

    // Tab → first focusable; Space activates it.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), Some(0));
    focus.deliver_key(&mut ui, GridKey::Space);
    assert_eq!(clicked.get(), 1);

    // Tab → second; Enter activates it.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), Some(1));
    focus.deliver_key(&mut ui, GridKey::Enter);
    assert_eq!(clicked.get(), 2);

    // Forward wraps to first; backward wraps to last.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), Some(0));
    focus.advance(&mut ui, false);
    assert_eq!(focus.focused(), Some(1));
}

#[test]
fn click_focuses_hit_widget_and_misses_clear() {
    use heca_grid_ui::FocusManager;

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    // Center of the second button (focus index 1).
    let b = ui.base().children[1].base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);

    let mut focus = FocusManager::new();
    focus.focus_at(&mut ui, center);
    assert_eq!(focus.focused(), Some(1), "click focuses the hit button");
    assert!(
        ui.base().children[1].base().focused.get_untracked(),
        "hit button shows focus"
    );

    // A click that misses every focusable clears focus.
    focus.focus_at(&mut ui, Point::new(9999.0, 9999.0));
    assert_eq!(focus.focused(), None, "missed click clears focus");
}

#[test]
fn glow_none_suppresses_glow() {
    use heca_grid_ui::GlowLevel;
    // Glow is owned solely by `glow_size` now (intensity controls only scanlines),
    // so `GlowLevel::None` — not `Intensity::Off` — is what suppresses the glow.
    let mut theme = Theme::grid_tron();
    theme.glow_size = GlowLevel::None;

    let root = Surface::new().glow(Color::rgb(64, 224, 255));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        root.paint(&mut cx);
    }

    let glow_present = scene.iter().any(|c| match c {
        DrawCommand::Rect(r) => r.glow.is_some(),
        _ => false,
    });
    assert!(!glow_present, "glow must be suppressed when glow_size is None");

    // And with a glow size set, the glow survives.
    theme.glow_size = GlowLevel::Medium;
    let mut scene2 = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene2, &theme);
        Surface::new().glow(Color::rgb(64, 224, 255)).paint(&mut cx);
    }
    let glow_present2 = scene2.iter().any(|c| matches!(c, DrawCommand::Rect(r) if r.glow.is_some()));
    assert!(glow_present2, "glow present when glow_size is Medium");
}

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
    toggle.event(&Event::PointerPressed { pos: outside });
    assert!(!toggle.is_on());
    assert!(log.borrow().is_empty(), "missed press emits no action");

    // A press inside flips it on and reports the new value.
    toggle.event(&Event::PointerPressed { pos: center });
    assert!(toggle.is_on(), "press flips the toggle on");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("toggle-change", SignalData::Bool(true))),
    );

    // Pressing again flips it back off.
    toggle.event(&Event::PointerPressed { pos: center });
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
        let theme = Theme::grid_tron();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            t.paint(&mut cx);
        }
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
    toggle.event(&Event::PointerPressed {
        pos: Point::new(
            toggle.base().bounds.loc.x + 1.0,
            toggle.base().bounds.loc.y + 1.0,
        ),
    });
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
    toggle.event(&Event::PointerPressed { pos: center });
    assert!(!toggle.is_on(), "disabled toggle ignores presses");
}

#[test]
fn disabled_button_ignores_clicks_and_focus() {
    use std::cell::Cell;
    use std::rc::Rc;

    let clicked = Rc::new(Cell::new(false));
    let flag = clicked.clone();
    let mut button = Button::new("X")
        .disabled(true)
        .on_click(move || flag.set(true));
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));

    let b = button.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    button.event(&Event::PointerPressed { pos: center });
    assert!(!clicked.get(), "disabled button ignores clicks");
    assert!(!button.focusable(), "disabled button is unfocusable");
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

    cb.event(&Event::PointerPressed { pos: center });
    assert!(cb.is_checked(), "press checks the box");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("checkbox-change", SignalData::Bool(true))),
    );

    cb.event(&Event::PointerPressed { pos: center });
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
    cb.event(&Event::PointerPressed { pos: far_right });
    assert!(
        cb.is_checked(),
        "clicking the (right) label toggles the box"
    );

    // Left label: the label text command sits left of the box.
    let theme = Theme::grid_tron();
    let mut left = Checkbox::new().label("ENABLE").label_side(LabelSide::Left);
    LayoutEngine::new().compute(&mut left, Size::new(400.0, 40.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        left.paint(&mut cx);
    }
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
    let theme = Theme::grid_tron();
    let rect_count = |cb: &Checkbox| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            cb.paint(&mut cx);
        }
        scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(_)))
            .count()
    };

    let mut unchecked = Checkbox::new();
    LayoutEngine::new().compute(&mut unchecked, Size::new(80.0, 80.0));
    let mut checked = Checkbox::new().checked(true);
    LayoutEngine::new().compute(&mut checked, Size::new(80.0, 80.0));

    // Unchecked: just the box. Checked: box + indicator.
    assert_eq!(rect_count(&unchecked), 1, "unchecked paints only the box");
    assert_eq!(rect_count(&checked), 2, "checked adds the indicator");
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
    assert_eq!(focus.focused(), Some(0), "first button focuses");
    // The disabled toggle is not focusable, so Tab lands on the second button.
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), Some(1), "disabled toggle is skipped");
}

#[test]
fn input_typing_emits_change_and_builds_text() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut input = Input::new().on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    for key in [GridKey::Char('H'), GridKey::Char('i')] {
        input.event(&Event::Key { key, pressed: true });
    }
    input.event(&Event::Key {
        key: GridKey::Space,
        pressed: true,
    });
    input.event(&Event::Key {
        key: GridKey::Char('5'),
        pressed: true,
    });

    assert_eq!(input.value_str(), "Hi 5");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value(
            "input-change",
            SignalData::String("Hi 5".into())
        )),
    );
}

#[test]
fn input_backspace_and_midword_insert_respect_cursor() {
    let mut input = Input::new().value("abc");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    // Caret starts at end (after 'c'). Move left → between 'b' and 'c'.
    input.event(&Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    input.event(&Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "ac",
        "backspace removes char before caret"
    );

    input.event(&Event::Key {
        key: GridKey::Char('X'),
        pressed: true,
    });
    assert_eq!(input.value_str(), "aXc", "insert lands at the caret");
}

#[test]
fn input_placeholder_shows_only_when_empty_and_unfocused() {
    let theme = Theme::grid_tron();
    let texts = |input: &Input| -> Vec<String> {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            input.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let mut empty = Input::new().placeholder("CALLSIGN");
    LayoutEngine::new().compute(&mut empty, Size::new(300.0, 60.0));
    assert_eq!(texts(&empty), vec!["CALLSIGN".to_string()]);

    let mut filled = Input::new().value("ZED");
    LayoutEngine::new().compute(&mut filled, Size::new(300.0, 60.0));
    assert_eq!(texts(&filled), vec!["ZED".to_string()]);
}

#[test]
fn input_click_cycle_selects_word_then_all_then_clears() {
    let mut input = Input::new().value("alpha beta gamma");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let y = input.base().bounds.loc.y + 5.0;
    // x inside the word "beta" (chars 6..10) — ~char 7 at advance 8.4, PAD 10.
    let x = input.base().bounds.loc.x + 10.0 + 60.0;
    // Consecutive presses with no tick share the clock → counted as multi-click.
    let press = |i: &mut Input| {
        i.event(&Event::PointerPressed {
            pos: Point::new(x, y),
        })
    };

    press(&mut input); // 1: caret
    assert_eq!(input.selection(), None, "single click places a caret");
    press(&mut input); // 2: word
    assert_eq!(
        input.selected_text().as_deref(),
        Some("beta"),
        "double-click selects the word"
    );
    press(&mut input); // 3: all
    assert_eq!(
        input.selected_text().as_deref(),
        Some("alpha beta gamma"),
        "triple-click selects all"
    );
    press(&mut input); // 4: clear
    assert_eq!(input.selection(), None, "fourth click clears the selection");
}

#[test]
fn input_typing_replaces_selection() {
    let mut input = Input::new().value("hello");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let pos = Point::new(
        input.base().bounds.loc.x + 14.0,
        input.base().bounds.loc.y + 5.0,
    );

    input.event(&Event::PointerPressed { pos }); // caret
    input.event(&Event::PointerPressed { pos }); // word = whole "hello"
    assert_eq!(input.selected_text().as_deref(), Some("hello"));

    input.event(&Event::Key {
        key: GridKey::Char('X'),
        pressed: true,
    });
    assert_eq!(input.value_str(), "X", "typing replaces the selection");
    assert_eq!(input.selection(), None, "selection cleared after replace");
}

#[test]
fn input_ctrl_backspace_deletes_previous_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    input.event(&Event::ModifiersChanged(Modifiers {
        ctrl: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(input.value_str(), "alpha ", "deletes the word at the caret");
    input.event(&Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "",
        "again removes the word + preceding space"
    );
}

#[test]
fn input_alt_delete_removes_next_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    for _ in 0..20 {
        input.event(&Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        });
    }
    // On macOS the word modifier is Alt/Option — accepted cross-platform.
    input.event(&Event::ModifiersChanged(Modifiers {
        alt: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::Delete,
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        " beta",
        "forward word delete from the start"
    );
}

#[test]
fn meta_backspace_and_delete_clear_to_boundary() {
    use heca_grid_ui::Modifiers;
    let arrow_left = |i: &mut Input| {
        i.event(&Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        });
    };

    // Meta+Backspace deletes from the caret to the start.
    let mut a = Input::new().value("alpha beta");
    LayoutEngine::new().compute(&mut a, Size::new(400.0, 60.0));
    for _ in 0..4 {
        arrow_left(&mut a); // caret 10 → 6 (start of "beta")
    }
    a.event(&Event::ModifiersChanged(Modifiers {
        meta: true,
        ..Default::default()
    }));
    a.event(&Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(a.value_str(), "beta", "meta+backspace deletes to start");

    // Meta+Delete deletes from the caret to the end.
    let mut b = Input::new().value("alpha beta");
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 60.0));
    for _ in 0..5 {
        arrow_left(&mut b); // caret 10 → 5 (after "alpha")
    }
    b.event(&Event::ModifiersChanged(Modifiers {
        meta: true,
        ..Default::default()
    }));
    b.event(&Event::Key {
        key: GridKey::Delete,
        pressed: true,
    });
    assert_eq!(b.value_str(), "alpha", "meta+delete deletes to end");
}

#[test]
fn shift_arrow_extends_and_shrinks_char_selection() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("hello");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    let left = |i: &mut Input| {
        i.event(&Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        })
    };
    let right = |i: &mut Input| {
        i.event(&Event::Key {
            key: GridKey::ArrowRight,
            pressed: true,
        })
    };

    input.event(&Event::ModifiersChanged(Modifiers {
        shift: true,
        ..Default::default()
    }));
    left(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("o"));
    left(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("lo"));
    right(&mut input);
    assert_eq!(input.selected_text().as_deref(), Some("o"));
    right(&mut input);
    assert_eq!(
        input.selection(),
        None,
        "shrinking onto the anchor deselects"
    );
}

#[test]
fn shift_ctrl_arrow_selects_to_boundary() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta");
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));

    input.event(&Event::ModifiersChanged(Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("alpha beta"),
        "shift+ctrl+left selects to the start"
    );
    input.event(&Event::Key {
        key: GridKey::ArrowRight,
        pressed: true,
    });
    assert_eq!(
        input.selection(),
        None,
        "extending back to the end deselects"
    );
}

#[test]
fn shift_alt_arrow_selects_by_word() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("alpha beta gamma");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    input.event(&Event::ModifiersChanged(Modifiers {
        alt: true,
        shift: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("gamma"),
        "first word back"
    );
    input.event(&Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("beta gamma"),
        "extends by another word"
    );
}

#[test]
fn cmd_a_selects_all_without_typing() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("hello world");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    input.event(&Event::ModifiersChanged(Modifiers {
        meta: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::Char('a'),
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello world"),
        "Cmd+A selects all"
    );
    assert_eq!(input.value_str(), "hello world", "the 'a' is not typed");
}

#[test]
fn home_end_move_caret_to_bounds() {
    let mut input = Input::new().value("hello");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    input.event(&Event::Key {
        key: GridKey::Home,
        pressed: true,
    });
    input.event(&Event::Key {
        key: GridKey::Char('X'),
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "Xhello",
        "Home moves the caret to the start"
    );

    input.event(&Event::Key {
        key: GridKey::End,
        pressed: true,
    });
    input.event(&Event::Key {
        key: GridKey::Char('Y'),
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "XhelloY",
        "End moves the caret to the end"
    );
}

#[test]
fn shift_home_end_select_to_bounds() {
    use heca_grid_ui::Modifiers;
    let mut input = Input::new().value("hello");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    input.event(&Event::ModifiersChanged(Modifiers {
        shift: true,
        ..Default::default()
    }));
    input.event(&Event::Key {
        key: GridKey::Home,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello"),
        "Shift+Home selects to start"
    );
    input.event(&Event::Key {
        key: GridKey::End,
        pressed: true,
    });
    assert_eq!(
        input.selection(),
        None,
        "Shift+End back to the end deselects"
    );
}

#[test]
fn disabled_input_ignores_typing() {
    let mut input = Input::new().disabled(true);
    LayoutEngine::new().compute(&mut input, Size::new(300.0, 60.0));
    input.event(&Event::Key {
        key: GridKey::Char('x'),
        pressed: true,
    });
    assert!(input.value_str().is_empty(), "disabled input ignores keys");
    assert!(!input.focusable(), "disabled input is unfocusable");
}

// ── Phase C: display widgets ──

#[test]
fn badge_colored_has_fill_outline_has_none() {
    let theme = Theme::grid_tron();
    let fill_of = |badge: Badge| {
        let mut badge = badge;
        LayoutEngine::new().compute(&mut badge, Size::new(200.0, 80.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            badge.paint(&mut cx);
        }
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.fill),
                _ => None,
            })
            .unwrap()
    };
    assert!(
        fill_of(Badge::success("OK")).a > 0,
        "colored badge has a translucent fill"
    );
    assert_eq!(
        fill_of(Badge::outline("OK")).a,
        0,
        "outline badge has no fill"
    );
}

#[test]
fn badge_renders_its_label() {
    let theme = Theme::grid_tron();
    let mut badge = Badge::new("LIVE");
    LayoutEngine::new().compute(&mut badge, Size::new(200.0, 80.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        badge.paint(&mut cx);
    }
    assert!(
        scene
            .iter()
            .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "LIVE")),
        "badge renders its label"
    );
}

#[test]
fn status_dot_color_and_glow_track_status() {
    let theme = Theme::grid_tron();
    let probe = |dot: StatusDot| {
        let mut dot = dot;
        LayoutEngine::new().compute(&mut dot, Size::new(50.0, 50.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            dot.paint(&mut cx);
        }
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some((r.fill, r.glow.is_some())),
                _ => None,
            })
            .unwrap()
    };
    let (online, online_glow) = probe(StatusDot::online());
    let (offline, offline_glow) = probe(StatusDot::offline());
    assert_eq!(online, theme.success);
    assert!(online_glow, "active dot glows");
    assert_eq!(offline, theme.muted);
    assert!(!offline_glow, "offline dot does not glow");
}

#[test]
fn select_opens_and_paints_options_in_overlay_layer() {
    let theme = Theme::grid_tron();
    let mut sel = Select::new(["LOW", "MEDIUM", "HIGH"]);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    let texts = |s: &Select| -> Vec<String> {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            s.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    // Closed: only the selected label shows; not overlay-active.
    assert!(!sel.overlay_active(), "closed select is not overlay-active");
    assert_eq!(
        texts(&sel),
        vec!["LOW".to_string()],
        "closed shows only the trigger label"
    );

    // Open via click on the trigger.
    let b = sel.base().bounds;
    sel.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    });
    assert!(
        sel.overlay_active(),
        "clicking the trigger opens + grabs input"
    );
    let open_texts = texts(&sel);
    assert!(open_texts.contains(&"MEDIUM".to_string()) && open_texts.contains(&"HIGH".to_string()));
}

#[test]
fn select_click_row_commits_and_closes() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut sel =
        Select::new(["LOW", "MEDIUM", "HIGH"]).on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    let b = sel.base().bounds;
    sel.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    }); // open

    // Click the third row (HIGH). Rows start below the trigger + gap + panel pad.
    // panel_gap(4) + panel_pad(4) + 2*ROW_H(30) + mid-row(15).
    let row2_y = b.loc.y + b.size.h + 4.0 + 4.0 + 2.0 * 30.0 + 15.0;
    sel.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + 10.0, row2_y),
    });
    assert_eq!(sel.index(), 2, "clicking a row selects it");
    assert!(!sel.overlay_active(), "selection closes the dropdown");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("select-change", SignalData::Usize(2))),
    );
}

#[test]
fn select_long_list_caps_visible_rows_and_scrolls() {
    let theme = Theme::grid_tron();
    let opts: Vec<String> = (0..20).map(|n| format!("OPT{n}")).collect();
    let mut sel = Select::new(opts);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    let row_texts = |s: &Select| -> Vec<String> {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            s.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let b = sel.base().bounds;
    sel.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    }); // open

    // Trigger label (1) + at most MAX_VISIBLE (6) rows are painted.
    let texts = row_texts(&sel);
    assert_eq!(texts.len(), 1 + 6, "long list caps the visible rows");
    assert_eq!(texts[1], "OPT0", "starts at the top");

    // Wheel-scroll moves the visible window down.
    sel.event(&Event::Scroll { delta: 5.0 });
    assert_eq!(row_texts(&sel)[1], "OPT5", "scroll reveals later options");

    // Scrolling past the end clamps to the last full window.
    sel.event(&Event::Scroll { delta: 999.0 });
    assert_eq!(row_texts(&sel)[1], "OPT14", "scroll clamps at max (20 - 6)");
}

#[test]
fn select_keyboard_navigates_and_escape_closes() {
    let mut sel = Select::new(["A", "B", "C"]);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));
    let key = |s: &mut Select, k: GridKey| {
        s.event(&Event::Key {
            key: k,
            pressed: true,
        })
    };

    key(&mut sel, GridKey::Enter); // open
    assert!(sel.overlay_active());
    key(&mut sel, GridKey::ArrowDown);
    key(&mut sel, GridKey::ArrowDown);
    key(&mut sel, GridKey::Enter); // commit highlight (index 2)
    assert_eq!(sel.index(), 2);
    assert!(!sel.overlay_active(), "Enter commits and closes");

    key(&mut sel, GridKey::Enter); // reopen
    assert!(sel.overlay_active());
    key(&mut sel, GridKey::Escape);
    assert!(
        !sel.overlay_active(),
        "Escape closes without changing selection"
    );
    assert_eq!(sel.index(), 2);
}

#[test]
fn tabs_arrow_keys_and_click_change_selection() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut tabs =
        Tabs::new(["ALPHA", "BETA", "GAMMA"]).on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    assert_eq!(tabs.index(), 0);
    tabs.event(&Event::Key {
        key: GridKey::ArrowRight,
        pressed: true,
    });
    assert_eq!(tabs.index(), 1);
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("tab-change", SignalData::Usize(1))),
    );

    tabs.event(&Event::Key {
        key: GridKey::ArrowRight,
        pressed: true,
    });
    assert_eq!(tabs.index(), 2);
    let before = log.borrow().len();
    tabs.event(&Event::Key {
        key: GridKey::ArrowRight,
        pressed: true,
    });
    assert_eq!(tabs.index(), 2, "ArrowRight clamps at the last tab");
    assert_eq!(
        log.borrow().len(),
        before,
        "no event emitted when selection is unchanged"
    );

    // A click near the left edge selects the first tab again.
    let b = tabs.base().bounds;
    tabs.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + 2.0, b.loc.y + b.size.h / 2.0),
    });
    assert_eq!(tabs.index(), 0, "click selects the hit tab");
}

#[test]
fn tabs_underline_slides_toward_selection() {
    let theme = Theme::grid_tron();
    let underline_x = |t: &Tabs| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            t.paint(&mut cx);
        }
        // Labels are Text; the underline is the only Rect.
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect.loc.x),
                _ => None,
            })
            .unwrap()
    };
    let mut tabs = Tabs::new(["ALPHA", "BETA", "GAMMA"]);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));
    let x0 = underline_x(&tabs);
    tabs.event(&Event::Key {
        key: GridKey::ArrowRight,
        pressed: true,
    });
    for _ in 0..40 {
        tabs.tick(0.016);
    }
    let x1 = underline_x(&tabs);
    assert!(x1 > x0, "underline slides right toward the next tab");
}

#[test]
fn horizontal_separator_spans_container_width() {
    let mut col = Flex::column()
        .width(Length::Px(120.0))
        .height(Length::Px(40.0))
        .child(Separator::horizontal());
    LayoutEngine::new().compute(&mut col, Size::new(120.0, 40.0));
    let sep = &col.base().children[0];
    assert_eq!(
        sep.base().bounds.size.w,
        120.0,
        "horizontal separator stretches to the container width"
    );
    assert!(sep.base().bounds.size.h <= 1.0, "separator is thin");
}

#[test]
fn spinner_animates_and_paints_its_ring() {
    let theme = Theme::grid_tron();
    let mut spinner = Spinner::new();
    LayoutEngine::new().compute(&mut spinner, Size::new(40.0, 40.0));
    assert!(spinner.tick(0.016), "spinner keeps requesting frames");

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        spinner.paint(&mut cx);
    }
    let dots = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect(_)))
        .count();
    assert_eq!(dots, 8, "the ring paints 8 dots");
}

#[test]
fn alert_renders_title_body_and_accent_bar() {
    let theme = Theme::grid_tron();
    let mut alert = Alert::success("DEPLOYED").body("grid online");
    LayoutEngine::new().compute(&mut alert, Size::new(400.0, 100.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        alert.paint(&mut cx);
    }
    let texts: Vec<&str> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"DEPLOYED"), "alert renders its title");
    assert!(texts.contains(&"grid online"), "alert renders its body");

    let rects = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Rect(_)))
        .count();
    assert_eq!(rects, 2, "alert paints a surface + accent bar");
}

#[test]
fn progress_bar_fill_eases_toward_value() {
    let theme = Theme::grid_tron();
    let fill_w = |bar: &ProgressBar| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            bar.paint(&mut cx);
        }
        // Track is the first Rect; the fill (if any) is the second.
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect.size.w),
                _ => None,
            })
            .nth(1)
    };

    let mut bar = ProgressBar::new().width(Length::Px(200.0));
    LayoutEngine::new().compute(&mut bar, Size::new(200.0, 20.0));
    assert_eq!(fill_w(&bar), None, "no fill at zero");

    bar.set(0.5);
    for _ in 0..40 {
        bar.tick(0.016);
    }
    let w = fill_w(&bar).expect("fill present after raising value");
    assert!(
        w > 90.0 && w < 110.0,
        "fill eases to ~half the 200px track, got {w}"
    );
}

#[test]
fn gauge_lights_segments_by_value() {
    let theme = Theme::grid_tron();
    let lit = |g: &Gauge| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            g.paint(&mut cx);
        }
        // Lit segments carry a glow; unlit do not.
        scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(r) if r.glow.is_some()))
            .count()
    };

    let mut empty = Gauge::new();
    LayoutEngine::new().compute(&mut empty, Size::new(168.0, 18.0));
    let mut full = Gauge::new().value(1.0);
    LayoutEngine::new().compute(&mut full, Size::new(168.0, 18.0));

    assert_eq!(lit(&empty), 0, "empty gauge lights nothing");
    assert_eq!(lit(&full), 12, "full gauge lights all 12 segments");
}

// ── Item (generic list row) ──

#[test]
fn item_activates_on_click_when_interactive() {
    use std::cell::Cell;
    use std::rc::Rc;

    let hits = Rc::new(Cell::new(0u32));
    let h = hits.clone();
    let mut item = Item::new("VIEW PROFILE").on_activate(move || h.set(h.get() + 1));
    LayoutEngine::new().compute(&mut item, Size::new(260.0, 40.0));

    assert!(item.focusable(), "interactive item is focusable");
    let b = item.base().bounds;
    item.event(&Event::PointerPressed {
        pos: Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
    });
    assert_eq!(hits.get(), 1, "click activates the row");

    // Space activates too (keyboard).
    item.event(&Event::Key {
        key: GridKey::Space,
        pressed: true,
    });
    assert_eq!(hits.get(), 2);
}

#[test]
fn display_only_item_is_inert_and_unfocusable() {
    let mut item = Item::new("STATIC");
    LayoutEngine::new().compute(&mut item, Size::new(260.0, 40.0));
    assert!(
        !item.focusable(),
        "an item without on_activate is not focusable"
    );
    let b = item.base().bounds;
    // No panic / no effect; just confirms it ignores the press.
    assert_eq!(
        item.event(&Event::PointerPressed {
            pos: Point::new(b.loc.x + 1.0, b.loc.y + 1.0),
        }),
        Handled::No,
    );
}

#[test]
fn item_label_color_tracks_selected_state() {
    let theme = Theme::grid_tron();
    let label_color = |item: &Item| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            item.paint(&mut cx);
        }
        scene.iter().find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.color),
            _ => None,
        })
    };

    let mut plain = Item::new("PROGRAMS");
    LayoutEngine::new().compute(&mut plain, Size::new(260.0, 40.0));
    let mut sel = Item::new("DASHBOARD").active(true);
    LayoutEngine::new().compute(&mut sel, Size::new(260.0, 40.0));

    assert_eq!(
        label_color(&plain),
        Some(theme.foreground),
        "plain label uses foreground"
    );
    assert_eq!(
        label_color(&sel),
        Some(theme.accent),
        "active label uses accent"
    );
}

#[test]
fn item_slots_lay_out_left_and_right() {
    // Leading badge on the left, trailing hint on the right; label sits between.
    let mut item = Item::new("SETTINGS")
        .leading(StatusDot::online())
        .trailing(Badge::neutral("CMD ,"));
    LayoutEngine::new().compute(&mut item, Size::new(300.0, 40.0));

    let b = item.base().bounds;
    let lead = item.base().children[0].base().bounds; // leading
    let trail = item.base().children[1].base().bounds; // trailing
    assert!(lead.loc.x < trail.loc.x, "leading sits left of trailing");
    assert!(
        trail.loc.x + trail.size.w <= b.loc.x + b.size.w + 0.5,
        "trailing stays within the row's right edge"
    );
    assert!(
        lead.size.w > 0.0 && trail.size.w > 0.0,
        "both slots are laid out"
    );
}

#[test]
fn item_trailing_border_draws_a_flat_frame_no_glow() {
    let theme = Theme::grid_tron();
    let frames = |item: &Item| -> usize {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            item.paint(&mut cx);
        }
        // A bordered, glow-free rect = the chip frame.
        scene
            .iter()
            .filter(|c| matches!(c, DrawCommand::Rect(r) if r.border.is_some() && r.glow.is_none()))
            .count()
    };

    let mut plain = Item::new("SETTINGS").trailing(Label::new("CMD ,"));
    LayoutEngine::new().compute(&mut plain, Size::new(300.0, 40.0));
    let mut bordered = Item::new("SETTINGS")
        .trailing(Label::new("CMD ,"))
        .trailing_bordered(true);
    LayoutEngine::new().compute(&mut bordered, Size::new(300.0, 40.0));

    assert_eq!(frames(&plain), 0, "no frame without trailing_bordered");
    assert_eq!(
        frames(&bordered),
        1,
        "trailing_bordered draws one flat frame"
    );
}

#[test]
fn pane_draws_rounded_accent_border_no_brackets() {
    let theme = Theme::grid_tron();
    let mut pane = Pane::new().background(theme.surface).child(Label::new("X"));
    LayoutEngine::new().compute(&mut pane, Size::new(200.0, 300.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        pane.paint(&mut cx);
    }
    // New design: the corner brackets are segments of a *rounded border* (so they
    // share the theme radius), drawn with rects + dimmed straights — not the flat,
    // always-square `BracketCmd` primitive, and never glowing.
    let brackets = scene
        .iter()
        .filter(|c| matches!(c, DrawCommand::Brackets(_)))
        .count();
    assert_eq!(brackets, 0, "pane no longer uses the square bracket primitive");

    let rounded_border = scene.iter().any(|c| {
        matches!(
            c,
            DrawCommand::Rect(r)
                if r.border.is_some() && r.radius == theme.radius && r.glow.is_none()
        )
    });
    assert!(rounded_border, "pane draws a rounded accent border at the theme radius");
}

#[test]
fn item_group_collapses_rows_out_of_layout() {
    use heca_grid_ui::ItemGroup;
    let row = || Item::new("row").on_activate(|| {});
    let mut group = ItemGroup::new("GROUP")
        .child(row())
        .child(row());

    // Expanded: header + 2 rows all take height.
    LayoutEngine::new().compute(&mut group, Size::new(200.0, 400.0));
    let expanded_h = group.base().bounds.size.h;
    let r1 = group.base().children[1].base().bounds.size.h;
    assert!(r1 > 0.0, "expanded rows have height");

    // Collapse via the expanded signal, relayout: rows fold away (display:none).
    group.state().set(false);
    LayoutEngine::new().compute(&mut group, Size::new(200.0, 400.0));
    let collapsed_h = group.base().bounds.size.h;
    let r1c = group.base().children[1].base().bounds.size.h;
    assert!(collapsed_h < expanded_h, "collapsed group is shorter ({collapsed_h} < {expanded_h})");
    assert_eq!(r1c, 0.0, "collapsed rows take no layout space");
}

#[test]
fn grid_places_children_in_named_areas_and_cells() {
    use heca_grid_ui::{Grid, Track};
    // 2 cols × 2 rows; areas: icon spans both rows in col 1, title top-right,
    // sub bottom-right. Fixed sizes so we can assert exact bounds.
    let mut grid = Grid::new()
        .columns([Track::Px(40.0), Track::Px(100.0)])
        .rows([Track::Px(20.0), Track::Px(20.0)])
        .areas(["icon title", "icon sub"])
        .area(Flex::column(), "icon")
        .area(Flex::column(), "title")
        .area(Flex::column(), "sub")
        // explicit cell: a 4th child pinned to col2,row2.
        .cell(Flex::column(), 2, 2, 1, 1);

    LayoutEngine::new().compute(&mut grid, Size::new(140.0, 40.0));

    let icon = grid.base().children[0].base().bounds;
    let title = grid.base().children[1].base().bounds;
    let sub = grid.base().children[2].base().bounds;

    // icon: col 1 (x≈0), spans both rows (height≈40).
    assert!(icon.loc.x < 1.0, "icon in column 1");
    assert!((icon.size.h - 40.0).abs() < 1.0, "icon spans both rows");
    // title: col 2 (x≈40), top row (y≈0).
    assert!((title.loc.x - 40.0).abs() < 1.0, "title in column 2");
    assert!(title.loc.y < 1.0, "title in top row");
    // sub: col 2, bottom row (y≈20).
    assert!((sub.loc.x - 40.0).abs() < 1.0, "sub in column 2");
    assert!((sub.loc.y - 20.0).abs() < 1.0, "sub in bottom row");
}

#[test]
fn dock_frame_body_has_height_when_expanded() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("FILES").child(fixed_box(120.0, 80.0));

    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));

    let body_h = dock.base().children[1].base().bounds.size.h;
    assert!(body_h > 0.0, "expanded body has height");
}

#[test]
fn dock_frame_collapse_folds_body_out_of_layout() {
    use heca_grid_ui::DockFrame;
    let mut dock = DockFrame::new("FILES").child(fixed_box(120.0, 80.0));
    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));
    let expanded_h = dock.base().bounds.size.h;

    // Collapse via the expanded signal, relayout: body folds away (display:none).
    dock.state().set(false);
    LayoutEngine::new().compute(&mut dock, Size::new(200.0, 400.0));

    let body_h = dock.base().children[1].base().bounds.size.h;
    assert_eq!(body_h, 0.0, "collapsed body takes no layout space");
    let collapsed_h = dock.base().bounds.size.h;
    assert!(collapsed_h < expanded_h, "collapsed dock is shorter ({collapsed_h} < {expanded_h})");
}

#[test]
fn dock_frame_header_click_toggles_and_emits_dock_toggle() {
    use heca_grid_ui::{Action, DockFrame, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut dock = DockFrame::new("FILES").on_toggle(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));
    assert!(dock.state().get_untracked(), "starts expanded");

    // Click the toggle area of the title bar (header child 0): collapses + reports.
    let toggle = dock.base().children[0].base().children[0].base().bounds;
    let center = Point::new(toggle.loc.x + toggle.size.w / 2.0, toggle.loc.y + toggle.size.h / 2.0);
    dock.event(&Event::PointerPressed { pos: center });

    assert!(!dock.state().get_untracked(), "header click collapses the frame");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("dock-toggle", SignalData::Bool(false))),
    );
}

#[test]
fn dock_frame_header_control_receives_events_before_toggle() {
    use heca_grid_ui::DockFrame;
    use std::cell::Cell;
    use std::rc::Rc;

    // A search-like interactive control living in the header-controls slot.
    let control_clicks = Rc::new(Cell::new(0u32));
    let sink = control_clicks.clone();
    let control = Item::new("search").on_activate(move || sink.set(sink.get() + 1));
    let mut dock = DockFrame::new("FILES").header(control);
    LayoutEngine::new().compute(&mut dock, Size::new(260.0, 400.0));

    // Click the control (header child 1): it consumes the event; frame must NOT toggle.
    let ctrl = dock.base().children[0].base().children[1].base().bounds;
    let center = Point::new(ctrl.loc.x + ctrl.size.w / 2.0, ctrl.loc.y + ctrl.size.h / 2.0);
    dock.event(&Event::PointerPressed { pos: center });

    assert_eq!(control_clicks.get(), 1, "header control received the click");
    assert!(dock.state().get_untracked(), "clicking the control did not toggle the frame");
}

#[test]
fn dock_frame_collapsed_body_is_skipped_by_focus_traversal() {
    use heca_grid_ui::{DockFrame, FocusManager};

    // Header toggle + one interactive body row are both focusable when expanded.
    let mut dock = DockFrame::new("FILES").child(Item::new("file.rs").on_activate(|| {}));
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut dock, true);
    assert_eq!(focus.focused(), Some(0), "header toggle is first in tab order");
    focus.advance(&mut dock, true);
    assert_eq!(focus.focused(), Some(1), "body row is tabbable while expanded");

    // Collapse + relayout: the body subtree becomes display:none and drops out of
    // the tab order, so only the header toggle remains (forward Tab wraps to it).
    dock.state().set(false);
    LayoutEngine::new().compute(&mut dock, Size::new(220.0, 400.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut dock, true);
    assert_eq!(focus.focused(), Some(0), "only the header toggle is focusable when collapsed");
    focus.advance(&mut dock, true);
    assert_eq!(focus.focused(), Some(0), "collapsed body row is not reachable by Tab");
}

#[test]
fn chrome_region_expanded_uses_full_width() {
    use heca_grid_ui::ChromeRegion;
    let mut region = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0)
        .dock(fixed_box(100.0, 60.0));

    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(region.base().bounds.size.w, 240.0, "expanded sidebar uses its full width");
}

#[test]
fn chrome_region_collapses_to_rail_width() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut region = ChromeRegion::vertical()
        .expanded_size(240.0)
        .rail_size(48.0)
        .dock(fixed_box(100.0, 60.0));

    region.mode_signal().set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(region.base().bounds.size.w, 48.0, "collapsed sidebar shrinks to the rail width");
}

#[test]
fn chrome_region_hidden_folds_out_of_layout() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut region = ChromeRegion::vertical().dock(fixed_box(100.0, 60.0));

    region.mode_signal().set(RegionMode::Hidden);
    LayoutEngine::new().compute(&mut region, Size::new(400.0, 600.0));

    assert_eq!(region.base().bounds.size.w, 0.0, "hidden region takes no layout space");
}

#[test]
fn chrome_region_horizontal_bar_collapses_height() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let mut bar = ChromeRegion::horizontal()
        .expanded_size(200.0)
        .rail_size(40.0)
        .dock(fixed_box(60.0, 100.0));

    bar.mode_signal().set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut bar, Size::new(800.0, 300.0));

    assert_eq!(bar.base().bounds.size.h, 40.0, "collapsed top/bottom bar shrinks to the rail height");
}

#[test]
fn chrome_region_toggle_flips_expanded_and_rail() {
    use heca_grid_ui::{ChromeRegion, RegionMode};
    let region = ChromeRegion::vertical();
    assert_eq!(region.mode_signal().get_untracked(), RegionMode::Expanded);

    region.toggle();
    assert_eq!(region.mode_signal().get_untracked(), RegionMode::CollapsedRail, "toggle collapses to rail");
    region.toggle();
    assert_eq!(region.mode_signal().get_untracked(), RegionMode::Expanded, "toggle expands again");
}

#[test]
fn collapsed_dock_body_is_not_painted() {
    use heca_grid_ui::{DockFrame, DrawCommand};

    // A row whose label must NOT be painted while the dock is collapsed — a
    // display:none subtree is collapsed to the top-left by layout, so painting it
    // would stamp overlapping text there (the showcase artifact this guards).
    let collect_labels = |dock: &mut DockFrame| -> Vec<String> {
        LayoutEngine::new().compute(dock, Size::new(220.0, 400.0));
        let theme = Theme::grid_tron();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            dock.paint(&mut cx);
        }
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.text.clone()),
                _ => None,
            })
            .collect()
    };

    let mut open = DockFrame::new("FILES").child(Item::new("SECRET.rs"));
    assert!(
        collect_labels(&mut open).iter().any(|t| t == "SECRET.rs"),
        "expanded dock paints its body row"
    );

    let mut collapsed = DockFrame::new("FILES").expanded(false).child(Item::new("SECRET.rs"));
    assert!(
        collect_labels(&mut collapsed).iter().all(|t| t != "SECRET.rs"),
        "collapsed dock must not paint its hidden body row"
    );
}

#[test]
fn icon_lays_out_as_a_square() {
    use heca_grid_ui::{Glyph, Icon};
    let mut icon = Icon::new(Glyph::GitBranch).size(24.0);
    LayoutEngine::new().compute(&mut icon, Size::new(200.0, 200.0));
    let b = icon.base().bounds;
    assert_eq!(b.size.w, 24.0, "icon width = glyph size");
    assert_eq!(b.size.h, 24.0, "icon is square");
}

#[test]
fn icon_paints_duotone_layers_in_the_icon_font() {
    use heca_grid_ui::{DrawCommand, FontRole, Glyph, Icon};
    let mut icon = Icon::new(Glyph::Folder).size(24.0);
    LayoutEngine::new().compute(&mut icon, Size::new(100.0, 100.0));

    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        icon.paint(&mut cx);
    }
    let glyphs: Vec<(String, FontRole)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.font)),
            _ => None,
        })
        .collect();

    // Two stacked layers: secondary (:before) then primary (secondary+1), both
    // shaped with the icon font.
    assert_eq!(glyphs.len(), 2, "duotone icon paints two layers");
    assert!(glyphs.iter().all(|(_, f)| *f == FontRole::Icon), "both shaped with the icon font");
    assert_eq!(glyphs[0].0, char::from_u32(0xe24a).unwrap().to_string(), "secondary layer first");
    assert_eq!(glyphs[1].0, char::from_u32(0xe24b).unwrap().to_string(), "primary layer on top");
}

#[test]
fn row_activates_on_click_and_key_when_interactive() {
    use heca_grid_ui::Row;
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut row = Row::new()
        .child(Label::new("PANE 1"))
        .on_activate(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    assert!(row.focusable(), "an interactive row is focusable");

    let b = row.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    let outside = Point::new(b.loc.x + b.size.w + 50.0, b.loc.y);

    row.event(&Event::PointerPressed { pos: outside });
    assert_eq!(clicks.get(), 0, "a click outside the row does nothing");
    row.event(&Event::PointerPressed { pos: center });
    assert_eq!(clicks.get(), 1, "a click inside the row activates it");
    row.event(&Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(clicks.get(), 2, "Enter activates the focused row");
}

#[test]
fn row_without_on_activate_is_not_focusable() {
    use heca_grid_ui::Row;
    let row = Row::new().child(Label::new("static"));
    assert!(!row.focusable(), "a display-only row is not focusable");
}

#[test]
fn tag_lays_out_leading_and_label_and_hugs_content() {
    use heca_grid_ui::{Glyph, Icon, Tag};
    let mut tag = Tag::new("main 1+").leading(Icon::new(Glyph::GitBranch).size(13.0));
    LayoutEngine::new().compute(&mut tag, Size::new(300.0, 40.0));

    // The first segment holds [leading, label].
    let seg0 = tag.base().children[0].base();
    assert_eq!(seg0.children.len(), 2, "first segment holds [leading, label]");
    assert!(seg0.children[0].base().bounds.size.w > 0.0, "leading icon is laid out");
    assert!(seg0.children[1].base().bounds.size.w > 0.0, "label is laid out");
    assert!(tag.base().bounds.size.w < 300.0, "chip hugs its content, not the full width");
}

#[test]
fn tag_with_multiple_segments_lays_them_in_a_row() {
    use heca_grid_ui::{Component, Glyph, Icon, Tag};
    let leading: Option<Box<dyn Component>> = Some(Box::new(Icon::new(Glyph::File).size(13.0)));
    let mut tag = Tag::new("main")
        .leading(Icon::new(Glyph::GitBranch).size(13.0))
        .segment_text("5 +152 -12", leading);
    LayoutEngine::new().compute(&mut tag, Size::new(400.0, 40.0));

    assert_eq!(tag.base().children.len(), 2, "two segments");
    let s0 = tag.base().children[0].base().bounds;
    let s1 = tag.base().children[1].base().bounds;
    assert!(s1.loc.x > s0.loc.x + s0.size.w - 1.0, "the second segment sits right of the first");
}

#[test]
fn dock_frame_rail_mode_folds_header_and_body_to_icon() {
    use heca_grid_ui::{ChromeRegion, DockFrame, Glyph, Item, RegionMode};

    // A rail-aware dock bound to its region's mode signal (obtained before the
    // region is moved into `.dock(...)`).
    let sidebar = ChromeRegion::vertical().expanded_size(240.0).rail_size(48.0);
    let mode = sidebar.mode_signal();
    let dock = DockFrame::new("FILES").rail(mode, Glyph::FolderOpen).child(Item::new("main.rs"));
    let mut sidebar = sidebar.dock(dock);

    // Expanded: header + body are shown; the rail icon is hidden.
    LayoutEngine::new().compute(&mut sidebar, Size::new(400.0, 600.0));
    {
        let dock = sidebar.base().children[0].base();
        assert!(!dock.children[0].base().style.hidden, "header shown while expanded");
        assert!(!dock.children[1].base().style.hidden, "body shown while expanded");
        assert!(dock.children[2].base().style.hidden, "rail icon hidden while expanded");
        assert!(dock.children[1].base().bounds.size.h > 0.0, "expanded body has height");
    }

    // Collapse the region to its rail: header + body fold away; the rail icon shows.
    mode.set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut sidebar, Size::new(400.0, 600.0));
    {
        let dock = sidebar.base().children[0].base();
        assert!(dock.children[0].base().style.hidden, "header folds away in rail mode");
        assert!(dock.children[1].base().style.hidden, "body folds away in rail mode");
        assert!(!dock.children[2].base().style.hidden, "rail icon shows in rail mode");
        assert_eq!(dock.children[1].base().bounds.size.h, 0.0, "folded body takes no layout space");
        assert!(dock.children[2].base().bounds.size.h > 0.0, "rail icon is laid out");
    }
}

#[test]
fn dock_frame_rail_paints_icon_not_title() {
    use heca_grid_ui::{ChromeRegion, DockFrame, DrawCommand, FontRole, Glyph, Item, RegionMode};

    let sidebar = ChromeRegion::vertical().expanded_size(240.0).rail_size(48.0);
    let mode = sidebar.mode_signal();
    let dock = DockFrame::new("FILES").rail(mode, Glyph::FolderOpen).child(Item::new("main.rs"));
    let mut sidebar = sidebar.dock(dock);

    let paint = |sidebar: &mut ChromeRegion| -> (Vec<String>, usize) {
        LayoutEngine::new().compute(sidebar, Size::new(400.0, 600.0));
        let theme = Theme::grid_tron();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            sidebar.paint(&mut cx);
        }
        let mut texts = Vec::new();
        let mut icons = 0usize;
        for c in scene.iter() {
            if let DrawCommand::Text(t) = c {
                if t.font == FontRole::Icon {
                    icons += 1;
                } else {
                    texts.push(t.text.clone());
                }
            }
        }
        (texts, icons)
    };

    // Expanded: the title + body row paint as text; no icon-rail glyph yet.
    let (texts, _) = paint(&mut sidebar);
    assert!(texts.iter().any(|t| t == "FILES"), "title paints while expanded");
    assert!(texts.iter().any(|t| t == "main.rs"), "body row paints while expanded");

    // Rail mode: the title + body text are gone; a duotone icon (2 glyph runs) paints.
    mode.set(RegionMode::CollapsedRail);
    let (texts, icons) = paint(&mut sidebar);
    assert!(texts.iter().all(|t| t != "FILES"), "title is not painted in rail mode");
    assert!(texts.iter().all(|t| t != "main.rs"), "body row is not painted in rail mode");
    assert!(icons >= 2, "rail paints the duotone dock icon (secondary + primary), got {icons}");
}

#[test]
fn rail_cell_lays_out_a_square_with_centered_icon() {
    use heca_grid_ui::{Glyph, Icon, RailCell};

    let mut cell = RailCell::new(Icon::new(Glyph::Terminal).size(20.0)).cell_size(44.0);
    LayoutEngine::new().compute(&mut cell, Size::new(200.0, 200.0));

    let b = cell.base().bounds;
    assert_eq!(b.size.w, 44.0, "cell is its configured width");
    assert_eq!(b.size.h, 44.0, "cell is square");

    // The single icon child sits centered in the square.
    let icon = cell.base().children[0].base().bounds;
    let icon_cx = icon.loc.x + icon.size.w / 2.0;
    let icon_cy = icon.loc.y + icon.size.h / 2.0;
    assert!((icon_cx - (b.loc.x + b.size.w / 2.0)).abs() < 1.0, "icon centered horizontally");
    assert!((icon_cy - (b.loc.y + b.size.h / 2.0)).abs() < 1.0, "icon centered vertically");
}

#[test]
fn rail_cell_activates_on_click_and_enter() {
    use heca_grid_ui::{Glyph, Icon, RailCell};
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut cell = RailCell::new(Icon::new(Glyph::GitBranch).size(20.0))
        .on_activate(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut cell, Size::new(200.0, 200.0));

    let b = cell.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    cell.event(&Event::PointerPressed { pos: center });
    cell.event(&Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(clicks.get(), 2, "click + Enter both activate the cell");
}

#[test]
fn key_hint_overlays_letter_only_when_set() {
    use heca_grid_ui::{FontRole, Glyph, Icon, KeyHint};

    let hint: Signal<Option<String>> = signal(None);
    let mut wrapped = KeyHint::new(Icon::new(Glyph::Terminal).size(20.0)).hint(hint);

    let paint = |w: &mut KeyHint| -> (Vec<String>, usize) {
        LayoutEngine::new().compute(w, Size::new(80.0, 80.0));
        let theme = Theme::grid_tron();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            w.paint(&mut cx);
        }
        let mut texts = Vec::new();
        let mut icons = 0usize;
        for c in scene.iter() {
            if let DrawCommand::Text(t) = c {
                match t.font {
                    FontRole::Icon => icons += 1,
                    FontRole::Text => texts.push(t.text.clone()),
                }
            }
        }
        (texts, icons)
    };

    // No hint: the child icon paints, no keycap letter.
    let (texts, icons) = paint(&mut wrapped);
    assert!(icons >= 2, "wrapped icon still paints (duotone = 2 runs)");
    assert!(texts.iter().all(|t| t != "a"), "no keycap letter while hint is None");

    // Hint set: the letter overlays; the child icon still paints underneath.
    hint.set(Some("a".to_string()));
    let (texts, icons) = paint(&mut wrapped);
    assert!(icons >= 2, "child icon still paints under the keycap");
    assert!(texts.iter().any(|t| t == "a"), "keycap letter paints while hint is Some");
}

#[test]
fn key_hint_is_transparent_to_focus_and_activation() {
    use heca_grid_ui::{FocusManager, KeyHint};
    use std::cell::Cell;
    use std::rc::Rc;

    // A focusable child wrapped in a KeyHint must stay reachable + activatable.
    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut wrapped = KeyHint::new(Item::new("file.rs").on_activate(move || sink.set(sink.get() + 1)));
    LayoutEngine::new().compute(&mut wrapped, Size::new(200.0, 60.0));

    // Focus traversal recurses through the transparent wrapper to the child.
    let mut focus = FocusManager::new();
    focus.advance(&mut wrapped, true);
    assert_eq!(focus.focused(), Some(0), "wrapped child is reachable by Tab");

    // Events route through the wrapper to the child.
    focus.deliver_key(&mut wrapped, heca_grid_ui::GridKey::Enter);
    assert_eq!(clicks.get(), 1, "Enter activates the wrapped child");
}

#[test]
fn attention_effect_plays_a_fixed_number_of_pulses() {
    use heca_grid_ui::Attention;

    let mut a = Attention::new();
    assert!(!a.is_active(), "idle until triggered");
    a.trigger(3);
    assert!(a.is_active(), "active after trigger");
    assert_eq!(a.amount(), 1.0, "starts at full strength");

    // Each tick of (≥) the pulse duration completes one pulse. After 3, it stops.
    assert!(a.tick(0.3), "pulse 1 done, pulse 2 begins");
    assert!(a.tick(0.3), "pulse 2 done, pulse 3 begins");
    assert!(!a.tick(0.3), "pulse 3 done — sequence ends");
    assert!(!a.is_active(), "inactive after the fixed pulse count");
}

#[test]
fn row_attention_request_pulses_then_settles() {
    use heca_grid_ui::Row;

    // Host-owned attention request: setting it true fires one pulse sequence and
    // is consumed back to false (so each request = one sequence).
    let req = signal(false);
    let mut row = Row::new().attention(req).on_activate(|| {});
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    req.set(true);
    assert!(row.tick(0.0), "attention request triggers an animating pulse");
    assert!(!req.get_untracked(), "the request signal is consumed");

    // The sequence is finite — ticking it out eventually settles (no animation).
    let mut settled = false;
    for _ in 0..60 {
        if !row.tick(0.05) {
            settled = true;
            break;
        }
    }
    assert!(settled, "attention pulse sequence ends and the row stops animating");
}

#[test]
fn icon_button_hugs_icon_by_default_and_pins_an_explicit_size() {
    use heca_grid_ui::{Glyph, Icon, IconButton};

    // Default: hugs the icon + padding (square-ish, larger than the glyph).
    let mut hug = IconButton::new(Icon::new(Glyph::Gear).size(18.0));
    LayoutEngine::new().compute(&mut hug, Size::new(200.0, 200.0));
    let b = hug.base().bounds;
    assert!(b.size.w > 18.0 && b.size.h > 18.0, "hugs icon + padding");
    assert!((b.size.w - b.size.h).abs() < 2.0, "roughly square");

    // Pinned: an exact square.
    let mut pinned = IconButton::new(Icon::new(Glyph::Gear).size(18.0)).size(40.0);
    LayoutEngine::new().compute(&mut pinned, Size::new(200.0, 200.0));
    let pb = pinned.base().bounds;
    assert_eq!(pb.size.w, 40.0, "pinned width");
    assert_eq!(pb.size.h, 40.0, "pinned square");
}

#[test]
fn icon_button_activates_on_click_and_enter_only_when_wired() {
    use heca_grid_ui::{Glyph, Icon, IconButton};
    use std::cell::Cell;
    use std::rc::Rc;

    // No on_click → inert + unfocusable.
    let mut bare = IconButton::new(Icon::new(Glyph::Search).size(18.0));
    LayoutEngine::new().compute(&mut bare, Size::new(100.0, 100.0));
    assert!(!bare.focusable(), "an icon button without on_click is not focusable");

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut btn = IconButton::new(Icon::new(Glyph::Search).size(18.0))
        .on_click(move || sink.set(sink.get() + 1));
    LayoutEngine::new().compute(&mut btn, Size::new(100.0, 100.0));
    assert!(btn.focusable(), "wired icon button is focusable");

    let b = btn.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    btn.event(&Event::PointerPressed { pos: center });
    btn.event(&Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(clicks.get(), 2, "click + Enter both fire on_click");
}

#[test]
fn tooltip_reveals_after_a_hover_delay_and_hides_on_leave() {
    use heca_grid_ui::Tooltip;

    let mut tip = Tooltip::new(Item::new("X"), "HELP").delay(0.5);

    // Render + report whether the bubble text was painted.
    let shows_help = |tip: &mut Tooltip| -> bool {
        LayoutEngine::new().compute(tip, Size::new(300.0, 200.0));
        let theme = Theme::grid_tron();
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            tip.paint(&mut cx);
        }
        scene.iter().any(|c| matches!(c, DrawCommand::Text(t) if t.text == "HELP"))
    };

    // Idle: no bubble.
    assert!(!shows_help(&mut tip), "hidden before hover");

    // Hover, but not past the delay yet.
    LayoutEngine::new().compute(&mut tip, Size::new(300.0, 200.0));
    let b = tip.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    tip.event(&Event::PointerMoved { pos: center });
    tip.tick(0.3);
    assert!(!shows_help(&mut tip), "still hidden before the delay elapses");

    // Past the delay: the bubble shows.
    tip.tick(0.3);
    assert!(shows_help(&mut tip), "bubble reveals after the hover delay");

    // Pointer leaves: hidden again immediately.
    tip.event(&Event::PointerMoved { pos: Point::new(-50.0, -50.0) });
    assert!(!shows_help(&mut tip), "hidden once the pointer leaves");
}

#[test]
fn tooltip_is_transparent_to_child_events() {
    use heca_grid_ui::{FocusManager, Tooltip};
    use std::cell::Cell;
    use std::rc::Rc;

    let clicks = Rc::new(Cell::new(0u32));
    let sink = clicks.clone();
    let mut tip = Tooltip::new(Item::new("file").on_activate(move || sink.set(sink.get() + 1)), "open");
    LayoutEngine::new().compute(&mut tip, Size::new(200.0, 60.0));

    let mut focus = FocusManager::new();
    focus.advance(&mut tip, true);
    assert_eq!(focus.focused(), Some(0), "wrapped child is reachable by Tab");
    focus.deliver_key(&mut tip, heca_grid_ui::GridKey::Enter);
    assert_eq!(clicks.get(), 1, "Enter activates the wrapped child through the tooltip");
}

#[test]
fn tooltip_flips_to_fit_the_viewport() {
    use heca_grid_ui::{Component, DrawCommand, Tooltip, TooltipSide};

    // A `Bottom` tooltip whose target sits near the viewport's bottom edge has no
    // room below → it must flip above the target.
    let vp = Size::new(300.0, 100.0);
    let mut tip = Tooltip::new(Item::new("X"), "HELP").side(TooltipSide::Bottom).delay(0.0);
    LayoutEngine::new().compute(&mut tip, vp);

    // Shove the whole subtree down so the target is near the bottom edge.
    fn shift(c: &mut dyn Component, dy: f64) {
        c.base_mut().bounds.loc.y += dy;
        for ch in c.base_mut().children.iter_mut() {
            shift(ch.as_mut(), dy);
        }
    }
    shift(&mut tip, 82.0);

    let b = tip.base().bounds;
    let center = Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0);
    tip.event(&Event::PointerMoved { pos: center });

    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        tip.paint(&mut cx);
    }
    let bubble = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "HELP" => Some(t.rect),
            _ => None,
        })
        .expect("tooltip bubble is painted");
    assert!(
        bubble.loc.y < b.loc.y,
        "Bottom tooltip with no room below flips above the target (bubble {} < target {})",
        bubble.loc.y,
        b.loc.y
    );
}

#[test]
fn modal_captures_input_only_while_open() {
    use heca_grid_ui::{Component, Modal};
    let closed = Modal::new("Title", "msg").confirm("OK", || {});
    assert!(!closed.overlay_active() && !closed.focusable(), "inert while closed");
    let open = Modal::new("Title", "msg").confirm("OK", || {}).open(true);
    assert!(open.overlay_active() && open.focusable(), "captures input while open");
}

#[test]
fn modal_enter_confirms_escape_cancels_then_closes() {
    use heca_grid_ui::{Component, Modal};
    use std::cell::Cell;
    use std::rc::Rc;

    let confirms = Rc::new(Cell::new(0u32));
    let cancels = Rc::new(Cell::new(0u32));
    let (c1, c2) = (confirms.clone(), cancels.clone());
    let mut m = Modal::new("Delete pane?", "This cannot be undone")
        .confirm("Delete", move || c1.set(c1.get() + 1))
        .cancel("Cancel", move || c2.set(c2.get() + 1))
        .open(true);

    // Enter = confirm → fires + closes.
    m.event(&Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(confirms.get(), 1, "Enter confirms");
    assert!(!m.overlay_active(), "closed after confirm");

    // Reopen; Esc = cancel → fires + closes.
    m.open_signal().set(true);
    m.event(&Event::Key { key: heca_grid_ui::GridKey::Escape, pressed: true });
    assert_eq!(cancels.get(), 1, "Escape cancels");
    assert!(!m.overlay_active(), "closed after cancel");
}

#[test]
fn modal_scrim_click_dismisses_but_panel_body_does_not() {
    use heca_grid_ui::{Component, Modal};
    use std::cell::Cell;
    use std::rc::Rc;

    let cancels = Rc::new(Cell::new(0u32));
    let c = cancels.clone();
    let mut m = Modal::new("Title", "a message")
        .confirm("OK", || {})
        .cancel("Cancel", move || c.set(c.get() + 1))
        .open(true);

    // Paint once so the modal caches the viewport for hit-testing.
    let vp = Size::new(400.0, 300.0);
    LayoutEngine::new().compute(&mut m, vp);
    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        m.paint(&mut cx);
    }

    // A click in the far corner (scrim) dismisses (= cancel).
    m.event(&Event::PointerPressed { pos: Point::new(3.0, 3.0) });
    assert_eq!(cancels.get(), 1, "scrim click cancels");
    assert!(!m.overlay_active(), "closed after scrim dismiss");

    // Reopen; a click in the panel body (its center, not a button) must NOT close.
    m.open_signal().set(true);
    m.event(&Event::PointerPressed { pos: Point::new(200.0, 150.0) });
    assert_eq!(cancels.get(), 1, "clicking the panel body does not dismiss");
    assert!(m.overlay_active(), "panel-body click keeps the dialog open");
}

#[test]
fn modal_non_dismissible_forces_a_button_choice() {
    use heca_grid_ui::{Component, Modal};
    use std::cell::Cell;
    use std::rc::Rc;

    let confirms = Rc::new(Cell::new(0u32));
    let c = confirms.clone();
    let mut m = Modal::new("Apply changes?", "Pick one")
        .confirm("Apply", move || c.set(c.get() + 1))
        .cancel("Cancel", || {})
        .dismissible(false)
        .open(true);

    let vp = Size::new(400.0, 300.0);
    LayoutEngine::new().compute(&mut m, vp);
    let theme = Theme::grid_tron();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        m.paint(&mut cx);
    }

    // Esc + scrim click are swallowed but DON'T close a non-dismissible dialog.
    m.event(&Event::Key { key: heca_grid_ui::GridKey::Escape, pressed: true });
    m.event(&Event::PointerPressed { pos: Point::new(3.0, 3.0) });
    assert!(m.overlay_active(), "non-dismissible dialog ignores Esc + scrim");

    // Only a button closes it (Enter = confirm).
    m.event(&Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(confirms.get(), 1, "a button still works");
    assert!(!m.overlay_active(), "closed once a button is chosen");
}

// --- CommandPalette --------------------------------------------------------

fn palette_with_markers() -> (heca_grid_ui::CommandPalette, std::rc::Rc<std::cell::Cell<u8>>) {
    use heca_grid_ui::{Command, CommandPalette};
    let ran = std::rc::Rc::new(std::cell::Cell::new(0u8));
    let (r1, r2, r3) = (ran.clone(), ran.clone(), ran.clone());
    let p = CommandPalette::new()
        .command(Command::new("Split pane", move || r1.set(1)))
        .command(Command::new("Close pane", move || r2.set(2)))
        .command(Command::new("Toggle sidebar", move || r3.set(3)));
    (p, ran)
}

#[test]
fn command_palette_is_overlay_active_only_while_open() {
    use heca_grid_ui::Component;
    let (p, _) = palette_with_markers();
    assert!(!p.overlay_active() && !p.focusable(), "inert while closed");
    let p = p.open(true);
    assert!(p.overlay_active() && p.focusable(), "captures input while open");
}

#[test]
fn command_palette_typing_filters_then_enter_runs_top_result() {
    use heca_grid_ui::{Component, GridKey};
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // Type "tog" → "Toggle sidebar" is the top (only) match.
    for c in "tog".chars() {
        p.event(&Event::Key { key: GridKey::Char(c), pressed: true });
    }
    p.event(&Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(ran.get(), 3, "Enter runs the filtered top result (Toggle sidebar)");
    assert!(!p.overlay_active(), "palette closes after running a command");
}

#[test]
fn command_palette_navigates_with_arrows_and_ctrl_jk() {
    use heca_grid_ui::{Component, GridKey, Modifiers};
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // No query → all three; selection starts at 0. Ctrl+J moves down twice → idx 2.
    p.event(&Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
    p.event(&Event::Key { key: GridKey::Char('j'), pressed: true });
    p.event(&Event::Key { key: GridKey::Char('j'), pressed: true });
    // ArrowUp moves back to idx 1.
    p.event(&Event::ModifiersChanged(Modifiers::default()));
    p.event(&Event::Key { key: GridKey::ArrowUp, pressed: true });
    p.event(&Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(ran.get(), 2, "Ctrl+J ×2 then ArrowUp lands on the 2nd command (Close pane)");
}

#[test]
fn command_palette_query_reuses_input_word_delete() {
    use heca_grid_ui::{GridKey, Modifiers};
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // "Toggle xyz" matches nothing (no command contains "...xyz").
    for c in "Toggle xyz".chars() {
        p.event(&Event::Key { key: GridKey::Char(c), pressed: true });
    }
    // Ctrl+Backspace word-deletes the whole "xyz" (not one char), leaving
    // "Toggle " — which now matches "Toggle sidebar". A char-delete would leave
    // "Toggle xy" (still no match), so this proves the Input editing is wired.
    p.event(&Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
    p.event(&Event::Key { key: GridKey::Backspace, pressed: true });
    p.event(&Event::ModifiersChanged(Modifiers::default()));
    p.event(&Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(ran.get(), 3, "Ctrl+Backspace word-delete leaves 'Toggle ' → runs Toggle sidebar");
}

#[test]
fn input_ctrl_h_deletes_char_and_ctrl_u_deletes_to_line_start() {
    use heca_grid_ui::{Input, Modifiers};
    let mut inp = Input::new().value("hello world");

    inp.event(&Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
    // Ctrl+H = delete one char back.
    inp.event(&Event::Key { key: heca_grid_ui::GridKey::Char('h'), pressed: true });
    assert_eq!(inp.value_str(), "hello worl", "Ctrl+H deletes one char back");
    // Ctrl+U = delete from caret to line start.
    inp.event(&Event::Key { key: heca_grid_ui::GridKey::Char('u'), pressed: true });
    assert_eq!(inp.value_str(), "", "Ctrl+U deletes to the start of the line");
}

// --- Toast ------------------------------------------------------------------

/// Lay a toast out as the root at its fixed width so `bounds` are set for
/// hit-testing, returning its resolved height.
fn layout_toast(t: &mut heca_grid_ui::Toast) -> f64 {
    LayoutEngine::new().compute(t, Size::new(400.0, 300.0));
    t.base().bounds.size.h
}

#[test]
fn toast_height_grows_with_body_then_action() {
    use heca_grid_ui::Toast;
    let bare = layout_toast(&mut Toast::info("Saved"));
    let with_body = layout_toast(&mut Toast::info("Saved").body("All files written"));
    let with_action =
        layout_toast(&mut Toast::info("Saved").body("All files written").action("Undo", || {}));
    assert!(with_body > bare, "a body line adds height");
    assert!(with_action > with_body, "an action row adds further height");
}

#[test]
fn toast_dismiss_button_fires_on_dismiss_and_consumes() {
    use heca_grid_ui::{Component, Toast};
    use std::cell::Cell;
    use std::rc::Rc;

    let dismissed = Rc::new(Cell::new(0u32));
    let d = dismissed.clone();
    let mut t = Toast::warning("Disk almost full").on_dismiss(move || d.set(d.get() + 1));
    layout_toast(&mut t);

    // The × lives in the top-right gutter (width 320, ~21px square inset by 13).
    let hit = t.event(&Event::PointerPressed { pos: Point::new(296.0, 23.0) });
    assert_eq!(dismissed.get(), 1, "clicking × fires on_dismiss");
    assert!(matches!(hit, Handled::Yes), "the × consumes the click");
}

#[test]
fn toast_action_button_fires_on_action() {
    use heca_grid_ui::{Component, Toast};
    use std::cell::Cell;
    use std::rc::Rc;

    let acted = Rc::new(Cell::new(0u32));
    let a = acted.clone();
    let mut t = Toast::info("File deleted").action("Undo", move || a.set(a.get() + 1));
    layout_toast(&mut t);

    // Action row sits below the title, left-aligned in the text column.
    t.event(&Event::PointerPressed { pos: Point::new(60.0, 50.0) });
    assert_eq!(acted.get(), 1, "clicking the action button fires on_action");
}

#[test]
fn toast_body_click_fires_on_click_only_when_set() {
    use heca_grid_ui::{Component, Toast};
    use std::cell::Cell;
    use std::rc::Rc;

    // Without on_click, a body click is not consumed (it can fall through).
    let mut inert = Toast::info("Build finished").dismissible(false);
    layout_toast(&mut inert);
    let hit = inert.event(&Event::PointerPressed { pos: Point::new(160.0, 20.0) });
    assert!(matches!(hit, Handled::No), "a non-clickable toast doesn't eat body clicks");

    // With on_click, the same click activates + consumes.
    let clicked = Rc::new(Cell::new(0u32));
    let c = clicked.clone();
    let mut t = Toast::info("Build finished")
        .dismissible(false)
        .on_click(move || c.set(c.get() + 1));
    layout_toast(&mut t);
    let hit = t.event(&Event::PointerPressed { pos: Point::new(160.0, 20.0) });
    assert_eq!(clicked.get(), 1, "body click fires on_click");
    assert!(matches!(hit, Handled::Yes), "a clickable toast consumes the body click");
}

#[test]
fn toast_focusable_only_when_clickable_and_enter_activates() {
    use heca_grid_ui::{Component, GridKey, Toast};
    use std::cell::Cell;
    use std::rc::Rc;

    let plain = Toast::info("Just an FYI");
    assert!(!plain.focusable(), "a non-clickable toast is not focusable");

    let clicked = Rc::new(Cell::new(0u32));
    let c = clicked.clone();
    let mut t = Toast::info("Open log?").on_click(move || c.set(c.get() + 1));
    assert!(t.focusable(), "a clickable toast is focusable");
    t.event(&Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(clicked.get(), 1, "Enter activates a focused clickable toast");
}

// --- Button respects theme border_width + radius ----------------------------

#[test]
fn button_derives_border_width_and_radius_from_theme() {
    use heca_grid_ui::{Button, Component};

    // A theme with a distinctive radius + border width.
    let mut theme = Theme::grid_tron();
    theme.radius = 10.0;
    theme.border_width = 2.0;
    let expected_radius = theme.control_radius();

    let mut btn = Button::primary("OK");
    LayoutEngine::new().base_font(theme.font_size).compute(&mut btn, Size::new(300.0, 80.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        btn.paint(&mut cx);
    }

    // The button's background box uses the surface fill; it must round to the
    // theme's control radius and stroke at the theme's border width — not the
    // old hardcoded 0.0 / 1.5.
    let bg = scene.iter().find_map(|cmd| match cmd {
        DrawCommand::Rect(r) if r.fill == theme.surface => Some(*r),
        _ => None,
    }).expect("button paints a surface-filled background box");
    assert_eq!(bg.radius, expected_radius, "button corner radius follows theme.control_radius()");
    assert_eq!(
        bg.border.expect("primary button has a border").width,
        theme.border_width,
        "button border width follows theme.border_width",
    );

    // border_width == 0 → no border drawn (borders off, like every surface).
    theme.border_width = 0.0;
    let mut btn = Button::primary("OK");
    LayoutEngine::new().base_font(theme.font_size).compute(&mut btn, Size::new(300.0, 80.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        btn.paint(&mut cx);
    }
    let bg = scene.iter().find_map(|cmd| match cmd {
        DrawCommand::Rect(r) if r.fill == theme.surface => Some(*r),
        _ => None,
    }).expect("button still paints its background box");
    assert!(bg.border.is_none(), "border_width == 0 means no button border");
}

// --- border_width == 0 ⇒ no borders (containers keep a thin uniform hairline) -

#[test]
fn bracket_frame_zero_border_is_a_uniform_hairline_not_broken_corners() {
    // Containers stay defined at border_width == 0, but via a single thin SOLID
    // uniform border — NOT the reticle (whose bright corners collapsed to nothing,
    // leaving empty corners + lingering dim straight edges).
    let mut theme = Theme::grid_tron();
    theme.border_width = 0.0;
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 120.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect, Some(theme.surface));
    }
    let rects: Vec<_> = scene.iter().filter_map(|c| match c {
        DrawCommand::Rect(r) => Some(*r),
        _ => None,
    }).collect();
    assert_eq!(rects.len(), 1, "border=0 frame is one uniform hairline (no dim-edge overlays)");
    assert!(rects[0].border.is_some_and(|b| b.width > 0.0), "the hairline is solid + visible");

    // With a real border the bright accent reticle (+ dim midsection overlays) returns.
    theme.border_width = 2.0;
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect, Some(theme.surface));
    }
    let bright = scene.iter().any(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color == theme.accent && b.width > 0.0)
    ));
    let n_rects = scene.iter().filter(|c| matches!(c, DrawCommand::Rect(_))).count();
    assert!(bright, "border>0 draws the bright accent reticle border");
    assert!(n_rects > 1, "border>0 also dims the straight midsections (overlay rects)");
}

/// Paint `w` under `border_width == 0` and return every visible (width>0) Rect
/// border stroke it emitted.
fn visible_border_widths_at_zero<C: heca_grid_ui::Component>(mut w: C) -> Vec<f32> {
    let mut theme = Theme::grid_tron();
    theme.border_width = 0.0;
    let vp = Size::new(400.0, 200.0);
    LayoutEngine::new().base_font(theme.font_size).compute(&mut w, vp);
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        w.paint(&mut cx);
    }
    scene.iter().filter_map(|c| match c {
        DrawCommand::Rect(r) => r.border.map(|b| b.width),
        _ => None,
    }).filter(|w| *w > 0.0).collect()
}

#[test]
fn non_container_widgets_drop_their_border_at_zero_border_width() {
    use heca_grid_ui::{Alert, Badge, Button, ProgressBar, Toggle};
    // Guard against the recurring regression: a widget that hardcodes a border
    // stroke instead of routing it through the theme (cx.border / border_width).
    for (name, widths) in [
        ("button", visible_border_widths_at_zero(Button::primary("OK"))),
        ("badge", visible_border_widths_at_zero(Badge::success("ON"))),
        ("alert", visible_border_widths_at_zero(Alert::warning("W").body("b"))),
        ("progress", visible_border_widths_at_zero(ProgressBar::new().value(0.5))),
        ("toggle", visible_border_widths_at_zero(Toggle::new().on(true))),
    ] {
        assert!(widths.is_empty(), "{name}: expected no border at border_width=0, got {widths:?}");
    }
}
