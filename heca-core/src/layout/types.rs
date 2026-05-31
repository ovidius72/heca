use std::fmt;

/// Unique identifier for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub u64);

/// Unique identifier for a workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorkspaceId(pub u64);

/// Unique identifier for a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ColumnId(pub u64);

/// Unique identifier for a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(pub u64);

/// Width of a column.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// Proportion of the current view width (0.0 - 1.0).
    Proportion(f64),
    /// Fixed width in logical pixels.
    Fixed(f64),
}

/// Height of a window/pane within a column.
///
/// In vertical layout, all but one pane must be Auto-sized so total height fills the column.
/// In horizontal/stacked layouts, this is ignored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowHeight {
    /// Automatically distributed tile height with weight.
    Auto { weight: f64 },
    /// Fixed window height in logical pixels.
    Fixed(f64),
    /// Preset height index.
    Preset(usize),
}

impl WindowHeight {
    pub const fn auto_1() -> Self {
        Self::Auto { weight: 1.0 }
    }
}

/// Horizontal direction for scroll/focus operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Left,
    Right,
}

/// Vertical direction for workspace/pane operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalDirection {
    Up,
    Down,
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
            Some(Self::new(
                Point::new(x1, y1),
                Size::new(x2 - x1, y2 - y1),
            ))
        } else {
            None
        }
    }
}

/// Working area within a viewport (excludes struts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorkingArea {
    /// Full viewport size.
    pub view_size: Size,
    /// Working area (viewport minus struts/exclusive zones).
    pub area: Rectangle,
    /// Scale factor (e.g., 1.0, 1.5, 2.0).
    pub scale: f64,
}

/// Hit result when querying what's under a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitType {
    /// Hit inside a pane's input region.
    Input,
    /// Hit on chrome (border, title bar) — activates but doesn't send input.
    Activate,
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
    /// Preset column widths.
    pub preset_column_widths: Vec<ColumnWidth>,
    /// Preset window heights.
    pub preset_window_heights: Vec<f64>,
    /// Default column display mode.
    pub default_column_display: ColumnDisplay,
    /// Border configuration.
    pub border: BorderConfig,
    /// Tab indicator configuration.
    pub tab_indicator: TabIndicatorConfig,
    /// Overview scale factor (e.g., 0.25 means workspaces rendered at 25%).
    pub overview_scale: f64,
    /// Gap between workspaces in overview mode.
    pub overview_gap: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            gaps: 8.0,
            center_focused_column: CenterFocusedColumn::OnOverflow,
            always_center_single_column: true,
            default_column_width: Some(ColumnWidth::Proportion(0.5)),
            preset_column_widths: vec![
                ColumnWidth::Proportion(1.0 / 3.0),
                ColumnWidth::Proportion(0.5),
                ColumnWidth::Proportion(2.0 / 3.0),
            ],
            preset_window_heights: vec![],
            default_column_display: ColumnDisplay::Normal,
            border: BorderConfig::default(),
            tab_indicator: TabIndicatorConfig::default(),
            overview_scale: 0.25,
            overview_gap: 16.0,
        }
    }
}

/// When to center the focused column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CenterFocusedColumn {
    Never,
    /// Center only when the column doesn't fit with neighbors.
    #[default]
    OnOverflow,
    Always,
}

/// Column display mode (how panes within a column are shown).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnDisplay {
    /// Panes stacked vertically (NIRI's default).
    #[default]
    Normal,
    /// Panes shown as tabs (only active visible, with tab bar).
    Tabbed,
}

/// Border configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderConfig {
    pub off: bool,
    pub width: f64,
    pub active_color: [f32; 4],
    pub inactive_color: [f32; 4],
}

impl Default for BorderConfig {
    fn default() -> Self {
        Self {
            off: false,
            width: 2.0,
            active_color: [0.0, 0.5, 1.0, 1.0],
            inactive_color: [0.3, 0.3, 0.3, 1.0],
        }
    }
}

/// Tab indicator configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabIndicatorConfig {
    pub off: bool,
    pub height: f64,
    pub hide_when_single_tab: bool,
}

impl Default for TabIndicatorConfig {
    fn default() -> Self {
        Self {
            off: false,
            height: 24.0,
            hide_when_single_tab: true,
        }
    }
}

/// Where to insert a new pane/column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertPosition {
    /// Create a new column at this index.
    NewColumn(usize),
    /// Insert into an existing column at (column_idx, pane_idx).
    InColumn(usize, usize),
    /// Insert as a floating pane.
    Floating,
}

/// Whether to activate a newly added pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivateWindow {
    /// Activate unconditionally.
    Yes,
    /// Activate based on heuristics (default).
    #[default]
    Smart,
    /// Do not activate.
    No,
}

impl ActivateWindow {
    pub fn map_smart(self, f: impl FnOnce() -> bool) -> bool {
        match self {
            Self::Yes => true,
            Self::Smart => f(),
            Self::No => false,
        }
    }
}
