//! Shared fixtures for the terminal surface's tests, so each test reads as its assertion rather
//! than its setup (AGENTS.md § 0b-bis rule 7).

use heca_grid_ui::PlaceExt;
use heca_grid_ui::builders::{LayoutExt, Parent};
use heca_grid_ui::style::Length;
use heca_grid_ui::widgets::{Grid, Surface};
use heca_grid_ui::{LayoutEngine, Size};

/// `content` alone in a box of `w` × `h`, laid out.
pub(crate) fn laid_out_in(content: heca_grid_ui::WidgetModel, w: f64, h: f64) -> heca_grid_ui::WidgetModel {
    let mut root: heca_grid_ui::WidgetModel = heca_grid_ui::WidgetModel::new(
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
    content: heca_grid_ui::WidgetModel,
    header: f64,
    w: f64,
    h: f64,
) -> heca_grid_ui::WidgetModel {
    let mut root: heca_grid_ui::WidgetModel = heca_grid_ui::WidgetModel::new(
        Grid::new()
            .template_row("auto 1fr")
            .template_area(["header", "content"])
            .child([
                heca_grid_ui::WidgetModel::new(Surface::new().height(Length::Px(header as f32)))
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

/// A terminal that shows the process `id`, as the host makes one for a pane that is running.
pub(crate) fn attached(id: u64) -> super::Terminal {
    let terminal = super::Terminal::unbound();
    terminal.attach(super::TerminalId(id));
    terminal
}
