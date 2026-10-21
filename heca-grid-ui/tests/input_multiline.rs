mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, KeyPress, Keymap, LayoutEngine, Size};
use heca_grid_ui::event::{RawPointer, RawPointerKind};

fn layout(input: &mut Input, theme: &Theme) {
    LayoutEngine::new().theme(theme).compute(input, Size::new(600.0, 400.0));
}

fn press(input: &mut Input, key: GridKey, mods: Modifiers) {
    heca_grid_ui::dispatch(input, &Event::ModifiersChanged(mods));
    Keymap::with_defaults().deliver_press(&KeyPress { key, text: None, mods }, |event| {
        heca_grid_ui::dispatch(input, event)
    });
    heca_grid_ui::dispatch(input, &Event::ModifiersChanged(Modifiers::default()));
}

#[test]
fn both_modes_keep_configured_editing_shortcuts_and_selection_replacement() {
    for multiline in [false, true] {
        let mut input = Input::new().multiline(multiline).value("alpha beta");
        common::give_keyboard(&mut input);
        layout(&mut input, &Theme::default());
        let ctrl = Modifiers { ctrl: true, ..Default::default() };
        let meta = Modifiers { meta: true, ..Default::default() };
        press(&mut input, GridKey::Char('h'), ctrl);
        assert_eq!(input.value_str(), "alpha bet");
        press(&mut input, GridKey::Char('u'), ctrl);
        assert_eq!(input.value_str(), "");
        common::type_text(&mut input, if multiline { "one\ntwo" } else { "one two" });
        let value = input.value_str();
        for modifier in [meta, ctrl] {
            press(&mut input, GridKey::Char('a'), modifier);
            assert_eq!(input.selected_text(), Some(value.clone()));
        }
        common::type_text(&mut input, "replacement");
        assert_eq!(input.value_str(), "replacement");
        press(&mut input, GridKey::ArrowLeft, Modifiers { shift: true, ..Default::default() });
        assert_eq!(input.selected_text().as_deref(), Some("t"));
        press(&mut input, GridKey::Backspace, Modifiers::default());
        assert_eq!(input.value_str(), "replacemen");
    }
}

#[test]
fn multiline_caret_and_select_all_match_the_painted_text_at_every_size() {
    use heca_grid_ui::font::{MONO_ADVANCE_RATIO, MONO_LINE_RATIO};
    let mut theme = Theme::default();
    theme.colors.input.blink_period = 0.0;
    for width in [110.0, 240.0] {
        for size in [WidgetSize::Small, WidgetSize::Normal, WidgetSize::Large] {
            let mut input = Input::new().multiline(true).size(size).width(width)
                .value("fsdafads  afdsfsad f afs\na afsd");
            common::give_keyboard(&mut input);
            layout(&mut input, &theme);
            let scene = common::paint(&input, &theme);
            let runs = common::texts(&scene);
            let last = runs.last().unwrap();
            assert_eq!(last.size, input.base().font, "text and editing geometry share the resolved font");
            let caret = input.base().children[0].base().children[0].base().children[1].base().bounds;
            let cell = (last.size * MONO_ADVANCE_RATIO) as f64;
            let end = last.rect.loc.x + last.text.chars().count() as f64 * cell;
            assert!((caret.loc.x - end).abs() < 1.0, "caret {:?}, last run {:?}", caret, last);
            press(&mut input, GridKey::Char('a'), Modifiers { meta: true, ..Default::default() });
            layout(&mut input, &theme);
            let scene = common::paint(&input, &theme);
            let selected: Vec<_> = scene.iter().filter_map(|draw| match draw {
                DrawCommand::Rect(rect) if rect.fill == theme.colors.accent.with_alpha(theme.colors.interaction.selection) => Some(rect.rect),
                _ => None,
            }).collect();
            assert_eq!(selected.len(), runs.len());
            for (rect, run) in selected.iter().zip(&runs) {
                assert!((rect.loc.x - run.rect.loc.x).abs() < 1.0);
                assert!((rect.loc.y - run.rect.loc.y).abs() < 1.0);
                assert!((rect.size.w - run.text.chars().count() as f64 * cell).abs() < 1.0);
                assert!((rect.size.h - (run.size * MONO_LINE_RATIO) as f64).abs() < 1.0);
            }
        }
    }
}

#[test]
fn multiline_honors_max_height_and_keeps_the_caret_inside_its_scroll_viewport() {
    let theme = Theme::default();
    let mut input = Input::new().multiline(true).max_lines(20).max_height(70.0)
        .value("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight");
    common::give_keyboard(&mut input);
    layout(&mut input, &theme);
    let outer = input.base().bounds;
    let viewport = input.base().children[0].base().bounds;
    let caret = input.base().children[0].base().children[0].base().children[1].base().bounds;
    assert!(outer.size.h <= 70.0);
    assert!(viewport.loc.y + viewport.size.h <= outer.loc.y + outer.size.h);
    assert!(caret.loc.y >= viewport.loc.y);
    assert!(caret.loc.y + caret.size.h <= viewport.loc.y + viewport.size.h + 1.0);
}

#[test]
fn multiline_scrollbar_drag_reaches_the_scroll_region_without_moving_the_caret() {
    let theme = Theme::default();
    let mut input = Input::new().multiline(true).max_lines(2)
        .value("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight");
    common::give_keyboard(&mut input);
    layout(&mut input, &theme);
    let viewport = input.base().children[0].base().bounds;
    let content_y = input.base().children[0].base().children[0].base().bounds.loc.y;
    let scene = common::paint(&input, &theme);
    let thumb = common::rects(&scene).into_iter().find(|rect| {
        rect.rect.loc.x > viewport.loc.x + viewport.size.w / 2.0
            && rect.rect.size.w < 10.0 && rect.rect.size.h > 5.0
    }).expect("overflow draws a scrollbar thumb").rect;
    let from = heca_grid_ui::Point::new(thumb.loc.x + thumb.size.w / 2.0, thumb.loc.y + thumb.size.h / 2.0);
    let to = heca_grid_ui::Point::new(from.x, viewport.loc.y);
    common::press_at(&mut input, from, PointerButton::Left);
    heca_grid_ui::dispatch(&mut input, &Event::Raw(RawPointer::new(RawPointerKind::Moved, to)));
    common::release_at(&mut input, to, PointerButton::Left);
    assert!(input.base().children[0].base().children[0].base().bounds.loc.y > content_y,
        "dragging the thumb scrolls toward the beginning");
    common::type_text(&mut input, "X");
    assert!(input.value_str().ends_with("eightX"), "a scrollbar press does not move the text caret");
}
