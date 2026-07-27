//! Phase A integration tests: the reactive + layout + component model, headless.

use heca_grid_ui::prelude::*;
use heca_grid_ui::{
    DrawCommand, Event, LayoutEngine, PaintCx, Point, Rectangle, Scene, Size, TextStyle, Theme,
};

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
fn label_weight_and_slant_are_font_attributes_decorations_are_rects() {
    let theme = Theme::default();
    let paint = |label: Label| {
        let mut label = label;
        LayoutEngine::new().compute(&mut label, Size::new(200.0, 40.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            label.paint(&mut cx);
        }
        let runs: Vec<TextStyle> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text(t) => Some(t.style),
                _ => None,
            })
            .collect();
        // The label paints no background of its own, so every rect it emits is a decoration.
        let rules: Vec<Rectangle> = scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Rect(r) => Some(r.rect),
                _ => None,
            })
            .collect();
        (runs, rules, label.base().bounds, label.base().font)
    };

    // Weight + slant reach the shaper as font attributes on the run…
    let (runs, rules, ..) = paint(Label::new("STATUS").bold(true).italic(true));
    assert_eq!(runs, vec![TextStyle::REGULAR.bold(true).italic(true)]);
    assert!(rules.is_empty(), "no decoration ⇒ no rects");

    // …while the decorations never touch it: they are rects the widget draws.
    let (runs, rules, bounds, font) = paint(Label::new("STATUS").underline(true));
    assert_eq!(runs, vec![TextStyle::REGULAR], "a rule is not a font attribute");
    assert_eq!(rules.len(), 1, "the underline");
    let rule = rules[0];
    let mid = bounds.loc.y + bounds.size.h / 2.0;
    assert!(rule.loc.y > mid, "the underline sits below the text centre");
    assert!(
        (rule.size.w - bounds.size.w).abs() < 0.5,
        "it spans the text run, which for a Start-aligned label is its whole box",
    );
    assert!(rule.size.h >= 1.0, "never thinner than a pixel: {}", rule.size.h);

    // Strikethrough goes through the text; both together draw two rules.
    let (_, rules, bounds, _) = paint(Label::new("STATUS").strikethrough(true));
    let mid = bounds.loc.y + bounds.size.h / 2.0;
    assert!(rules[0].loc.y < mid, "the strike sits at/above the centre");
    let (_, rules, ..) = paint(Label::new("STATUS").underline(true).strikethrough(true));
    assert_eq!(rules.len(), 2, "both rules");

    // An empty label has a zero-width run, so it draws no rule at all.
    let (_, rules, ..) = paint(Label::new("").underline(true));
    assert!(rules.is_empty(), "nothing to underline");
    let _ = font;
}

#[test]
fn label_decorations_follow_the_text_run_not_the_box() {
    let theme = Theme::default();
    // A label normally hugs its text (`remeasure` sizes the box to the run), but a *container* can
    // widen a child's bounds — a `Select` does exactly that to its option rows, so its pill spans
    // the panel. In that box, `align` decides where the run sits, and the rule must follow the run:
    // an underline spanning the whole box, most of it empty, would be plainly wrong.
    let mut label = Label::new("HI").align(TextAlign::End).underline(true);
    LayoutEngine::new().compute(&mut label, Size::new(300.0, 40.0));
    let run_w = label.base().bounds.size.w;
    label.base_mut().bounds.size.w = 300.0;

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        label.paint(&mut cx);
    }
    let rule = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Rect(r) => Some(r.rect),
            _ => None,
        })
        .expect("the underline is painted");
    let bounds = label.base().bounds;
    assert!(
        (rule.size.w - run_w).abs() < 0.5,
        "the rule is as wide as the two-character run ({run_w}), not the 300px box: {}",
        rule.size.w,
    );
    assert!(
        (rule.loc.x + rule.size.w - (bounds.loc.x + bounds.size.w)).abs() < 0.5,
        "End-aligned: the run — and its rule — sit at the right edge of the box",
    );
}

#[test]
fn paint_emits_background_rect_and_label_text() {
    let root = Surface::new()
        .background(Color::rgb(10, 10, 10))
        .child(Label::new("HI"));

    let theme = Theme::default();
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
    let theme = Theme::default();
    let surface = Surface::new()
        .background(Color::rgb(12, 18, 24))
        .border(theme.colors.accent, 1.5)
        .glow(theme.colors.glow);

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
    let theme = Theme::default();
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

    heca_grid_ui::dispatch(&mut button, &Event::PointerPressed { pos: outside });
    assert!(!clicked.get(), "click outside bounds must not fire");
    heca_grid_ui::dispatch(&mut button, &Event::PointerPressed { pos: center });
    assert!(clicked.get(), "click inside bounds must fire");
}

#[test]
fn widget_size_scales_font_and_box_proportionally() {
    // Big is the reference look; Normal (the default) and Small scale down — font
    // and the whole box shrink together so the control stays balanced.
    let measure = |size: WidgetSize| -> (f32, Rectangle) {
        let mut b = Button::new("RUN").size(size);
        LayoutEngine::new().compute(&mut b, Size::new(400.0, 100.0));
        (b.base().font, b.base().bounds)
    };
    let (small_f, small_b) = measure(WidgetSize::Small);
    let (normal_f, normal_b) = measure(WidgetSize::Normal);
    let (big_f, big_b) = measure(WidgetSize::Large);

    assert!(small_f < normal_f && normal_f < big_f, "font grows Small < Normal < Big");
    assert!(
        small_b.size.h < normal_b.size.h && normal_b.size.h < big_b.size.h,
        "box height grows with the size variant"
    );
    assert!(small_b.size.w < big_b.size.w, "box width grows with the size variant");

    // The default is Normal.
    let mut default_btn = Button::new("RUN");
    LayoutEngine::new().compute(&mut default_btn, Size::new(400.0, 100.0));
    assert_eq!(default_btn.base().font, normal_f, "default size is Normal");
}

#[test]
fn widget_size_scales_text_only_widgets_via_font() {
    // A Label has no padding, so the size variant shows purely as a smaller font.
    let font = |size: WidgetSize| {
        let mut l = Label::new("status").size(size);
        LayoutEngine::new().compute(&mut l, Size::new(200.0, 50.0));
        l.base().font
    };
    assert!(
        font(WidgetSize::Small) < font(WidgetSize::Normal)
            && font(WidgetSize::Normal) < font(WidgetSize::Large),
        "text widgets inherit the size variant through the resolved font"
    );
}

#[test]
fn button_hover_tracks_pointer() {
    let mut button = Button::new("HOVER");
    LayoutEngine::new().compute(&mut button, Size::new(200.0, 80.0));
    let hovered = button.hovered();
    let b = button.base().bounds;

    heca_grid_ui::dispatch(&mut button, &Event::PointerMoved {
        pos: Point::new(b.loc.x + 2.0, b.loc.y + 2.0),
    });
    assert!(hovered.get_untracked(), "entering bounds sets hover");
    heca_grid_ui::dispatch(&mut button, &Event::PointerMoved {
        pos: Point::new(b.loc.x + b.size.w + 50.0, b.loc.y),
    });
    assert!(!hovered.get_untracked(), "leaving bounds clears hover");
}

#[test]
fn button_variants_paint_distinct_fills() {
    let theme = Theme::default();
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
        Some(theme.colors.surface),
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
        focus.offer_to_overlay(&mut ui, &Event::Scroll { delta_x: 0.0, delta_y: 1.0 }),
        Handled::No,
        "no open overlay → nothing consumes the offer"
    );

    // A press dispatches with focus-on-press semantics: the clicked widget focuses.
    focus.dispatch(&mut ui, &Event::PointerPressed { pos: center });
    assert_eq!(focus.focused(), Some(1), "dispatch focuses the pressed widget");

    // A press that misses every focusable clears focus.
    focus.dispatch(&mut ui, &Event::PointerPressed {
        pos: Point::new(9999.0, 9999.0),
    });
    assert_eq!(focus.focused(), None, "dispatch clears focus on a miss");

    // No widget consumes a scroll → dispatch reports No so the host can page-scroll.
    assert_eq!(
        focus.dispatch(&mut ui, &Event::Scroll { delta_x: 0.0, delta_y: 1.0 }),
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
    focus.dispatch(&mut ui, &Event::PointerPressed {
        pos: Point::new(sb.loc.x + 5.0, sb.loc.y + 5.0),
    });
    assert!(
        focus.overlay_active(&mut ui),
        "pressing the trigger opens the dropdown overlay"
    );

    // With the dropdown open, a press on a row (outside the trigger's layout bounds)
    // is grabbed by the overlay first — it commits the selection and closes — rather
    // than being treated as a fresh focus/click on the tree behind it. The row is
    // found by its **bounds**: the options are real children, placed in the panel.
    let row2 = ui.base().children[1].base().children[2].base().bounds;
    let handled = focus.dispatch(&mut ui, &Event::PointerPressed {
        pos: Point::new(row2.loc.x + 10.0, row2.loc.y + row2.size.h / 2.0),
    });
    assert_eq!(handled, Handled::Yes, "the open overlay consumes the press");
    assert!(
        !focus.overlay_active(&mut ui),
        "committing a row closes the dropdown"
    );
}

#[test]
fn glow_none_suppresses_glow() {
    use heca_grid_ui::GlowLevel;
    // Glow is owned solely by `glow_size` now (intensity controls only scanlines),
    // so `GlowLevel::None` — not `Intensity::Off` — is what suppresses the glow.
    let mut theme = Theme::default();
    theme.colors.glow_size = GlowLevel::None;

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
    theme.colors.glow_size = GlowLevel::Medium;
    let mut scene2 = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene2, &theme);
        Surface::new().glow(Color::rgb(64, 224, 255)).paint(&mut cx);
    }
    let glow_present2 = scene2.iter().any(|c| matches!(c, DrawCommand::Rect(r) if r.glow.is_some()));
    assert!(glow_present2, "glow present when glow_size is Medium");
}

