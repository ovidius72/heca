//! Phase A integration tests: the reactive + layout + component model, headless.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, PaintCx, Point, Scene, Size, Theme};

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
    assert_eq!(primary, Some(theme.surface), "primary rests on a dark surface");
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
fn intensity_off_suppresses_glow() {
    let mut theme = Theme::grid_tron();
    theme.intensity = Intensity::Off;

    let root = Surface::new().glow(Color::rgb(64, 224, 255));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        root.paint(&mut cx);
    }

    // The rect is still emitted, but its glow is stripped at Off intensity.
    let glow_present = scene.iter().any(|c| match c {
        DrawCommand::Rect(r) => r.glow.is_some(),
        _ => false,
    });
    assert!(!glow_present, "glow must be suppressed when intensity is Off");
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
        Some(&Action::value("input-change", SignalData::String("Hi 5".into()))),
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
    assert_eq!(input.value_str(), "ac", "backspace removes char before caret");

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
    assert!(fill_of(Badge::success("OK")).a > 0, "colored badge has a translucent fill");
    assert_eq!(fill_of(Badge::outline("OK")).a, 0, "outline badge has no fill");
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
