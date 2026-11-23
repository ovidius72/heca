//! [`Surface`] — a styled box: the proper place for visual decoration
//! (background, border, glow, radius), as opposed to the layout-only
//! [`Flex`](super::Flex).
//!
//! A surface also arranges its own children (it has padding/gap/direction via
//! [`LayoutExt`]), so it is both a decorated panel and a container.
//!
//! [`elevation`](Surface::elevation) gives it the **panel look** every menu, dialog and palette
//! wears — the theme's surface fill, corner radius, edge (`overlay_border_style`) and drop shadow,
//! from the one place that paints them ([`paint_panel_chrome`](super::paint_panel_chrome)).

use crate::builders::{LayoutExt, Parent, StyleExt};
use crate::component::{Base, Component, PaintCx, paint_child};
use crate::reactive::SignalGet;
use crate::style::Direction;
use crate::widgets::{PanelChrome, PanelElevation, paint_panel_chrome};

/// A decorated container.
pub struct Surface {
    base: Base,
    /// The panel look, when asked for — see [`elevation`](Self::elevation).
    elevation: Option<PanelElevation>,
}

#[heca_grid_ui_macros::props]
impl Surface {
    /// A vertical surface (column).
    pub fn new() -> Self {
        let mut base = Base::container();
        base.style.layout.direction = Direction::Column;
        Self {
            base,
            elevation: None,
        }
    }

    /// **Look like a panel that sits above the page** — the same material as a menu, a dialog or
    /// the command palette: the theme's `surface` fill, its corner radius, the edge the user's
    /// `overlay_border_style` draws, and a drop shadow as deep as `elevation` says
    /// ([`Panel`](PanelElevation::Panel) for a panel, [`Hover`](PanelElevation::Hover) for
    /// something hovering just above its target). It follows a theme reload.
    ///
    /// A `.border(..)` set on the surface is the edge's colour when the style draws one, and a
    /// `.glow(..)` is kept; the fill, radius and shadow are the theme's.
    ///
    /// ```ignore
    /// Surface::column().elevation(PanelElevation::Panel).padding(Spacing::Sm).child(field)
    /// ```
    #[heca_grid_ui_macros::prop]
    pub fn elevation(mut self, elevation: PanelElevation) -> Self {
        self.elevation = Some(elevation);
        self
    }

    /// A horizontal surface (row).
    pub fn row() -> Self {
        let mut s = Self::new();
        s.base.style.layout.direction = Direction::Row;
        s
    }

    /// A vertical surface (column). Alias for [`Surface::new`].
    pub fn column() -> Self {
        Self::new()
    }
}

impl Default for Surface {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Surface {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }
    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        match self.elevation {
            Some(elevation) => {
                let visual = &self.base.style.visual;
                let chrome = PanelChrome {
                    border: visual.border,
                    glow: visual.glow,
                    elevation,
                };
                paint_panel_chrome(cx, self.base.bounds, chrome);
            }
            None => cx.paint_base(&self.base),
        }
        for child in &self.base.children {
            paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for Surface {}
impl StyleExt for Surface {}
impl Parent for Surface {}