#[test]
fn glow_strength_scales_with_glow_size() {
    use heca_grid_ui::GlowLevel;
    // `glow_size` owns glow STRENGTH (alpha), not just radius: the painted glow's
    // intensity scales by the level's `strength_scale` (thin 0.5×, medium 1.0×,
    // large 1.6×) through the single `scaled_glow` chokepoint, so config drives it.
    let intensity_at = |level: GlowLevel| -> f32 {
        let mut theme = Theme::default();
        theme.colors.glow_size = level;
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            Surface::new().glow(Color::rgb(64, 224, 255)).paint(&mut cx);
        }
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) => r.glow.as_ref().map(|g| g.intensity),
                _ => None,
            })
            .expect("glow present")
    };
    let medium = intensity_at(GlowLevel::Medium);
    let thin = intensity_at(GlowLevel::Thin);
    let large = intensity_at(GlowLevel::Large);
    assert!(medium > 0.0, "medium glow intensity must be positive");
    // Assert against the curve itself (Thin must sit strictly between None and
    // Medium — its exact value is a tuning knob, e.g. 0.5→0.75 when Thin read
    // the same as None on the faint rest glows).
    assert!(
        (thin - medium * GlowLevel::Thin.strength_scale()).abs() < 1e-4,
        "thin follows its strength_scale"
    );
    assert!(thin > 0.0 && thin < medium, "thin sits between none and medium");
    assert!(
        (large - medium * GlowLevel::Large.strength_scale()).abs() < 1e-4,
        "large follows its strength_scale"
    );
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
    heca_grid_ui::dispatch(&mut toggle, &Event::PointerPressed { pos: outside });
    assert!(!toggle.is_on());
    assert!(log.borrow().is_empty(), "missed press emits no action");

    // A press inside flips it on and reports the new value.
    heca_grid_ui::dispatch(&mut toggle, &Event::PointerPressed { pos: center });
    assert!(toggle.is_on(), "press flips the toggle on");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("toggle-change", SignalData::Bool(true))),
    );

    // Pressing again flips it back off.
    heca_grid_ui::dispatch(&mut toggle, &Event::PointerPressed { pos: center });
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
    let press = Point::new(
        toggle.base().bounds.loc.x + 1.0,
        toggle.base().bounds.loc.y + 1.0,
    );
    heca_grid_ui::dispatch(&mut toggle, &Event::PointerPressed { pos: press });
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
    heca_grid_ui::dispatch(&mut toggle, &Event::PointerPressed { pos: center });
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
    heca_grid_ui::dispatch(&mut button, &Event::PointerPressed { pos: center });
    assert!(!clicked.get(), "disabled button ignores clicks");
    assert!(!button.focusable(), "disabled button is unfocusable");
}

#[test]
fn focusable_is_driven_by_the_base_flag_and_disabled() {
    // Non-interactive widgets stay unfocusable (default `Base.focusable == false`).
    assert!(!Label::new("x").focusable(), "a plain label is not focusable");

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

/// The label color of a button after layout + paint (its single Text run).
fn button_label_color(button: &mut Button, theme: &Theme) -> Color {
    LayoutEngine::new().compute(button, Size::new(200.0, 80.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
        button.paint(&mut cx);
    }
    scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) => Some(t.color),
            _ => None,
        })
        .expect("button paints a label text run")
}

#[test]
fn disabled_button_label_is_muted_and_faded_on_every_variant() {
    use heca_grid_ui::ButtonVariant;
    let theme = Theme::default();
    let muted = theme.colors.muted;
    for variant in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ] {
        let disabled =
            button_label_color(&mut Button::new("OK").variant(variant).disabled(true), &theme);
        let enabled = button_label_color(&mut Button::new("OK").variant(variant), &theme);
        // Disabled label is the theme `muted` hue (not the variant's vivid color)…
        assert_eq!(
            disabled.with_alpha(255),
            muted.with_alpha(255),
            "disabled label should use the theme muted hue",
        );
        // …at a clearly reduced opacity, so the disabled state reads even on Ghost/Link
        // (whose enabled rest label is already `muted` at full alpha).
        assert!(disabled.a < 255, "disabled label should be faded");
        assert!(
            disabled.a < enabled.a,
            "disabled label must be fainter than the enabled label",
        );
    }
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

    heca_grid_ui::dispatch(&mut cb, &Event::PointerPressed { pos: center });
    assert!(cb.is_checked(), "press checks the box");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("checkbox-change", SignalData::Bool(true))),
    );

    heca_grid_ui::dispatch(&mut cb, &Event::PointerPressed { pos: center });
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
    heca_grid_ui::dispatch(&mut cb, &Event::PointerPressed { pos: far_right });
    assert!(
        cb.is_checked(),
        "clicking the (right) label toggles the box"
    );

    // Left label: the label text command sits left of the box.
    let theme = Theme::default();
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
    let theme = Theme::default();
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
        heca_grid_ui::dispatch(&mut input, &Event::Key { key, pressed: true });
    }
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Space,
        pressed: true,
    });
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "ac",
        "backspace removes char before caret"
    );

    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Char('X'),
        pressed: true,
    });
    assert_eq!(input.value_str(), "aXc", "insert lands at the caret");
}

#[test]
fn input_placeholder_shows_only_when_empty_and_unfocused() {
    let theme = Theme::default();
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
        heca_grid_ui::dispatch(&mut *i, &Event::PointerPressed {
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

    heca_grid_ui::dispatch(&mut input, &Event::PointerPressed { pos }); // caret
    heca_grid_ui::dispatch(&mut input, &Event::PointerPressed { pos }); // word = whole "hello"
    assert_eq!(input.selected_text().as_deref(), Some("hello"));

    heca_grid_ui::dispatch(&mut input, &Event::Key {
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

    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
        ctrl: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Backspace,
        pressed: true,
    });
    assert_eq!(input.value_str(), "alpha ", "deletes the word at the caret");
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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
        heca_grid_ui::dispatch(&mut input, &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        });
    }
    // On macOS the word modifier is Alt/Option — accepted cross-platform.
    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
        alt: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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
        heca_grid_ui::dispatch(&mut *i, &Event::Key {
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
    heca_grid_ui::dispatch(&mut a, &Event::ModifiersChanged(Modifiers {
        meta: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut a, &Event::Key {
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
    heca_grid_ui::dispatch(&mut b, &Event::ModifiersChanged(Modifiers {
        meta: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut b, &Event::Key {
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
        heca_grid_ui::dispatch(&mut *i, &Event::Key {
            key: GridKey::ArrowLeft,
            pressed: true,
        })
    };
    let right = |i: &mut Input| {
        heca_grid_ui::dispatch(&mut *i, &Event::Key {
            key: GridKey::ArrowRight,
            pressed: true,
        })
    };

    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
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

    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("alpha beta"),
        "shift+ctrl+left selects to the start"
    );
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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

    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
        alt: true,
        shift: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::ArrowLeft,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("gamma"),
        "first word back"
    );
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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
fn input_edit_select_all_selects_without_typing() {
    use heca_grid_ui::WidgetIntent;
    // Select-all is host-configured (`edit_select_all`, default Ctrl+a / Cmd+a) and arrives as
    // the semantic `WidgetIntent::EditSelectAll`; a raw modified 'a' is never typed (separately).
    let mut input = Input::new().value("hello world");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(&mut input, &Event::Widget(WidgetIntent::EditSelectAll));
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello world"),
        "SelectAll selects the whole field"
    );
    assert_eq!(input.value_str(), "hello world", "nothing is typed");
}

#[test]
fn home_end_move_caret_to_bounds() {
    let mut input = Input::new().value("hello");
    LayoutEngine::new().compute(&mut input, Size::new(400.0, 60.0));

    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Home,
        pressed: true,
    });
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Char('X'),
        pressed: true,
    });
    assert_eq!(
        input.value_str(),
        "Xhello",
        "Home moves the caret to the start"
    );

    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::End,
        pressed: true,
    });
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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

    heca_grid_ui::dispatch(&mut input, &Event::ModifiersChanged(Modifiers {
        shift: true,
        ..Default::default()
    }));
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Home,
        pressed: true,
    });
    assert_eq!(
        input.selected_text().as_deref(),
        Some("hello"),
        "Shift+Home selects to start"
    );
    heca_grid_ui::dispatch(&mut input, &Event::Key {
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
    heca_grid_ui::dispatch(&mut input, &Event::Key {
        key: GridKey::Char('x'),
        pressed: true,
    });
    assert!(input.value_str().is_empty(), "disabled input ignores keys");
    assert!(!input.focusable(), "disabled input is unfocusable");
}

// ── Phase C: display widgets ──

#[test]
fn badge_colored_has_fill_outline_has_none() {
    let theme = Theme::default();
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
    let theme = Theme::default();
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
    let theme = Theme::default();
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
    assert_eq!(online, theme.colors.success);
    assert!(online_glow, "active dot glows");
    assert_eq!(offline, theme.colors.muted);
    assert!(!offline_glow, "offline dot does not glow");
}

#[test]
fn select_opens_and_paints_options_in_overlay_layer() {
    let theme = Theme::default();
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
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed {
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
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    }); // open

    // Click the third row (HIGH) **where it actually is**: the options are child components, and
    // opening the list placed them in the panel, so their bounds are the rows on screen. No row
    // arithmetic — what is drawn is what is clicked.
    let row2 = sel.base().children[2].base().bounds;
    assert!(
        row2.loc.y > b.loc.y + b.size.h,
        "the rows are placed in the panel, below the trigger"
    );
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed {
        pos: Point::new(row2.loc.x + 10.0, row2.loc.y + row2.size.h / 2.0),
    });
    assert_eq!(sel.index(), 2, "clicking a row selects it");
    assert!(!sel.overlay_active(), "selection closes the dropdown");
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("select-change", SignalData::Usize(2))),
    );
}

#[test]
fn select_sugar_builds_choice_children_and_composed_options_carry_their_content() {
    let theme = Theme::default();

    // The string constructor is sugar: every option is a `Choice` child whose value is the text.
    let sugar = Select::new(["LOW", "HIGH"]);
    assert_eq!(sugar.base().children.len(), 2, "one child per option");
    assert_eq!(
        sugar.base().children[1].text_summary().as_deref(),
        Some("HIGH"),
        "the option's content is a Label the widget can name",
    );

    // A composed option: any content, plus a value that is not the text.
    let mut sel = Select::empty()
        .option(Choice::new("low").child(Flex::row().child(Label::new("LOW"))))
        .option(
            Choice::new("high").child(
                Flex::row()
                    .child(Icon::new(Glyph::Warning))
                    .child(Label::new("HIGH")),
            ),
        )
        .selected(1);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));

    // The closed trigger shows the chosen option **itself** — it stands the child inside the
    // trigger box, so the icon comes with it. (Its text alone is still available as the option's
    // accessible name, which is what `selected_label` reports.)
    assert_eq!(sel.selected_label(), "HIGH");
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        sel.paint(&mut cx);
    }
    let closed: Vec<String> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        closed.contains(&"HIGH".to_string()),
        "the closed trigger shows the chosen option's label",
    );
    assert!(
        closed.len() > 1,
        "…and its Icon child, painted with it: {closed:?}",
    );
    let trigger = sel.base().bounds;
    let chosen = sel.base().children[1].base().bounds;
    let slack = 0.5; // the trigger hugs the tallest option, so they agree to within rounding
    assert!(
        chosen.loc.y >= trigger.loc.y - slack
            && chosen.loc.y + chosen.size.h <= trigger.loc.y + trigger.size.h + slack,
        "the chosen option is placed inside the trigger while closed",
    );
    assert_eq!(
        sel.base().children[0].base().bounds.size,
        Size::new(0.0, 0.0),
        "the options not chosen are collapsed",
    );

    // Opening draws the options' own content — including the icon, which no `Vec<String>` of
    // options could ever have carried.
    let press = Point::new(sel.base().bounds.loc.x + 5.0, sel.base().bounds.loc.y + 5.0);
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed { pos: press });
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        sel.paint(&mut cx);
    }
    let runs: Vec<(String, Color)> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.text.clone(), t.color)),
            _ => None,
        })
        .collect();
    let texts: Vec<&str> = runs.iter().map(|(t, _)| t.as_str()).collect();
    assert!(
        texts.contains(&"LOW") && texts.contains(&"HIGH"),
        "the open list paints each option's Label child",
    );
    assert!(
        runs.len() > 3,
        "beyond the trigger + the two labels, the option's Icon child draws its glyph too",
    );
    // The chosen row tints its whole content: the `Choice` publishes the accent as the inherited
    // content color, and its unstyled Label picks it up — with no wiring from the Select.
    let high_row = runs
        .iter()
        .rposition(|(t, _)| t == "HIGH")
        .expect("the HIGH row is painted");
    assert_eq!(
        runs[high_row].1,
        theme.colors.accent,
        "the selected option's content is accent-tinted",
    );

    // …and the trigger keeps showing the chosen option — icon *and* label — while the list is open.
    // The option itself is in the list now, so the trigger draws a second image of its content,
    // translated back into the trigger. Two runs land inside the trigger: the glyph and the word.
    let trigger = sel.base().bounds;
    let in_trigger: Vec<String> = scene
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text(t) => Some((t.rect, t.text.clone())),
            _ => None,
        })
        .filter(|(r, _)| {
            trigger.contains(Point::new(
                r.loc.x + r.size.w / 2.0,
                r.loc.y + r.size.h / 2.0,
            ))
        })
        .map(|(_, t)| t)
        .collect();
    assert!(
        in_trigger.contains(&"HIGH".to_string()),
        "the open trigger still names the chosen option: {in_trigger:?}",
    );
    assert!(
        in_trigger.len() > 1,
        "…and still shows its icon, not just the word: {in_trigger:?}",
    );
}

