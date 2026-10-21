mod common;
use heca_grid_ui::prelude::*;
use heca_grid_ui::{LayoutEngine, MessageList, TreeLayout, Line, DrawCommand, Point, Size, PaintCx, Scene};
use heca_grid_ui::scene::ArrowHeads;

fn layout(widget: &mut dyn Component, theme: &Theme) {
    LayoutEngine::new().theme(theme).compute(widget, Size::new(600.0, 400.0));
}

#[test]
fn multiline_grows_to_its_limit_and_preserves_committed_text_and_selection() {
    let theme = Theme::default();
    let mut input = Input::new().multiline(true).max_lines(2).width(200.0);
    common::give_keyboard(&mut input);
    layout(&mut input, &theme);
    let empty_height = input.base().bounds.size.h;
    common::type_text(&mut input, "A  中\né\nthird\nfourth");
    layout(&mut input, &theme);
    assert_eq!(input.value_str(), "A  中\né\nthird\nfourth");
    assert!(input.base().bounds.size.h > empty_height);
    assert!(input.base().bounds.size.h <= theme.font_size as f64 * 1.4 * 2.0
        + theme.colors.input.padding as f64 * 2.0 + 1.0);
    let viewport = input.base().children[0].base().bounds;
    let caret = input.base().children[0].base().children[0].base().children[1].base().bounds;
    assert!(caret.loc.y >= viewport.loc.y);
    assert!(caret.loc.y + caret.size.h <= viewport.loc.y + viewport.size.h + 1.0);
    assert!(heca_grid_ui::dispatch(&mut input, &Event::Widget(WidgetIntent::EditSelectAll)) == Handled::Yes);
    assert_eq!(input.selected_text().as_deref(), Some("A  中\né\nthird\nfourth"));
    common::type_text(&mut input, "replacement");
    assert_eq!(input.value_str(), "replacement");
    layout(&mut input, &theme);
    assert_eq!(input.base().bounds.size.h, empty_height);
}

#[test]
fn an_input_inside_a_scrolled_container_selects_the_word_under_the_pointer() {
    let mut viewport = ScrollRegion::new().width(220.0).height(90.0)
        .child(Flex::column().height(120.0))
        .child(Input::new().value("alpha beta"));
    layout(&mut viewport, &Theme::default());
    viewport.scroll_to(120.0);
    let origin = viewport.base().children[1].base().children[0].base().bounds.loc;
    let point = Point::new(origin.x + 70.0, origin.y + 10.0);
    common::click_at(&mut viewport, point, PointerButton::Left);
    common::click_at(&mut viewport, point, PointerButton::Left);
    heca_grid_ui::dispatch(&mut viewport, &Event::TextInput("X".into()));
    let scene = common::paint(&viewport, &Theme::default());
    assert!(scene.iter().any(|draw| matches!(draw, DrawCommand::Text(text) if text.text == "alpha X")),
        "selection and replacement use the displayed coordinates after outer scrolling");
}

#[test]
fn enter_adds_a_newline_and_submission_is_a_separate_intent() {
    use std::{cell::RefCell, rc::Rc};
    let sent = Rc::new(RefCell::new(Vec::new()));
    let sink = sent.clone();
    let mut input = Input::new().multiline(true).value("draft")
        .on_submit(move |action| sink.borrow_mut().push(action));
    common::give_keyboard(&mut input);
    layout(&mut input, &Theme::default());
    heca_grid_ui::dispatch(&mut input, &Event::Key { key: GridKey::Enter, pressed: true });
    assert_eq!(input.value_str(), "draft\n");
    assert!(sent.borrow().is_empty());
    heca_grid_ui::dispatch(&mut input, &Event::Widget(WidgetIntent::EditSubmit));
    assert_eq!(sent.borrow().len(), 1);
    assert_eq!(input.value_str(), "draft\n");
}

