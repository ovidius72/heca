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
//! The widget catalog is `docs/widgets.md`; outstanding work is in the planner.

// So the generated `impl ::heca_grid_ui::SetProp` from `#[props]` resolves inside this crate too,
// not only in downstream ones. Without it the macro would need a different path when used here.
extern crate self as heca_grid_ui;

pub mod action;
pub mod animation;
pub mod builders;
/// The library's clock: what time it is, for every widget that moves with time. See [`clock`].
pub mod clock;
pub mod color;
pub mod component;
pub mod cursor;
pub mod drag;
pub mod effects;
pub mod event;
mod flash;
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
pub mod tone;
pub use flash::FlashHandle;
pub mod widgets;

/// Geometry primitives, re-exported from `heca-core` so consumers of the public
/// API (which uses `Rectangle`/`Size`) need no direct `heca-core` dependency.
pub use heca_core::layout::{Point, Rectangle, Size};

pub use action::{Action, SignalData};
pub use animation::{
    Animate, Animation, AnimationFrame, Fade, Presence, Sequence, Slide, SlideFrom, Zoom, ZoomFade,
    smoothstep,
};
pub use builders::{
    BoundChildren, DragExt, EventExt, FlashExt, FocusExt, HintExt, IdentityExt, IntoTextValue,
    LayoutExt, MapChildren, MenuExt, Parent, PlaceExt, PresenceExt, StyleExt, ToneExt, TooltipExt,
};
pub use color::Color;
pub use component::{
    Base, Component, Event, FrameWake, GridKey, Handled, Modifiers, PaintCx, WidgetIntent,
    WidgetModel, area, area_slot, click, collect_damage, deliver, dispatch, frame_served,
    frame_waker, holds_keyboard, install_frame_request, needs_layout, overlay_occluded_at,
    paint_child, request_frame, shift_subtree, unmount_child,
};
pub use cursor::{Cursor, cursor_at};
pub use drag::{DropAction, DropHit, DropSide, Flow, resolve_at, source_at};
pub use effects::{Attention, Eased, Flash};
pub use event::typed_text;
pub use event::{
    DragEvent, EventCx, EventKind, Handlers, PointerButton, PointerEvent, RawPointer,
    RawPointerKind, WheelTurn,
};
pub use focus::{
    FocusManager, KeyboardArea, KeyboardOwner, contains_keyboard, focus_scope, focus_widget_by_id,
    keyboard_area, keyboard_holder, keyboard_owner, page_scope, release_scope, replace_child,
    settle_focus,
};
pub use hint::{
    DeclaredAction, Hint, SurfaceHints, clear_hints, collect_actions, collect_hints,
    collect_hints_by_surface, fire_action, fire_hint, hint_intent, hint_targets_of, offer_hint,
    offer_hint_by_key, select_key_in_scope, set_selected_by_key, set_text_by_key,
};
pub use keymap::{KeyChord, KeyPress, Keymap};
pub use layout::LayoutEngine;
pub use menu::{has_menu_sink, install_menu_sink, open_for_keyboard};
pub use nav::{
    child_name, collect_keys, identity_of, key_at, node_with_key, node_with_key_mut, path_with_key,
};
pub use pointer::{
    EdgeScroll, PointerState, clear_hover, dragged_kind, dragging, edge_scroll, hit_test,
    pointer_held,
};
pub use scene::{
    BackdropAt, BaseRun, DrawCommand, Escape, FontRole, HostWork, ImageAt, Scene, SurfaceAt,
    TextStyle,
};
pub mod prop;
pub mod props;
pub use heca_grid_ui_macros::{PropName, prop, props};
pub use prop::{PropInput, PropName, SetProp};
pub use style::{
    Align, Direction, Fill, FontWeight, GridCell, Justify, Layout, Length, LetterSpacing,
    LineHeight, Spacing, Style, TextTransform, ThemeFill, Track, Visual, WidgetSize,
};
pub use theme::{FrameStyle, GlowLevel, Intensity, Theme};
pub use tone::Tone;
pub use widgets::{
    ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button, ButtonVariant,
    Caption, Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette, Container, Dialog,
    DockFrame, DockFrameVariant, DotStatus, Elapsed, ElapsedFormat, Ellipsis, Flex, FocusScope,
    Gauge, Glyph, Grid, HintPlacement, Icon, IconButton, Image, ImageFit, ImageState, Input, Item,
    ItemGroup, KeyCap, KeyHint, KeyHintGroup, KeycapVariant, Label, LabelSide, LandingSlot, Line,
    MarkerGroup, MessageFeed, MessageList, NfGlyph, NfIcon, Orientation, Overlay, OverlayPosition,
    Pane, Panel, ProgressBar, RailCell, RegionMode, Resizable, RevealAlign, Row, RowVariant,
    ScrollAxes, ScrollBar, ScrollInfo, ScrollRegion, Select, Separator, Spinner, Splitter,
    StatusDot, Surface, Svg, Tabs, Tag, Tile, Toast, ToastAction, ToastPosition, ToastSeverity,
    ToastSpec, ToastStack, Toggle, Tooltip, TooltipSide, TreeLayout, Visibility, container,
    keycap_size, keycap_size_nf, paint_keycap, paint_keycap_nf,
};

/// Common imports for building UIs.
pub mod prelude {
    pub use crate::FlashHandle;
    pub use crate::action::{Action, SignalData};
    pub use crate::animation::{
        Animate, Animation, AnimationFrame, Fade, Sequence, Slide, SlideFrom, Zoom, ZoomFade,
    };
    pub use crate::builders::{
        BoundChildren, DragExt, EventExt, FlashExt, FocusExt, HintExt, IdentityExt, IntoTextValue,
        LayoutExt, MapChildren, MenuExt, Parent, PlaceExt, PresenceExt, StyleExt, ToneExt,
        TooltipExt,
    };
    pub use crate::color::Color;
    pub use crate::component::{
        Component, Event, GridKey, Handled, Modifiers, WidgetIntent, WidgetModel,
    };
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
        Align, Direction, Fill, FontWeight, GridCell, Justify, Length, LetterSpacing, LineHeight,
        Spacing, TextTransform, ThemeFill, Track, WidgetSize,
    };
    pub use crate::theme::{FrameStyle, GlowLevel, Intensity, Theme};
    pub use crate::tone::Tone;
    pub use crate::widgets::{
        ActiveMarker, Alert, AlertVariant, Badge, BadgeButton, BadgeVariant, Button, ButtonGroup,
        ButtonVariant, Caption, Card, Checkbox, Choice, ChromeRegion, Command, CommandPalette,
        Container, ContextMenu, Dialog, DockFrame, DockFrameVariant, DotStatus, Elapsed,
        ElapsedFormat, Ellipsis, Flex, FocusScope, Gauge, Glyph, Grid, HintPlacement, Icon,
        IconButton, Input, Item, ItemGroup, KeyHint, KeyHintGroup, Label, LabelSide, LandingSlot,
        Line, MarkerGroup, MenuEntry, MenuItem, MessageFeed, MessageList, NfGlyph, NfIcon,
        Orientation, Pane, ProgressBar, RailCell, RegionMode, Resizable, RevealAlign, Row,
        RowVariant, ScrollAxes, ScrollBar, ScrollInfo, ScrollRegion, Select, Separator, Spinner,
        StatusDot, Surface, Tabs, Tag, Tile, Toast, ToastAction, ToastPosition, ToastSeverity,
        ToastSpec, ToastStack, Toggle, Tooltip, TooltipSide, TreeLayout, Visibility, container,
    };
}

/// Shared local image sources, asynchronous decoding and vector rasterisation.
pub mod image;