#[test]
fn select_rows_outside_the_visible_window_are_not_clickable() {
    let opts: Vec<String> = (0..20).map(|n| format!("OPT{n}")).collect();
    let mut sel = Select::new(opts);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 400.0));

    let b = sel.base().bounds;
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    }); // open — 6 rows visible of 20

    // A row past the window is collapsed: it is not drawn, so it cannot be hit either. (Were its
    // stale bounds left behind, they would sit under the trigger and swallow clicks.)
    for i in 6..20 {
        assert_eq!(
            sel.base().children[i].base().bounds.size,
            Size::new(0.0, 0.0),
            "row {i} is outside the visible window",
        );
    }
    // Closing collapses every row **except the chosen one**, which goes back to standing in the
    // trigger (that is how the trigger shows the option's own content).
    heca_grid_ui::dispatch(&mut sel, &Event::Widget(heca_grid_ui::WidgetIntent::Dismiss));
    let chosen = sel.index();
    for i in 0..20 {
        if i == chosen {
            continue;
        }
        assert_eq!(
            sel.base().children[i].base().bounds.size,
            Size::new(0.0, 0.0),
            "row {i} is collapsed while the list is closed",
        );
    }
    let trigger = sel.base().bounds;
    assert!(
        trigger.contains(Point::new(
            trigger.loc.x + 5.0,
            sel.base().children[chosen].base().bounds.loc.y + 2.0,
        )),
        "the chosen option stands in the trigger",
    );
}

#[test]
fn select_flips_above_the_trigger_when_there_is_no_room_below() {
    let theme = Theme::default();
    // The Select sits at the bottom of the viewport: the panel cannot open downward.
    let mut ui = Flex::column()
        .height(Length::Px(300.0))
        .justify(Justify::End)
        .child(Select::new(["A", "B", "C"]));
    LayoutEngine::new().compute(&mut ui, Size::new(300.0, 300.0));

    // Paint once so the widget learns the viewport height (that is what it flips against).
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(300.0, 300.0));
        ui.paint(&mut cx);
    }

    let trigger = ui.base().children[0].base().bounds;
    heca_grid_ui::dispatch(ui.base_mut().children[0].as_mut(), &Event::PointerPressed {
        pos: Point::new(trigger.loc.x + 5.0, trigger.loc.y + 5.0),
    });

    let first_row = ui.base().children[0].base().children[0].base().bounds;
    assert!(
        first_row.loc.y + first_row.size.h <= trigger.loc.y,
        "no room below → the rows are placed above the trigger",
    );
}

#[test]
fn select_long_list_caps_visible_rows_and_scrolls() {
    let theme = Theme::default();
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
    heca_grid_ui::dispatch(&mut sel, &Event::PointerPressed {
        pos: Point::new(b.loc.x + 5.0, b.loc.y + 5.0),
    }); // open

    // Trigger label (1) + at most MAX_VISIBLE (6) rows are painted.
    let texts = row_texts(&sel);
    assert_eq!(texts.len(), 1 + 6, "long list caps the visible rows");
    assert_eq!(texts[1], "OPT0", "starts at the top");

    // Wheel-scroll moves the visible window down.
    heca_grid_ui::dispatch(&mut sel, &Event::Scroll { delta_x: 0.0, delta_y: 5.0 });
    assert_eq!(row_texts(&sel)[1], "OPT5", "scroll reveals later options");

    // Scrolling past the end clamps to the last full window.
    heca_grid_ui::dispatch(&mut sel, &Event::Scroll { delta_x: 0.0, delta_y: 999.0 });
    assert_eq!(row_texts(&sel)[1], "OPT14", "scroll clamps at max (20 - 6)");
}

#[test]
fn select_keyboard_navigates_and_escape_closes() {
    use heca_grid_ui::WidgetIntent;
    let mut sel = Select::new(["A", "B", "C"]);
    LayoutEngine::new().compute(&mut sel, Size::new(300.0, 200.0));
    let raw = |s: &mut Select, k: GridKey| heca_grid_ui::dispatch(&mut *s, &Event::Key { key: k, pressed: true });
    let nav = |s: &mut Select, i: WidgetIntent| heca_grid_ui::dispatch(&mut *s, &Event::Widget(i));

    // A closed Select opens on a raw activation key (Enter/Space/↓), like a button.
    raw(&mut sel, GridKey::Enter);
    assert!(sel.overlay_active());
    // While open it is a vertical overlay: the host drives it with `MenuUp`/`MenuDown`.
    nav(&mut sel, WidgetIntent::MenuDown);
    nav(&mut sel, WidgetIntent::MenuDown);
    nav(&mut sel, WidgetIntent::Activate); // commit highlight (index 2)
    assert_eq!(sel.index(), 2);
    assert!(!sel.overlay_active(), "Activate commits and closes");

    raw(&mut sel, GridKey::Enter); // reopen
    assert!(sel.overlay_active());
    nav(&mut sel, WidgetIntent::Dismiss);
    assert!(
        !sel.overlay_active(),
        "Dismiss closes without changing selection"
    );
    assert_eq!(sel.index(), 2);
}

#[test]
fn tabs_menu_nav_and_click_change_selection() {
    use heca_grid_ui::{Action, SignalData};
    use std::cell::RefCell;
    use std::rc::Rc;

    let log: Rc<RefCell<Vec<Action>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let mut tabs =
        Tabs::new(["ALPHA", "BETA", "GAMMA"]).on_change(move |a| sink.borrow_mut().push(a));
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    use heca_grid_ui::WidgetIntent;
    assert_eq!(tabs.index(), 0);
    // A horizontal selector: nav arrives as `ItemPrevious`/`ItemNext` (the host maps the
    // configurable item keys — ←/Ctrl+h → previous, →/Ctrl+l → next). No literal keys.
    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 1);
    assert_eq!(
        log.borrow().last(),
        Some(&Action::value("tab-change", SignalData::Usize(1))),
    );

    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 2);
    let before = log.borrow().len();
    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(WidgetIntent::ItemNext));
    assert_eq!(tabs.index(), 2, "ItemNext clamps at the last tab");
    assert_eq!(
        log.borrow().len(),
        before,
        "no event emitted when selection is unchanged"
    );

    // A click near the left edge selects the first tab again.
    let b = tabs.base().bounds;
    heca_grid_ui::dispatch(&mut tabs, &Event::PointerPressed {
        pos: Point::new(b.loc.x + 2.0, b.loc.y + b.size.h / 2.0),
    });
    assert_eq!(tabs.index(), 0, "click selects the hit tab");
}

#[test]
fn tabs_underline_slides_toward_the_selected_tabs_bounds() {
    let theme = Theme::default();
    // The tabs paint their own pills, so pick the underline out by its thickness — it is the only
    // 2px-tall rect in the strip.
    let underline = |t: &Tabs| {
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            t.paint(&mut cx);
        }
        scene
            .iter()
            .find_map(|c| match c {
                DrawCommand::Rect(r) if (r.rect.size.h - 2.0).abs() < 0.01 => Some(r.rect),
                _ => None,
            })
            .expect("the underline is painted")
    };
    let mut tabs = Tabs::new(["ALPHA", "BETA", "GAMMA"]);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    // It starts on the selected tab — snapped to that child's real bounds, not slid in from the
    // origin — and it is exactly as wide as the tab.
    let first = tabs.base().children[0].base().bounds;
    let u0 = underline(&tabs);
    assert!((u0.loc.x - first.loc.x).abs() < 0.01, "starts on tab 0");
    assert!((u0.size.w - first.size.w).abs() < 0.01, "as wide as tab 0");

    heca_grid_ui::dispatch(&mut tabs, &Event::Widget(heca_grid_ui::WidgetIntent::ItemNext));
    let mid = underline(&tabs);
    assert!(
        mid.loc.x == u0.loc.x,
        "it does not jump: the slide happens in tick",
    );
    for _ in 0..40 {
        tabs.tick(0.016);
    }

    // …and it lands on the *bounds* of the newly selected tab, whatever that tab contains.
    let second = tabs.base().children[1].base().bounds;
    let u1 = underline(&tabs);
    assert!(u1.loc.x > u0.loc.x, "it slid right");
    assert!(
        (u1.loc.x - second.loc.x).abs() < 0.01 && (u1.size.w - second.size.w).abs() < 0.01,
        "it tracks the selected tab's real bounds",
    );
}

