/// Unique identifier for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Unique identifier for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub u64);

/// Unique identifier for a column.
// `Ord` so panes can key an ordered map: iteration order is then stable frame to
// frame, which matters wherever per-pane state is *drawn* (e.g. each pane's
// scrollback-search bar) and would otherwise wander with hash seeding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PaneId(pub u64);

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnId(pub u64);

/// Width of a column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// Proportion of the current view width (0.0 - 1.0).
    Proportion(f64),
    /// Fixed width in logical pixels.
    Fixed(f64),
}

/// Sizing mode for a column or pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizingMode {
    Normal,
    Maximized,
    Fullscreen,
}

impl SizingMode {
    #[inline]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        matches!(self, Self::Maximized)
    }

    #[inline]
    pub fn is_fullscreen(&self) -> bool {
        matches!(self, Self::Fullscreen)
    }
}

/// 2D position in logical pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn downscale(&self, factor: f64) -> Self {
        Self {
            x: self.x / factor,
            y: self.y / factor,
        }
    }
}

impl std::ops::Add for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl std::ops::Sub for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl std::ops::AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl std::ops::SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

/// 2D size in logical pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub const fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }

    pub fn to_point(&self) -> Point {
        Point::new(self.w, self.h)
    }
}

impl From<Point> for Size {
    fn from(p: Point) -> Self {
        Self::new(p.x, p.y)
    }
}

/// Rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rectangle {
    pub loc: Point,
    pub size: Size,
}

impl Rectangle {
    pub const fn new(loc: Point, size: Size) -> Self {
        Self { loc, size }
    }

    pub fn from_size(size: Size) -> Self {
        Self::new(Point::default(), size)
    }

    pub fn contains(&self, point: Point) -> bool {
        self.loc.x <= point.x
            && point.x < self.loc.x + self.size.w
            && self.loc.y <= point.y
            && point.y < self.loc.y + self.size.h
    }

    pub fn intersection(&self, other: Self) -> Option<Self> {
        let x1 = self.loc.x.max(other.loc.x);
        let y1 = self.loc.y.max(other.loc.y);
        let x2 = (self.loc.x + self.size.w).min(other.loc.x + other.size.w);
        let y2 = (self.loc.y + self.size.h).min(other.loc.y + other.size.h);

        if x1 < x2 && y1 < y2 {
            Some(Self::new(Point::new(x1, y1), Size::new(x2 - x1, y2 - y1)))
        } else {
            None
        }
    }
}

/// Layout options derived from config.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutOptions {
    /// Gap between columns and panes in logical pixels.
    pub gaps: f64,
    /// Whether to center the focused column.
    pub center_focused_column: CenterFocusedColumn,
    /// Center single column even if it fits.
    pub always_center_single_column: bool,
    /// Default width for new columns.
    pub default_column_width: Option<ColumnWidth>,
    /// **The largest** the overview draws things, as a fraction of life size — niri's
    /// `overview { zoom }`.
    ///
    /// **Everything is drawn at real size times the resolved zoom**, which is what makes the map a
    /// map: a workspace row is the viewport's shape, a column keeps its real proportion of the
    /// screen, and a strip scrolled past one screen really is wider than its row.
    ///
    /// It is a **maximum**, not the zoom (changed 2026-08-12, F003/P082/T419). An exposé shows
    /// everything at once — that is what the name means — so the map fits itself to the view and
    /// uses this only to stop a small session being blown up to fill the screen. The floor is
    /// [`overview_min_card_width`](Self::overview_min_card_width).
    pub overview_scale: f64,
    /// **The narrowest a card may be drawn**, in logical pixels — the floor under the overview's
    /// computed zoom.
    ///
    /// A *scale* floor would not say what it means: columns differ in width, so the same scale
    /// leaves one session legible and another a set of slivers. A width does, and it is the
    /// question actually being asked — "can I still tell what that pane is?".
    ///
    /// Below it the map stops shrinking and scrolls instead, which is the only honest answer for a
    /// session of thirty panes.
    pub overview_min_card_width: f64,
    /// The scale the overview **opens from**, relative to [`overview_scale`](Self::overview_scale)
    /// — it animates out of this and back into it on the way out. `1.0` means no animation.
    ///
    /// Here beside the scale it animates from rather than as a constant in the surface, for the
    /// same reason the scale itself is: a value the user sets belongs where the layout keeps its
    /// other answers, not in the one screen that happens to read it first.
    pub overview_zoom_from: f64,
    /// Gap between workspace rows in the overview, as a **fraction of a screen height** (times the
    /// scale) — niri's `workspace_gap = view_size.h * 0.1 * zoom`.
    ///
    /// A fraction rather than a pixel count, because the gap has to stay the same *picture* at
    /// every scale: 16px is a canyon at 0.75 and a hairline at 0.1.
    pub overview_gap: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            gaps: 8.0,
            center_focused_column: CenterFocusedColumn::Never,
            always_center_single_column: false,
            default_column_width: Some(ColumnWidth::Proportion(0.5)),
            // niri's defaults: zoom 0.5, gap a tenth of a screen.
            overview_scale: 0.5,
            overview_zoom_from: 0.8,
            overview_gap: 0.1,
            // **Off by default: an exposé shows everything.** A floor sounds prudent and is not —
            // set to 140px it bound on an ordinary session of three workspaces, holding the zoom a
            // hair above the vertical fit so two of the three rows fell off the bottom (Antonio,
            // with a screenshot, 2026-08-12). Small cards are an honest picture of a large session;
            // hiding two thirds of it is not. A user who would rather scroll than squint sets one.
            overview_min_card_width: 0.0,
        }
    }
}

/// When to center the focused column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CenterFocusedColumn {
    /// Focusing an off-screen column scrolls it to the nearest edge and leaves it there.
    ///
    /// **The default, matching niri's.** It was `OnOverflow` while nothing could configure it; a
    /// view that re-centres itself on every focus change moves more than the user asked for, and
    /// the ones who want that now have a setting to say so.
    #[default]
    Never,
    /// Center only when the column doesn't fit with neighbors.
    OnOverflow,
    Always,
}

/// Drop target for interactive move.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneInsertTarget {
    /// Insert as a new column at the given index.
    NewColumn(usize),
    /// Insert into an existing column at the given pane index.
    InColumn { col_idx: usize, pane_idx: usize },
}
