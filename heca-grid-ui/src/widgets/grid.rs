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
//!     .template_column("auto 1fr auto")
//!     .template_row("auto auto")
//!     .template_area(["icon title  status",
//!             "icon subtext status"])
//!     .area(Icon::new(ICON_TERM), "icon")
//!     .area(Label::new("zsh"),    "title")
//!     .area(StatusDot::online(),  "status")
//!     .area(Label::new("~/proj"), "subtext")
//! ```

use crate::builders::{LayoutExt, Parent};
use crate::component::{Base, Component, PaintCx};
use crate::reactive::SignalGet;
use crate::style::Track;
use crate::style::{IntoRows, IntoTracks};

/// A layout-only CSS-Grid container.
pub struct Grid {
    base: Base,
    columns: Vec<Track>,
    rows: Vec<Track>,
    /// The named areas **as written**, one string per row.
    ///
    /// Kept unparsed because a child names the area it belongs in and that name is resolved during
    /// layout, when this template is known — not when the child was built.
    areas: Vec<String>,
}

impl Grid {
    /// An empty grid (no tracks). Add tracks with `.template_row(..)` / `.template_column(..)`.
    pub fn new() -> Self {
        Self {
            base: Base::new(),
            columns: Vec::new(),
            rows: Vec::new(),
            areas: Vec::new(),
        }
    }

    /// **The row tracks** — `"auto 1fr"`, or a list: `["auto", "1fr"]`, `[Track::Auto, ..]`.
    ///
    /// Named for the axis it sets, because a bare `template` cannot say which axis it means and a
    /// reader should never have to remember. CSS spells the pair the same way.
    pub fn template_row(mut self, tracks: impl IntoTracks) -> Self {
        self.rows = tracks.into_tracks();
        self
    }

    /// **The column tracks.** Same spellings as [`template_row`](Self::template_row).
    pub fn template_column(mut self, tracks: impl IntoTracks) -> Self {
        self.columns = tracks.into_tracks();
        self
    }

    /// **Named areas**, as a stylesheet writes `grid-template-areas`: one string per row,
    /// whitespace-separated cell names, `.` or `_` for an empty cell. One row or a list of them.
    ///
    /// ```ignore
    /// Grid::new()
    ///     .template_area(["dot title tag",
    ///                     ".   sub   ."])
    ///     .child([icon.area("dot"), title.area("title"), tag.area("tag"), sub.area("sub")])
    /// ```
    ///
    /// A child says which area it belongs in — [`LayoutExt::area`](crate::builders::LayoutExt::area)
    /// — exactly as CSS has it. The name is resolved against this template during layout.
    pub fn template_area(mut self, rows: impl IntoRows) -> Self {
        self.areas = rows.into_rows();
        self
    }

    /// **This grid's template**, so the layout pass can resolve what its children say about
    /// themselves — `1 / -1` against the column count, and a named area against these rows.
    pub(crate) fn template(&self) -> crate::style::GridTemplate<'_> {
        crate::style::GridTemplate {
            columns: &self.columns,
            rows: &self.rows,
            areas: &self.areas,
        }
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
        self.base
            .style
            .layout
            .to_taffy_grid(self.base.font, &self.columns, &self.rows)
    }

    /// What this grid's children resolve their own placement against.
    fn grid_template(&self) -> Option<crate::style::GridTemplate<'_>> {
        Some(self.template())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builders::PlaceExt;
    use crate::style::Track;

    /// **A track list arrives in whichever shape the caller holds it** — one stylesheet line, or a
    /// list of strings, numbers or variants. One builder per axis, so there is no plural spelling
    /// to discover and no way to set the same thing two ways.
    ///
    /// ⚠️ Ran red first: the vocabulary lived in `heca-view-realize`, so a described grid said
    /// `"1fr"` while native code had to write `Track::Fr(1.0)`, one variant per track.
    #[test]
    fn a_template_takes_a_line_or_a_list() {
        let expected = [Track::Auto, Track::Fr(1.0)];
        for grid in [
            Grid::new().template_row("auto 1fr"),
            Grid::new().template_row(["auto", "1fr"]),
            Grid::new().template_row([Track::Auto, Track::Fr(1.0)]),
        ] {
            assert_eq!(grid.rows, expected);
        }

        assert_eq!(
            Grid::new().template_column([200, 100]).columns,
            [Track::Px(200.0), Track::Px(100.0)],
            "a bare number is pixels, as everywhere else",
        );
    }

    /// **Each axis is named**, so a template can never be read as the wrong one. A bare `template`
    /// could not say which axis it meant, and a caller had to remember.
    #[test]
    fn a_template_says_which_axis_it_sets() {
        let grid = Grid::new().template_row("auto").template_column("1fr 1fr");
        assert_eq!(grid.rows, [Track::Auto]);
        assert_eq!(grid.columns, [Track::Fr(1.0), Track::Fr(1.0)]);
    }

    /// **One template row or a list of them** — a single-row template needs no brackets, and the
    /// rows are kept as written so a child naming an area is resolved during layout.
    #[test]
    fn a_named_area_template_takes_one_row_or_several() {
        let one = Grid::new().template_area("dot title tag");
        assert_eq!(
            one.template().area("title").map(|c| (c.col, c.row)),
            Some((2, 1))
        );

        let two = Grid::new().template_area(["dot title tag", ".   sub   ."]);
        let sub = two.template().area("sub").expect("named in row two");
        assert_eq!((sub.col, sub.row), (2, 2));
        let dot = two.template().area("dot").expect("named in row one");
        assert_eq!((dot.col, dot.row, dot.row_span), (1, 1, 1));
        assert_eq!(
            two.template().area("nothing"),
            None,
            "an unknown name places nothing"
        );
    }

    /// **A name that spans several cells is the box around them**, as `grid-template-areas` has it.
    #[test]
    fn an_area_named_across_cells_is_the_box_around_them() {
        let grid = Grid::new().template_area(["icon title", "icon sub"]);
        let icon = grid.template().area("icon").expect("spans two rows");
        assert_eq!(
            (icon.col, icon.row, icon.col_span, icon.row_span),
            (1, 1, 1, 2)
        );
    }

    /// **Children name the area they belong in**, and the name is resolved during layout — so a
    /// child can be built before the parent that places it.
    #[test]
    fn a_child_names_its_own_area_and_is_placed_at_layout() {
        use crate::widgets::Label;
        use crate::{Component, LayoutEngine};
        use heca_core::layout::Size;

        let mut grid: Box<dyn Component> = Box::new(
            Grid::new()
                .template_column("40px 100px")
                .template_row("20px 20px")
                .template_area(["icon title", "icon sub"])
                .child([
                    Label::new("i").area("icon"),
                    Label::new("t").area("title"),
                    Label::new("s").area("sub"),
                ]),
        );
        LayoutEngine::new().compute(grid.as_mut(), Size::new(140.0, 40.0));

        let cell = |i: usize| {
            let c = grid.base().children[i]
                .base()
                .style
                .layout
                .grid_cell
                .expect("placed by name");
            (c.col, c.row, c.col_span, c.row_span)
        };
        assert_eq!(
            cell(0),
            (1, 1, 1, 2),
            "the icon spans both rows of its column"
        );
        assert_eq!(cell(1), (2, 1, 1, 1));
        assert_eq!(cell(2), (2, 2, 1, 1));
    }
}