#[test]
fn tabs_sugar_builds_choice_children_and_composed_tabs_carry_their_content() {
    let theme = Theme::default();

    // The string constructor is sugar: every tab is a `Choice` child whose value is the text.
    let sugar = Tabs::new(["ALPHA", "BETA"]);
    assert_eq!(sugar.base().children.len(), 2, "one child per tab");
    assert_eq!(
        sugar.base().children[1].text_summary().as_deref(),
        Some("BETA"),
    );

    // A composed tab: an icon, a label and a count Badge — none of which a char-count could have
    // measured, and all of which the underline must now span.
    let mut tabs = Tabs::empty()
        .tab(Choice::labeled("files", "FILES"))
        .tab(
            Choice::new("issues")
                .child(Icon::new(Glyph::Warning))
                .child(Label::new("ISSUES"))
                .child(Badge::new("3")),
        )
        .selected(1);
    LayoutEngine::new().compute(&mut tabs, Size::new(600.0, 60.0));

    assert_eq!(tabs.selected_label(), "ISSUES");
    let issues = tabs.base().children[1].base().bounds;
    let files = tabs.base().children[0].base().bounds;
    assert!(
        issues.size.w > files.size.w,
        "the composed tab measures wider than the plain one (it holds more)",
    );

    // The selected tab tints its whole content through the `Choice` — with no wiring from Tabs.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        tabs.paint(&mut cx);
    }
    let issues_color = scene
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text(t) if t.text == "ISSUES" => Some(t.color),
            _ => None,
        })
        .expect("the composed tab paints its label");
    assert_eq!(
        issues_color, theme.colors.accent,
        "the selected tab's content is accent-tinted",
    );
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

/// A separator's two properties do not depend on each other, in either order.
///
/// `length` used to write straight onto `width`, assuming the rule was horizontal. Set the
/// orientation afterwards — which a described separator does, since properties arrive sorted by
/// name and `length` sorts before `orientation` — and the length landed on the axis the rule runs
/// *across*, leaving the span unset. The widget now recomputes both axes from the pair, so this
/// passes whichever way round it is written.
#[test]
fn a_separators_length_and_orientation_can_be_set_in_either_order() {
    use heca_grid_ui::{PropInput, SetProp};

    for (first, second) in [("length", "orientation"), ("orientation", "length")] {
        let apply = |sep: Separator, key: &str| match key {
            "length" => sep.set_prop("length", &PropInput::Number(60.0)),
            _ => sep.set_prop("orientation", &PropInput::Text("vertical".into())),
        };
        let sep = apply(apply(Separator::horizontal(), first), second);

        let mut row = Flex::row()
            .width(Length::Px(200.0))
            .height(Length::Px(200.0))
            .child(sep);
        LayoutEngine::new().compute(&mut row, Size::new(200.0, 200.0));
        let bounds = row.base().children[0].base().bounds;

        assert_eq!(
            bounds.size.h, 60.0,
            "setting {first} then {second}: a vertical rule runs 60px down",
        );
        assert!(
            bounds.size.w <= 1.0,
            "setting {first} then {second}: a vertical rule stays thin ({}px wide)",
            bounds.size.w,
        );
    }

    // And `vertical()` still means what it always meant, without any property being set.
    let mut row = Flex::row()
        .width(Length::Px(200.0))
        .height(Length::Px(80.0))
        .child(Separator::vertical());
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 80.0));
    let bounds = row.base().children[0].base().bounds;
    assert_eq!(bounds.size.h, 80.0, "a vertical rule stretches to the container height");
    assert!(bounds.size.w <= 1.0, "and stays thin");
}

/// A `Panel`'s heading is a real composed child, and setting it works **whichever side of the
/// children it happens on** — which is what a description needs, since properties are applied after
/// children are attached.
///
/// Before F003/P017/T008 there was no `Panel` widget at all: `WidgetKind::Panel` realized to a bare
/// `Surface`, so the published examples showed `Panel::new().title("…")` against something with no
/// title, and no reader could tell.
#[test]
fn a_panel_titles_itself_whichever_order_it_is_built_in() {
    use heca_grid_ui::Panel;

    // Title first, then content.
    let a = Panel::new()
        .title("Containers")
        .child(Label::new("nginx"))
        .child(Label::new("redis"));
    // Content first, then title — the order `realize` uses.
    let b = Panel::new()
        .child(Label::new("nginx"))
        .child(Label::new("redis"))
        .title("Containers");

    for (which, panel) in [("title first", &a), ("children first", &b)] {
        let kids = &panel.base().children;
        assert_eq!(kids.len(), 4, "{which}: header + rule + two content children");
        assert!(
            !kids[0].base().style.layout.hidden && !kids[1].base().style.layout.hidden,
            "{which}: the header AND its rule show once titled",
        );
    }
    assert_eq!(a.title_signal().get_untracked(), "Containers");
    assert_eq!(b.title_signal().get_untracked(), "Containers");

    // An untitled panel keeps the header out of the layout rather than leaving a blank line.
    let plain = Panel::new().child(Label::new("body"));
    assert!(
        plain.base().children[0].base().style.layout.hidden
            && plain.base().children[1].base().style.layout.hidden,
        "no title means no heading and no bare rule across the top of the content",
    );

    // And a title can be cleared back to nothing.
    let cleared = Panel::titled("Gone").title("");
    assert!(
        cleared.base().children[0].base().style.layout.hidden
            && cleared.base().children[1].base().style.layout.hidden,
    );
}

/// A separator with no length spans its container **even when the container centres its children**.
///
/// Found by looking at it: the showcase row centres, like most rows do, so the rule was laid out
/// one pixel by zero and simply did not appear. The widget's answer used to be a line in its docs
/// telling the caller to pass a `length` — a workaround repeated at every call site for something
/// the rule can say once about itself, and one that silently produces nothing when forgotten.
#[test]
fn a_separator_spans_a_container_that_centres_its_children() {
    let mut row = Flex::row()
        .align(Align::Center)
        .width(Length::Px(200.0))
        .height(Length::Px(40.0))
        .child(Separator::vertical())
        .child(Separator::vertical().length(24.0));
    LayoutEngine::new().compute(&mut row, Size::new(200.0, 40.0));

    let stretched = row.base().children[0].base().bounds;
    assert_eq!(
        stretched.size.h, 40.0,
        "with no length, the rule spans the row despite Align::Center",
    );

    let cut = row.base().children[1].base().bounds;
    assert_eq!(cut.size.h, 24.0, "an explicit length still wins");
    assert!(
        cut.loc.y > stretched.loc.y,
        "…and the container's own alignment centres the shorter one",
    );

    // Same story the other way round: a column that centres still gets a full-width rule.
    let mut col = Flex::column()
        .align(Align::Center)
        .width(Length::Px(200.0))
        .height(Length::Px(40.0))
        .child(Separator::horizontal());
    LayoutEngine::new().compute(&mut col, Size::new(200.0, 40.0));
    assert_eq!(
        col.base().children[0].base().bounds.size.w,
        200.0,
        "a horizontal rule spans a centring column too",
    );
}

#[test]
fn spinner_animates_and_paints_its_ring() {
    let theme = Theme::default();
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
    let theme = Theme::default();
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
    let theme = Theme::default();
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
    let theme = Theme::default();
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
    heca_grid_ui::dispatch(&mut item, &Event::PointerPressed {
        pos: Point::new(b.loc.x + b.size.w / 2.0, b.loc.y + b.size.h / 2.0),
    });
    assert_eq!(hits.get(), 1, "click activates the row");

    // Space activates too (keyboard).
    heca_grid_ui::dispatch(&mut item, &Event::Key {
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
        heca_grid_ui::dispatch(&mut item, &Event::PointerPressed {
            pos: Point::new(b.loc.x + 1.0, b.loc.y + 1.0),
        }),
        Handled::No,
    );
}

#[test]
fn item_label_color_tracks_selected_state() {
    let theme = Theme::default();
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
        Some(theme.colors.foreground),
        "plain label uses foreground"
    );
    assert_eq!(
        label_color(&sel),
        Some(theme.colors.accent),
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
    let theme = Theme::default();
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
    let theme = Theme::default();
    let mut pane = Pane::new()
        .background(theme.colors.surface)
        .border(theme.colors.accent, theme.colors.border_width)
        .child(Label::new("X"));
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
                if r.border.is_some() && r.radius == theme.colors.border_radius
        )
    });
    assert!(rounded_border, "pane draws a rounded accent border at the theme radius");

    // The surface carries the faint theme REST glow (`interaction.control_rest_glow`)
    // so the `glow_size` setting visibly scales panes at rest too (T011).
    let expected_i = theme.colors.interaction.control_rest_glow as f32 / 255.0;
    let rest_glow = scene.iter().any(|c| {
        matches!(
            c,
            DrawCommand::Rect(r)
                if r.border.is_some()
                    && r.glow.is_some_and(|g| (g.intensity - expected_i).abs() < 1e-6)
        )
    });
    assert!(rest_glow, "pane surface carries the theme rest glow");
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
fn grid_areas_template_defines_the_rows_not_the_row_tracks() {
    use heca_grid_ui::{Grid, Track};

    // The template is what defines the structure; `rows(..)` only *sizes* the tracks it implies.
    // A template with more lines than there are row tracks therefore creates **implicit** rows —
    // and an item spanning them is centred over a taller area than its neighbours, so it silently
    // stops sharing their centre line. This is the mistake that reads as "the text is off-centre".
    let centres = |areas: &[&str]| {
        let mut grid = Grid::new()
            .columns([Track::Px(30.0), Track::Fr(1.0)])
            .rows([Track::Auto]) // one row track, whatever the template says
            .areas(areas.iter().copied())
            .align(Align::Center)
            .area(
                Surface::new()
                    .width(Length::Px(26.0))
                    .height(Length::Px(26.0)),
                "icon",
            )
            .area(
                Surface::new()
                    .width(Length::Px(40.0))
                    .height(Length::Px(10.0)),
                "title",
            );
        LayoutEngine::new().compute(&mut grid, Size::new(200.0, 60.0));
        let mid = |i: usize| {
            let b = grid.base().children[i].base().bounds;
            b.loc.y + b.size.h / 2.0
        };
        (mid(0), mid(1))
    };

    // One line in, one row out: the icon and the title share a centre line.
    let (icon, title) = centres(&["icon title"]);
    assert!(
        (icon - title).abs() < 0.5,
        "a one-line template centres both in the same row: icon {icon}, title {title}",
    );

    // Two lines in — even with a single row *track* — gives the icon an implicit second row to span,
    // and the two centres part company.
    let (icon, title) = centres(&["icon title", "icon ."]);
    assert!(
        (icon - title).abs() > 0.5,
        "the template's second line adds an implicit row the icon spans: icon {icon}, title {title}",
    );
}

