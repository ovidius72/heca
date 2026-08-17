//! [`Grid`] — a **layout-only** CSS-Grid container.
//!
//! The flexible "put whatever we want" layout for rich item content (a pane row
//! with `[icon | title | status]` over `[· | subtext | exit-code]`, …). Define
//! column/row [`Track`]s, then place any child by **named area** or **explicit
//! cell + span**. Like [`Flex`](super::Flex) it carries no visual styling —
//! decoration lives on surfaces.
//!
//! ```ignore
//! Grid::new()
//!     .columns([Track::Auto, Track::Fr(1.0), Track::Auto])
//!     .rows([Track::Auto, Track::Auto])
//!     .areas(["icon title  status",
//!             "icon subtext status"])
//!     .area(Icon::new(ICON_TERM), "icon")
//!     .area(Label::new("zsh"),    "title")
//!     .area(StatusDot::online(),  "status")
//!     .area(Label::new("~/proj"), "subtext")
//! ```

use std::collections::HashMap;

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::style::{GridCell, Track};

/// A layout-only CSS-Grid container.
pub struct Grid {
    base: Base,
    columns: Vec<Track>,
    rows: Vec<Track>,
    /// Named areas parsed from [`Grid::areas`] → 1-based cell rectangles.
    areas: HashMap<String, GridCell>,
}

impl Grid {
    /// An empty grid (no tracks). Add tracks with `.columns(..)`/`.rows(..)`.
    pub fn new() -> Self {
        Self {
            base: Base::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            areas: HashMap::new(),
        }
    }

    /// Set the column tracks.
    pub fn columns(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.columns = tracks.into_iter().collect();
        self
    }

    /// Set the row tracks.
    pub fn rows(mut self, tracks: impl IntoIterator<Item = Track>) -> Self {
        self.rows = tracks.into_iter().collect();
        self
    }

    /// Define named areas from a CSS-`grid-template-areas`-style grid: one string
    /// per row, whitespace-separated cell names. `.` (or `_`) marks an empty cell.
    /// Each name becomes a rectangular [`GridCell`] usable via [`Grid::area`].
    pub fn areas<'a>(mut self, rows: impl IntoIterator<Item = &'a str>) -> Self {
        // Collect a (row, col) → name token grid.
        let mut bounds: HashMap<String, (u16, u16, u16, u16)> = HashMap::new(); // name → (min_c,max_c,min_r,max_r) 1-based
        for (r, line) in rows.into_iter().enumerate() {
            for (c, tok) in line.split_whitespace().enumerate() {
                if tok == "." || tok == "_" {
                    continue;
                }
                let (col, row) = (c as u16 + 1, r as u16 + 1);
                bounds
                    .entry(tok.to_string())
                    .and_modify(|b| {
                        b.0 = b.0.min(col);
                        b.1 = b.1.max(col);
                        b.2 = b.2.min(row);
                        b.3 = b.3.max(row);
                    })
                    .or_insert((col, col, row, row));
            }
        }
        self.areas = bounds
            .into_iter()
            .map(|(name, (min_c, max_c, min_r, max_r))| {
                (
                    name,
                    GridCell {
                        col: min_c,
                        row: min_r,
                        col_span: max_c - min_c + 1,
                        row_span: max_r - min_r + 1,
                    },
                )
            })
            .collect();
        self
    }

    /// Place a child at an explicit 1-based cell with spans.
    pub fn cell(
        mut self,
        mut child: impl Component + 'static,
        col: u16,
        row: u16,
        col_span: u16,
        row_span: u16,
    ) -> Self {
        child.base_mut().style.layout.grid_cell = Some(GridCell {
            col,
            row,
            col_span,
            row_span,
        });
        self.base.children.push(Box::new(child));
        self
    }

    /// Place a child into a previously-defined named [`area`](Grid::areas).
    /// Unknown names fall back to grid auto-placement.
    pub fn area(mut self, mut child: impl Component + 'static, name: &str) -> Self {
        child.base_mut().style.layout.grid_cell = self.areas.get(name).copied();
        self.base.children.push(Box::new(child));
        self
    }

    /// [`area`](Grid::area) for an **already-boxed** child — what a host mapper has after
    /// realizing a declarative subtree. `Box<dyn Component>` is not itself `Component`, so it
    /// cannot go through the `impl Component` setters; this is the same boxed-setter seam as
    /// [`Dialog::body_boxed`](super::Dialog::body_boxed).
    ///
    /// Call it **after** [`areas`](Grid::areas) — an unknown (or not-yet-defined) name falls back to
    /// grid auto-placement rather than erroring.
    pub fn area_boxed(mut self, mut child: Box<dyn Component>, name: &str) -> Self {
        child.base_mut().style.layout.grid_cell = self.areas.get(name).copied();
        self.base.children.push(child);
        self
    }

    /// [`cell`](Grid::cell) for an already-boxed child — see [`area_boxed`](Grid::area_boxed).
    pub fn cell_boxed(
        mut self,
        mut child: Box<dyn Component>,
        col: u16,
        row: u16,
        col_span: u16,
        row_span: u16,
    ) -> Self {
        child.base_mut().style.layout.grid_cell = Some(GridCell {
            col,
            row,
            col_span,
            row_span,
        });
        self.base.children.push(child);
        self
    }
}

impl Default for Grid {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for Grid {
    fn base(&self) -> &Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut Base {
        &mut self.base
    }

    /// Inject `display: grid` + the track templates; child placement comes from
    /// each child's `style.grid_cell` (see [`Style::to_taffy`]).
    fn taffy_style(&self) -> taffy::Style {
        self.base.style.layout.to_taffy_grid(&self.columns, &self.rows)
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            crate::component::paint_child(child.as_ref(), cx);
        }
    }
}

impl LayoutExt for Grid {}
// Plain `.child()` appends with no placement → grid auto-placement.
impl Parent for Grid {}
