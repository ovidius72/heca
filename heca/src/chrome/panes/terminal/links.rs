//! **The terminal's links as pick targets** — one child per link on screen, laid over its cells.
//!
//! `prefix+Shift+o` letters every link in every terminal on screen and opens the one you pick. The
//! letters, the keyboard and the pick are the library's one picker (the one `prefix+/` is), asked
//! for the `"links"` scope; what the terminal contributes is the part only it knows: **where its
//! links are**. So it holds a layer over its picture with one region per link, each declaring
//! "pick me and I open this address" — the line a plugin's own widget writes for its own targets.
//!
//! The layer lets the pointer through (`pointer_transparent`): a click, a selection or a wheel on a
//! link still reaches the terminal underneath, exactly as before the layer existed.

use std::rc::Rc;

use heca_core::backend::HyperlinkSpan;
use heca_core::layout::Rectangle;
use heca_grid_ui::builders::{HintExt, LayoutExt};
use heca_grid_ui::component::{Base, Component};
use heca_grid_ui::style::Placement;
use heca_grid_ui::widgets::{Flex, HintPlacement};

use super::shared::Shared;

/// **The scope the link picker letters** — the `"links"` set, apart from what `prefix+/` letters.
/// The `follow_link` action opens the one picker over this scope.
pub(crate) const LINK_SCOPE: &str = "links";

/// The action a picked link sends — the same one a click on a link with the window's key held
/// sends (`Shared::open_link`).
const OPEN_LINK: &str = "open_link";

/// **The layer of link regions over a terminal's picture.** It rebuilds its regions only when the
/// links on screen, or the cells they sit in, change — never per frame.
pub(super) struct LinkTargets {
    base: Base,
    shared: Rc<Shared>,
    /// What the regions were last built from, so an unchanged screen builds nothing.
    shown: Option<Shown>,
}

/// The inputs a set of regions is built from.
#[derive(PartialEq)]
struct Shown {
    links: Vec<HyperlinkSpan>,
    cell: (f32, f32),
    content: Rectangle,
}

impl LinkTargets {
    pub(super) fn new(shared: Rc<Shared>) -> Self {
        let mut base = Base::container();
        // The whole of the terminal, out of the flow of its controls, and invisible to the pointer.
        base.style.layout.placement = Some(Placement::FILL);
        base.pointer_transparent = true;
        // **The terminal is the room a link's letter may use.** Clipping the layer to it makes the
        // letter's "where is there room" (`hint_outside`) ask about the terminal, not the pane
        // around it: a link on the first row gets its letter beside it, not on the pane's header.
        // A letter is never cut by it — it is drawn in the overlay band and only nudged in.
        base.clip_children = true;
        Self {
            base,
            shared,
            shown: None,
        }
    }

    /// **The region over one link**: its cells, in the layer's own coordinates (the layer fills the
    /// terminal, so a cell's place is the content box's offset plus the cell's row and column).
    fn region(link: &HyperlinkSpan, cell: (f32, f32), content: Rectangle) -> Flex {
        let (cell_w, cell_h) = (f64::from(cell.0), f64::from(cell.1));
        let cells = link.end_col.saturating_sub(link.start_col).max(1);
        Flex::row()
            .at_rect(
                content.loc.x + link.start_col as f64 * cell_w,
                content.loc.y + link.row as f64 * cell_h,
                cells as f64 * cell_w,
                cell_h,
            )
            .hint_scope([LINK_SCOPE])
            .hint_placement(HintPlacement::TopLeftCorner)
            // Beside the link's first character, never over the address it opens.
            .hint_outside(true)
            .on_hint(heca_view::Intent::new(OPEN_LINK).arg("url", link.uri.clone().into()))
    }

    /// Build the regions again if what they show has changed.
    fn follow(&mut self) {
        let now = Shown {
            links: self.shared.links.borrow().clone(),
            cell: self.shared.cell.get(),
            content: self.shared.content_box(),
        };
        if self.shown.as_ref() == Some(&now) {
            return;
        }
        self.base.children = now
            .links
            .iter()
            .map(|link| heca_grid_ui::WidgetModel::new(Self::region(link, now.cell, now.content)))
            .collect();
        self.shown = Some(now);
        self.base.mark_needs_layout();
    }
}

impl Component for LinkTargets {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    fn tick(&mut self, dt: f32) -> bool {
        self.follow();
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}