#[test]
fn grid_items_align_in_their_cell_on_both_axes() {
    use heca_grid_ui::{Grid, Track};

    // One 100×40 cell holding a 20×10 item, so the alignment is unambiguous.
    let item = || Surface::new().width(Length::Px(20.0)).height(Length::Px(10.0));
    let cell = |grid: Grid| {
        let mut grid = grid;
        LayoutEngine::new().compute(&mut grid, Size::new(100.0, 40.0));
        grid.base().children[0].base().bounds
    };

    // Default (Stretch on both axes): the item is pinned to the top-left of its cell — an explicit
    // size means there is nothing to stretch. This is why an Icon (h = font) and a Label
    // (h = font × 1.4) in the same row do NOT share a centre line by default.
    let default = cell(Grid::new()
        .columns([Track::Px(100.0)])
        .rows([Track::Px(40.0)])
        .child(item()));
    assert!(default.loc.y < 0.01, "default: pinned to the top of the cell");
    assert!(default.loc.x < 0.01, "default: pinned to the left of the cell");

    // `.align(..)` is the VERTICAL knob: it centres the items in their cells.
    let centered = cell(Grid::new()
        .columns([Track::Px(100.0)])
        .rows([Track::Px(40.0)])
        .align(Align::Center)
        .child(item()));
    assert!(
        (centered.loc.y - 15.0).abs() < 0.5,
        "align(Center) centres vertically: (40 - 10) / 2 = 15, got {}",
        centered.loc.y,
    );

    // `.justify_items(..)` is the HORIZONTAL one.
    let justified = cell(Grid::new()
        .columns([Track::Px(100.0)])
        .rows([Track::Px(40.0)])
        .justify_items(Align::Center)
        .child(item()));
    assert!(
        (justified.loc.x - 40.0).abs() < 0.5,
        "justify_items(Center) centres horizontally: (100 - 20) / 2 = 40, got {}",
        justified.loc.x,
    );

    // The per-item overrides win over the grid's defaults, one axis each.
    let overridden = cell(Grid::new()
        .columns([Track::Px(100.0)])
        .rows([Track::Px(40.0)])
        .align(Align::Center)
        .justify_items(Align::Center)
        .child(item().align_self(Align::End).justify_self(Align::End)));
    assert!(
        (overridden.loc.y - 30.0).abs() < 0.5 && (overridden.loc.x - 80.0).abs() < 0.5,
        "align_self / justify_self override the grid, got {overridden:?}",
    );

    // The trap this exists to avoid: on a grid, `justify` is `justify-content` — it distributes the
    // whole TRACK SET inside the container and does not move the item within its cell. With one
    // 100px track filling a 100px container there is nothing to distribute, so the item stays put.
    let justify_content = cell(Grid::new()
        .columns([Track::Px(100.0)])
        .rows([Track::Px(40.0)])
        .justify(Justify::Center)
        .child(item()));
    assert!(
        justify_content.loc.x < 0.01,
        "`justify` does not align items in their cells — use `justify_items`",
    );
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
    heca_grid_ui::dispatch(&mut dock, &Event::PointerPressed { pos: center });

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
    heca_grid_ui::dispatch(&mut dock, &Event::PointerPressed { pos: center });

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
        let theme = Theme::default();
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
    // Large == the reference (un-scaled) size; the explicit px is taken verbatim.
    // `set_size` (not a raw `style.size = ..`) because the variant must be marked *explicit*,
    // or the layout pass replaces it with the one inherited from the parent — here, the root
    // default. `Icon::size` is glyph pixels, so it can't be the variant builder.
    icon.base_mut().style.layout.set_size(WidgetSize::Large);
    LayoutEngine::new().compute(&mut icon, Size::new(200.0, 200.0));
    let b = icon.base().bounds;
    assert_eq!(b.size.w, 24.0, "icon width = glyph size");
    assert_eq!(b.size.h, 24.0, "icon is square");

    // The size variant scales an explicit glyph size too (so icon-only buttons
    // resize): Small renders the same icon smaller.
    let mut small = Icon::new(Glyph::GitBranch).size(24.0);
    small.base_mut().style.layout.set_size(WidgetSize::Small);
    LayoutEngine::new().compute(&mut small, Size::new(200.0, 200.0));
    assert!(small.base().bounds.size.w < 24.0, "Small scales the explicit glyph size down");
}

#[test]
fn icon_paints_duotone_layers_in_the_icon_font() {
    use heca_grid_ui::{DrawCommand, FontRole, Glyph, Icon};
    let mut icon = Icon::new(Glyph::Folder).size(24.0);
    LayoutEngine::new().compute(&mut icon, Size::new(100.0, 100.0));

    let theme = Theme::default();
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

    heca_grid_ui::dispatch(&mut row, &Event::PointerPressed { pos: outside });
    assert_eq!(clicks.get(), 0, "a click outside the row does nothing");
    heca_grid_ui::dispatch(&mut row, &Event::PointerPressed { pos: center });
    assert_eq!(clicks.get(), 1, "a click inside the row activates it");
    heca_grid_ui::dispatch(&mut row, &Event::Key { key: GridKey::Enter, pressed: true });
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
        assert!(!dock.children[0].base().style.layout.hidden, "header shown while expanded");
        assert!(!dock.children[1].base().style.layout.hidden, "body shown while expanded");
        assert!(dock.children[2].base().style.layout.hidden, "rail icon hidden while expanded");
        assert!(dock.children[1].base().bounds.size.h > 0.0, "expanded body has height");
    }

    // Collapse the region to its rail: header + body fold away; the rail icon shows.
    mode.set(RegionMode::CollapsedRail);
    LayoutEngine::new().compute(&mut sidebar, Size::new(400.0, 600.0));
    {
        let dock = sidebar.base().children[0].base();
        assert!(dock.children[0].base().style.layout.hidden, "header folds away in rail mode");
        assert!(dock.children[1].base().style.layout.hidden, "body folds away in rail mode");
        assert!(!dock.children[2].base().style.layout.hidden, "rail icon shows in rail mode");
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
        let theme = Theme::default();
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
    heca_grid_ui::dispatch(&mut cell, &Event::PointerPressed { pos: center });
    heca_grid_ui::dispatch(&mut cell, &Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(clicks.get(), 2, "click + Enter both activate the cell");
}

#[test]
fn key_hint_overlays_letter_only_when_set() {
    use heca_grid_ui::{FontRole, Glyph, Icon, KeyHint};

    let hint: Signal<Option<String>> = signal(None);
    let mut wrapped = KeyHint::new(Icon::new(Glyph::Terminal).size(20.0)).hint(hint);

    let paint = |w: &mut KeyHint| -> (Vec<String>, usize) {
        LayoutEngine::new().compute(w, Size::new(80.0, 80.0));
        let theme = Theme::default();
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
    let mut pinned = IconButton::new(Icon::new(Glyph::Gear).size(18.0)).cell(40.0);
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
    heca_grid_ui::dispatch(&mut btn, &Event::PointerPressed { pos: center });
    heca_grid_ui::dispatch(&mut btn, &Event::Key { key: heca_grid_ui::GridKey::Enter, pressed: true });
    assert_eq!(clicks.get(), 2, "click + Enter both fire on_click");
}

#[test]
fn tooltip_reveals_after_a_hover_delay_and_hides_on_leave() {
    use heca_grid_ui::Tooltip;

    // Reveal is wall-clock timed (like the Input caret), so the test sleeps past a
    // short delay rather than feeding simulated `dt`.
    let mut tip = Tooltip::new(Item::new("X"), "HELP").delay(0.05);

    // Render + report whether the bubble text was painted.
    let shows_help = |tip: &mut Tooltip| -> bool {
        LayoutEngine::new().compute(tip, Size::new(300.0, 200.0));
        let theme = Theme::default();
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
    heca_grid_ui::dispatch(&mut tip, &Event::PointerMoved { pos: center });
    assert!(!shows_help(&mut tip), "still hidden before the delay elapses");

    // Past the delay: the bubble shows.
    std::thread::sleep(std::time::Duration::from_millis(120));
    assert!(shows_help(&mut tip), "bubble reveals after the hover delay");

    // Pointer leaves: hidden again immediately.
    heca_grid_ui::dispatch(&mut tip, &Event::PointerMoved { pos: Point::new(-50.0, -50.0) });
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
    heca_grid_ui::dispatch(&mut tip, &Event::PointerMoved { pos: center });

    let theme = Theme::default();
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
fn command_palette_typing_filters_then_activate_runs_top_result() {
    use heca_grid_ui::{Component, GridKey, WidgetIntent};
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // Type "tog" → "Toggle sidebar" is the top (only) match.
    for c in "tog".chars() {
        heca_grid_ui::dispatch(&mut p, &Event::Key { key: GridKey::Char(c), pressed: true });
    }
    // Nav is host-resolved: `activate` arrives as WidgetIntent::Activate.
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), 3, "activate runs the filtered top result (Toggle sidebar)");
    assert!(!p.overlay_active(), "palette closes after running a command");
}

#[test]
fn command_palette_navigates_via_menu_nav() {
    use heca_grid_ui::WidgetIntent;
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // No query → all three; selection starts at 0. Down ×2 → idx 2, Up → idx 1.
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuDown));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::MenuUp));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), 2, "MenuDown ×2 then MenuUp lands on the 2nd command (Close pane)");
}

