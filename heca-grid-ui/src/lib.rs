//! # heca-grid-ui
//!
//! A GPU-free, signal-driven, composable component library that gives heca a
//! *Tron/Grid* visual identity. It is a **component framework**, not a theme.
//!
//! ## Architecture
//! - **GPU-free.** Components emit a [`Scene`] (a list of [`DrawCommand`]s); the
//!   renderer rasterizes it. This crate never depends on `wgpu`/`winit`, so it
//!   stays headless and unit-testable. (Same boundary as `heca-core`.)
//! - **Composition over inheritance.** Every widget embeds a [`Base`] and
//!   implements [`Component`] — "extends base" the idiomatic-Rust way.
//! - **Reactive.** Fine-grained signals via the [`reactive`] facade.
//! - **Flex layout.** Widget layout via `taffy`, through [`Style`] (component
//!   layout only — not the window-manager layout, which lives in `heca-core`).
//!
//! ```
//! use heca_grid_ui::prelude::*;
//!
//! let ui = Flex::row()
//!     .gap(8.0)
//!     .padding(12.0)
//!     .child(Label::new("STATUS"))
//!     .child(Label::new("ONLINE"));
//! ```
//!
//! See `grid-ui-plan.md` for the full phase plan.

pub mod action;
pub mod builders;
pub mod color;
pub mod component;
pub mod drag;
pub mod effects;
pub mod focus;
pub mod font;
pub mod hint;
pub mod keymap;
pub mod layout;
pub mod reactive;
pub mod scene;
pub mod style;
pub mod theme;
pub mod widgets;

/// Geometry primitives, re-exported from `heca-core` so consumers of the public
/// API (which uses `Rectangle`/`Size`) need no direct `heca-core` dependency.
pub use heca_core::layout::{Point, Rectangle, Size};

pub use action::{Action, SignalData};
pub use builders::{DragExt, HintExt, LayoutExt, Parent, StyleExt};
pub use color::Color;
pub use component::{
    Base, Component, Event, GridKey, Handled, Modifiers, PaintCx, WidgetIntent, collect_damage,
    overlay_occluded_at,
    install_frame_request, request_frame,
};
pub use keymap::{KeyChord, Keymap};
pub use drag::{DragContext, DragItemId, DragLabel, DragPhase, DragSurfaceId, DropHit, DropSide, SurfaceDragState, resolve_at, source_at};
pub use hint::{HintTargetId, collect_hint_targets};
pub use effects::{Attention, Flash};
pub use focus::FocusManager;
pub use layout::LayoutEngine;
pub use scene::{DrawCommand, FontRole, Scene, TextStyle};
pub use style::{Align, Direction, GridCell, Justify, Length, Spacing, Style, Track, WidgetSize};
pub use theme::{GlowLevel, Intensity, Theme};
pub use widgets::{
    container, ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button,
    ButtonVariant, Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette, Container, Dialog, DockFrame, DotStatus, Flex, Gauge, Glyph,
    Grid, HintPlacement, Icon, IconButton, Input, Item, ItemGroup, KeyHint, Label,
    LabelSide, MarkerGroup, Orientation, Pane, PaneFrame, ProgressBar, RailCell, RegionMode, Row, ScrollAxes, ScrollBar, ScrollRegion, Select, Separator, Spinner,
    StatusDot, Surface, Tabs, Tag, Toast, ToastCorner, ToastSeverity, ToastSpec, ToastStack, Toggle, Tooltip, TooltipSide, Visibility, KeycapVariant, keycap_size, paint_keycap,
};

/// Common imports for building UIs.
pub mod prelude {
    pub use crate::action::{Action, SignalData};
    pub use crate::builders::{DragExt, HintExt, LayoutExt, Parent, StyleExt};
    pub use crate::color::Color;
    pub use crate::component::{Component, Event, GridKey, Handled, Modifiers, WidgetIntent};
    pub use crate::keymap::{KeyChord, Keymap};
    pub use crate::drag::{DragContext, DragItemId, DragLabel, DragPhase, DragSurfaceId, SurfaceDragState};
    pub use crate::hint::{HintTargetId, collect_hint_targets};
    pub use crate::focus::FocusManager;
    pub use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
    pub use crate::scene::{TextAlign, TextStyle};
    pub use crate::style::{Align, Direction, GridCell, Justify, Length, Spacing, Track, WidgetSize};
    pub use crate::theme::{GlowLevel, Intensity, Theme};
    pub use crate::widgets::{
        container, ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button,
        ButtonVariant, Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette, Container, ContextMenu, Dialog, DockFrame, DotStatus, Flex, Gauge,
        Glyph, Grid, HintPlacement, Icon, IconButton, Input, Item, ItemGroup, KeyHint, Label,
        LabelSide, MarkerGroup, MenuEntry, Orientation, Pane, PaneFrame, ProgressBar, RailCell, RegionMode, Row, ScrollAxes, ScrollBar, ScrollRegion, Select, Separator, Spinner,
        StatusDot, Surface, Tabs, Tag, Toast, ToastCorner, ToastSeverity, ToastSpec, ToastStack, Toggle, Tooltip, TooltipSide, Visibility,
    };
}
