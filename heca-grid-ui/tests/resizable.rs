mod common;

use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, Point, Resizable, Size};

fn layout(widget: &mut dyn Component) {
    LayoutEngine::new().theme(&Theme::default()).compute(widget, Size::new(500.0, 500.0));
}

fn drag(widget: &mut Resizable, delta: f64) {
    drag_in(widget, delta, &Theme::default());
}

fn drag_in(widget: &mut Resizable, delta: f64, theme: &Theme) {
    let handle = widget.base().children[1].base().children[1].base().bounds;
    let from = Point::new(handle.loc.x + handle.size.w / 2.0, handle.loc.y + handle.size.h / 2.0);
    let to = Point::new(from.x, from.y + delta);
    common::press_at(widget, from, PointerButton::Left);
    heca_grid_ui::dispatch(widget, &Event::pointer_moved(to));
    widget.tick(0.0);
    assert!(heca_grid_ui::needs_layout(widget));
    LayoutEngine::new().theme(theme).compute(widget, Size::new(500.0, 500.0));
    common::release_at(widget, to, PointerButton::Left);
}

#[test]
fn an_input_cannot_be_dragged_below_one_readable_line_at_any_theme_font() {
    for multiline in [false, true] {
        let mut input = Input::new().multiline(multiline).value("readable");
        common::give_keyboard(&mut input);
        let mut widget = Resizable::new(input).width(220.0);
        for font in [12.0, 24.0] {
            let theme = Theme { font_size: font, ..Default::default() };
            LayoutEngine::new().theme(&theme).compute(&mut widget, Size::new(500.0, 500.0));
            drag_in(&mut widget, -1000.0, &theme);
            let input = widget.base().children[0].base();
            let viewport = input.children[0].base().bounds;
            let caret = input.children[0].base().children[0].base().children[1].base().bounds;
            assert!(viewport.size.h + 1.0 >= (input.font * heca_grid_ui::font::MONO_LINE_RATIO) as f64,
                "a complete text line fits: font {}, viewport {:?}", input.font, viewport);
            assert!(caret.loc.y >= viewport.loc.y);
            assert!(caret.loc.y + caret.size.h <= viewport.loc.y + viewport.size.h + 1.0);
            assert!(input.bounds.size.h <= widget.base().bounds.size.h);
        }
    }
}

#[test]
fn resizing_respects_both_limits_and_keeps_the_editor_and_scrollbar_alive() {
    let mut input = Input::new().multiline(true).value("one\ntwo\nthree\nfour\nfive\nsix");
    let text = input.text();
    common::give_keyboard(&mut input);
    let mut widget = Resizable::new(input).width(220.0).min_height(70.0).max_height(160.0);
    layout(&mut widget);
    let footer = widget.base().children[1].base();
    let grip = footer.children[1].base().bounds;
    assert!(grip.loc.x > widget.base().bounds.loc.x + widget.base().bounds.size.w / 2.0,
        "widget {:?}, grip {:?}", widget.base().bounds, grip);
    assert!(grip.loc.x + grip.size.w < widget.base().bounds.loc.x + widget.base().bounds.size.w,
        "the grip is inset inside the right edge");
    assert!(grip.loc.y + grip.size.h < widget.base().bounds.loc.y + widget.base().bounds.size.h,
        "the grip is inset inside the bottom edge");
    drag(&mut widget, 400.0);
    assert_eq!(widget.base().bounds.size.h, 160.0);
    drag(&mut widget, -400.0);
    assert_eq!(widget.base().bounds.size.h, 70.0);
    let input = widget.base().children[0].base();
    let viewport = input.children[0].base().bounds;
    let caret = input.children[0].base().children[0].base().children[1].base().bounds;
    assert!(viewport.loc.y + viewport.size.h <= input.bounds.loc.y + input.bounds.size.h);
    assert!(caret.loc.y >= viewport.loc.y);
    assert!(caret.loc.y + caret.size.h <= viewport.loc.y + viewport.size.h + 1.0);
    common::type_text(&mut widget, "X");
    assert_eq!(text.get_untracked(), "one\ntwo\nthree\nfour\nfive\nsixX");
}

#[test]
fn the_same_control_resizes_non_editor_content() {
    let mut widget = Resizable::new(ScrollRegion::new().child(Label::new("content")))
        .width(200.0).min_height(60.0).max_height(200.0);
    layout(&mut widget);
    let before = widget.base().bounds.size.h;
    drag(&mut widget, 40.0);
    assert_eq!(widget.base().bounds.size.h, before + 40.0);
    assert!(!common::texts(&common::paint(&widget, &Theme::default())).is_empty());
}

#[test]
fn pressing_the_grip_hands_focus_to_the_editor_instead_of_an_unrelated_control() {
    let input = Input::new().multiline(true);
    let text = input.text();
    let mut other = Input::new().value("other");
    common::give_keyboard(&mut other);
    let mut root = Flex::column().width(300.0)
        .child(Resizable::new(input).width("100%")).child(other);
    layout(&mut root);
    let grip = root.base().children[0].base().children[1].base().children[1].base().bounds;
    common::click_at(&mut root, Point::new(grip.loc.x + grip.size.w / 2.0,
        grip.loc.y + grip.size.h / 2.0), PointerButton::Left);
    common::type_text(&mut root, "editor");
    assert_eq!(text.get_untracked(), "editor");
    assert!(!root.base().children[1].base().focused.get_untracked());
}