#[test]
fn command_palette_query_reuses_input_word_delete() {
    use heca_grid_ui::{GridKey, Modifiers, WidgetIntent};
    let (mut p, ran) = palette_with_markers();
    p = p.open(true);

    // "Toggle xyz" matches nothing (no command contains "...xyz").
    for c in "Toggle xyz".chars() {
        heca_grid_ui::dispatch(&mut p, &Event::Key { key: GridKey::Char(c), pressed: true });
    }
    // Ctrl+Backspace word-deletes the whole "xyz" (not one char), leaving
    // "Toggle " — which now matches "Toggle sidebar". A char-delete would leave
    // "Toggle xy" (still no match), so this proves the Input editing is wired.
    heca_grid_ui::dispatch(&mut p, &Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
    heca_grid_ui::dispatch(&mut p, &Event::Key { key: GridKey::Backspace, pressed: true });
    heca_grid_ui::dispatch(&mut p, &Event::ModifiersChanged(Modifiers::default()));
    heca_grid_ui::dispatch(&mut p, &Event::Widget(WidgetIntent::Activate));
    assert_eq!(ran.get(), 3, "Ctrl+Backspace word-delete leaves 'Toggle ' → runs Toggle sidebar");
}

#[test]
fn input_edit_deletes_char_and_deletes_to_line_start() {
    use heca_grid_ui::{Input, WidgetIntent};
    // The readline shortcuts are host-configured (`edit_delete_back` / `edit_delete_to_line_start`,
    // default Ctrl+h / Ctrl+u) and arrive as semantic `Edit*` intents, not a raw key.
    let mut inp = Input::new().value("hello world");

    heca_grid_ui::dispatch(&mut inp, &Event::Widget(WidgetIntent::EditDeleteBack));
    assert_eq!(inp.value_str(), "hello worl", "EditDeleteBack removes one char back");
    heca_grid_ui::dispatch(&mut inp, &Event::Widget(WidgetIntent::EditDeleteToLineStart));
    assert_eq!(inp.value_str(), "", "EditDeleteToLineStart clears to the start of the line");
}

#[test]
fn input_raw_ctrl_char_is_ignored_not_typed() {
    use heca_grid_ui::{Input, Modifiers};
    // A modified char is never typed as text — it is left for the host to resolve into an
    // `Edit*` shortcut (Ctrl+h, Ctrl+u, Ctrl/Cmd+A). The widget ignores the raw key.
    let mut inp = Input::new().value("hi");
    heca_grid_ui::dispatch(&mut inp, &Event::ModifiersChanged(Modifiers { ctrl: true, ..Default::default() }));
    assert_eq!(
        heca_grid_ui::dispatch(&mut inp, &Event::Key { key: heca_grid_ui::GridKey::Char('h'), pressed: true }),
        heca_grid_ui::Handled::No,
        "a raw Ctrl+char is not consumed by the input",
    );
    assert_eq!(inp.value_str(), "hi", "the modified char is not typed");
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
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    let dismissed = Rc::new(Cell::new(0u32));
    let d = dismissed.clone();
    let mut t = Toast::warning("Disk almost full").on_dismiss(move || d.set(d.get() + 1));
    layout_toast(&mut t);

    // The × lives in the top-right gutter (width 320, ~21px square inset by 13).
    let hit = heca_grid_ui::dispatch(&mut t, &Event::PointerPressed { pos: Point::new(296.0, 23.0) });
    assert_eq!(dismissed.get(), 1, "clicking × fires on_dismiss");
    assert!(matches!(hit, Handled::Yes), "the × consumes the click");
}

#[test]
fn toast_action_button_fires_on_action() {
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    let acted = Rc::new(Cell::new(0u32));
    let a = acted.clone();
    let mut t = Toast::info("File deleted").action("Undo", move || a.set(a.get() + 1));
    layout_toast(&mut t);

    // Action row sits below the title, left-aligned in the text column.
    heca_grid_ui::dispatch(&mut t, &Event::PointerPressed { pos: Point::new(60.0, 50.0) });
    assert_eq!(acted.get(), 1, "clicking the action button fires on_action");
}

#[test]
fn toast_body_click_fires_on_click_only_when_set() {
    use heca_grid_ui::Toast;
    use std::cell::Cell;
    use std::rc::Rc;

    // Without on_click, a body click is not consumed (it can fall through).
    let mut inert = Toast::info("Build finished").dismissible(false);
    layout_toast(&mut inert);
    let hit = heca_grid_ui::dispatch(&mut inert, &Event::PointerPressed { pos: Point::new(160.0, 20.0) });
    assert!(matches!(hit, Handled::No), "a non-clickable toast doesn't eat body clicks");

    // With on_click, the same click activates + consumes.
    let clicked = Rc::new(Cell::new(0u32));
    let c = clicked.clone();
    let mut t = Toast::info("Build finished")
        .dismissible(false)
        .on_click(move || c.set(c.get() + 1));
    layout_toast(&mut t);
    let hit = heca_grid_ui::dispatch(&mut t, &Event::PointerPressed { pos: Point::new(160.0, 20.0) });
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
    heca_grid_ui::dispatch(&mut t, &Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(clicked.get(), 1, "Enter activates a focused clickable toast");
}

// --- Button respects theme border_width + radius ----------------------------

#[test]
fn button_derives_border_width_and_radius_from_theme() {
    use heca_grid_ui::{Button, Component};

    // A theme with a distinctive radius + border width.
    let mut theme = Theme::default();
    theme.colors.border_radius = 10.0;
    theme.colors.border_width = 2.0;
    let expected_radius = theme.colors.control_radius();

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
        DrawCommand::Rect(r) if r.fill == theme.colors.surface => Some(*r),
        _ => None,
    }).expect("button paints a surface-filled background box");
    assert_eq!(bg.radius, expected_radius, "button corner radius follows theme.colors.control_radius()");
    assert_eq!(
        bg.border.expect("primary button has a border").width,
        theme.colors.border_width,
        "button border width follows theme.colors.border_width",
    );

    // border_width == 0 → no border drawn (borders off, like every surface).
    theme.colors.border_width = 0.0;
    let mut btn = Button::primary("OK");
    LayoutEngine::new().base_font(theme.font_size).compute(&mut btn, Size::new(300.0, 80.0));
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        btn.paint(&mut cx);
    }
    let bg = scene.iter().find_map(|cmd| match cmd {
        DrawCommand::Rect(r) if r.fill == theme.colors.surface => Some(*r),
        _ => None,
    }).expect("button still paints its background box");
    assert!(bg.border.is_none(), "border_width == 0 means no button border");
}

// --- border_width == 0 ⇒ no borders anywhere (bracket_frame draws nothing) ----

#[test]
fn bracket_frame_zero_border_draws_nothing_nonzero_draws_reticle() {
    // `border_width == 0` means borders off everywhere — `bracket_frame` draws
    // NOTHING (no hairline). A container that needs definition at 0 carries a fill,
    // not a forced border. This keeps the bracket frame consistent with every other
    // widget's border gate.
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0;
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 120.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect);
    }
    assert!(scene.is_empty(), "border_width == 0 draws no frame at all");

    // With a real border: one dimmed continuous accent line tracing the perimeter,
    // plus four bright accent corners — each redrawn clipped to its corner box.
    theme.colors.border_width = 2.0;
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame(rect);
    }
    let bright_corners = scene.iter().filter(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color == theme.colors.accent && b.width > 0.0)
    )).count();
    let dim_line = scene.iter().any(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color.a < theme.colors.accent.a && b.width > 0.0)
    ));
    let clips = scene.iter().filter(|c| matches!(c, DrawCommand::PushClip(_))).count();
    assert_eq!(bright_corners, 4, "border>0 draws four bright accent corner brackets");
    assert!(dim_line, "border>0 traces a dimmed continuous accent line under the corners");
    assert_eq!(clips, 4, "each bright corner is clipped to its own corner box");
}

#[test]
fn bracket_frame_with_honors_explicit_width_independent_of_theme() {
    // `bracket_frame_with` sizes the reticle from the passed width/radius, not the
    // theme — the seam that lets a bracketed sidebar honor `sidebar_border_width`
    // even when the global/theme border is 0. (Issue 1.)
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0; // global borders OFF
    let rect = Rectangle::new(Point::new(10.0, 10.0), Size::new(200.0, 120.0));

    // Theme says 0, but an explicit width of 3 still draws the reticle.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame_with(rect, 3.0, 8.0);
    }
    let bright_corners = scene.iter().filter(|c| matches!(
        c, DrawCommand::Rect(r) if r.border.is_some_and(|b| b.color == theme.colors.accent && b.width > 0.0)
    )).count();
    assert_eq!(bright_corners, 4, "explicit width draws the reticle even when theme.colors.border_width == 0");

    // An explicit width of 0 draws nothing, regardless of the theme.
    theme.colors.border_width = 5.0;
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.bracket_frame_with(rect, 0.0, 8.0);
    }
    assert!(scene.is_empty(), "explicit width 0 draws no reticle even with theme border on");
}

#[test]
fn bordered_pane_border_width_follows_theme_and_vanishes_at_zero() {
    // A default `Bordered` Pane with NO explicit `.border()` derives its border
    // from `theme.colors.border_width` (the global border control): a theme-colored
    // border when borders are on, and nothing at `border_width == 0`. This is the
    // consistency contract — the global control governs every container.
    use heca_grid_ui::{Component, Pane};
    // `explicit_border` width is the literal a caller passes to `.border()` — it must
    // be ignored in favour of the live theme width, so a build-time literal can't
    // survive a global border change (the showcase bug). `None` ⇒ no `.border()`.
    let border_rects = |theme: &Theme, explicit: Option<(heca_grid_ui::Color, f32)>| -> Vec<heca_grid_ui::scene::RectCmd> {
        let mut p = Pane::new()
            .background(theme.colors.surface)
            .width(Length::Px(120.0))
            .height(Length::Px(80.0));
        if let Some((c, w)) = explicit {
            p = p.border(c, w);
        }
        LayoutEngine::new().compute(&mut p, Size::new(200.0, 200.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, theme);
            p.paint(&mut cx);
        }
        scene.iter().filter_map(|c| match c {
            DrawCommand::Rect(r) if r.border.is_some_and(|b| b.width > 0.0) => Some(*r),
            _ => None,
        }).collect()
    };

    let mut theme = Theme::default();
    theme.colors.border_width = 2.0;
    let on = border_rects(&theme, None);
    assert!(
        on.iter().any(|r| r.border.is_some_and(|b| b.color == theme.colors.border && b.width == 2.0)),
        "Bordered pane draws theme.colors.border at theme.colors.border_width without an explicit .border()",
    );

    // An explicit `.border(accent, 9.0)` keeps the COLOR but the width follows the
    // theme (2.0), never the 9.0 literal.
    let explicit = border_rects(&theme, Some((theme.colors.accent, 9.0)));
    assert!(
        explicit.iter().any(|r| r.border.is_some_and(|b| b.color == theme.colors.accent && b.width == 2.0)),
        "explicit .border() supplies color only; width tracks theme.colors.border_width",
    );

    theme.colors.border_width = 0.0;
    assert!(
        border_rects(&theme, None).is_empty() && border_rects(&theme, Some((theme.colors.accent, 9.0))).is_empty(),
        "border_width == 0 leaves the Bordered pane with no visible border, even with an explicit .border()",
    );
}

#[test]
fn bordered_pane_border_width_override_is_independent_of_theme() {
    // `.border_width(w)` pins a Bordered pane's frame width regardless of the
    // global `theme.colors.border_width` — the seam that lets the sidebar shell carry its
    // own thickness (`[appearance] sidebar_border_width`). Color still resolves
    // from `.border(color, _)` when set, else `theme.colors.border`.
    use heca_grid_ui::{Color, Component, Pane};
    let border_rects = |theme: &Theme, override_w: Option<f32>, explicit: Option<Color>| -> Vec<heca_grid_ui::scene::RectCmd> {
        let mut p = Pane::new()
            .background(theme.colors.surface)
            .border_width(override_w)
            .width(Length::Px(120.0))
            .height(Length::Px(80.0));
        if let Some(c) = explicit {
            p = p.border(c, 0.0);
        }
        LayoutEngine::new().compute(&mut p, Size::new(200.0, 200.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, theme);
            p.paint(&mut cx);
        }
        scene.iter().filter_map(|c| match c {
            DrawCommand::Rect(r) if r.border.is_some_and(|b| b.width > 0.0) => Some(*r),
            _ => None,
        }).collect()
    };

    let mut theme = Theme::default();

    // Theme borders OFF, but the override forces a 3px frame in theme.colors.border.
    theme.colors.border_width = 0.0;
    let forced = border_rects(&theme, Some(3.0), None);
    assert!(
        forced.iter().any(|r| r.border.is_some_and(|b| b.color == theme.colors.border && b.width == 3.0)),
        "override draws its own width even when the global border is off",
    );

    // Override width + explicit color: width = override, color = explicit.
    let colored = border_rects(&theme, Some(3.0), Some(theme.colors.accent));
    assert!(
        colored.iter().any(|r| r.border.is_some_and(|b| b.color == theme.colors.accent && b.width == 3.0)),
        "override sets width; explicit .border() sets color",
    );

    // override = 0 ⇒ no border, even with the global border ON.
    theme.colors.border_width = 5.0;
    assert!(
        border_rects(&theme, Some(0.0), None).is_empty(),
        "override of 0 removes the border regardless of the global width",
    );
}

