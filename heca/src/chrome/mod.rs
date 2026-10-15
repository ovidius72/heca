//! **The chrome** — everything the app draws around and over the panes: sidebars, bars, pane
//! headers, docks, overlays, the palette, menus and pick letters.
//!
//! This file only lists the parts and what they export. The `use` lines at the bottom are shared by
//! the child modules, which take them with `use super::*`.

pub(crate) mod theme;
pub(crate) use theme::{SidebarLook, alpha_u8, chrome_colors, chrome_gui_theme, chrome_status};
pub(crate) mod signals;
pub(crate) use signals::{ChromeSignals, sync_chrome_signals, sync_chrome_state};
pub(crate) mod column;
pub(crate) use workspace::{clear_workspace, seat_workspace};
pub(crate) mod startup_queue;
pub(crate) use startup_queue::StartupQueue;
pub(crate) mod pane;
pub(crate) mod pane_header;
pub(crate) mod terminal;
pub(crate) mod workspace;
pub(crate) use pane_header::{
    ActionShortcuts, HeaderEnv, PaneHeader, action_tooltip, give_header_facts, home_relative_path,
    pane_header_inputs, pane_info_view,
};
pub(crate) mod drag;
pub(crate) mod pane_items;
pub(crate) use drag::{ChromeDragItem, DragItemRegistry, pane_drop_row};
pub(crate) use expose::expose_show_cwd;
pub(crate) use pane_items::{PaneButtons, PaneChips, PaneFacts, PaneRowLines};
pub(crate) mod hint;
// The hint half of the chrome — declarations, the visibility rule, and who is lettering what.
// Re-exported so call sites keep naming `crate::chrome::…` while the code lives where it belongs.
pub(crate) use hint::{
    HintTarget, active_hint_targets, clear_hint_letters, fire_hint, fire_widget_action,
    target_identity,
};
pub(crate) mod context_menu;
mod contribution;
mod events;
mod expose;
mod regions;
pub(crate) use expose::register as register_expose;

mod dispatch;
mod focus;
mod scene;
pub(crate) use dispatch::{
    cancel_every_tree, deliver, dispatch_surface_pointer, drag_in_flight, drain_pending_drops,
    drain_pending_menus, next_redraw_across_trees, open_declared_menu_for_focus,
};
pub(crate) use scene::{build_chrome_root, paint_bell_flash, paint_chrome_root, paint_link_hints};
mod layers_glue;
pub(crate) use layers_glue::{rebuild_named_layer, register_named_layer};
mod notification_layer;
pub(crate) use notification_layer::{
    mount_notification_stack, surface_key as notification_surface_key,
};
mod host;
mod identity;
pub(crate) use identity::warn_author;
mod layers;
mod overlay;
mod palette;
mod state;
pub(crate) use expose::record_expose_cursor;
pub(crate) use layers::{HOST_OWNER, LayerId, LayerKind, LayerRegistry, surface_key_of};
// The described-UI vocabulary lives in `heca-view` so a plugin can depend on it; re-exported so
// the app has one path to it.
pub(crate) use heca_view::{Intent, PropMap, PropValue};
// Turns a described tree into widgets; it lives below this crate so the showcase can use it too.
pub(crate) use heca_view_realize::{FormBindings, IntentEmitter, realize};
// The overlay stack. `OverlayId` is public because `WmAction` carries it.
pub use overlay::OverlayId;
pub(crate) use overlay::{
    DropdownItem, DropdownSpec, ModalAction, ModalResult, ModalSpec, OverlayHost,
    collect_form as collect_overlay_form, content_covered, open_dropdown, open_modal,
    resolve as resolve_overlay, top_modal,
};
pub use context_menu::MenuBuild;
pub(crate) use context_menu::{
    ContextMenuRegistry, ContextPath, open_context_menu_for, resolve_active_context,
};
pub use contribution::{ContextMenuContribution, Contribution, RegionSet};
pub use events::{ChromeEvent, ChromeEventBus, ChromeSubscription, RegionId, SidebarSelection};
pub(crate) use palette::open_command_palette;
pub use regions::{PlaceDock, RegionMap, regions, shown_from_settings};
pub(crate) use focus::{dock_candidates, navigable_dock, placement_for, region_on_screen};
pub use host::ChromeHost;
pub use state::{SharedChromeState, WorkspacesContainerState};
pub use contribution::{BuildCx, ContainerContribution, ContainerId, WidgetModel};

use heca_core::layout::types::{Point, Rectangle, Size};

mod division;
mod emitter;
mod names;
mod retained;
mod window_root;
pub use division::ChromeConfig;
pub use emitter::{ChromeIntentEmitter, fires, picks};
pub(crate) use emitter::layer_emitter;
pub(crate) use names::{
    COLUMN_PICK_SCOPE, DOCK_PICK_SCOPE, PANE_DRAG_KIND, column_key, pane_control_key, pane_id_of_key,
    pane_key, row_key, row_of_key, slot_key, slot_of_key,
};
pub(crate) use retained::{RetainedChrome, chrome_signature};
pub(crate) use window_root::{
    CHROME_KEY, new_window_root, place_surface, remove_surface, seat_chrome, surface_node,
    surface_node_mut, surface_slot,
};

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
