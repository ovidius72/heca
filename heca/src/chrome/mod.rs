//! **The chrome** — everything the app draws around and over the panes: sidebars, bars, pane
//! headers, docks, overlays, the palette, menus and pick letters.
//!
//! This file only lists the parts and what they export. The `use` lines at the bottom are shared by
//! the child modules, which take them with `use super::*`.

pub(crate) mod docks;
pub(crate) mod hint;
pub(crate) mod input;
pub(crate) mod panes;
mod state;
pub(crate) mod surfaces;
pub(crate) mod window;

// ── docks ──
pub use docks::contribution::{
    BuildCx, ContainerContribution, ContainerId, ContextMenuContribution, Contribution, RegionSet,
    WidgetModel,
};
pub(crate) use docks::focus::{dock_candidates, navigable_dock, placement_for, region_on_screen};
pub use docks::host::ChromeHost;
pub use docks::regions::{PlaceDock, RegionMap, regions, shown_from_settings};
pub(crate) use docks::startup_queue::StartupQueue;

// ── input ──
pub(crate) use input::dispatch::{
    cancel_every_tree, deliver, dispatch_surface_pointer, drag_in_flight, drain_pending_drops,
    drain_pending_menus, next_redraw_across_trees, open_declared_menu_for_focus,
};
pub(crate) use input::drag::{ChromeDragItem, DragItemRegistry, pane_drop_row};
pub(crate) use input::emitter::layer_emitter;
pub use input::emitter::{ChromeIntentEmitter, fires, picks};
pub(crate) use input::identity::warn_author;
pub(crate) use input::names::{
    COLUMN_PICK_SCOPE, DOCK_PICK_SCOPE, PANE_DRAG_KIND, column_key, pane_control_key, pane_id_of_key,
    pane_key, row_key, row_of_key, slot_key, slot_of_key,
};

// ── panes ──
pub(crate) use panes::pane_header::{
    ActionShortcuts, HeaderEnv, PaneHeader, action_tooltip, give_header_facts, home_relative_path,
    pane_header_inputs, pane_info_view,
};
pub(crate) use panes::pane_items::{PaneButtons, PaneChips, PaneFacts, PaneRowLines};
pub(crate) use panes::workspace::{clear_workspace, seat_workspace};

// ── hint ──
// The hint half of the chrome — declarations, the visibility rule, and who is lettering what.
pub(crate) use hint::{
    HintTarget, active_hint_targets, clear_hint_letters, fire_hint, fire_widget_action,
    target_identity,
};

// ── state ──
pub use state::events::{
    ChromeEvent, ChromeEventBus, ChromeSubscription, RegionId, SidebarSelection,
};
pub use state::{PaneRuntimeStore, SharedChromeState, WorkspacesContainerState};

// ── surfaces ──
pub use surfaces::context_menu::MenuBuild;
pub(crate) use surfaces::context_menu::{
    ContextMenuRegistry, ContextPath, open_context_menu_for, resolve_active_context,
};
pub(crate) use surfaces::expose::{record_expose_cursor, register as register_expose};
pub(crate) use surfaces::glue::{rebuild_named_layer, register_named_layer};
pub(crate) use surfaces::layers::{HOST_OWNER, LayerId, LayerKind, LayerRegistry, surface_key_of};
pub(crate) use surfaces::notifications::{
    mount_notification_stack, surface_key as notification_surface_key,
};
// The overlay stack. `OverlayId` is public because `WmAction` carries it.
pub use surfaces::overlay::OverlayId;
pub(crate) use surfaces::overlay::{
    DropdownItem, DropdownSpec, ModalAction, ModalResult, ModalSpec, OverlayHost,
    collect_form as collect_overlay_form, content_covered, open_dropdown, open_modal,
    resolve as resolve_overlay, top_modal,
};
pub(crate) use surfaces::palette::open_command_palette;

// ── window ──
pub use window::division::ChromeConfig;
pub(crate) use window::retained::{RetainedChrome, chrome_signature};
pub(crate) use window::scene::{
    build_chrome_root, paint_bell_flash, paint_chrome_root, paint_link_hints,
};
pub(crate) use window::signals::{ChromeSignals, sync_chrome_signals, sync_chrome_state};
pub(crate) use window::theme::{
    SidebarLook, alpha_u8, chrome_colors, chrome_gui_theme, chrome_status,
};
pub(crate) use window::window_root::{
    CHROME_KEY, new_window_root, place_surface, remove_surface, seat_chrome, surface_node,
    surface_node_mut, surface_slot,
};

// The described-UI vocabulary lives in `heca-view` so a plugin can depend on it; re-exported so
// the app has one path to it.
pub(crate) use heca_view::{Intent, PropMap, PropValue};
// Turns a described tree into widgets; it lives below this crate so the showcase can use it too.
pub(crate) use heca_view_realize::{FormBindings, IntentEmitter, realize};

use heca_core::layout::types::{Point, Rectangle, Size};

use heca_config::programs::{ProgramIcon, ProgramsConfig};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_grid_ui::builders::{EventExt, HintExt, IdentityExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate};
use heca_grid_ui::style::{Length, Spacing, WidgetSize};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    Flex, FocusScope, Glyph, HintPlacement, Icon, IconButton, KeyHint, Label, Pane, Separator,
    Surface, Tag, TooltipSide,
};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