/// Paint `w` under `border_width == 0` and return every visible (width>0) Rect
/// border stroke it emitted.
fn visible_border_widths_at_zero<C: heca_grid_ui::Component>(mut w: C) -> Vec<f32> {
    let mut theme = Theme::default();
    theme.colors.border_width = 0.0;
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

// --- drop shadow ------------------------------------------------------------

#[test]
fn drop_shadow_emits_a_shadow_rect_and_respects_zero_alpha() {
    use heca_grid_ui::scene::Shadow;
    let theme = Theme::default();
    let rect = Rectangle::new(Point::new(50.0, 50.0), Size::new(120.0, 80.0));

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.drop_shadow(rect, 8.0, Shadow { color: theme.shadow_color(), radius: 24.0, dx: 0.0, dy: 10.0 });
    }
    let sh = scene.iter().find_map(|c| match c {
        DrawCommand::Rect(r) => r.shadow,
        _ => None,
    }).expect("drop_shadow emits a rect carrying a Shadow");
    assert_eq!((sh.radius, sh.dy), (24.0, 10.0), "shadow blur + offset are threaded through");

    // A fully-transparent shadow (alpha 0) or zero radius is a no-op.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.drop_shadow(rect, 8.0, Shadow { color: theme.shadow_color().with_alpha(0), radius: 24.0, dx: 0.0, dy: 10.0 });
    }
    assert!(scene.is_empty(), "a zero-alpha shadow draws nothing (shadows-off)");
}

#[test]
fn toast_action_press_flashes_only_the_action_not_the_whole_card() {
    use heca_grid_ui::Toast;
    let theme = Theme::default();

    // Press the Retry action, then paint: the press flash must cover only the
    // action button, not the whole card (no "whole widget clicked" feedback).
    let mut t = Toast::info("File deleted").action("Retry", || {});
    layout_toast(&mut t);
    let card_w = t.base().bounds.size.w;
    heca_grid_ui::dispatch(&mut t, &Event::PointerPressed { pos: Point::new(60.0, 50.0) });

    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        t.paint(&mut cx);
    }
    // The flash is drawn in the theme's foreground color (see PaintCx::flash).
    let fg = theme.colors.foreground;
    let flash = scene.iter().find_map(|c| match c {
        DrawCommand::Rect(r) if r.fill.r == fg.r && r.fill.g == fg.g && r.fill.b == fg.b && r.fill.a > 0 => Some(*r),
        _ => None,
    }).expect("an action press emits a press-flash rect");
    assert!(
        flash.rect.size.w < card_w - 1.0,
        "action flash ({}) must be narrower than the whole card ({card_w})",
        flash.rect.size.w,
    );
}

// --- ToastStack -------------------------------------------------------------

#[test]
fn toast_stack_is_overlay_active_only_when_it_has_toasts() {
    use heca_grid_ui::{Component, ToastSpec, ToastStack};

    let items = signal(Vec::<ToastSpec>::new());
    let mut stack = ToastStack::new(items);
    stack.tick(0.0); // reconcile (empty)
    assert!(!stack.overlay_active(), "empty stack doesn't grab input");

    items.set(vec![ToastSpec::new(1, "Saved"), ToastSpec::new(2, "Done")]);
    stack.tick(0.0); // reconcile (now 2)
    assert!(stack.overlay_active(), "a non-empty stack is overlay-active");
}

#[test]
fn toast_stack_dismiss_reports_the_clicked_id() {
    use heca_grid_ui::{Component, ToastCorner, ToastSpec, ToastStack};
    use std::cell::Cell;
    use std::rc::Rc;

    let dismissed = Rc::new(Cell::new(0u64));
    let d = dismissed.clone();
    let items = signal(vec![ToastSpec::new(7, "Connection lost").body("Retrying")]);
    let mut stack = ToastStack::new(items)
        .corner(ToastCorner::TopLeft)
        .on_dismiss(move |id| d.set(id));

    // Settle the slide-in, then paint to cache the viewport + lay the toast out.
    stack.tick(1.0);
    let theme = Theme::default();
    let vp = Size::new(800.0, 600.0);
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        stack.paint(&mut cx);
    }

    // Top-left toast sits at (16,16), width 320; its × is in the top-right gutter.
    let hit = heca_grid_ui::dispatch(&mut stack, &Event::PointerPressed { pos: Point::new(310.0, 38.0) });
    assert!(matches!(hit, Handled::Yes), "a click on a toast's × is consumed");
    assert_eq!(dismissed.get(), 7, "the dismissed toast's id is reported to the host");
}

#[test]
fn toast_stack_passes_through_clicks_that_miss_every_toast() {
    use heca_grid_ui::{Component, ToastCorner, ToastSpec, ToastStack};

    let items = signal(vec![ToastSpec::new(1, "Hi")]);
    let mut stack = ToastStack::new(items).corner(ToastCorner::TopLeft);
    stack.tick(1.0);
    let theme = Theme::default();
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(800.0, 600.0));
        stack.paint(&mut cx);
    }
    // Far from the top-left toast → not consumed, so the UI behind still gets it.
    let hit = heca_grid_ui::dispatch(&mut stack, &Event::PointerPressed { pos: Point::new(700.0, 500.0) });
    assert!(matches!(hit, Handled::No), "clicks that miss every toast pass through");
}

// --- viewport culling -------------------------------------------------------

#[test]
fn paint_cx_culls_offscreen_content_but_not_headless() {
    let theme = Theme::default();
    let vp = Size::new(800.0, 600.0);
    let off = Rectangle::new(Point::new(10.0, 5000.0), Size::new(100.0, 40.0)); // far below
    let on = Rectangle::new(Point::new(10.0, 10.0), Size::new(100.0, 40.0));

    // On-screen content is painted.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        cx.rect(on, theme.colors.surface, None, 0.0, None);
    }
    assert_eq!(scene.len(), 1, "on-screen rect is painted");

    // Content fully outside the viewport (rect + text) emits nothing.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(vp);
        cx.rect(off, theme.colors.surface, None, 0.0, None);
        cx.text(off, "hidden", theme.colors.foreground, 15.0, TextAlign::Start, TextStyle::REGULAR);
    }
    assert!(scene.is_empty(), "content far below the viewport is culled");

    // With no viewport set (headless / tests) nothing is ever culled.
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, &theme);
        cx.rect(off, theme.colors.surface, None, 0.0, None);
    }
    assert_eq!(scene.len(), 1, "no viewport ⇒ no culling (headless default)");
}

#[test]
fn with_clip_wraps_body_draws_in_push_and_pop_clip() {
    use heca_grid_ui::PaintCx;

    let theme = Theme::default();
    let mut scene = Scene::new();
    let clip = Rectangle::new(Point::new(0.0, 0.0), Size::new(50.0, 50.0));
    {
        let mut cx = PaintCx::new(&mut scene, &theme).with_viewport(Size::new(100.0, 100.0));
        cx.with_clip(clip, |cx| {
            cx.rect(
                Rectangle::new(Point::new(5.0, 5.0), Size::new(10.0, 10.0)),
                theme.colors.surface,
                None,
                0.0,
                None,
            );
        });
    }

    let cmds: Vec<&DrawCommand> = scene.iter().collect();
    assert!(
        matches!(cmds.first(), Some(DrawCommand::PushClip(r)) if *r == clip),
        "the body is opened by a PushClip carrying the clip rect"
    );
    assert!(
        matches!(cmds.last(), Some(DrawCommand::PopClip)),
        "the clip is popped after the body"
    );
    assert!(
        cmds.iter().any(|c| matches!(c, DrawCommand::Rect(_))),
        "the clipped rect sits between the push and pop"
    );
}

#[test]
fn focused_input_requests_a_timed_caret_redraw_not_continuous() {
    use heca_grid_ui::{Component, FocusManager, Input};

    let mut input = Input::new().value("hi");
    LayoutEngine::new().compute(&mut input, Size::new(200.0, 60.0));

    // The caret is never a continuous animation: tick reports no animating frame.
    assert!(!input.tick(0.016), "an input never drives the continuous redraw loop");
    // Unfocused: nothing to redraw on a timer.
    assert_eq!(input.next_redraw(), None, "an unfocused input asks for no timed redraw");

    // Focused: it schedules a wake at its next caret toggle (within a half period),
    // so the host sleeps until then instead of redrawing every frame.
    let mut focus = FocusManager::new();
    focus.advance(&mut input, true);
    let nr = input
        .next_redraw()
        .expect("a focused input schedules a timed caret redraw");
    assert!(
        nr > 0.0 && nr <= BLINK_PERIOD_HALF + 1e-3,
        "caret wake is within the half blink period, got {nr}"
    );
}
const BLINK_PERIOD_HALF: f32 = 0.5;

#[test]
fn collect_damage_unions_dirty_widgets_then_clears_flags() {
    use heca_grid_ui::{Component, collect_damage};

    let mut ui = Flex::row()
        .child(Button::primary("A"))
        .child(Button::secondary("B"));
    LayoutEngine::new().compute(&mut ui, Size::new(400.0, 100.0));

    // A fresh tree needs its first paint; collecting reports damage and clears flags.
    assert!(collect_damage(&ui).is_some(), "a fresh tree needs its first paint");
    assert!(
        collect_damage(&ui).is_none(),
        "flags cleared → no damage on the next collect"
    );

    // Marking one widget dirty → damage covers (at least) that widget's bounds.
    let b = ui.base().children[1].base().bounds;
    ui.base().children[1].base().mark_needs_paint();
    let d = collect_damage(&ui).expect("a marked widget reports damage");
    assert!(
        d.loc.x <= b.loc.x && d.loc.x + d.size.w >= b.loc.x + b.size.w,
        "damage horizontally covers the marked widget"
    );
}

// ── Button: content is composed from children (viewnode-all-widgets) ────────────────────────

/// The sugar builders are exactly that: they build **children**. There is no separate "simple
/// mode" — `Button::new(..)` and `.icon(..)` produce the same child vector a caller (or the
/// `ViewNode` mapper) would compose by hand, so there is one layout and one paint path.
#[test]
fn button_sugar_desugars_into_children() {
    assert_eq!(Button::empty().base().children.len(), 0, "empty button has no content");
    assert_eq!(Button::new("Delete").base().children.len(), 1, "label sugar → one Label child");

    let with_icon = Button::new("Delete").icon(Glyph::Trash);
    assert_eq!(with_icon.base().children.len(), 2, "icon sugar prepends → [Icon, Label]");

    // Arbitrary content, any depth — the same vector, composed instead of sugared.
    let composed = Button::empty().child(
        Flex::column()
            .child(Flex::row().child(Icon::new(Glyph::Trash)).child(Label::new("Delete")))
            .child(Label::new("Ctrl+D")),
    );
    assert_eq!(composed.base().children.len(), 1, "one composed subtree");
    assert_eq!(
        composed.base().children[0].base().children.len(),
        2,
        "the subtree keeps its own structure (row + accelerator label)",
    );
}