#[test]
fn a_message_feed_appends_wrapped_entries_without_replacing_old_widgets() {
    let mut list = MessageList::new().width(160.0).height(90.0).message("First entry");
    let feed = list.feed();
    layout(&mut list, &Theme::default());
    let first_id = list.base().children[0].base().children[0].base().node;
    heca_grid_ui::needs_layout(&list);
    feed.message("A long entry that needs several wrapped lines to fit the viewport.");
    list.tick(0.0);
    assert!(heca_grid_ui::needs_layout(&list), "arrival requests layout, not only a repaint");
    layout(&mut list, &Theme::default());
    let entries = &list.base().children[0].base().children;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].base().node, first_id);
    assert!(entries[1].base().bounds.size.h > entries[0].base().bounds.size.h);
}

#[test]
fn nested_trees_share_space_and_leave_controls_in_the_dispatch_tree() {
    let clicked = signal(false);
    let mut tree = TreeLayout::new(Label::new("root"))
        .branch(Label::new("one"))
        .branch(TreeLayout::new(Label::new("two")).branch(Button::new("leaf").on_click(move || clicked.set(true))))
        .width(500.0);
    layout(&mut tree, &Theme::default());
    let branches = &tree.base().children[0].base().children[1].base().children;
    assert_eq!(branches.len(), 2);
    assert!((branches[0].base().bounds.size.w - branches[1].base().bounds.size.w).abs() <= 1.0);
    let scene = common::paint(&tree, &Theme::default());
    assert!(scene.iter().any(|draw| matches!(draw, DrawCommand::Text(t) if t.text == "leaf")));
    assert!(scene.iter().any(|draw| matches!(draw, DrawCommand::Line(_))));
    fn button_bounds(widget: &dyn Component) -> Option<heca_grid_ui::Rectangle> {
        if widget.base().focusable { return Some(widget.base().bounds); }
        widget.base().children.iter().find_map(|child| button_bounds(child.as_ref()))
    }
    let bounds = button_bounds(&tree).unwrap();
    common::click_at(&mut tree, Point::new(bounds.loc.x + bounds.size.w / 2.0,
        bounds.loc.y + bounds.size.h / 2.0), PointerButton::Left);
    assert!(clicked.get_untracked(), "a branch button receives ordinary routed input");
}

#[test]
fn input_metrics_reflow_when_the_layout_theme_changes() {
    let mut theme = Theme::default();
    let mut input = Input::new().multiline(true).size(WidgetSize::Large).value("one\ntwo\nthree");
    layout(&mut input, &theme);
    let initial = input.base().bounds.size.h;
    theme.colors.input.max_lines = 1;
    theme.colors.input.padding = 4.0;
    theme.font_size = 20.0;
    layout(&mut input, &theme);
    assert!(input.base().bounds.size.h < initial);
    assert!((input.base().bounds.size.h - 36.0).abs() <= 1.0, "bounds: {:?}, font: {}", input.base().bounds, input.base().font);
}

#[test]
fn lines_follow_scale_opacity_and_nested_clipping() {
    let theme = Theme::default();
    let mut line = Line::new().arrows(ArrowHeads::Both).width(100.0).height(20.0);
    layout(&mut line, &theme);
    let mut scene = Scene::new();
    let clip = heca_grid_ui::Rectangle::new(Point::new(0.0, 0.0), Size::new(40.0, 40.0));
    let mut cx = PaintCx::new(&mut scene, &theme);
    cx.with_clip(clip, |cx| cx.with_scale(2.0, Point::new(0.0, 0.0), |cx|
        cx.with_opacity(0.5, |cx| line.paint(cx))));
    let command = scene.iter().find_map(|draw| match draw { DrawCommand::Line(line) => Some(line), _ => None }).unwrap();
    assert_eq!(command.end.x, 200.0);
    assert_eq!(command.width, theme.colors.border_width * 2.0);
    assert_eq!(command.arrows, ArrowHeads::Both);
    assert!(command.color.a < theme.colors.accent.a);
    assert!(scene.iter().any(|draw| matches!(draw, DrawCommand::PushClip(_))));
}
