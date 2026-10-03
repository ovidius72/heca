//! Shared fixtures for the terminal surface's tests, so each test reads as its assertion rather
//! than its setup (AGENTS.md § 0b-bis rule 7).

use heca_grid_ui::PlaceExt;
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::component::Component;
use heca_grid_ui::style::Length;
use heca_grid_ui::widgets::{Grid, Surface};
use heca_grid_ui::{LayoutEngine, Size};

/// `content` alone in a box of `w` × `h`, laid out.
pub(crate) fn laid_out_in(content: Box<dyn Component>, w: f64, h: f64) -> Box<dyn Component> {
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .template_row("1fr")
            .template_area(["content"])
            .child(content.area("content"))
            .width(Length::Percent(1.0))
            .height(Length::Percent(1.0)),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(w, h));
    root
}

/// `content` under a header `header` tall, in a box of `w` × `h`, laid out — the arrangement a
/// pane gives its content (`auto 1fr`, each part naming its row).
pub(crate) fn header_above(
    content: Box<dyn Component>,
    header: f64,
    w: f64,
    h: f64,
) -> Box<dyn Component> {
    let mut root: Box<dyn Component> = Box::new(
        Grid::new()
            .template_row("auto 1fr")
            .template_area(["header", "content"])
            .child([
                (Box::new(Surface::new().height(Length::Px(header as f32))) as Box<dyn Component>)
                    .area("header"),
                content.area("content"),
            ])
            .width(Length::Percent(1.0))
            .height(Length::Percent(1.0)),
    );
    LayoutEngine::new().compute(root.as_mut(), Size::new(w, h));
    root
}

/// Seams that keep what the terminal said, and the list it said it to.
pub(crate) fn recording() -> (
    super::input::Seams,
    std::rc::Rc<std::cell::RefCell<Vec<super::input::TerminalInput>>>,
) {
    use super::input::Seams;
    use super::viewport::ScrollIntents;
    let said = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = said.clone();
    let seams = Seams {
        scroll: ScrollIntents {
            to_bottom: Box::new(|| {}),
            to_offset: Box::new(|_| {}),
        },
        input: Box::new(move |i| seen.borrow_mut().push(i)),
        command: Box::new(|_, _| {}),
    };
    (seams, said)
}