/// The button no longer computes its own width from a character count — it **hugs its content**
/// and taffy measures it. The resulting box must still match the old hand-computed geometry
/// exactly: label (`chars × font × advance`) + one character of breathing room per side + the
/// size-scaled base padding.
#[test]
fn button_hugs_its_content_at_the_historical_size() {
    use heca_grid_ui::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
    const BASE_PAD: f32 = 10.0; // Button::BASE_PAD (private)

    let mut b = Button::new("DELETE").size(WidgetSize::Large); // Large ⇒ pad_scale == 1.0
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
    let fs = b.base().font;
    let bounds = b.base().bounds;

    let chars = "DELETE".chars().count() as f32;
    let expected_w = (chars + 2.0) * fs * MONO_ADVANCE_RATIO + BASE_PAD * 2.0;
    let expected_h = fs * MONO_LINE_RATIO + BASE_PAD * 2.0;
    assert!(
        (bounds.size.w - expected_w as f64).abs() < 0.5,
        "width hugs content at the historical size: got {}, want {expected_w}",
        bounds.size.w,
    );
    assert!(
        (bounds.size.h - expected_h as f64).abs() < 0.5,
        "height hugs content at the historical size: got {}, want {expected_h}",
        bounds.size.h,
    );
}

/// A button's content is sized by the tree, so richer content makes the button grow — the thing a
/// hand-computed, label-only width could never do.
#[test]
fn button_grows_to_fit_composed_content() {
    let measure = |mut b: Button| {
        LayoutEngine::new().compute(&mut b, Size::new(500.0, 200.0));
        b.base().bounds.size
    };
    let plain = measure(Button::new("Delete"));
    let with_icon = measure(Button::new("Delete").icon(Glyph::Trash));
    let stacked = measure(Button::empty().child(
        Flex::column().child(Label::new("Delete")).child(Label::new("Ctrl+D")),
    ));

    assert!(with_icon.w > plain.w, "a leading icon widens the button");
    assert!(stacked.h > plain.h, "a two-line column makes the button taller");
}

/// The size variant cascades into composed content: a `Small` button's `Label` must shrink with
/// it. Before the layout pass inherited the variant, a child kept the default and a small button
/// rendered full-size text.
#[test]
fn button_size_variant_cascades_to_composed_content() {
    let label_font = |size: WidgetSize| {
        let mut b = Button::new("SAVE").icon(Glyph::Check).size(size);
        LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
        // children = [Icon, Label]; read the label's resolved font.
        b.base().children[1].base().font
    };
    assert!(
        label_font(WidgetSize::Small) < label_font(WidgetSize::Large),
        "the button's size variant reaches its composed Label",
    );

    // An explicit choice on the child wins over the inherited one.
    let mut b = Button::new("SAVE").size(WidgetSize::Small);
    b.base_mut().children[0].base_mut().style.layout.set_size(WidgetSize::Large);
    LayoutEngine::new().compute(&mut b, Size::new(400.0, 200.0));
    let pinned = b.base().children[0].base().font;
    assert!(pinned > label_font(WidgetSize::Small), "an explicit child variant is not overwritten");
}

/// Composed content inherits the button's **state color**: the button publishes one color per
/// frame and unstyled children pick it up, which is what makes them animate with the hover sweep
/// and fade when disabled — with no wiring between the two widgets.
#[test]
fn composed_content_inherits_the_buttons_state_color() {
    let theme = Theme::default();

    // Disabled ⇒ the content fades to `muted` at reduced alpha, on every variant.
    let disabled = button_label_color(&mut Button::primary("OK").disabled(true), &theme);
    assert_eq!(disabled.r, theme.colors.muted.r, "disabled content takes the muted tone");
    assert!(disabled.a < 255, "disabled content is faded");

    // Enabled ⇒ the variant's own tone, not the theme foreground.
    let enabled = button_label_color(&mut Button::primary("OK"), &theme);
    assert_ne!(enabled, disabled, "enabled and disabled content differ");

    // An explicit child color opts out of inheritance entirely.
    let pinned = button_label_color(
        &mut Button::empty().child(Label::new("OK").color(theme.colors.success)),
        &theme,
    );
    assert_eq!(pinned, theme.colors.success, "an explicit child color wins over the inherited one");
}

/// A control is **one** Tab stop, whatever it composes. Focus traversal must not descend into a
/// button's content — otherwise a focusable child would take its own Tab stop while being
/// click-dead (the button consumes the press and never routes it to children).
#[test]
fn a_button_is_one_tab_stop_whatever_it_contains() {
    let mut ui = Flex::row()
        .child(Button::new("A").child(Toggle::new())) // an interactive child, as decoration
        .child(Button::new("B"));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    let first = focus.focused();
    focus.advance(&mut ui, true);
    let second = focus.focused();
    focus.advance(&mut ui, true);

    assert_eq!(focus.focused(), first, "exactly two focusables: focus wraps after the 2nd button");
    assert_ne!(first, second, "each button is its own (single) Tab stop");
}

// ── Choice: the value-carrying, content-composable option primitive (viewnode-choice) ────────

/// A `Choice` carries a **value** (what it means) independently of its **content** (what it shows).
/// The `labeled` sugar builds exactly the child a caller would compose by hand — one content model.
#[test]
fn choice_carries_a_value_and_composes_its_content() {
    let sugar = Choice::labeled("high", "HIGH");
    assert_eq!(sugar.value(), "high", "the value is what the option means");
    assert_eq!(sugar.base().children.len(), 1, "labeled sugar → one Label child");

    // Composed: arbitrary content, and the value is unchanged by it.
    let composed = Choice::new("high").child(
        Flex::row().child(Icon::new(Glyph::Warning)).child(Label::new("HIGH")),
    );
    assert_eq!(composed.value(), "high");
    assert_eq!(composed.base().children.len(), 1, "one composed subtree");
    assert_eq!(composed.base().children[0].base().children.len(), 2, "icon + label inside");

    // Empty is legal — content is the caller's business.
    assert_eq!(Choice::new("v").base().children.len(), 0);
}

/// The option's **state color is inherited** by its unstyled content: a selected option's label
/// turns accent without the label knowing anything about selection.
#[test]
fn choice_content_inherits_the_selected_state_color() {
    let theme = Theme::default();
    let color_of = |mut c: Choice| -> Color {
        LayoutEngine::new().compute(&mut c, Size::new(200.0, 60.0));
        let mut scene = Scene::new();
        {
            let mut cx = PaintCx::new(&mut scene, &theme);
            c.paint(&mut cx);
        }
        scene
            .iter()
            .find_map(|cmd| match cmd {
                DrawCommand::Text(t) => Some(t.color),
                _ => None,
            })
            .expect("the option paints its composed label")
    };

    assert_eq!(color_of(Choice::labeled("a", "A").selected(true)), theme.colors.accent);
    assert_eq!(color_of(Choice::labeled("a", "A")), theme.colors.foreground);
    // An explicit child color opts out of the inheritance.
    let pinned = color_of(
        Choice::new("a").child(Label::new("A").color(theme.colors.danger)).selected(true),
    );
    assert_eq!(pinned, theme.colors.danger, "an explicit child color wins");
}

/// An option is **one** Tab stop, whatever it composes — its content is content, not focus targets.
#[test]
fn a_choice_is_one_tab_stop_whatever_it_contains() {
    let mut ui = Flex::row()
        .child(Choice::labeled("a", "A").child(Toggle::new()).on_activate(|| {}))
        .child(Choice::labeled("b", "B").on_activate(|| {}));

    let mut focus = FocusManager::new();
    focus.advance(&mut ui, true);
    let first = focus.focused();
    focus.advance(&mut ui, true);
    assert_ne!(focus.focused(), first, "each option is its own Tab stop");
    focus.advance(&mut ui, true);
    assert_eq!(focus.focused(), first, "exactly two focusables — focus wraps");
}

/// Containers must resolve a pick from the children's **real bounds**, never from row arithmetic,
/// so what is drawn and what is clickable can never disagree.
#[test]
fn choice_at_resolves_a_pick_from_real_bounds() {
    let mut list = Flex::column()
        .child(Choice::labeled("a", "AAA").on_activate(|| {}))
        .child(Choice::labeled("b", "BBB").on_activate(|| {}));
    LayoutEngine::new().compute(&mut list, Size::new(200.0, 200.0));

    let second = list.base().children[1].base().bounds;
    let inside_second = Point::new(second.loc.x + 2.0, second.loc.y + 2.0);
    assert_eq!(heca_grid_ui::widgets::choice_at(&list.base().children, inside_second), Some(1));

    let miss = Point::new(second.loc.x - 50.0, second.loc.y - 500.0);
    assert_eq!(heca_grid_ui::widgets::choice_at(&list.base().children, miss), None, "a miss picks nothing");
}

/// Whole-page scroll premise (T009): a root `ScrollRegion` sized to the viewport,
/// holding a natural-width page column (`align(Start)`, width `Auto`), must report
/// horizontal overflow measured from that DIRECT child when a grandchild row is
/// wider than the viewport — with `flex_shrink: 0` the column adopts its widest
/// child instead of being clamped to the available width.
#[test]
fn root_scroll_region_sees_horizontal_overflow_through_a_natural_width_page() {
    let page = Flex::column()
        .align(Align::Center)
        .child(fixed_box(300.0, 20.0)) // wider than the 100px viewport
        .child(fixed_box(50.0, 20.0));
    let mut root = ScrollRegion::new()
        .both()
        .align(Align::Start) // don't stretch the page to the viewport width
        .width(Length::Px(100.0))
        .height(Length::Px(100.0))
        .child(page);

    LayoutEngine::new().compute(&mut root, Size::new(100.0, 100.0));

    let page_w = root.base().children[0].base().bounds.size.w;
    assert!(
        page_w >= 300.0,
        "page column adopts its widest child (got {page_w}), not the viewport width"
    );
    // The region measures overflow from its direct child → horizontal scrolling works.
    assert!(
        root.scroll_to_x(10_000.0) > 0.0,
        "root region reports a positive max horizontal offset"
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
    focus.dispatch(&mut ui, &Event::PointerPressed { pos: center });
    let a = &ui.base().children[0];
    assert!(a.base().focused.get_untracked(), "click focuses the widget");
    assert!(
        !a.base().shows_focus_ring(),
        "mouse focus is not focus-visible — no ring"
    );

    // Keyboard navigation: the newly-focused widget rings.
    focus.advance(&mut ui, true);
    assert!(
        ui.base().children.iter().any(|c| c.base().shows_focus_ring()),
        "keyboard focus (advance) shows the ring"
    );
}
