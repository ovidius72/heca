//! P103: connections, a nested tree, a growing editor and a live message feed.
use super::*;
use heca_grid_ui::{Line, MessageList, Resizable, TreeLayout};
use heca_grid_ui::scene::ArrowHeads;

pub(super) fn build(theme: &Theme) -> Grid {
    let messages = MessageList::new().visible_lines(theme.colors.input.max_lines)
        .message("Scroll up to pause following. Return to the bottom to follow new entries.");
    let feed = messages.feed();
    let editor = Input::new().multiline(true).width("100%")
        .placeholder("Enter adds a new line…")
        .on_submit({ let feed = feed.clone(); move |action| {
            if let SignalData::String(text) = action.data { feed.message(text); }
        }});
    let text = editor.text();
    let send = Button::new("Append entry").on_click(move || {
        let message = text.get_untracked();
        if !message.is_empty() { feed.message(message); text.set(String::new()); }
    });
    Grid::new().template_column("1fr 1fr").template_row("auto auto")
        .gap("lg").width("100%")
        .child(Grid::new().template_row("auto auto").gap("md")
            .child(Label::new("Line and arrows"))
            .child(Line::new().start_y(0.0).end_y(1.0).arrows(ArrowHeads::Both)))
        .child(TreeLayout::new(Button::new("Parent"))
            .branch(Button::secondary("One"))
            .branch(TreeLayout::new(Button::secondary("Two"))
                .branch(Button::secondary("Nested"))))
        .child(Flex::column().gap("sm").align_self("start")
            .child(Label::new("Multi-line Input")).child(Resizable::new(editor).width("100%"))
            .child(Flex::row().justify("end").child(send)))
        .child(Grid::new().template_row("auto 1fr").gap("sm")
            .child(Label::new("MessageList")).child(messages))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizing_the_editor_keeps_its_top_and_the_button_gap_fixed() {
        let theme = Theme::default();
        let mut demo = build(&theme).width(900.0);
        let frame = |root: &mut dyn Component| {
            LayoutEngine::new().theme(&theme).compute(root, Size::new(900.0, 900.0));
        };
        frame(&mut demo);
        let positions = |root: &dyn Component| {
            let editor = root.base().children[2].base();
            let field = editor.children[1].base().bounds;
            let footer = editor.children[2].base().bounds;
            (field.loc.y, footer.loc.y - field.loc.y - field.size.h)
        };
        let before = positions(&demo);
        for delta in [80.0, -1000.0, 40.0] {
            let height = demo.base().children[2].base().children[1].base().bounds.size.h;
            let grip = demo.base().children[2].base().children[1].base().children[1].base().children[1].base().bounds;
            let from = Point::new(grip.loc.x + grip.size.w / 2.0, grip.loc.y + grip.size.h / 2.0);
            let to = Point::new(from.x, from.y + delta);
            heca_grid_ui::dispatch(&mut demo, &Event::pointer_pressed(from, PointerButton::Left));
            heca_grid_ui::dispatch(&mut demo, &Event::pointer_moved(to));
            demo.tick(0.0);
            frame(&mut demo);
            heca_grid_ui::dispatch(&mut demo, &Event::pointer_released(to, PointerButton::Left));
            let after = positions(&demo);
            let resized = demo.base().children[2].base().children[1].base().bounds.size.h;
            assert!((resized - height) * delta > 0.0, "the gesture must actually resize the field");
            assert!((after.0 - before.0).abs() <= 1.0, "top moved: {before:?} -> {after:?}");
            assert!((after.1 - before.1).abs() <= 1.0, "button gap changed: {before:?} -> {after:?}");
        }
    }

}
