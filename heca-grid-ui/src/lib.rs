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

// So the generated `impl ::heca_grid_ui::SetProp` from `#[props]` resolves inside this crate too,
// not only in downstream ones. Without it the macro would need a different path when used here.
extern crate self as heca_grid_ui;

pub mod action;
pub mod animation;
pub mod builders;
pub mod color;
pub mod component;
pub mod drag;
pub mod effects;
pub mod event;
pub mod focus;
pub mod font;
pub mod hint;
pub mod intent;
pub mod keymap;
pub mod layout;
pub mod menu;
pub mod nav;
pub mod order;
pub mod pointer;
pub mod reactive;
pub mod reconcile;
pub mod scene;
pub mod search;
pub mod style;
pub mod theme;
pub mod widgets;

/// Geometry primitives, re-exported from `heca-core` so consumers of the public
/// API (which uses `Rectangle`/`Size`) need no direct `heca-core` dependency.
pub use heca_core::layout::{Point, Rectangle, Size};

pub use action::{Action, SignalData};
pub use animation::{
    Animate, Animation, AnimationFrame, Fade, Presence, Sequence, Slide, SlideFrom, Zoom, ZoomFade,
    smoothstep,
};
pub use builders::{ComponentExt, LayoutExt, Parent, PlaceExt, StyleExt};
pub use color::Color;
pub use component::{
    Base, Component, Event, GridKey, Handled, Modifiers, PaintCx, WidgetIntent, area, area_slot,
    collect_damage, deliver, dispatch, frame_served, holds_keyboard, install_frame_request,
    needs_layout, overlay_occluded_at, paint_child, request_frame, shift_subtree,
};
pub use drag::{DropAction, DropHit, DropSide, resolve_at, source_at};
pub use effects::{Attention, Eased, Flash};
pub use event::typed_text;
pub use event::{
    DragEvent, EventCx, EventKind, Handlers, PointerButton, PointerEvent, RawPointer,
    RawPointerKind,
};
pub use focus::FocusManager;
pub use hint::{
    DeclaredAction, Hint, SurfaceHints, clear_hints, collect_actions, collect_hints,
    collect_hints_by_surface, fire_action, fire_hint, hint_intent, hint_targets_of, offer_hint,
    offer_hint_by_key, set_selected_by_key, set_text_by_key,
};
pub use keymap::{KeyChord, KeyPress, Keymap};
pub use layout::LayoutEngine;
pub use menu::{has_menu_sink, install_menu_sink, open_for_keyboard};
pub use nav::{child_name, collect_keys, identity_of, key_at, node_with_key, node_with_key_mut};
pub use pointer::{PointerState, clear_hover, dragging, hit_test};
pub use scene::{DrawCommand, Escape, FontRole, Scene, TextStyle};
pub mod prop;
pub use heca_grid_ui_macros::{PropName, prop, props};
pub use prop::{PropInput, PropName, SetProp};
pub use style::{
    Align, Direction, GridCell, Justify, Layout, Length, Spacing, Style, Track, Visual, WidgetSize,
};
pub use theme::{FrameStyle, GlowLevel, Intensity, Theme};
pub use widgets::{
    ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button, ButtonVariant,
    Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette, Container, Dialog, DockFrame,
    DotStatus, Ellipsis, Flex, FocusScope, Gauge, Glyph, Grid, HintPlacement, Icon, IconButton,
    Input, Item, ItemGroup, KeyCap, KeyHint, KeyHintGroup, KeycapVariant, Label, LabelSide,
    MarkerGroup, NfGlyph, NfIcon, Orientation, Overlay, OverlayPosition, Pane, Panel, ProgressBar,
    RailCell, RegionMode, RevealAlign, Row, ScrollAxes, ScrollBar, ScrollInfo, ScrollRegion,
    Select, Separator, Spinner, StatusDot, Surface, Tabs, Tag, Tile, Toast, ToastAction,
    ToastPosition, ToastSeverity, ToastSpec, ToastStack, Toggle, Tooltip, TooltipSide, Visibility,
    container, keycap_size, keycap_size_nf, paint_keycap, paint_keycap_nf,
};

/// Common imports for building UIs.
pub mod prelude {
    pub use crate::action::{Action, SignalData};
    pub use crate::animation::{
        Animate, Animation, AnimationFrame, Fade, Sequence, Slide, SlideFrom, Zoom, ZoomFade,
    };
    pub use crate::builders::{ComponentExt, LayoutExt, Parent, PlaceExt, StyleExt};
    pub use crate::color::Color;
    pub use crate::component::{Component, Event, GridKey, Handled, Modifiers, WidgetIntent};
    pub use crate::event::{DragEvent, EventCx, EventKind, PointerButton, PointerEvent};
    pub use crate::focus::FocusManager;
    pub use crate::hint::{
        Hint, SurfaceHints, clear_hints, collect_actions, collect_hints, collect_hints_by_surface,
        fire_action, fire_hint, hint_intent, hint_targets_of, offer_hint, offer_hint_by_key,
    };
    pub use crate::keymap::{KeyChord, Keymap};
    pub use crate::nav::{child_name, collect_keys, identity_of, key_at, node_with_key};
    pub use crate::reactive::{Signal, SignalGet, SignalUpdate, signal};
    pub use crate::scene::{TextAlign, TextStyle};
    pub use crate::style::{
        Align, Direction, GridCell, Justify, Length, Spacing, Track, WidgetSize,
    };
    pub use crate::theme::{FrameStyle, GlowLevel, Intensity, Theme};
    pub use crate::widgets::{
        ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button, ButtonGroup,
        ButtonVariant, Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette, Container,
        ContextMenu, Dialog, DockFrame, DotStatus, Ellipsis, Flex, FocusScope, Gauge, Glyph, Grid,
        HintPlacement, Icon, IconButton, Input, Item, ItemGroup, KeyHint, KeyHintGroup, Label,
        LabelSide, MarkerGroup, MenuEntry, MenuItem, NfGlyph, NfIcon, Orientation, Pane,
        ProgressBar, RailCell, RegionMode, RevealAlign, Row, ScrollAxes, ScrollBar, ScrollInfo,
        ScrollRegion, Select, Separator, Spinner, StatusDot, Surface, Tabs, Tag, Tile, Toast,
        ToastAction, ToastPosition, ToastSeverity, ToastSpec, ToastStack, Toggle, Tooltip,
        TooltipSide, Visibility, container,
    };
}
