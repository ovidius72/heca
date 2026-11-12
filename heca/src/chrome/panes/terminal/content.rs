//! The terminal's optional header and body, independent of the frame that hosts them.

use heca_grid_ui::{LayoutExt, Parent, PlaceExt, PresenceExt};
use heca_grid_ui::widgets::Grid;

use crate::chrome::PaneHeader;

pub(crate) const HEADER_AREA: &str = "terminal-header";
pub(crate) const BODY_AREA: &str = "terminal-body";
const CONTENT_AREA: &str = "terminal-content";

/// A header is part of terminal content, never a special pane-frame slot.
pub(crate) fn compose(header: Option<PaneHeader>, body: impl heca_grid_ui::builders::IntoChildren) -> Grid {
    match header {
        Some(header) => {
            let grid = layout(Some(header.seat()), body);
            header.answer_props(grid)
        }
        None => layout(None, body),
    }
}

pub(super) fn layout(header: Option<heca_grid_ui::WidgetModel>, body: impl heca_grid_ui::builders::IntoChildren) -> Grid {
    let grid = Grid::new().area(CONTENT_AREA).keyboard_to(BODY_AREA).width("100%").height("100%");
    let body = heca_grid_ui::widgets::Flex::column()
        .area(BODY_AREA).width("100%").height("100%").child(body);
    match header {
        Some(header) => {
            grid.template_row("auto 1fr")
                .template_area([HEADER_AREA, BODY_AREA])
                .child(header.area(HEADER_AREA))
                .child(body)
        }
        None => grid.template_row("1fr").template_area([BODY_AREA]).child(body),
    }
}

/// Update terminal data without teaching its generic enclosing frame about that data.
pub(crate) fn give_facts(frame: &mut dyn heca_grid_ui::Component, input: &crate::chrome::panes::pane_header::PaneHeaderInput) {
    if let Some(content) = heca_grid_ui::area_slot(frame.base_mut(), CONTENT_AREA) {
        crate::chrome::give_header_facts(content.as_mut(), input);
    }
}
