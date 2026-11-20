//! The palette's tree, assembled once.
//!
//! ```text
//! CommandPalette                  a full-window layer: owns the open state, the keys, the selection
//! └─ Overlay                      scrim, panel chrome, placement, the cap at the window
//!    └─ Flex::column  (panel)     sized by `extent`
//!       ├─ Input                  the query
//!       └─ ScrollRegion           scrolls, and keeps the selected row in view
//!          └─ Row …               one per command — see `row`
//! ```
//!
//! Nothing here positions anything: the overlay places the panel, the column divides it, the
//! region scrolls its rows and each row is laid out by its own grid.

use super::extent::{BOTTOM_SHARE, Extent, MAX_WIDTH_SHARE, TOP_SHARE, extent};
use super::*;
use crate::builders::{Parent, PlaceExt, PresenceExt};
use crate::component::Base;
use crate::style::Length;
use crate::widgets::{Grid, Overlay, PanelChrome, ScrollRegion};

/// The query field's name among the panel's parts.
const QUERY: &str = "palette-query";
/// The scrolling list's name among the panel's parts.
pub(super) const LIST: &str = "palette-list";

/// The panel: two parts, so a grid — one column that takes the panel's width, and rows of `auto`
/// (the query, as tall as its line) and `1fr` (the list, the rest). Each part stretches across its
/// cell, so neither is given a width.
fn panel(query: Input, extent: Extent) -> Grid {
    let list = ScrollRegion::new().area(LIST).gap("none").padding_y("none");
    let mut panel = Grid::new()
        .template_column("1fr")
        .template_row("auto 1fr")
        .padding("md")
        .gap("md")
        .child(seat_query(query))
        .child(list);
    size_panel(panel.base_mut(), extent);
    panel
}

/// The query field as the panel holds it: named. One place, because the field is built twice — at
/// construction and again when a placeholder replaces it.
fn seat_query(field: Input) -> Input {
    field.area(QUERY)
}

/// Give the panel its size: its width, the share of the window it may not exceed, and how tall its
/// list grows before it scrolls. The cap against the window itself is the overlay's.
fn size_panel(panel: &mut Base, extent: Extent) {
    panel.style.layout.width = extent.width;
    panel.style.layout.max_width = Some(Length::Percent(MAX_WIDTH_SHARE));
    if let Some(list) = panel.child_in_area(LIST) {
        list.base_mut().style.layout.max_height = Some(extent.list_height);
    }
}

/// The overlay the palette's layer holds, around `query`.
///
/// The layer root (a full-window `Base`, like a `Dialog`'s) holds the keyboard and the open state;
/// the overlay follows that same `open` signal rather than keeping one of its own. The panel hangs
/// from the top, centred across: a palette that hugs its rows and stays centred up and down would
/// move its top edge with every keystroke that narrows the list. It wears the accented chrome.
pub(super) fn overlay(query: Input, open: Signal<bool>, size: WidgetSize) -> crate::WidgetModel {
    crate::WidgetModel::new(
        Overlay::new()
            .blocking(true)
            .lock(true)
            .top(TOP_SHARE, BOTTOM_SHARE)
            .chrome(PanelChrome::accented)
            .panel(panel(query, extent(size)))
            .open_when(open),
    )
}

impl CommandPalette {
    /// The panel — the overlay's one child.
    pub(super) fn panel_mut(&mut self) -> &mut Base {
        self.base.children[0].base_mut().children[0].base_mut()
    }

    /// The panel's part called `name`. The overlay and panel are built here and cannot lack one.
    pub(super) fn part_mut(&mut self, name: &str) -> &mut crate::WidgetModel {
        self.panel_mut()
            .child_in_area(name)
            .expect("the palette builds every part it names")
    }

    /// The scrolling list; its children are the rows, in command order.
    pub(super) fn list_mut(&mut self) -> &mut Base {
        self.part_mut(LIST).base_mut()
    }

    /// Swap the query field for `field`, keeping its place in the panel.
    pub(super) fn set_query_field(&mut self, field: Input) {
        *self.part_mut(QUERY) = crate::WidgetModel::new(seat_query(field));
    }

    /// Re-size the panel for `size`.
    pub(super) fn resize_panel(&mut self, size: WidgetSize) {
        size_panel(self.panel_mut(), extent(size));
    }

    /// The panel's laid-out box — zero before the first layout.
    pub(super) fn panel_bounds(&self) -> heca_core::layout::Rectangle {
        self.base.children[0].base().children[0].base().bounds
    }
}
