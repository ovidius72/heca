//! **Where an item goes in the grid that holds it.**

use crate::component::Component;

/// What a grid item starts as when it names one axis and leaves the other to auto-placement:
/// the first line, one track. Auto-placement then moves it if nothing pinned it.
const DEFAULT_CELL: crate::style::GridCell = crate::style::GridCell {
    col: 1,
    row: 1,
    col_span: 1,
    row_span: 1,
};

/// **Where an item goes in the grid that holds it** — CSS's `grid-column`, `grid-row` and
/// `grid-area`, said by the item about itself.
///
/// Both a concrete component and an opaque [`WidgetModel`](crate::WidgetModel) can name their
/// place in a parent grid. The model retains the original component's base and placement.
pub trait PlaceExt {
    /// The base this placement is written into.
    fn placement_base(&mut self) -> &mut crate::component::Base;

    /// **Which column this item sits in, and how far it reaches** — CSS `grid-column`, said by the
    /// item about itself.
    ///
    /// ```ignore
    /// Label::new("title").column("2")          // column 2
    /// header.column("1 / -1")                  // the whole width, however many columns there are
    /// wide.column("1 / span 2")                // two columns from the first
    /// ```
    #[heca_grid_ui_macros::prop]
    fn column(mut self, line: impl Into<crate::style::GridLine>) -> Self
    where
        Self: Sized,
    {
        let line = line.into();
        let cell = self
            .placement_base()
            .style
            .layout
            .grid_cell
            .get_or_insert(DEFAULT_CELL);
        cell.col = line.start;
        cell.col_span = line.span.stored();
        self
    }

    /// **Which row this item sits in, and how far it reaches** — CSS `grid-row`. Same spellings as
    /// [`column`](Self::column).
    #[heca_grid_ui_macros::prop]
    fn row(mut self, line: impl Into<crate::style::GridLine>) -> Self
    where
        Self: Sized,
    {
        let line = line.into();
        let cell = self
            .placement_base()
            .style
            .layout
            .grid_cell
            .get_or_insert(DEFAULT_CELL);
        cell.row = line.start;
        cell.row_span = line.span.stored();
        self
    }

    /// **How many columns this item covers**, leaving where it starts to auto-placement — CSS
    /// `grid-column: span n`. `Span::All` is every column there are, now and after one is added.
    #[heca_grid_ui_macros::prop]
    fn column_span(mut self, span: impl Into<crate::style::Span>) -> Self
    where
        Self: Sized,
    {
        let cell = self
            .placement_base()
            .style
            .layout
            .grid_cell
            .get_or_insert(DEFAULT_CELL);
        cell.col_span = span.into().stored();
        self
    }

    /// **How many rows this item covers** — CSS `grid-row: span n`.
    #[heca_grid_ui_macros::prop]
    fn row_span(mut self, span: impl Into<crate::style::Span>) -> Self
    where
        Self: Sized,
    {
        let cell = self
            .placement_base()
            .style
            .layout
            .grid_cell
            .get_or_insert(DEFAULT_CELL);
        cell.row_span = span.into().stored();
        self
    }

    /// **The named area this item belongs in** — CSS `grid-area: title`.
    ///
    /// The name is resolved against whichever grid holds it, during layout. So a child can be
    /// built before its parent, and changing the parent's template re-places the children without
    /// rebuilding any of them.
    #[heca_grid_ui_macros::prop]
    fn area(mut self, name: impl Into<String>) -> Self
    where
        Self: Sized,
    {
        self.placement_base().grid_area = Some(name.into());
        self
    }
}

impl<C: Component + Sized> PlaceExt for C {
    fn placement_base(&mut self) -> &mut crate::component::Base {
        self.base_mut()
    }
}

impl PlaceExt for crate::WidgetModel {
    fn placement_base(&mut self) -> &mut crate::component::Base {
        (**self).base_mut()
    }
}
