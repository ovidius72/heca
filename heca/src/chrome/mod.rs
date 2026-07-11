//! Chrome metrics and central application constants.
//!
//! UI dimensions, timing defaults, layout proportions, and render parameters
//! that were previously scattered as magic numbers across the codebase.

mod contribution;
mod context_menu;
mod events;
mod host;
mod layers;
mod overlay;
mod realize;
mod state;
mod view;
// Registry API surface consumed by the next migration steps (ShowLayer/HideLayer, the
// confirm dialog as a layer, plugins) — some names not yet referenced in-binary.
#[allow(unused_imports)]
pub(crate) use layers::{DynamicLayer, LayerBand, LayerId, LayerKind, LayerRegistry};
// Declarative UI model (plugin-task-ui-1); consumed by `realize` (ui-3) + Modal body (ui-4).
#[allow(unused_imports)]
pub(crate) use view::{
    Intent, PropMap, PropValue, ViewAlign, ViewNode, ViewSize, ViewVariant, WidgetKind,
};
// Host mapper (plugin-task-ui-3): `ViewNode` → retained grid-ui `Component`. Consumed by the
// OverlayHost/Modal body (ui-4) and plugin panels — not yet referenced in-binary.
#[allow(unused_imports)]
pub(crate) use realize::{realize, FormBindings};
// Host-owned overlay stack (plugin-task-ui-4, §2.7.1/§2.7.2), built on `LayerRegistry`.
// `OverlayId` is pub (carried by `WmAction`); the rest is crate-internal.
pub use overlay::OverlayId;
#[allow(unused_imports)]
pub(crate) use overlay::{
    collect_form as collect_overlay_form, open_dropdown, open_modal, resolve as resolve_overlay,
    top_modal, DropdownItem, DropdownSpec, ModalAction, ModalResult, ModalSpec, OverlayHost,
};
// Context-menu resolution: ContextPath + ContextTarget + ContextMenuRegistry + the unified
// `open_context_menu_for`. Built-in providers seeded at startup; plugins attach via
// `Contribution::ContextMenu` (context-menu-5).
#[allow(unused_imports)]
pub(crate) use context_menu::{
    open_context_menu_for, resolve_active_context, ContextMenuProvider, ContextMenuRegistry,
    ContextPath, ContextTarget, PendingContext,
};
pub use contribution::{Contribution, RegionSet};
pub use events::{ChromeEvent, ChromeEventBus, ChromeSubscription, RegionId, SidebarSelection};
pub use host::ChromeHost;
pub use state::{SharedChromeState, WorkspacesContainerState};
// Contribution/placement API surface for the render + provider phases (plugin-03).
// These are public seam types not yet consumed by name in-binary — same rationale
// as the `#![allow(dead_code)]` carried by the modules that define them.
// `MoveError` is `ChromeHost::move_container`'s error; handlers report it via
// `Display` today and the mouse/DnD path names it in plugin-03.
#[allow(unused_imports)]
pub use contribution::{
    ContainerContribution, ContainerId, OverlaySpec, PanelContribution, StatusSegment,
    ToolbarGroup, WidgetModel,
};
#[allow(unused_imports)]
pub use host::{MountedContribution, MoveError, RegionHost};

use heca_core::layout::ColumnWidth;
use heca_core::layout::types::{Point, Rectangle, Size};
use std::time::Duration;

// ── Chrome metrics ──

/// Default tab bar height in logical pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 32.0;
/// Default status bar height in logical pixels.
pub const DEFAULT_STATUS_BAR_HEIGHT: f32 = 24.0;
/// Default expanded sidebar width in logical pixels.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;

// ── Timing ──

/// Prefix mode auto-exit timeout (ms). After this time with no key, prefix mode cancels.
pub const PREFIX_TIMEOUT: Duration = Duration::from_millis(500);
/// Target frame interval (~60 FPS).
pub const FRAME_INTERVAL: Duration = Duration::from_millis(16);

// ── Layout defaults ──

/// Default width proportion for newly created columns.
pub(crate) const DEFAULT_COLUMN_PROPORTION: f64 = 0.5;
/// Helper to get the default ColumnWidth for new columns.
pub const fn default_column_width() -> ColumnWidth {
    ColumnWidth::Proportion(DEFAULT_COLUMN_PROPORTION)
}

// ── Pane name overlay ──

/// Font size factor for the pane name overlay (fraction of min(width, height)).
pub const PANE_NAME_SIZE_FACTOR: f32 = 0.25;
/// Minimum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MIN: f32 = 24.0;
/// Maximum pane name font size in logical pixels.
pub const PANE_NAME_SIZE_MAX: f32 = 72.0;

// ── Chrome text ──

/// Default font size for chrome text (tab bar, status bar).
pub const CHROME_TEXT_SIZE: f32 = 14.0;

// ── Mouse ──

/// Distance from content area edge that triggers edge scrolling (logical pixels).
pub const EDGE_SCROLL_TRIGGER: f32 = 80.0;

/// Layout configuration for chrome elements around the pane area.
#[derive(Clone, Copy, Debug)]
pub struct ChromeConfig {
    pub tab_bar_height: f32,
    pub status_bar_height: f32,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub sidebar_gap: f32,
}

impl ChromeConfig {
    /// Compute the rectangle available for pane content, given a window size.
    /// Chrome occupies the outer edges; panes get the center.
    pub fn content_rect(&self, window_width: f32, window_height: f32) -> Rectangle {
        let sidebar_gap = self.sidebar_gap.max(0.0);
        let x = self.left_sidebar_width
            + if self.left_sidebar_width > 0.0 {
                sidebar_gap
            } else {
                0.0
            };
        let y = self.tab_bar_height;
        let right_reserved = self.right_sidebar_width
            + if self.right_sidebar_width > 0.0 {
                sidebar_gap
            } else {
                0.0
            };
        let w = (window_width - x - right_reserved.min((window_width - x).max(0.0))).max(0.0);
        let h = (window_height - self.tab_bar_height - self.status_bar_height).max(0.0);
        Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(w as f64, h as f64),
        )
    }
}

// ── Grid-UI chrome scene builder ──────────────────────────────────────────────

use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree};
use heca_config::programs::{ProgramIcon, ProgramsConfig};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_grid_ui::builders::{DragExt, HintExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::drag::{DragItemId, DragPhase, DragSurfaceId};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, signal};
use heca_grid_ui::style::{Align, Justify, Length, Spacing, WidgetSize};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    ActiveMarker, Badge, BadgeButton, DockFrame, Flex, Glyph, HintPlacement, Icon, IconButton,
    KeyHint, Label, MarkerGroup, Pane, Row, ScrollBar, StatusDot, Surface, Tag, Tooltip,
    TooltipSide, Visibility,
};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PaneInfoView {
    icon: Glyph,
    /// Custom name if set, else the program name — the sidebar card's label.
    title: String,
    /// The program/application name (always the process, never the rename) — the info-bar
    /// `AppName` segment shows this, so renaming a pane doesn't hide what's running in it.
    app_name: String,
    /// The process/program name shown as a small dimmed label *next to* a custom name (e.g.
    /// `(nvim)`). `Some` only when the pane has a custom name and `pane_renamed_add_process_name`
    /// is on — a pane that merely tracks its process has the process name *as* its title already.
    process_hint: Option<String>,
    status: ProcessStatus,
    git_branch: Option<String>,
    git_added: Option<String>,
    git_modified: Option<String>,
    git_deleted: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneInfoSignals {
    icon: Signal<Glyph>,
    title_active: Signal<String>,
    title_inactive: Signal<String>,
    title_active_visible: Signal<bool>,
    title_inactive_visible: Signal<bool>,
    /// The dimmed `(process)` suffix text beside a renamed pane's name (e.g. `(nvim)`),
    /// or empty when hidden. Signal-driven so a rename toggles it live without a tree
    /// rebuild (renames update signals, they don't rebuild the sidebar card).
    process_hint: Signal<String>,
    process_hint_visible: Signal<bool>,
    /// The pane's working-directory row text (home-relative path), and its visibility
    /// (`[settings] pane_show_cwd` and the pane has a cwd). Signal-driven so a `cd` in the
    /// pane updates the path live, mirroring the git-branch row.
    cwd: Signal<String>,
    cwd_visible: Signal<bool>,
    status_idle_visible: Signal<bool>,
    status_running_visible: Signal<bool>,
    status_success_visible: Signal<bool>,
    status_error_visible: Signal<bool>,
    git_visible: Signal<bool>,
    git_branch: Signal<String>,
    git_branch_display: Signal<String>,
    git_added_visible: Signal<bool>,
    git_added: Signal<String>,
    git_modified_visible: Signal<bool>,
    git_modified: Signal<String>,
    git_deleted_visible: Signal<bool>,
    git_deleted: Signal<String>,
}

fn program_glyph(icon: ProgramIcon) -> Glyph {
    match icon {
        ProgramIcon::Terminal => Glyph::Terminal,
        ProgramIcon::FileCode => Glyph::FileCode,
        ProgramIcon::Folder => Glyph::Folder,
        ProgramIcon::FolderOpen => Glyph::FolderOpen,
        ProgramIcon::GitBranch => Glyph::GitBranch,
        ProgramIcon::Gear => Glyph::Gear,
        ProgramIcon::Search => Glyph::Search,
    }
}

fn pane_info_view(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    add_process_name: bool,
) -> PaneInfoView {
    let raw = runtime
        .and_then(|pane| pane.program.as_deref())
        .filter(|raw| !raw.is_empty())
        .unwrap_or(fallback_name);
    let program = programs.resolve(raw);
    let git = runtime.and_then(|pane| pane.git.as_ref());
    let program_name = program.name.to_string();
    // A user-set custom name wins over the process-derived program name; the icon still tracks
    // the running program. When the pane has a custom name (and the setting is on), the program
    // name is surfaced separately as `process_hint` (a small dimmed label next to the name).
    let has_custom = custom_name.is_some_and(|name| !name.is_empty());
    let title = if has_custom {
        custom_name.unwrap_or_default().to_string()
    } else {
        program_name.clone()
    };
    let process_hint = (has_custom && add_process_name).then(|| program_name.clone());
    PaneInfoView {
        icon: program_glyph(program.icon),
        title,
        app_name: program_name,
        process_hint,
        status: runtime
            .map(|pane| pane.status.clone())
            .unwrap_or(ProcessStatus::Idle),
        git_branch: git.map(|info| {
            info.branch
                .clone()
                .unwrap_or_else(|| "detached".to_string())
        }),
        git_added: git.and_then(|info| (info.added > 0).then(|| format!("+{}", info.added))),
        git_modified: git
            .and_then(|info| (info.modified > 0).then(|| format!("~{}", info.modified))),
        git_deleted: git.and_then(|info| (info.deleted > 0).then(|| format!("-{}", info.deleted))),
    }
}

/// Left-truncate `text` to `max_chars`, keeping the **tail** with a leading
/// ellipsis (`…/HypeSupport`) — paths read most usefully from the end.
fn truncate_path_left(text: &str, max_chars: usize) -> String {
    let len = text.chars().count();
    if len <= max_chars {
        return text.to_string();
    }
    match max_chars {
        0 => String::new(),
        1 => "…".to_string(),
        n => {
            let tail: String = text.chars().skip(len - (n - 1)).collect();
            format!("…{tail}")
        }
    }
}

/// A path shown home-relative (`/Users/x/proj` → `~/proj`).
fn home_relative_path(path: &std::path::Path) -> String {
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::Path::new(&home);
        if let Ok(rest) = path.strip_prefix(home) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}

/// Build the pane info bar's segmented [`Tag`] from the configured `segments` and
/// the pane's runtime, reusing the same catalog projection as the sidebar card.
/// Segments with no data (e.g. git outside a repo) are skipped; returns `None`
/// when nothing is produced. Used by the terminal pane shell (`app::terminal_render`).
#[expect(
    clippy::too_many_arguments,
    reason = "pane-info projection threads program/name/runtime + layout context explicitly; grouping into a struct is a later chrome refactor"
)]
pub(crate) fn build_pane_info_bar(
    programs: &ProgramsConfig,
    fallback_name: &str,
    custom_name: Option<&str>,
    runtime: Option<&PaneRuntime>,
    segments: &[heca_config::appearance::PaneSegment],
    theme: &GuiTheme,
    max_width: f32,
    font: f32,
) -> Option<Tag> {
    use heca_config::appearance::PaneSegment;

    // The info bar never renders the `(process)` suffix — that's a sidebar-card affordance
    // (see `pane_card`) — so it always projects the view without the hint.
    let view = pane_info_view(programs, fallback_name, custom_name, runtime, false);
    let git = runtime.and_then(|pane| pane.git.as_ref());

    // Collect the produced (icon, text) segments, noting the location (the long,
    // truncatable one), so we can fit the bar to `max_width` before building it.
    let mut items: Vec<(Glyph, String)> = Vec::new();
    let mut location_idx: Option<usize> = None;
    for seg in segments {
        let item = match seg {
            PaneSegment::Location => match runtime.and_then(|pane| pane.cwd.as_ref()) {
                Some(cwd) => {
                    location_idx = Some(items.len());
                    (Glyph::Folder, home_relative_path(cwd))
                }
                None => continue,
            },
            PaneSegment::AppName => (view.icon, view.app_name.clone()),
            PaneSegment::PaneName => {
                if view.title.is_empty() {
                    continue;
                }
                (view.icon, view.title.clone())
            }
            PaneSegment::GitBranch => match git.and_then(|info| info.branch.clone()) {
                Some(branch) => (Glyph::GitBranch, branch),
                None => continue,
            },
            PaneSegment::GitStatus => {
                let Some(info) = git else { continue };
                let mut parts = Vec::new();
                if info.added > 0 {
                    parts.push(format!("+{}", info.added));
                }
                if info.modified > 0 {
                    parts.push(format!("~{}", info.modified));
                }
                if info.deleted > 0 {
                    parts.push(format!("-{}", info.deleted));
                }
                if parts.is_empty() {
                    continue;
                }
                (Glyph::GitCommit, parts.join(" "))
            }
        };
        items.push(item);
    }
    if items.is_empty() {
        return None;
    }

    // Fit to width: if the bar would overflow the pane, shrink the location
    // segment (left-ellipsised) by the overflow. The per-pane render clip is the
    // hard backstop; this keeps it readable instead of a hard cut.
    let char_w = (font * 0.6).max(1.0);
    let per_segment_overhead = font * 2.5; // icon + gaps + segment padding + divider
    let text_chars: usize = items.iter().map(|(_, text)| text.chars().count()).sum();
    let estimated = text_chars as f32 * char_w + items.len() as f32 * per_segment_overhead;
    if estimated > max_width
        && let Some(idx) = location_idx
    {
        let overflow_chars = ((estimated - max_width) / char_w).ceil() as usize;
        let loc_chars = items[idx].1.chars().count();
        let keep = loc_chars.saturating_sub(overflow_chars).max(1);
        items[idx].1 = truncate_path_left(&items[idx].1, keep);
    }

    let mut tag: Option<Tag> = None;
    for (glyph, text) in items {
        // No explicit size → the icon inherits the bar's base font, so glyph and
        // label stay balanced when the bar font changes.
        let leading = Icon::new(glyph).color(theme.colors.foreground);
        tag = Some(match tag.take() {
            None => Tag::new(text).leading(leading),
            Some(existing) => existing.segment_text(text, Some(Box::new(leading))),
        });
    }
    tag
}

// ── In-pane info-bar header: segments (left) + interactive action buttons (right) ──

/// Horizontal margin from the pane edge to the header content (matches the render
/// side's `TITLE_BAR_MARGIN` in `terminal_render.rs`).
const HEADER_MARGIN: f32 = 6.0;
/// Gap between adjacent action buttons (logical px) — tight, so the cluster reads
/// as one control group.
const HEADER_BUTTON_GAP: f32 = 1.0;

/// Font multiplier for a sidebar card's **secondary metadata** — the dimmed `(process)`
/// suffix and the cwd row — smaller than the name so it reads as supporting detail.
const CARD_META_FONT_SCALE: f32 = 0.8;

/// Per-pane context the header buttons need to build their (parameterized) actions
/// and emit them through the app event loop.
pub(crate) struct PaneHeaderCtx {
    pub(crate) pane_id: PaneId,
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    pub(crate) event_proxy: winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
}

/// A retained per-pane info-bar header (segment `Tag` + action `IconButton`s).
/// Rebuilt only when [`pane_header_key`] changes (so button hover/press signals
/// survive across frames); re-laid-out + positioned every frame by
/// [`sync_pane_headers`]; painted read-only in `terminal_render` and dispatched
/// pointer events by `mouse.rs`.
pub(crate) struct RetainedPaneHeader {
    pub(crate) root: Flex,
    /// Content key (see [`pane_header_key`]) the tree was built from.
    pub(crate) key: String,
    /// The contiguous [`HintTargetId`](heca_grid_ui::HintTargetId) range this header's
    /// action buttons registered into the shared [`HintTargetRegistry`]. Removed from
    /// the shared map when the header is rebuilt or its pane is pruned, so the universal
    /// KeyHint picker (`prefix+/`) can target pane-header buttons without stale ids.
    pub(crate) hint_range: std::ops::Range<usize>,
}

/// Retained per-pane terminal viewport widgets (scrollbar + scrolled-up badge).
/// Built once per pane, updated/repositioned every frame by
/// [`sync_pane_viewport_widgets`], painted read-only in `terminal_render`, and
/// dispatched pointer events in `events.rs`.
pub(crate) struct RetainedPaneViewportWidgets {
    pub(crate) scrollbar: ScrollBar,
    pub(crate) badge: BadgeButton,
}

/// Resolved display shortcuts for **every bound action**, keyed by config name
/// (`"close"`, `"sidebar_left"`, …). Built once at config load/reload from the
/// default+user keybindings and stored on `AppState`; any button looks its own
/// shortcut up by the name of the action it triggers, so the choice is never
/// hand-made by a caller or plugin. The leader renders through the central
/// [`PREFIX_SYMBOL`](crate::shortcut) seam, uniform across the app.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct ActionShortcuts(std::collections::HashMap<String, String>);

impl ActionShortcuts {
    /// Resolve a display shortcut for every action that has a binding (user
    /// override or bundled default), mirroring `build_keymap`'s merge.
    pub(crate) fn from_config(config: &heca_config::theme::Config) -> Self {
        let defaults = heca_config::keys::KeysConfig::default();
        let mut map = std::collections::HashMap::new();
        for name in defaults.bindings.keys().chain(config.keys.bindings.keys()) {
            if map.contains_key(name) {
                continue;
            }
            if let Some(s) =
                crate::shortcut::shortcut_for_action(name, &config.keys.bindings, &defaults.bindings)
            {
                map.insert(name.clone(), s);
            }
        }
        Self(map)
    }

    /// The display shortcut for `action_name`, or `None` when unbound.
    pub(crate) fn get(&self, action_name: &str) -> Option<&str> {
        self.0.get(action_name).map(String::as_str)
    }
}

/// The config action name a [`PaneAction`] button triggers — the canonical
/// identity used to resolve its shortcut (the emitted `WmAction` may be a
/// button-only variant like `ClosePaneById`, so the name is the stable key).
pub(crate) fn pane_action_name(action: heca_config::appearance::PaneAction) -> &'static str {
    use heca_config::appearance::PaneAction;
    match action {
        PaneAction::Split => "split_vertical",
        PaneAction::MoveLeft => "move_pane_left",
        PaneAction::MoveRight => "move_pane_right",
        PaneAction::Close => "close",
        PaneAction::Zoom => "zoom_column",
        PaneAction::Float => "float",
    }
}

/// Wrap a clickable `child` in a tooltip = `label` + the action's current shortcut,
/// resolved centrally from `shortcuts` by `action_name`. The one place any button's
/// tooltip is composed: callers name the action, never the shortcut, so a rebind
/// updates every tip and no surface can drift on the leader symbol or format.
fn action_tooltip(
    child: impl Component + 'static,
    action_name: &str,
    label: &str,
    shortcuts: &ActionShortcuts,
) -> Tooltip {
    let tip = match shortcuts.get(action_name) {
        Some(sc) if !sc.is_empty() => format!("{label}  {sc}"),
        _ => label.to_string(),
    };
    Tooltip::new(child, tip).side(TooltipSide::Bottom)
}

/// Map a configured [`PaneAction`] to its `(icon, WM action, label, needs_focus)`.
///
/// `needs_focus` is `true` for **active-targeted** unit actions (`ZoomColumn`/
/// `Float`) — the button must focus its owning pane before dispatching so the
/// action lands on the clicked pane, not whatever happened to be active. The other
/// buttons carry the pane/column in the action itself, so they don't steal focus.
fn pane_action_spec(
    catalog: &crate::actions::ActionCatalog,
    action: heca_config::appearance::PaneAction,
    pane_id: PaneId,
    ws_idx: usize,
    col_idx: usize,
) -> (Glyph, crate::input::WmAction, &'static str, bool) {
    use crate::input::WmAction;
    use heca_config::appearance::PaneAction;
    // Icons come from the action catalog (the single source); the literal is a
    // defensive fallback only, so the bar and the context menu can never drift.
    let icon = |name: &str, fallback: Glyph| catalog.icon(name).unwrap_or(fallback);
    match action {
        PaneAction::Split => (
            // Icon = the add-pane identity (FolderSimplePlus); the shortcut/tooltip name
            // stays `split_vertical` (see `pane_action_name`) so the button still hints `v`.
            icon("add_pane_to_column", Glyph::FolderSimplePlus),
            WmAction::AddPaneToColumn { ws_idx, col_idx },
            "New pane",
            false,
        ),
        PaneAction::MoveLeft => (
            icon("move_pane_left", Glyph::ArrowLineLeft),
            WmAction::MovePaneLeft {
                pane_id: Some(pane_id),
            },
            "Move left",
            false,
        ),
        PaneAction::MoveRight => (
            icon("move_pane_right", Glyph::ArrowLineRight),
            WmAction::MovePaneRight {
                pane_id: Some(pane_id),
            },
            "Move right",
            false,
        ),
        PaneAction::Close => (
            icon("close", Glyph::FolderSimpleMinus),
            WmAction::ClosePaneById { pane_id },
            "Close",
            false,
        ),
        PaneAction::Zoom => (
            icon("zoom_column", Glyph::FrameCorners),
            WmAction::ZoomColumn,
            "Zoom",
            true,
        ),
        PaneAction::Float => (icon("float", Glyph::Cards), WmAction::Float, "Float", true),
    }
}

/// What a pane header *renders* — the projection inputs shared by the rebuild key
/// and the tree builder (groups args so neither fn explodes).
pub(crate) struct PaneHeaderContent<'a> {
    pub(crate) programs: &'a ProgramsConfig,
    pub(crate) fallback_name: &'a str,
    /// User-set override name (wins over the process-derived title), if any.
    pub(crate) custom_name: Option<&'a str>,
    pub(crate) runtime: Option<&'a PaneRuntime>,
    pub(crate) segments: &'a [heca_config::appearance::PaneSegment],
    pub(crate) actions: &'a [heca_config::appearance::PaneAction],
    /// The pane's workspace + column — baked into the split action and tracked in
    /// the rebuild key so the header re-bakes when the pane changes column.
    pub(crate) ws_idx: usize,
    pub(crate) col_idx: usize,
    /// The pane's column is zoomed or full-width → the `zoom` button shows active.
    pub(crate) zoomed: bool,
    /// The pane is in the floating domain → the `float` button shows active and
    /// non-floating buttons (split/zoom/move) are hidden (per the action policy).
    pub(crate) floating: bool,
    /// Tooltip keybind hints (tracked in the key so a config reload rebuilds tips).
    pub(crate) shortcuts: &'a ActionShortcuts,
    /// Runtime action-metadata catalog — the single source of each button's icon (and, later,
    /// plugin-contributed action metadata). Threaded alongside `shortcuts`.
    pub(crate) catalog: &'a crate::actions::ActionCatalog,
}

/// Whether a pane-action button stays visible when its pane is **floating**, driven
/// by the shared [`action_policy`](crate::app::interaction) classification (split /
/// zoom / move are tiled-only ⇒ hidden; float / close are focused-pane-local ⇒ kept).
fn pane_action_visible_when_floating(
    catalog: &crate::actions::ActionCatalog,
    action: heca_config::appearance::PaneAction,
) -> bool {
    // Policy ignores the concrete ids (and the icon), so dummy ids are fine here.
    let (_, wm, _, _) = pane_action_spec(catalog, action, PaneId(0), 0, 0);
    crate::app::interaction::action_allowed_when_floating(&wm)
}

/// One resolved pane-header action button — the generic unit the header renders. The
/// header is a **dynamic vector** of these, so nothing about the button set is baked
/// into the render loop: today they come from `config.toml`'s `[pane] title_actions`
/// (map via [`pane_action_spec`]), and this is also the seam where a **plugin** will
/// append its own buttons once the plugin action surface exists (a plugin declares the
/// same fields: an icon, an action name, and the `WmAction` to emit). The loop in
/// [`build_pane_header`] never matches on the concrete [`PaneAction`] enum — it only
/// reads these fields — so a new source of buttons needs no loop changes.
struct PaneHeaderButton {
    glyph: Glyph,
    /// Canonical action name — the stable key for the tooltip shortcut lookup (the
    /// emitted `WmAction` may be a button-only variant, so the name is the identity).
    action_name: &'static str,
    /// The action the button emits (click) and the KeyHint fires (picker).
    wm_action: crate::input::WmAction,
    label: &'static str,
    /// Active-targeted (zoom/float): must focus the owning pane before the action so it
    /// lands on this pane, not whatever is active. Cleared while the pane is floating.
    needs_focus: bool,
    /// Held-on status (zoomed column / floating pane) → the icon paints as toggled-on.
    is_active: bool,
    /// Destructive (close) → danger-hued glyph + hover/press.
    is_close: bool,
}

/// Resolve the header's action buttons for `content` into the generic
/// [`PaneHeaderButton`] vector the render loop consumes. Floating panes drop the
/// tiled-only buttons (per the shared action policy). This is the single place the
/// button *set* is decided — config-driven today, plugin-extensible later (see
/// [`PaneHeaderButton`]).
fn pane_header_buttons(content: &PaneHeaderContent, ctx: &PaneHeaderCtx) -> Vec<PaneHeaderButton> {
    use heca_config::appearance::PaneAction;
    let mut out: Vec<PaneHeaderButton> = content
        .actions
        .iter()
        .copied()
        .filter(|&a| !content.floating || pane_action_visible_when_floating(content.catalog, a))
        .map(|action| {
            let (glyph, wm_action, label, needs_focus) =
                pane_action_spec(content.catalog, action, ctx.pane_id, ctx.ws_idx, ctx.col_idx);
            let is_active = match action {
                PaneAction::Zoom => content.zoomed,
                PaneAction::Float => content.floating,
                _ => false,
            };
            PaneHeaderButton {
                glyph,
                action_name: pane_action_name(action),
                wm_action,
                label,
                // Don't focus-first when floating: the floating pane is already active,
                // and a `FocusPane` from MouseContent is blocked in the floating domain.
                needs_focus: needs_focus && !content.floating,
                is_active,
                is_close: matches!(action, PaneAction::Close),
            }
        })
        .collect();
    // ── Plugin seam ──────────────────────────────────────────────────────────────
    // Plugin-contributed pane-header buttons will be appended to `out` here once the
    // plugin action surface lands (each plugin supplies a `PaneHeaderButton`). Keeping
    // the render loop descriptor-driven means that wiring needs no changes below.
    let _ = &mut out;
    out
}

/// A content key identifying everything the header *renders* — used to decide when
/// the retained tree must be rebuilt (vs. just re-laid-out). Cheap per-frame string
/// build (≤20 panes); avoids deriving `Hash` on the projection enums.
pub(crate) fn pane_header_key(content: &PaneHeaderContent, font: f32, avail_w: f32) -> String {
    let view = pane_info_view(
        content.programs,
        content.fallback_name,
        content.custom_name,
        content.runtime,
        // The info bar never shows the process suffix, so it plays no part in the key.
        false,
    );
    let cwd = content
        .runtime
        .and_then(|r| r.cwd.as_ref())
        .map(|c| c.display().to_string());
    // Bucket width so layout jitter doesn't thrash the rebuild, but real resizes
    // re-truncate the location segment.
    let w_bucket = (avail_w / 16.0) as i32;
    // Tooltip hints for the configured actions (so a rebind rebuilds the tips).
    let hints: Vec<&str> = content
        .actions
        .iter()
        .map(|&a| content.shortcuts.get(pane_action_name(a)).unwrap_or(""))
        .collect();
    format!(
        "{:?}|{}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{:?}",
        view.icon,
        view.title,
        view.status,
        view.git_branch,
        view.git_added,
        view.git_modified,
        view.git_deleted,
        cwd,
        content.segments,
        content.actions,
        content.ws_idx,
        content.col_idx,
        content.zoomed,
        content.floating,
        font.to_bits(),
        w_bucket,
        hints,
    )
}

/// Build the retained header tree: the segment `Tag` (left) + an `IconButton`
/// cluster (right), each button wrapped in a `Tooltip` and wired to emit its
/// (parameterized) WM action through `app.on`-style `ChromeIntent`. Returns `None`
/// when there are neither segments-with-data nor actions.
pub(crate) fn build_pane_header(
    content: &PaneHeaderContent,
    theme: &GuiTheme,
    font: f32,
    avail_w: f32,
    ctx: PaneHeaderCtx,
    hints: &mut HintTargetRegistry,
) -> Option<Flex> {
    // The button set is a dynamic vector of descriptors (config-driven today,
    // plugin-extensible later) — the loop below never matches on a concrete action.
    let specs = pane_header_buttons(content, &ctx);
    // Build the action-button cluster first: each button self-sizes from its
    // `WidgetSize::Header` variant (emphasized glyph + snug cluster padding), so its
    // width is owned by the widget, not hand-computed here.
    let mut buttons = if specs.is_empty() {
        None
    } else {
        let mut row = Flex::row().align(Align::Center).gap(HEADER_BUTTON_GAP);
        for spec in specs {
            // Close is destructive → its glyph + hover/press use the theme danger
            // hue; the rest use the foreground glyph with an accent hover. The danger
            // glyph is softened toward the header surface so the red reads as a cue,
            // not an alarm (full-intensity danger was too vibrant).
            let (icon_color, tone) = if spec.is_close {
                (theme.colors.danger.lerp(theme.colors.surface, 0.25), theme.colors.danger)
            } else {
                (theme.colors.foreground, theme.colors.accent)
            };
            // Register this button as a universal KeyHint target (prefix+/). The intent
            // mirrors the click exactly: active-targeted buttons (zoom/float) focus this
            // pane first via the composite intent, the rest carry their pane in the
            // action. Registered per-button in the loop, so config-added / plugin-added
            // buttons are hinted automatically — nothing hardcoded.
            let hint_intent = if spec.needs_focus {
                crate::app::interaction::InteractionIntent::FocusPaneThenAction {
                    pane_id: ctx.pane_id,
                    action: Box::new(spec.wm_action.clone()),
                }
            } else {
                crate::app::interaction::InteractionIntent::ActivateAction(spec.wm_action.clone())
            };
            let hint_id = hints.register(hint_intent);
            let proxy = ctx.event_proxy.clone();
            let pane_id = ctx.pane_id;
            let wm_action = spec.wm_action.clone();
            let needs_focus = spec.needs_focus;
            let button = IconButton::new(Icon::new(spec.glyph).color(icon_color))
                .size(WidgetSize::Header)
                .tone(tone)
                .active(spec.is_active)
                .hint_target(hint_id)
                .on_click(move || {
                    use crate::app::interaction::{InteractionIntent, InteractionSource};
                    // Active-targeted actions (zoom/float) act on the focused pane, so
                    // focus this pane first — the events are queued and processed in
                    // order on the UI thread, so the action lands on this pane.
                    if needs_focus {
                        let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                            source: InteractionSource::MouseContent,
                            intent: InteractionIntent::FocusPane { pane_id },
                        });
                    }
                    let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                        source: InteractionSource::MouseContent,
                        intent: InteractionIntent::ActivateAction(wm_action.clone()),
                    });
                });
            // Tooltip = label + the action's current keybind(s), resolved centrally
            // by name (never hand-picked here); the leader renders via PREFIX_SYMBOL.
            row = row.child(action_tooltip(button, spec.action_name, spec.label, content.shortcuts));
        }
        Some(row)
    };

    // Measure the cluster's natural width by laying it out on its own — the widget
    // reports its size, we don't compute it. (The outer tree is re-laid-out every
    // frame in `sync_pane_headers`; this pass only feeds the bar's truncation budget.)
    let buttons_w = if let Some(row) = buttons.as_mut() {
        LayoutEngine::new()
            .base_font(font)
            .compute(row, Size::new(avail_w as f64, avail_w as f64));
        row.base().bounds.size.w as f32
    } else {
        0.0
    };
    // The bar yields width to the button cluster first.
    let bar_max = (avail_w
        - buttons_w
        - if buttons_w > 0.0 { HEADER_BUTTON_GAP } else { 0.0 })
    .max(0.0);
    let bar = build_pane_info_bar(
        content.programs,
        content.fallback_name,
        content.custom_name,
        content.runtime,
        content.segments,
        theme,
        bar_max,
        font,
    );

    let root = Flex::row().width(Length::Px(avail_w)).align(Align::Center);
    let root = match (bar, buttons) {
        (Some(bar), Some(buttons)) => root
            .justify(Justify::SpaceBetween)
            .child(bar)
            .child(buttons),
        (Some(bar), None) => root.justify(Justify::Start).child(bar),
        (None, Some(buttons)) => root.justify(Justify::End).child(buttons),
        (None, None) => return None,
    };
    Some(root)
}

const VIEWPORT_BADGE_MARGIN: f32 = 0.0;

fn format_lines_above(lines: usize) -> String {
    if lines == 1 {
        "1 line above".to_string()
    } else {
        format!("{lines} lines above")
    }
}

fn build_pane_viewport_widgets(
    pane_id: PaneId,
    event_proxy: &winit::event_loop::EventLoopProxy<crate::app::events::AppEvent>,
) -> RetainedPaneViewportWidgets {
    use crate::app::interaction::{InteractionIntent, InteractionSource};
    let badge_proxy = event_proxy.clone();
    let mut badge = BadgeButton::accent("0 lines above").on_click(move || {
        let _ = badge_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::FocusPane { pane_id },
        });
        let _ = badge_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: InteractionSource::MouseContent,
            intent: InteractionIntent::ActivateAction(crate::input::WmAction::ScrollToBottom),
        });
    });
    badge.base_mut().visible.set(false);

    let mut scrollbar = ScrollBar::new();
    let content_signal = scrollbar.content_extent_signal();
    let viewport_signal = scrollbar.viewport_extent_signal();
    let bar_proxy = event_proxy.clone();
    scrollbar = scrollbar.on_change(move |action| {
        if let heca_grid_ui::SignalData::Float(offset_top) = action.data {
            let max = (content_signal.get_untracked() - viewport_signal.get_untracked()).max(0.0);
            let rows = ((max as f64) - offset_top)
                .round()
                .clamp(0.0, max as f64) as usize;
            let _ = bar_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::FocusPane { pane_id },
            });
            let _ = bar_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                source: InteractionSource::MouseContent,
                intent: InteractionIntent::ActivateAction(crate::input::WmAction::ScrollToOffset {
                    rows,
                }),
            });
        }
    });
    scrollbar.base_mut().visible.set(false);

    RetainedPaneViewportWidgets { scrollbar, badge }
}

/// Build/update/position the retained per-pane terminal viewport widgets for every
/// visible terminal pane. Unlike pane headers, the tree shape is static, so the
/// widgets are built once per pane and then driven by signals / relaid out.
pub(crate) fn sync_pane_viewport_widgets(
    state: &mut crate::app_state::AppState,
    panes: &[&crate::app::terminal_render::PaneRenderState],
) {
    let font = chrome_gui_theme(state).font_size;
    let show_mode = state.appearance.terminal.show_scrollbar;
    let pane_info_bar_shown = state.appearance.pane_info_bar_visible();
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for pane in panes {
        seen.insert(pane.pane_id);
        let Some(mount) = pane.mount.as_ref() else {
            state.pane_viewport_widgets.remove(&pane.pane_id);
            continue;
        };
        let widgets = state
            .pane_viewport_widgets
            .entry(pane.pane_id)
            .or_insert_with(|| build_pane_viewport_widgets(pane.pane_id, &state.event_proxy));

        let scrollable = mount.snapshot.scrollback_rows > mount.snapshot.rows;
        let max_offset = mount
            .snapshot
            .scrollback_rows
            .saturating_sub(mount.snapshot.rows) as f32;
        widgets
            .scrollbar
            .content_extent_signal()
            .set(mount.snapshot.scrollback_rows as f32);
        widgets
            .scrollbar
            .viewport_extent_signal()
            .set(mount.snapshot.rows as f32);
        widgets
            .scrollbar
            .offset_signal()
            .set((max_offset - mount.snapshot.viewport_offset as f32).max(0.0));
        let scrollbar_visible = match show_mode {
            heca_config::appearance::ScrollbarVisibility::Always => scrollable,
            heca_config::appearance::ScrollbarVisibility::WhenNeeded => {
                scrollable && !mount.snapshot.at_bottom
            }
            heca_config::appearance::ScrollbarVisibility::Never => false,
        };
        widgets.scrollbar.base_mut().visible.set(scrollbar_visible);

        let badge_visible = state.appearance.terminal.show_scrolled_up_badge
            && !mount.snapshot.at_bottom
            && mount.snapshot.viewport_offset > 0;
        widgets.badge.base_mut().visible.set(badge_visible);
        widgets
            .badge
            .label_signal()
            .set(format_lines_above(mount.snapshot.viewport_offset));

        let Some(content_rect) = pane.content_rect else {
            continue;
        };
        if scrollbar_visible {
            widgets.scrollbar.base_mut().style.width =
                heca_grid_ui::style::Length::Px(8.0);
            widgets.scrollbar.base_mut().style.height =
                heca_grid_ui::style::Length::Px(content_rect.size.h as f32);
            LayoutEngine::new().base_font(font).compute(
                &mut widgets.scrollbar,
                Size::new(8.0, content_rect.size.h),
            );
            let bar_bounds = widgets.scrollbar.base().bounds;
            // X hugs the pane's outer right edge; Y/H follow the terminal content
            // rect so the thumb stays below the header and above the bottom inset.
            let bar_x = pane.x + pane.w - bar_bounds.size.w as f32;
            translate_tree(&mut widgets.scrollbar, bar_x as f64, content_rect.loc.y);
        }
        if badge_visible {
            LayoutEngine::new().base_font(font).compute(
                &mut widgets.badge,
                Size::new(pane.w as f64, pane.h as f64),
            );
            let badge_bounds = widgets.badge.base().bounds;
            // Align to the pane's outer right edge (flush, like the scrollbar).
            let badge_x = pane.x + pane.w - badge_bounds.size.w as f32;
            // Sit *below* the pane info-bar header so it doesn't cover the action
            // buttons. When the info bar is hidden there is no header to avoid.
            let header_h = if pane_info_bar_shown {
                crate::app::terminal_render::title_bar_reserve(font)
            } else {
                0.0
            };
            let badge_y = pane.y + header_h + VIEWPORT_BADGE_MARGIN;
            translate_tree(&mut widgets.badge, badge_x as f64, badge_y as f64);
        }
    }
    state.pane_viewport_widgets.retain(|id, _| seen.contains(id));
}

/// Drop every retained pane header, first releasing the hint-target ids each one
/// registered (so the shared [`HintTargetRegistry`] doesn't leak ranges). Used
/// when headers are globally invalidated: the info-bar is turned off, or a config
/// reload changes the theme/font baked into the trees (see `reload_config`, which
/// mirrors this alongside `terminal_layers.clear()`). `sync_pane_headers` rebuilds
/// them from scratch next frame.
pub(crate) fn clear_pane_headers(state: &mut crate::app_state::AppState) {
    let ranges: Vec<_> = state
        .pane_headers
        .values()
        .map(|h| h.hint_range.clone())
        .collect();
    for r in ranges {
        state.hint_targets.remove_range(r);
    }
    state.pane_headers.clear();
}

/// Build/position the retained per-pane info-bar headers for every visible pane.
/// Runs at the **top** of `render_frame` (before the `scene_view` borrow of
/// `state.compositor`) so it can mutate `state.pane_headers`; render then paints
/// them read-only and `mouse.rs` dispatches pointer events into them. Rebuilds a
/// pane's tree only when its content key changes; re-lays-out + repositions every
/// frame; prunes panes that disappeared.
pub(crate) fn sync_pane_headers(state: &mut crate::app_state::AppState) {
    let segments = state.appearance.pane.title_segments.clone();
    let actions = state.appearance.pane.title_actions.clone();
    if segments.is_empty() && actions.is_empty() {
        // Info bar disabled: drop every header (releasing its hint targets).
        clear_pane_headers(state);
        return;
    }
    let theme = chrome_gui_theme(state);
    let font = theme.font_size;
    let band = crate::app::terminal_render::title_bar_reserve(font);

    // Phase 1: gather per-pane inputs with only immutable borrows of `state`.
    struct Input {
        pane_id: PaneId,
        ws_idx: usize,
        col_idx: usize,
        name: String,
        custom_name: Option<String>,
        runtime: Option<PaneRuntime>,
        zoomed: bool,
        floating: bool,
        x: f32,
        y: f32,
        avail_w: f32,
    }
    let frames = crate::app::terminal_host::pane_outer_frames(state);
    let active_ws = state.session.active_workspace_idx;
    let mut inputs = Vec::with_capacity(frames.len());
    for (pane_id, x, y, w, _h) in frames {
        let (ws_idx, col_idx) = crate::find_pane_location(&state.session, pane_id)
            .map(|(ws, col, _)| (ws, col))
            .unwrap_or((active_ws, 0));
        let (name, custom_name, runtime) = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pane_id))
            .map(|p| {
                (
                    p.title.clone(),
                    p.custom_name.clone(),
                    Some(p.runtime.clone()),
                )
            })
            .unwrap_or_else(|| (String::new(), None, None));
        // Floating panes aren't in any column (`find_pane_location` returns None);
        // detect them directly so the bar hides tiled-only buttons + flags float active.
        let floating = state
            .session
            .active_workspace()
            .map(|ws| ws.floating_panes.iter().any(|f| f.pane.id == pane_id))
            .unwrap_or(false);
        let zoomed = !floating
            && state
                .session
                .active_workspace()
                .and_then(|ws| ws.scrolling.columns.get(col_idx))
                .map(|c| c.is_zoomed() || c.is_full_width)
                .unwrap_or(false);
        inputs.push(Input {
            pane_id,
            ws_idx,
            col_idx,
            name,
            custom_name,
            runtime,
            zoomed,
            floating,
            x,
            y,
            avail_w: (w - 2.0 * HEADER_MARGIN).max(0.0),
        });
    }

    // Phase 2: build (if changed) + position each header (mutates `state.pane_headers`).
    // The shared hint registry is taken out for the loop so headers can register their
    // KeyHint targets into it (borrow-disjoint from the `&state` reads in `content`);
    // it's restored at the end.
    let mut hints = std::mem::take(&mut state.hint_targets);
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for input in &inputs {
        seen.insert(input.pane_id);
        let content = PaneHeaderContent {
            programs: &state.programs,
            fallback_name: &input.name,
            custom_name: input.custom_name.as_deref(),
            runtime: input.runtime.as_ref(),
            segments: &segments,
            actions: &actions,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            zoomed: input.zoomed,
            floating: input.floating,
            shortcuts: &state.action_shortcuts,
            catalog: &state.action_catalog,
        };
        let key = pane_header_key(&content, font, input.avail_w);
        let needs_build = state
            .pane_headers
            .get(&input.pane_id)
            .map(|h| h.key != key)
            .unwrap_or(true);
        if needs_build {
            // Drop the hint ids the previous version of this header registered, then
            // build the new one and record the fresh contiguous id range it registers.
            if let Some(old) = state.pane_headers.get(&input.pane_id) {
                hints.remove_range(old.hint_range.clone());
            }
            let ctx = PaneHeaderCtx {
                pane_id: input.pane_id,
                ws_idx: input.ws_idx,
                col_idx: input.col_idx,
                event_proxy: state.event_proxy.clone(),
            };
            let start = hints.checkpoint();
            match build_pane_header(&content, &theme, font, input.avail_w, ctx, &mut hints) {
                Some(root) => {
                    let hint_range = start..hints.checkpoint();
                    state.pane_headers.insert(
                        input.pane_id,
                        RetainedPaneHeader { root, key, hint_range },
                    );
                }
                None => {
                    state.pane_headers.remove(&input.pane_id);
                    continue;
                }
            }
        }
        if let Some(header) = state.pane_headers.get_mut(&input.pane_id) {
            LayoutEngine::new().base_font(font).compute(
                &mut header.root,
                Size::new(input.avail_w as f64, band as f64),
            );
            let bar_h = header.root.base().bounds.size.h as f32;
            let bar_y = input.y + f64::from(((band - bar_h) / 2.0).max(0.0)) as f32;
            translate_tree(
                &mut header.root,
                (input.x + HEADER_MARGIN) as f64,
                bar_y as f64,
            );
        }
    }
    // Prune vanished panes, dropping the hint targets they had registered.
    state.pane_headers.retain(|id, h| {
        if seen.contains(id) {
            true
        } else {
            hints.remove_range(h.hint_range.clone());
            false
        }
    });
    state.hint_targets = hints;
}

/// Translate a freshly-laid-out widget subtree (positioned from the origin by
/// [`LayoutEngine::compute`]) to an absolute `(dx, dy)`. Mirrors the helper in
/// `terminal_render` so the retained header can be placed at its pane.
fn translate_tree(c: &mut dyn Component, dx: f64, dy: f64) {
    let b = c.base().bounds;
    c.base_mut().bounds = Rectangle::new(Point::new(b.loc.x + dx, b.loc.y + dy), b.size);
    for child in c.base_mut().children.iter_mut() {
        translate_tree(child.as_mut(), dx, dy);
    }
}

/// Dispatch a pointer press at `pos` into the retained pane headers. Returns
/// `Some((pane_id, consumed))` when the press lands inside a header's bounds:
/// `consumed = true` if an action button handled it (caller must not forward to
/// the terminal); `false` for the header band's empty area (caller focuses the
/// pane, treating the band as chrome — no terminal selection). `None` off any header.
pub(crate) fn dispatch_pane_header_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<(PaneId, bool)> {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    // Collect candidate ids first (avoid holding the map borrow across the dispatch).
    let hit = state
        .pane_headers
        .iter()
        .find(|(_, h)| rect_contains(h.root.base().bounds, point))
        .map(|(id, _)| *id)?;
    let header = state.pane_headers.get_mut(&hit)?;
    let consumed =
        header.root.event(&Event::PointerPressed { pos: point }) == heca_grid_ui::Handled::Yes;
    Some((hit, consumed))
}

/// Dispatch a pointer move at `pos` into the retained pane headers so the action
/// buttons' hover affordance updates. Returns `true` if the pointer is over any
/// header (the caller requests a repaint). Does not discard the trees (hover is
/// transient and must persist across moves).
pub(crate) fn dispatch_pane_header_move(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut over = false;
    for header in state.pane_headers.values_mut() {
        let _ = header.root.event(&Event::PointerMoved { pos: point });
        if rect_contains(header.root.base().bounds, point) {
            over = true;
        }
    }
    over
}

/// Feed a pointer press into the retained terminal viewport widgets. Returns
/// `true` when any widget consumed the press (badge click or scrollbar drag).
pub(crate) fn dispatch_pane_viewport_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    for widgets in state.pane_viewport_widgets.values_mut() {
        if widgets.badge.event(&Event::PointerPressed { pos: point }) == heca_grid_ui::Handled::Yes
            || widgets.scrollbar.event(&Event::PointerPressed { pos: point })
                == heca_grid_ui::Handled::Yes
        {
            return true;
        }
    }
    false
}

/// Feed pointer motion into the retained terminal viewport widgets so hover and
/// scrollbar drags update. Returns `true` if the pointer is over any widget.
pub(crate) fn dispatch_pane_viewport_move(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut over = false;
    for widgets in state.pane_viewport_widgets.values_mut() {
        let badge_handled = widgets.badge.event(&Event::PointerMoved { pos: point })
            == heca_grid_ui::Handled::Yes;
        let scrollbar_handled = widgets.scrollbar.event(&Event::PointerMoved { pos: point })
            == heca_grid_ui::Handled::Yes;
        if badge_handled || scrollbar_handled {
            over = true;
        }
        if (widgets.badge.base().visible.get_untracked()
            && rect_contains(widgets.badge.base().bounds, point))
            || (widgets.scrollbar.base().visible.get_untracked()
                && rect_contains(widgets.scrollbar.base().bounds, point))
        {
            over = true;
        }
    }
    over
}

/// Feed a pointer release into the retained terminal viewport widgets so a
/// scrollbar drag can end even when released outside its bounds.
pub(crate) fn dispatch_pane_viewport_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    let mut handled = false;
    for widgets in state.pane_viewport_widgets.values_mut() {
        handled |= widgets.badge.event(&Event::PointerReleased { pos: point })
            == heca_grid_ui::Handled::Yes;
        handled |= widgets.scrollbar.event(&Event::PointerReleased { pos: point })
            == heca_grid_ui::Handled::Yes;
    }
    handled
}

fn rect_contains(r: Rectangle, p: Point) -> bool {
    p.x >= r.loc.x && p.x <= r.loc.x + r.size.w && p.y >= r.loc.y && p.y <= r.loc.y + r.size.h
}

fn runtime_snapshot(state: &WorkspacesContainerState, pane_id: PaneId) -> Option<PaneRuntime> {
    state.pane_runtime(pane_id)
}

// Kept short so the branch + git counts fit the sidebar card width without
// overflowing (the row isn't width-clipped). The full branch is on hover.
const SIDEBAR_GIT_BRANCH_MAX_CHARS: usize = 22;

/// Truncate a branch for the sidebar, keeping the **tail** (the meaningful end,
/// e.g. `…security-upgrade`) rather than the boilerplate `feature/` prefix.
fn truncate_sidebar_git_branch(branch: &str) -> String {
    truncate_path_left(branch, SIDEBAR_GIT_BRANCH_MAX_CHARS)
}

type ChromeIntentEmitter = Rc<dyn Fn(crate::app::interaction::InteractionIntent)>;

/// Transparent wrapper that marks only its own bounds dirty when the host bumps
/// `request`. This lets retained chrome updates damage the specific card/marker/label
/// instead of the entire chrome root.
struct RepaintWatch {
    base: heca_grid_ui::Base,
    request: Signal<u64>,
    seen: u64,
}

impl RepaintWatch {
    fn new(child: impl Component + 'static) -> (Self, Signal<u64>) {
        let mut base = heca_grid_ui::Base::new();
        base.style.width = Length::Auto;
        base.style.height = Length::Auto;
        base.style.direction = heca_grid_ui::Direction::Column;
        base.children.push(Box::new(child));
        let request = signal(0_u64);
        (
            Self {
                base,
                request,
                seen: 0,
            },
            request,
        )
    }
}

impl Component for RepaintWatch {
    fn base(&self) -> &heca_grid_ui::Base {
        &self.base
    }

    fn base_mut(&mut self) -> &mut heca_grid_ui::Base {
        &mut self.base
    }

    fn paint(&self, cx: &mut PaintCx) {
        if !self.base.visible.get_untracked() {
            return;
        }
        for child in &self.base.children {
            if child.base().style.hidden {
                continue;
            }
            child.paint(cx);
        }
    }

    fn tick(&mut self, dt: f32) -> bool {
        let next = self.request.get_untracked();
        if next != self.seen {
            self.seen = next;
            self.base.mark_needs_paint();
        }
        let mut animating = false;
        for child in self.base.children.iter_mut() {
            animating |= child.tick(dt);
        }
        animating
    }
}

/// A single pane **card**, styled like the showcase PANES rows: a state-tinted
/// background + radius, an active accent bar, a leading program icon, the display
/// name, an optional exceptional-state indicator, and git metadata when present.
#[expect(
    clippy::too_many_arguments,
    reason = "pane-card projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn pane_card(
    pane: &SidebarPaneEntry,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> RepaintWatch {
    let active = active_pane == Some(pane.pane_id);
    let pane_id = pane.pane_id;
    let runtime = runtime_snapshot(ws_state, pane_id);
    let info = pane_info_view(
        programs,
        &pane.name,
        pane.custom_name.as_deref(),
        runtime.as_ref(),
        // A renamed pane shows a dimmed `(process)` suffix here when the setting is on,
        // so the sidebar keeps surfacing what's actually running under a custom name.
        ws_state.pane_renamed_add_process_name(),
    );
    // A constant theme-driven card; the *selected* look (accent pill + border + bar)
    // is drawn by `Row` from its `active` signal, not baked into the background. This
    // keeps styling fully signal-driven (active flips in place via `sync_chrome_signals`,
    // no tree rebuild) and theme-driven (no ad-hoc per-state alphas).
    // The card is both a drag source and a drop target (F4.5); its opaque DragItemId
    // is assigned by the registry (which records that it's this pane) so the kind
    // round-trips through `drag::source_at`/`resolve_at` without trusting raw ids.
    let drag_id = drag.register(ChromeDragItem::Pane(pane_id));
    // Hint target: the universal picker (`prefix+/`) focuses this pane by its letter.
    let hint_id = hints.register(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
    let icon_widget = Icon::new(info.icon).size(14.0).color(theme.colors.foreground);
    let icon_signal = icon_widget.glyph_signal();
    let active_title_label = Label::new(info.title.clone())
        .color(theme.colors.accent)
        .bold(true);
    let active_title_signal = active_title_label.text_signal();
    let active_title = Visibility::new(active_title_label, active);
    let active_title_visible = active_title.visible_signal();
    let inactive_title_label = Label::new(info.title.clone())
        .color(theme.colors.foreground)
        .bold(true);
    let inactive_title_signal = inactive_title_label.text_signal();
    let inactive_title = Visibility::new(inactive_title_label, !active);
    let inactive_title_visible = inactive_title.visible_signal();
    let idle_dot = Visibility::new(StatusDot::offline(), info.status == ProcessStatus::Idle);
    let idle_dot_visible = idle_dot.visible_signal();
    let running_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Running);
    let running_dot_visible = running_dot.visible_signal();
    let success_dot = Visibility::new(StatusDot::online(), info.status == ProcessStatus::Success);
    let success_dot_visible = success_dot.visible_signal();
    let error_dot = Visibility::new(StatusDot::error(), info.status == ProcessStatus::Error);
    let error_dot_visible = error_dot.visible_signal();
    let branch_label_widget = Label::new(truncate_sidebar_git_branch(
        info.git_branch.as_deref().unwrap_or_default(),
    ))
    .color(theme.colors.foreground)
    .font_scale(0.8);
    let branch_display_signal = branch_label_widget.text_signal();
    let branch_signal = signal(info.git_branch.clone().unwrap_or_default());
    let add_label_widget = Label::new(info.git_added.clone().unwrap_or_default())
        .color(theme.colors.success)
        .font_scale(0.8);
    let add_label = add_label_widget.text_signal();
    let add_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Plus).size(12.0).color(theme.colors.success))
            .child(add_label_widget),
        info.git_added.is_some(),
    );
    let add_text_visible_signal = add_segment.visible_signal();
    let modified_label_widget = Label::new(info.git_modified.clone().unwrap_or_default())
        .color(theme.colors.warning)
        .font_scale(0.8);
    let modified_label = modified_label_widget.text_signal();
    let modified_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Warning).size(12.0).color(theme.colors.warning))
            .child(modified_label_widget),
        info.git_modified.is_some(),
    );
    let modified_text_visible_signal = modified_segment.visible_signal();
    let deleted_label_widget = Label::new(info.git_deleted.clone().unwrap_or_default())
        .color(theme.colors.danger)
        .font_scale(0.8);
    let deleted_label = deleted_label_widget.text_signal();
    let deleted_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Minus).size(12.0).color(theme.colors.danger))
            .child(deleted_label_widget),
        info.git_deleted.is_some(),
    );
    let deleted_text_visible_signal = deleted_segment.visible_signal();
    let git_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::GitBranch).size(12.0).color(theme.colors.warning))
            .child(
                Tooltip::new_signal(branch_label_widget, branch_signal)
                    .side(TooltipSide::Bottom)
                    .delay(0.25),
            )
            .child(add_segment)
            .child(modified_segment)
            .child(deleted_segment),
        info.git_branch.is_some(),
    );
    let git_visible_signal = git_row.visible_signal();
    // A renamed pane surfaces its running program as a dimmed `(process)` suffix after
    // the name (config-gated). Appended to the shared title area so it renders the same
    // in both the git and no-git card layouts. Like the title, it is **signal-driven**
    // (text + visibility updated in `sync_chrome_signals`) so renaming toggles it live —
    // a rename updates signals, it does not rebuild the sidebar card tree.
    let process_hint_text = info
        .process_hint
        .as_ref()
        .map(|program| format!("({program})"))
        .unwrap_or_default();
    // `foreground` (not `muted`) so it's readable on every theme; it still reads as
    // secondary next to the accent + bold name (regular weight, smaller scale).
    let process_hint_label = Label::new(process_hint_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let process_hint_signal = process_hint_label.text_signal();
    let process_hint = Visibility::new(process_hint_label, info.process_hint.is_some());
    let process_hint_visible = process_hint.visible_signal();
    let title_area = Flex::row()
        .align(Align::Center)
        .gap(4.0)
        .child(Flex::column().child(active_title).child(inactive_title))
        .child(process_hint);
    // Optional cwd row (folder icon + home-relative path), stacked between the name and
    // git rows. Signal-driven like the git branch: the path updates live on `cd`, and the
    // row's visibility follows `[settings] pane_show_cwd` and whether the pane has a cwd.
    let cwd_path = runtime.as_ref().and_then(|rt| rt.cwd.clone());
    let show_cwd = ws_state.pane_show_cwd() && cwd_path.is_some();
    let cwd_text = cwd_path
        .as_deref()
        .map(home_relative_path)
        .unwrap_or_default();
    // Readable, matching the sibling git-branch row (which colors its label
    // `foreground`); `muted` was too dim for a primary info row.
    let cwd_label = Label::new(cwd_text)
        .color(theme.colors.foreground)
        .font_scale(CARD_META_FONT_SCALE);
    let cwd_signal = cwd_label.text_signal();
    let cwd_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::Folder).size(12.0).color(theme.colors.foreground))
            .child(cwd_label),
        show_cwd,
    );
    let cwd_visible_signal = cwd_row.visible_signal();
    // The pane's identity row (status dots + program icon + name) — shared by every card
    // layout so the cwd and git rows just stack beneath it in one column.
    let name_row = Flex::row()
        .align(Align::Center)
        .gap(8.0)
        .child(
            Flex::row()
                .align(Align::Center)
                .width(Length::Px(12.0))
                .child(idle_dot)
                .child(running_dot)
                .child(success_dot)
                .child(error_dot),
        )
        .child(Flex::row().align(Align::Center).child(icon_widget))
        .child(title_area);
    let emit = emit_intent.clone();
    // One column: the name row, then the optional cwd and git rows (each 2px-indented and
    // added only when shown, mirroring the git row). A card with only the name row lays
    // out exactly like the former single-row layout — a one-child column adds no gap.
    let mut content = Flex::column().gap(4.0).grow(1.0).child(name_row);
    if show_cwd {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(cwd_row),
        );
    }
    if info.git_branch.is_some() {
        content = content.child(
            Flex::row()
                .child(Flex::row().width(Length::Px(2.0)))
                .child(git_row),
        );
    }
    let card = Row::new()
        .background(
            theme
                .colors
                .foreground
                .with_alpha(alpha_u8(theme.colors.card_background_alpha)),
        )
        .highlight(theme.colors.accent)
        .radius(theme.colors.control_radius())
        .padding(6.0)
        .marker(ActiveMarker::Bar)
        .active(active)
        .nav_selected(false)
        .draggable(drag_id)
        .drop_target(drag_id)
        .hint_target(hint_id)
        // On click/Enter the card records its pane id in the host sink; the app reads
        // it after dispatch and focuses that pane (read-via-signal / write-via-action).
        .on_activate(move || {
            emit(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
        })
        .child(content);
    // Bind the card's active signal so focus changes update it without a rebuild.
    signals.pane_active.push((pane_id, card.state()));
    signals.pane_nav.push((pane_id, card.nav_state()));
    // Wrap the card in a universal `KeyHint` so a move/swap/take pick can stamp this
    // pane's letter over it. `KeyHint` is transparent — it hugs the child and routes
    // events/focus/drag straight through — so the card stays a drag source + target
    // and clickable. The hint signal is driven each frame in `sync_chrome_signals`
    // from the active `InputMode` candidates (keyboard logic stays the source of truth).
    let hint = signal(None);
    signals.pane_hint.push((pane_id, hint));
    signals.pane_info.push((
        pane_id,
        PaneInfoSignals {
            icon: icon_signal,
            title_active: active_title_signal,
            title_inactive: inactive_title_signal,
            title_active_visible: active_title_visible,
            title_inactive_visible: inactive_title_visible,
            process_hint: process_hint_signal,
            process_hint_visible,
            cwd: cwd_signal,
            cwd_visible: cwd_visible_signal,
            status_idle_visible: idle_dot_visible,
            status_running_visible: running_dot_visible,
            status_success_visible: success_dot_visible,
            status_error_visible: error_dot_visible,
            git_visible: git_visible_signal,
            git_branch: branch_signal,
            git_branch_display: branch_display_signal,
            git_added_visible: add_text_visible_signal,
            git_added: add_label,
            git_modified_visible: modified_text_visible_signal,
            git_modified: modified_label,
            git_deleted_visible: deleted_text_visible_signal,
            git_deleted: deleted_label,
        },
    ));
    let (watch, _repaint) = RepaintWatch::new(
        KeyHint::new(card)
            .hint(hint)
            .placement(HintPlacement::CenterRight),
    );
    watch
}

/// One **column**: a generic [`MarkerGroup`] (left marker bar + grip gutter) holding
/// the column's stacked pane cards — no per-column header row (columns are spatial
/// groupings whose only user-facing job is to be a move/swap target + drag handle).
/// The `MarkerGroup` bar brightens to the accent when the column holds the active
/// pane, and its grip gutter is the seam for the future move/swap [`KeyHint`] target
/// and DnD drag handle (F4.4/F4.5) — applied by the host via `KeyHint`/`DragExt`, not
/// baked into the widget.
#[expect(
    clippy::too_many_arguments,
    reason = "column projection still threads host/runtime context explicitly; phase-local fix before a larger ChromeCx refactor"
)]
fn column_view(
    c: &SidebarColEntry,
    ws_idx: usize,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    active_pane: Option<PaneId>,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> RepaintWatch {
    let active = c.panes.iter().any(|p| active_pane == Some(p.pane_id));
    // The MarkerGroup is a column drag source + drop target (F4.5 step 2). Its grip
    // gutter is the only surface not covered by a child pane card, so innermost-first
    // hit-testing routes a grip press → column and a card press → pane, for free.
    let drag_id = drag.register(ChromeDragItem::Column {
        ws: ws_idx,
        col: c.col_idx,
    });
    let mut col = MarkerGroup::new()
        .active(active)
        .gap(3.0)
        .draggable(drag_id)
        .drop_target(drag_id);
    for pane in &c.panes {
        col = col.child(pane_card(
            pane,
            programs,
            theme,
            emit_intent,
            active_pane,
            ws_state,
            signals,
            drag,
            hints,
        ));
    }
    // Bind the column bar's active signal (lit iff it holds the active pane).
    let pane_ids = c.panes.iter().map(|p| p.pane_id).collect::<Vec<_>>();
    signals.col_active.push((pane_ids, col.state()));
    // Wrap the column in the universal `KeyHint` so a "move pane → column" pick can
    // stamp this column's letter over it (tinted `success`, distinct from pane/workspace
    // picks). Driven each frame in `sync_chrome_signals`.
    let col_hint = signal::<Option<String>>(None);
    signals.col_hint.push((ws_idx, c.col_idx, col_hint));
    let hinted = KeyHint::new(col)
        .hint(col_hint)
        .color(theme.colors.success)
        .placement(HintPlacement::CenterRight);
    let (watch, _repaint) = RepaintWatch::new(hinted);
    watch
}

/// Build the **WorkspacesContainer** content — the workspace tree mounted inside the
/// sidebar shell (see `heca-sidebar-design-spec`). Each workspace is a `.frameless()`
/// [`DockFrame`] (header count [`Badge`] = total panes); its columns are compact
/// [`column_view`]s (left marker bar + pane cards, no "Col N" header rows — those ate
/// the sidebar for no user value). Pure projection of the [`SidebarTree`].
#[expect(
    clippy::too_many_arguments,
    reason = "workspace-container projection threads host/runtime context + the drag and hint registries explicitly; phase-local before a larger ChromeCx refactor"
)]
fn build_workspaces_container(
    tree: &SidebarTree,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
    hints: &mut HintTargetRegistry,
) -> Flex {
    // Selection is sourced from the container's shared state (the Phase-2 boundary),
    // not from `Session`/`SidebarItemState`. A workspace is "active" iff it hosts the
    // active pane.
    let active_pane = ws_state.active_pane();
    let mut col = Flex::column().gap(6.0).grow(1.0);
    for ws in &tree.workspaces {
        let pane_count =
            ws.columns.iter().map(|c| c.panes.len()).sum::<usize>() + ws.floating_panes.len();
        let active_ws = active_pane.is_some_and(|pid| {
            ws.columns
                .iter()
                .flat_map(|c| &c.panes)
                .chain(&ws.floating_panes)
                .any(|p| p.pane_id == pid)
        });
        let badge = if active_ws {
            Badge::accent(pane_count.to_string())
        } else {
            Badge::neutral(pane_count.to_string())
        };
        // The header toggle records the workspace in the toggle sink; the app flips
        // its collapsed state (canonical, in `chrome_state.workspaces`) and the tree
        // rebuilds (F4.3). Collapse is read back from that same shared state.
        let ws_idx = ws.ws_idx;
        let emit = emit_intent.clone();
        let mut dock = DockFrame::new(ws.name.clone())
            .frameless()
            .gap(4.0) // tighten the workspace header → body spacing
            .expanded(!ws_state.is_ws_collapsed(ws_idx))
            .on_toggle(move |_| {
                emit(
                    crate::app::interaction::InteractionIntent::ToggleWorkspaceCollapsed { ws_idx },
                );
            })
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        // Light accent wash over the whole active workspace area (+ the accent
        // count badge) makes the active workspace clearly prominent. Signal-driven
        // (like the pane/column highlights) so it flips in place via
        // `sync_chrome_signals` instead of forcing a tree rebuild; the alpha is the
        // theme's `active_wash_alpha` token, not a baked-in literal.
        dock = dock.active(active_ws);
        dock = dock.nav_selected(false);
        let ws_pane_ids = ws
            .columns
            .iter()
            .flat_map(|c| &c.panes)
            .chain(&ws.floating_panes)
            .map(|p| p.pane_id)
            .collect::<Vec<_>>();
        signals.ws_active.push((ws_pane_ids, dock.active_state()));
        signals.ws_nav.push((ws_idx, dock.nav_state()));
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        dock = dock.drop_target(drag.register(ChromeDragItem::Workspace { ws: ws_idx }));
        // Hint target: the universal picker (`prefix+/`) can focus this workspace by
        // its letter. The keycap is stamped over the dock's bounds by `paint_hint_targets`.
        dock = dock.hint_target(hints.register(
            crate::app::interaction::InteractionIntent::FocusWorkspace { ws_idx },
        ));
        // Columns stacked with a clear gap between them (the gap + bar mark each
        // column); panes inside a column are tight. Floating panes have no column.
        let mut cols = Flex::column().gap(8.0);
        for c in &ws.columns {
            cols = cols.child(column_view(
                c,
                ws_idx,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
                hints,
            ));
        }
        for float in &ws.floating_panes {
            cols = cols.child(pane_card(
                float,
                programs,
                theme,
                emit_intent,
                active_pane,
                ws_state,
                signals,
                drag,
                hints,
            ));
        }
        dock = dock.child(cols);
        // Wrap the whole workspace dock in the universal `KeyHint` so a
        // "move column/pane → workspace" pick can stamp this workspace's letter over
        // it. The keycap is tinted `warning` (not accent) so a workspace target reads
        // distinctly from a pane target. The hint signal is driven each frame in
        // `sync_chrome_signals` from the active pick candidates.
        let ws_hint = signal::<Option<String>>(None);
        signals.ws_hint.push((ws_idx, ws_hint));
        col = col.child(
            KeyHint::new(dock)
                .hint(ws_hint)
                .color(theme.colors.warning)
                // Top-right (like the pane cards' right-aligned keycap), nudged down
                // onto the workspace title row so it lines up with the name.
                .placement(HintPlacement::TopRight)
                .offset_y((theme.font_size * 0.45) as f64),
        );
    }
    col
}

/// Build a sidebar **SHELL** — a full-height, bracket-framed, frosted panel filling a
/// sidebar column. This is **one component, two instances**: the left and right
/// sidebars are the same shell differing only by width/position and the `content`
/// mounted inside. Per the chrome plan (`pluggable-chrome-plugin-plan.md` §2.1 / §2.8,
/// `docs/sidebar-provider-modes.md`) the sidebar is a *shell* that hosts a Provider's
/// content; the left passes its `WorkspacesContainer`, the right passes `None` (empty
/// placeholder) until it gains a Provider.
#[allow(clippy::too_many_arguments)]
fn build_sidebar_shell(
    region_w: f32,
    sidebar_h: f32,
    shell_bg: Color,
    sidebar_gap: f32,
    border_style: heca_config::appearance::BorderStyle,
    border_width: f32,
    border_radius: f32,
    content: Option<Flex>,
) -> Flex {
    let inner_w = (region_w - sidebar_gap * 2.0).max(0.0);
    let inner_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
    // The collapse toggle lives in the always-visible top bar (sidebar-fu-14), so the
    // shell has no header row — the mounted content (if any) fills the body.
    let mut body = apply_pane_frame(Pane::new(), border_style)
        .border_width(border_width)
        .radius(border_radius)
        .width(Length::Px(inner_w))
        .height(Length::Px(inner_h))
        .padding(10.0)
        .gap(8.0)
        .background(shell_bg);
    if let Some(content) = content {
        body = body.child(content);
    }
    Flex::column()
        .width(Length::Px(region_w))
        .height(Length::Px(sidebar_h))
        .child(
            Surface::column()
                .width(Length::Px(region_w))
                .height(Length::Px(sidebar_h))
                .background(shell_bg)
                .padding(sidebar_gap)
                .child(body),
        )
}

/// The chrome frame's geometry, colors, and status text — grouped so the assembly
/// helpers stay under clippy's argument-count lint. Borrowed `status` keeps the
/// caller's `String` in place.
#[derive(Clone, Copy)]
struct ChromeFrame<'a> {
    w: f32,
    h: f32,
    tab_bar_height: f32,
    status_bar_height: f32,
    status: &'a str,
    side_bg: Color,
    fg: Color,
}

/// A sidebar collapse toggle for the **top bar** (sidebar-fu-14): a small arrow
/// `IconButton` that emits `ActivateAction(action)` (expand↔rail for its region),
/// wrapped in a tooltip carrying its keybind (resolved centrally by `action_name`
/// — like every other chrome button). Lives in the always-visible top bar so it
/// works in both expanded and collapsed states.
#[allow(clippy::too_many_arguments)]
fn sidebar_toggle_button(
    glyph: Glyph,
    action: crate::input::WmAction,
    action_name: &str,
    shortcuts: &ActionShortcuts,
    catalog: &crate::actions::ActionCatalog,
    hints: &mut HintTargetRegistry,
    emit: ChromeIntentEmitter,
    color: Color,
) -> Tooltip {
    use crate::app::interaction::InteractionIntent;
    // Label from the action descriptor (catalog-owned), never re-spelled here.
    let label = catalog.label(action_name).unwrap_or(action_name);
    // A hint target firing the *same* intent as a click, so `prefix+/` can pick this
    // button by letter — every action button is both clickable and hintable.
    let hint_id = hints.register(InteractionIntent::ActivateAction(action.clone()));
    // Just pick the size variant — the widget derives icon px + padding from the
    // theme font internally (`Icon` with no explicit px uses the variant-scaled font,
    // `IconButton` scales its padding). No caller-side size math.
    let button = IconButton::new(Icon::new(glyph).color(color))
        .size(WidgetSize::Small)
        .hint_target(hint_id)
        .on_click(move || {
            emit(InteractionIntent::ActivateAction(action.clone()));
        });
    action_tooltip(button, action_name, label, shortcuts)
}

/// Assemble the chrome root widget tree (no layout/paint): a transparent tab band
/// (carrying the left/right sidebar collapse toggles at its outer corners), a middle
/// row hosting the (optional) full-height sidebar shell + a transparent content spacer,
/// and the opaque status bar at the bottom. Returns the concrete [`Flex`] so it can be
/// **retained** across frames (see [`RetainedChrome`]).
fn chrome_root(
    frame: &ChromeFrame,
    left_sidebar: Option<Flex>,
    right_sidebar: Option<Flex>,
    left_toggle: Option<Tooltip>,
    right_toggle: Option<Tooltip>,
    signals: &mut ChromeSignals,
) -> Flex {
    let ChromeFrame {
        w,
        h,
        tab_bar_height,
        status_bar_height,
        status,
        side_bg,
        fg,
    } = *frame;
    let middle_h = (h - tab_bar_height - status_bar_height).max(0.0);

    // Middle row: the full-height sidebar shell (when expanded) + a transparent
    // spacer over the content area (panes are drawn by the hand-drawn path under
    // this scene). The shell sizes its own width/height.
    let mut middle = Flex::row()
        .width(Length::Px(w))
        .height(Length::Px(middle_h));
    if let Some(shell) = left_sidebar {
        middle = middle.child(shell);
    }
    middle = middle.child(Flex::row().grow(1.0));
    if let Some(shell) = right_sidebar {
        middle = middle.child(shell);
    }

    let mut root = Flex::column().width(Length::Px(w)).height(Length::Px(h));
    // Transparent tab band — the hand-drawn tab bar paints underneath. Omitted
    // entirely when the top bar is hidden (`show_top_bar = false`).
    if tab_bar_height > 0.0 {
        // Left toggle at the far-left corner, right toggle at the far-right, spacer
        // between (over the hand-drawn tab bar). sidebar-fu-14.
        // Edge inset from a theme spacing token (resolved from the font at layout — no
        // hand-computed px). Vertical breathing room comes from centering a `Small` toggle.
        let mut band = Flex::row()
            .width(Length::Px(w))
            .height(Length::Px(tab_bar_height))
            .align(Align::Center)
            .pad_x(Spacing::Sm);
        if let Some(t) = left_toggle {
            band = band.child(t);
        }
        band = band.child(Flex::row().grow(1.0));
        if let Some(t) = right_toggle {
            band = band.child(t);
        }
        root = root.child(band);
    }
    root = root.child(middle);
    // Status (bottom) bar. Built only when shown — a zero-height `Surface` would
    // still paint its overflowing `Label`, so when `show_bottom_bar = false` we drop
    // the whole bar (and leave `signals.status` unset, which the per-frame updater
    // already treats as "nothing to update").
    if status_bar_height > 0.0 {
        // The status label's text is bound so mode/focus changes update it in place.
        let status_label = Label::new(status).font_size(CHROME_TEXT_SIZE).color(fg);
        let status_signal = status_label.text_signal();
        let (status_watch, _status_repaint) = RepaintWatch::new(status_label);
        signals.status = Some(status_signal);
        root = root.child(
            Surface::row()
                .width(Length::Px(w))
                .height(Length::Px(status_bar_height))
                .background(side_bg)
                .radius(0.0)
                .align(Align::Center)
                .padding_xy(8.0, 0.0)
                .child(status_watch),
        );
    }
    root
}

/// Layout + paint a (retained) chrome root tree into a [`Scene`] at the window size.
/// Re-run every frame; cheap and creates no signals (those live in the retained tree).
pub(crate) fn paint_chrome_root(root: &mut Flex, w: f32, h: f32, theme: &GuiTheme) -> Scene {
    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(root, Size::new(w as f64, h as f64));
    {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        root.paint(&mut cx);
    }
    scene
}

/// Paint the in-flight sidebar-drag overlay (drop indicator + ghost chip) into the
/// chrome `scene`, on top of the **expanded** grid-ui sidebar (F4.5 1b). Driven by the
/// retained-tree geometry (`resolve_at`) — not the legacy fixed-row hit-test — so the
/// indicator tracks the real laid-out pane cards. No-op unless a sidebar drag is in its
/// `Dragging` phase. The collapsed rail keeps its own hand-drawn ghost/highlight, so the
/// caller only invokes this for the expanded sidebar.
pub(crate) fn paint_drag_overlay(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let Some(surf) = state.mouse.drag_ctx.surface(DragSurfaceId::LeftSidebar) else {
        return;
    };
    // During paint the drag is in flight (phase is still `Dragging`), so the live
    // payload gives the source kind (for the source-aware filter) + the swap flag.
    let (source, swap) = match &surf.phase {
        DragPhase::Dragging { payload } => match payload {
            crate::app_state::AppDragPayload::Pane { swap, .. } => (DragSourceKind::Pane, *swap),
            crate::app_state::AppDragPayload::Column { swap, .. } => {
                (DragSourceKind::Column, *swap)
            }
        },
        _ => return,
    };
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));

    // Indicator on the hovered target, resolved with the *source-aware* filter (a
    // column drag hints columns/workspaces, not the nested pane cards). A swap targets
    // the WHOLE item (no before/after), so it uses the distinct swap indicator.
    if let Some((_item, hit)) = resolve_sidebar_drop(state, state.mouse.pos, source) {
        if swap {
            cx.swap_indicator(hit.bounds);
        } else {
            cx.drop_indicator(hit.bounds, hit.side);
        }
    }

    // Ghost chip following the cursor (offset off the pointer + vertically centered,
    // mirroring the legacy hand-drawn ghost so the two paths look identical).
    if let Some(label) = &surf.ghost_label {
        let rect = Rectangle::new(
            Point::new(
                (label.x + 10.0) as f64,
                (label.y - label.height / 2.0) as f64,
            ),
            Size::new(label.width as f64, label.height as f64),
        );
        cx.drag_ghost(rect, &label.text, swap);
    }
}

/// Keycap glyph size (logical px) for follow-link hints — compact so a label sits
/// legibly over a single terminal cell.
const LINK_HINT_FONT: f32 = 13.0;
/// Smaller keycap glyph size for **compact** targets (pane-header action buttons), so the
/// keycap stays proportional to the little button it captions rather than dwarfing it.
const HINT_COMPACT_FONT: f32 = 12.0;

/// Vertical band (from a hint target's top edge) the keycap is centered within. A
/// tall target (a workspace dock spanning its panes) gets its keycap centered on the
/// header row rather than floating in the middle; a short target (a pane card) gets it
/// centered on its single row. Roughly one sidebar row tall.
const HINT_BAND_H: f64 = 40.0;
/// Small inset from a hint target's left edge so the keycap sits just inside it.
const HINT_INSET_X: f64 = 2.0;
/// A hint target this small in **both** axes is icon-like (a pane-header action button):
/// stamping the keycap *over* it would hide the very glyph the user is choosing, so it is
/// placed just *outside* the target instead (see [`paint_hint_targets`]).
const HINT_COMPACT_MAX: f64 = 34.0;
/// Gap between a compact target's edge and its adjacent keycap.
const HINT_ADJACENT_GAP: f64 = 2.0;
/// Fraction of the keycap height a compact target's keycap is pulled back *toward* the
/// button, so it reads as attached to it rather than floating too far below/above.
const HINT_COMPACT_RISE: f64 = 0.42;

/// Peak alpha of the visual-bell flash overlay (faded out over the flash window).
const BELL_FLASH_MAX_ALPHA: u8 = 56;

/// Paint the **visual-bell** flash: a brief accent-tinted overlay over the content
/// area that fades out, while `state.bell_flash_until` is in the future. Drawn into
/// the chrome scene (on top). No-op when no flash is active. terminal-task-17.
pub(crate) fn paint_bell_flash(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    content_rect: Rectangle,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let Some(deadline) = state.bell_flash_until else {
        return;
    };
    let now = std::time::Instant::now();
    if now >= deadline {
        return;
    }
    let frac = deadline.saturating_duration_since(now).as_secs_f32()
        / crate::app::lifecycle::BELL_FLASH_DURATION.as_secs_f32();
    let alpha = (frac.clamp(0.0, 1.0) * BELL_FLASH_MAX_ALPHA as f32).round() as u8;
    if alpha == 0 {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    cx.rect(content_rect, theme.colors.accent.with_alpha(alpha), None, 0.0, None);
}

/// Paint follow-link keycaps over the focused terminal's hyperlinks while
/// [`InputMode::FollowLink`](crate::app_state::InputMode::FollowLink) is active.
/// Drawn into the chrome scene (painted last, on top of pane content) so the
/// letters sit above the terminal text, reusing the shared
/// [`paint_keycap`](heca_grid_ui::paint_keycap) visual. terminal-task-18.
pub(crate) fn paint_link_hints(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let crate::app_state::InputMode::FollowLink { candidates } = &state.input_mode else {
        return;
    };
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for hint in candidates {
        let Some((x, y)) = crate::app::terminal_host::cell_screen_pos(
            state,
            hint.pane_id,
            hint.row,
            hint.start_col,
        ) else {
            continue;
        };
        let label = hint.label.to_string();
        let size = heca_grid_ui::keycap_size(LINK_HINT_FONT, &label);
        // Anchor the keycap's top-left at the link's first cell.
        let cap = Rectangle::new(Point::new(x as f64, y as f64), size);
        heca_grid_ui::paint_keycap(
            &mut cx,
            cap,
            &label,
            LINK_HINT_FONT,
            None,
            heca_grid_ui::KeycapVariant::Filled,
        );
    }
}

/// One layer of the on-screen surface stack (front → back) for resolving which hint
/// targets are reachable. See `docs/surface-compositor.md`: a surface owns its targets,
/// the opaque region(s) it paints over lower layers (from real layout — never hardcoded),
/// and whether it is `modal` (a blocking context that suppresses everything beneath it).
struct HintLayer {
    band: LayerBand,
    targets: Vec<(heca_grid_ui::HintTargetId, Rectangle)>,
    occluders: Vec<Rectangle>,
    modal: bool,
}

/// The single visibility rule (`docs/surface-compositor.md` §3): walk the layers
/// **front → back**; a target is eligible iff it lies in `viewport` and its **centre** is
/// not covered by any higher layer's occluder; a **modal** layer cuts off everything
/// beneath it. This one rule subsumes every case — off-screen, hidden behind the sidebar,
/// a zoomed/floating pane drawn over another, a modal over the whole app — and extends to
/// new surfaces for free.
fn resolve_hint_layers(
    layers: &[HintLayer],
    viewport: Rectangle,
) -> Vec<(heca_grid_ui::HintTargetId, Rectangle)> {
    let covers = |r: &Rectangle, x: f64, y: f64| {
        x >= r.loc.x && x < r.loc.x + r.size.w && y >= r.loc.y && y < r.loc.y + r.size.h
    };
    let vr = viewport.loc.x + viewport.size.w;
    let vb = viewport.loc.y + viewport.size.h;
    let mut kept = Vec::new();
    let mut occluders: Vec<Rectangle> = Vec::new();
    for layer in layers {
        for &(id, b) in &layer.targets {
            let in_view = b.loc.x < vr
                && b.loc.x + b.size.w > viewport.loc.x
                && b.loc.y < vb
                && b.loc.y + b.size.h > viewport.loc.y;
            let cx = b.loc.x + b.size.w / 2.0;
            let cy = b.loc.y + b.size.h / 2.0;
            if in_view && !occluders.iter().any(|o| covers(o, cx, cy)) {
                kept.push((id, b));
            }
        }
        occluders.extend(layer.occluders.iter().copied());
        if layer.modal {
            break;
        }
    }
    kept
}

/// Build the current surface stack (front → back) and resolve the reachable hint targets
/// for the universal picker. **The one place hint visibility is decided.** The stack
/// mirrors the paint order so hints match what is visually on top; adding a new surface
/// (overlay / exposé / …) means adding a layer here, never a bespoke filter. See
/// `docs/surface-compositor.md`.
pub(crate) fn active_hint_targets(
    state: &crate::app_state::AppState,
) -> Vec<(heca_grid_ui::HintTargetId, Rectangle)> {
    let (vw, vh) = {
        let phys = state.window.inner_size();
        let s = state.scale_factor;
        (phys.width as f64 / s, phys.height as f64 / s)
    };
    let viewport = Rectangle::new(Point::new(0.0, 0.0), Size::new(vw, vh));
    let content = ChromeConfig {
        tab_bar_height: state.tab_bar_height(),
        status_bar_height: state.status_bar_height(),
        left_sidebar_width: state.left_sidebar_width(),
        right_sidebar_width: state.right_sidebar_width(),
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    }
    .content_rect(vw as f32, vh as f32);

    let mut layers: Vec<HintLayer> = Vec::new();

    // (Overlays — the context menu + the destructive-confirm dialog — are now
    //  dynamically-registered overlay-band layers, collected in step 4.)

    // 2. Chrome (top bar + sidebars), drawn on top of all pane content. Its own targets
    //    are eligible; the chrome frame AROUND the content (bars + sidebars) occludes pane
    //    targets beneath it. Occluders come from `content_rect`, not constants.
    if let Some(tree) = state.chrome_tree.as_ref() {
        let (cl, ct) = (content.loc.x, content.loc.y);
        let (cr, cb) = (content.loc.x + content.size.w, content.loc.y + content.size.h);
        layers.push(HintLayer {
            band: LayerBand::Overlay,
            targets: heca_grid_ui::collect_hint_targets(&tree.root),
            occluders: vec![
                Rectangle::new(Point::new(0.0, 0.0), Size::new(vw, ct)), // top bar
                Rectangle::new(Point::new(0.0, 0.0), Size::new(cl, vh)), // left sidebar
                Rectangle::new(Point::new(cr, 0.0), Size::new(vw - cr, vh)), // right sidebar
                Rectangle::new(Point::new(0.0, cb), Size::new(vw, vh - cb)), // status bar
            ],
            modal: false,
        });
    }

    // 3. Panes, front (topmost draw) → back. `pane_outer_frames` is in draw order (tiled
    //    then floats; last = on top), so reversing it yields front→back: floats over tiled,
    //    and a zoomed pane (a later sibling) over the panes behind it. Each pane occludes
    //    the ones beneath by its own frame.
    for (pane_id, x, y, w, h) in crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .rev()
    {
        let Some(header) = state.pane_headers.get(&pane_id) else {
            continue;
        };
        layers.push(HintLayer {
            band: LayerBand::Content,
            targets: heca_grid_ui::collect_hint_targets(&header.root),
            occluders: vec![Rectangle::new(
                Point::new(x as f64, y as f64),
                Size::new(w as f64, h as f64),
            )],
            modal: false,
        });
    }

    // 4. Dynamically registered layers (on-demand exposé, plugin panel). They join the
    //    same stack by their band; their targets + occluder come from their laid-out tree.
    for layer in state.layers.visible_front_to_back() {
        let bounds = layer.root.base().bounds;
        layers.push(HintLayer {
            band: layer.band,
            targets: heca_grid_ui::collect_hint_targets(layer.root.as_ref()),
            occluders: vec![bounds],
            modal: layer.modal,
        });
    }

    // Order the whole stack front → back by band (Modal in front … Background at the back),
    // stable so within-band order — built-ins before dynamics, and the draw order among
    // panes (zoomed/float on top) — is preserved.
    layers.sort_by_key(|l| std::cmp::Reverse(l.band.rank()));

    resolve_hint_layers(&layers, viewport)
}

/// Paint the universal hint-picker overlay (`prefix+/`): a glowing keycap over every
/// actionable chrome target, driven by `InputMode::HintPick`. Target bounds are
/// re-collected from the retained tree each frame and matched to the mode's candidates
/// by opaque id, so the keycaps track layout. Mirrors [`paint_link_hints`]. No-op when
/// the picker isn't active.
pub(crate) fn paint_hint_targets(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let crate::app_state::InputMode::HintPick { candidates } = &state.input_mode else {
        return;
    };
    // Look up the current bounds of every candidate id. Collect from the SAME surfaces the
    // picker resolves over (`active_hint_targets`) — chrome, per-pane headers, and any open
    // overlay — keyed by the shared globally-unique id, re-collected each frame so keycaps
    // track live layout. Which ids are *eligible* was already decided at pick time; here we
    // only need their live bounds.
    let mut bounds_by_id: std::collections::HashMap<heca_grid_ui::HintTargetId, Rectangle> =
        std::collections::HashMap::new();
    if let Some(tree) = state.chrome_tree.as_ref() {
        bounds_by_id.extend(heca_grid_ui::collect_hint_targets(&tree.root));
    }
    for header in state.pane_headers.values() {
        bounds_by_id.extend(heca_grid_ui::collect_hint_targets(&header.root));
    }
    // Dynamically-registered layers (overlay dialogs incl. the confirm prompt, plugin panels):
    // collect their targets
    // too, so an overlay's buttons show keycaps like any other surface.
    for layer in state.layers.visible_front_to_back() {
        bounds_by_id.extend(heca_grid_ui::collect_hint_targets(layer.root.as_ref()));
    }
    if bounds_by_id.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for (label, id) in candidates {
        let Some(bounds) = bounds_by_id.get(id) else {
            continue;
        };
        let text = label.to_string();
        // Icon-like targets (pane-header action buttons) are too small to hold a keycap
        // without hiding their glyph, so the letter is placed just *below* them, centered
        // and clamped on-screen — the icon stays fully visible with its letter as a
        // caption. It also gets a smaller keycap so it stays proportional to the little
        // button. Larger chrome targets (docks, cards, rows) keep the full-size keycap in
        // their top band (inset from the left), where it never occludes meaningful content.
        let compact = bounds.size.w <= HINT_COMPACT_MAX && bounds.size.h <= HINT_COMPACT_MAX;
        let font = if compact { HINT_COMPACT_FONT } else { LINK_HINT_FONT };
        let size = heca_grid_ui::keycap_size(font, &text);
        let (x, y) = if compact {
            let x = (bounds.loc.x + (bounds.size.w - size.w) / 2.0).clamp(0.0, w as f64 - size.w);
            // Pull the cap back toward the button so it sits snug under it, not floating.
            let rise = size.h * HINT_COMPACT_RISE;
            let below = bounds.loc.y + bounds.size.h + HINT_ADJACENT_GAP - rise;
            // Prefer below; flip above if it would fall off the bottom edge.
            let y = if below + size.h <= h as f64 {
                below
            } else {
                bounds.loc.y - size.h - HINT_ADJACENT_GAP + rise
            };
            (x, y)
        } else {
            let band = bounds.size.h.min(HINT_BAND_H);
            (bounds.loc.x + HINT_INSET_X, bounds.loc.y + (band - size.h) / 2.0)
        };
        let cap = Rectangle::new(Point::new(x, y), size);
        heca_grid_ui::paint_keycap(
            &mut cx,
            cap,
            &text,
            font,
            None,
            heca_grid_ui::KeycapVariant::Filled,
        );
    }
}

/// Lay out every visible dynamically-registered layer (an overlay dialog, a plugin panel)
/// at full viewport size. Mutable pass, run before the scene-texture borrow so
/// [`paint_layers`] can take a shared `&AppState`. Each layer's root is a self-centering /
/// self-positioning tree (e.g. a [`Dialog`](heca_grid_ui::Dialog) fills the viewport and
/// centers its panel). No-op when the registry is empty. This is the generic replacement for
/// the per-overlay `layout_*` passes (`docs/surface-compositor.md` §9).
pub(crate) fn layout_layers(state: &mut crate::app_state::AppState, w: f32, h: f32) {
    let font = chrome_gui_theme(state).font_size;
    for root in state.layers.visible_roots_mut() {
        LayoutEngine::new()
            .base_font(font)
            .compute(root.as_mut(), Size::new(w as f64, h as f64));
    }
}

/// Paint every visible dynamically-registered layer, **back → front** by band (so a Modal
/// paints over an Overlay paints over Content), on top of the chrome scene. Each layer's root
/// paints itself (overlay widgets draw their own scrim on `cx.with_overlay`). Run
/// [`layout_layers`] first. Generic replacement for the per-overlay `paint_*` passes.
pub(crate) fn paint_layers(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let layers = state.layers.visible_back_to_front();
    if layers.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for layer in layers {
        layer.root.paint(&mut cx);
    }
}

/// Peak alpha for a non-current search-match highlight; the current match is bolder.
const SEARCH_HL_ALPHA: u8 = 64;
const SEARCH_HL_CURRENT_ALPHA: u8 = 150;
/// Search bar glyph size (logical px).
const SEARCH_BAR_FONT: f32 = 13.0;

/// Paint the scrollback-search overlay: a highlight rect over every visible match
/// (the focused one bolder) plus a `/query` bar anchored to the searched pane's
/// bottom-right. Drawn into the chrome scene (on top). No-op when no search is
/// active. terminal-task-19.
pub(crate) fn paint_search(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let Some(search) = state.search.as_ref() else {
        return;
    };
    let pane_id = search.pane_id;
    let Some(snapshot) = state
        .backends
        .get(pane_id)
        .and_then(|b| b.terminal_snapshot())
    else {
        return;
    };
    let (cell_w, cell_h) = state
        .backends
        .get(pane_id)
        .map(|b| b.cell_size())
        .unwrap_or((8.0, 16.0));
    let top = snapshot.viewport_top_stable_row;
    let rows = snapshot.rows as isize;

    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));

    // Match highlights over the visible viewport.
    for (i, m) in search.matches.iter().enumerate() {
        let visible = m.stable_row - top;
        if visible < 0 || visible >= rows {
            continue;
        }
        let Some((x, y)) = crate::app::terminal_host::cell_screen_pos(
            state,
            pane_id,
            visible as usize,
            m.start_col,
        ) else {
            continue;
        };
        let width = m.end_col.saturating_sub(m.start_col) as f32 * cell_w;
        let rect = Rectangle::new(
            Point::new(x as f64, y as f64),
            Size::new(width as f64, cell_h as f64),
        );
        let alpha = if Some(i) == search.current {
            SEARCH_HL_CURRENT_ALPHA
        } else {
            SEARCH_HL_ALPHA
        };
        cx.rect(rect, theme.colors.accent.with_alpha(alpha), None, 2.0, None);
    }

    paint_search_bar(state, &mut cx, search, theme);
}

/// The `/query  n/total` bar at the searched pane's bottom-right corner.
fn paint_search_bar(
    state: &crate::app_state::AppState,
    cx: &mut PaintCx,
    search: &crate::app_state::SearchState,
    theme: &GuiTheme,
) {
    let Some((_, px, py, pw, ph)) = crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .find(|(id, ..)| *id == search.pane_id)
    else {
        return;
    };
    let count = if search.query.is_empty() {
        String::new()
    } else if search.matches.is_empty() {
        "  no matches".to_string()
    } else {
        let pos = search.current.map(|i| i + 1).unwrap_or(0);
        format!("  {}/{}", pos, search.matches.len())
    };
    let label = format!("/{}{}", search.query, count);

    let font = SEARCH_BAR_FONT;
    let pad = 8.0_f64;
    let advance = font as f64 * 0.62;
    let bar_w = (label.chars().count() as f64 * advance + 2.0 * pad).clamp(120.0, 480.0);
    let bar_h = font as f64 + 2.0 * pad;
    // Bottom-right of the pane, inset a little.
    let inset = 8.0_f64;
    let x = (px + pw) as f64 - bar_w - inset;
    let y = (py + ph) as f64 - bar_h - inset;
    let rect = Rectangle::new(Point::new(x, y), Size::new(bar_w, bar_h));

    let border = cx.border(theme.colors.accent.with_alpha(200));
    cx.rect(rect, theme.colors.surface, border, theme.colors.control_radius(), None);
    let text_rect = Rectangle::new(
        Point::new(x + pad, y),
        Size::new(bar_w - 2.0 * pad, bar_h),
    );
    cx.text(
        text_rect,
        &label,
        theme.colors.foreground,
        font,
        heca_grid_ui::scene::TextAlign::Start,
        false,
    );
}

/// Test helper: build + layout + paint in one shot. Runtime uses the retained tree
/// ([`build_chrome_root`] + [`paint_chrome_root`]) instead.
#[cfg(test)]
fn chrome_scene(
    frame: &ChromeFrame,
    theme: &GuiTheme,
    left_sidebar: Option<Flex>,
    right_sidebar: Option<Flex>,
) -> Scene {
    let mut signals = ChromeSignals::default();
    let mut root = chrome_root(frame, left_sidebar, right_sidebar, None, None, &mut signals);
    paint_chrome_root(&mut root, frame.w, frame.h, theme)
}

/// Convert a `0.0..=1.0` theme alpha token into an 8-bit channel value for
/// [`Color::with_alpha`]. Clamped so out-of-range config values can't wrap.
fn alpha_u8(a: f32) -> u8 {
    (a.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Projects the loaded app theme into the grid-ui widget theme contract.
///
/// With the compose model, `heca_grid_ui::Theme` embeds `heca_theme::Theme`
/// directly (the `colors` field), and `heca_config::theme::Theme` *is*
/// `heca_theme::Theme` (re-exported), so no field-by-field conversion is
/// needed — we clone the theme straight into `colors` and layer the
/// system-local font tokens on top. Font family/size come from `font_config`
/// (decoupled from the color theme).
fn app_theme_to_gui_theme(
    theme: &heca_config::theme::Theme,
    font_config: &heca_config::font::FontConfig,
) -> GuiTheme {
    GuiTheme {
        colors: theme.clone(),
        font_family: font_config.family.ui_normal().to_string(),
        font_size: font_config.size.ui,
        // TODO: map from config `focus_border_width` once added to heca-theme; for
        // now the affordance outlines (focus ring + selection) keep a visible default.
        focus_border_width: 1.5,
    }
}

fn chrome_surface_color(theme: &heca_config::theme::Theme) -> Color {
    theme.surface
}

fn top_bottom_pane_background_color(theme: &heca_config::theme::Theme) -> Color {
    theme.effective_top_bottom_pane_background()
}

fn chrome_bar_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    top_bottom_pane_background_color(theme).with_alpha(alpha_u8(appearance.chrome_opacity()))
}

fn sidebar_shell_background_color_for(
    sidebar_bg: Color,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    sidebar_bg.with_alpha(alpha_u8(appearance.opacity()))
}

fn chrome_shell_surface_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    chrome_surface_color(theme).with_alpha(alpha_u8(appearance.opacity()))
}

fn chrome_bar_color(state: &crate::app_state::AppState) -> Color {
    chrome_bar_color_for(&state.theme, &state.appearance)
}

fn left_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    let base = state.theme.effective_left_sidebar_background();
    let resolved = state.appearance.effective_sidebar_background_color(base);
    sidebar_shell_background_color_for(resolved, &state.appearance)
}

fn right_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    let base = state.theme.effective_right_sidebar_background();
    let resolved = state.appearance.effective_sidebar_background_color(base);
    sidebar_shell_background_color_for(resolved, &state.appearance)
}

fn chrome_shell_surface_color(state: &crate::app_state::AppState) -> Color {
    chrome_shell_surface_color_for(&state.theme, &state.appearance)
}

/// Returns the shared chrome bar background, sidebar inner-surface background,
/// and foreground text color for the current app state.
///
/// Sidebar shell backgrounds are driven separately so left/right sidebars can
/// use distinct theme tokens without changing the inner chrome surface color.
pub(crate) fn chrome_colors(state: &crate::app_state::AppState) -> (Color, Color, Color) {
    let bar_bg = chrome_bar_color(state);
    let sidebar_bg = chrome_shell_surface_color(state);
    let fg = state.theme.foreground;
    (bar_bg, sidebar_bg, fg)
}

/// Bridge the loaded app theme into a GuiTheme for chrome widgets.
pub(crate) fn chrome_gui_theme(state: &crate::app_state::AppState) -> GuiTheme {
    let (_, sidebar_bg, _) = chrome_colors(state);
    let mut theme = app_theme_to_gui_theme(&state.theme, &state.font_config);
    // App-wide font zoom scales the chrome/UI font alongside the terminals, so the
    // sidebar/tabs/status bar grow/shrink together with the panes. Read at paint
    // time so it live-updates; a zoom change also bumps `chrome_signature` to force
    // a tree rebuild at the new size.
    theme.font_size = state.app_ui_font_size();
    theme.colors.background = state.theme.background;
    theme.colors.surface = sidebar_bg;
    // `[appearance]` effect-token overrides take precedence over the theme.
    // `glow_size` owns glow (presence + radius + strength); `intensity` owns
    // scanline/CRT overlay opacity only. Unset → the theme value already set
    // above by `app_theme_to_gui_theme` wins.
    theme.colors.glow_size = state.appearance.effective_glow_size(&state.theme);
    theme.colors.intensity = state.appearance.effective_intensity(&state.theme);
    // Global decorative border width (config `border_width`, theme fallback) — the
    // app-wide BORDER control. Read at paint time, so it live-reloads. Drives the
    // chrome + sidebar frame width.
    theme.colors.border_width = state.appearance.effective_border_width(&state.theme);
    // Global decorative border color (config `border_color`, theme fallback) —
    // drives the chrome/sidebar `bordered` frame. Read at paint, so it live-reloads.
    theme.colors.border = state.appearance.effective_border_color(&state.theme);
    // Affordance outlines (focus ring + selection) get their own configurable
    // width, independent of the decorative border so they stay visible at
    // `border_width = 0`.
    theme.focus_border_width = state.appearance.effective_focus_border_width();
    theme
}

/// Apply a config [`BorderStyle`](heca_config::appearance::BorderStyle) as the
/// grid-ui [`Pane`] frame — the single mapping used for both terminal panes
/// (`pane_border_style`) and the sidebar shell (`sidebar_border_style`).
pub(crate) fn apply_pane_frame(
    pane: Pane,
    style: heca_config::appearance::BorderStyle,
) -> Pane {
    use heca_config::appearance::BorderStyle;
    match style {
        BorderStyle::None => pane.frameless(),
        BorderStyle::Bordered => pane.bordered(),
        BorderStyle::Bracketed => pane.bracketed(),
    }
}

/// The status-bar text projection (`N panes | focus | MODE…`).
fn chrome_status(state: &crate::app_state::AppState) -> String {
    let pane_count = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.scrolling
                .columns
                .iter()
                .map(|c| c.panes.len())
                .sum::<usize>()
        })
        .unwrap_or(0);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.custom_name.as_deref().unwrap_or(p.title.as_str()))
        .unwrap_or("—");
    let (mode_str, rename_hint) =
        crate::app::render::status_mode_parts(&state.input_mode, &state.action_catalog);
    format!(
        "{} panes | {} | {}{}",
        pane_count, focus_title, mode_str, rename_hint
    )
}

/// A **retained** chrome tree + the signature of the state that produced it. The
/// tree is rebuilt only when [`chrome_signature`] changes; otherwise it is just
/// re-laid-out and painted each frame. This keeps the widget signals alive across
/// frames (no per-frame signal churn) and gives a live tree to dispatch events into
/// (F4.2). The collapsed sidebar rail is still hand-drawn in `render.rs`.
pub(crate) struct RetainedChrome {
    pub(crate) root: Flex,
    pub(crate) sig: u64,
    /// Handles to the tree's **value** signals (selection + status), so they update
    /// in place via [`sync_chrome_signals`] instead of forcing a rebuild.
    pub(crate) signals: ChromeSignals,
    /// Maps each draggable/droppable widget's opaque [`DragItemId`] back to *what it
    /// is* (pane / column / workspace). Populated during [`build_chrome_root`] and
    /// queried by [`sidebar_drag_source`]/[`sidebar_drop_target`].
    pub(crate) drag_items: DragItemRegistry,
    /// The contiguous [`HintTargetId`](heca_grid_ui::HintTargetId) range this tree
    /// registered into the shared [`HintTargetRegistry`] on
    /// [`AppState`](crate::app_state::AppState). Removed from the shared map when the
    /// tree is rebuilt (see the chrome-rebuild path in `render.rs`).
    pub(crate) hint_range: std::ops::Range<usize>,
}

/// What a sidebar [`DragItemId`] refers to. The drag framework is domain-neutral
/// (ids are opaque `usize`); this app-side map gives them meaning. `ColumnId` can't
/// be the id directly — it's assigned inconsistently and can collide with a `PaneId`
/// (`scrolling.rs` builds `ColumnId(pane.id.0)`), so kind is decided by this map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChromeDragItem {
    /// A pane card (drag source + drop target).
    Pane(PaneId),
    /// A column [`MarkerGroup`], addressed positionally (drag source + drop target).
    Column { ws: usize, col: usize },
    /// A workspace [`DockFrame`] (drop target only — drop a column here to move it
    /// into that workspace).
    Workspace { ws: usize },
}

/// Build-time registry that hands out dense [`DragItemId`]s (id = push index) and
/// records what each refers to. Lives on [`RetainedChrome`]; rebuilt with the tree.
#[derive(Default, Clone, Debug)]
pub(crate) struct DragItemRegistry {
    items: Vec<ChromeDragItem>,
}

impl DragItemRegistry {
    /// Register a draggable/droppable item and return its freshly-assigned id.
    fn register(&mut self, item: ChromeDragItem) -> DragItemId {
        let id = DragItemId::new(self.items.len());
        self.items.push(item);
        id
    }

    /// Decode an id back to what it refers to (`None` if not from this build).
    pub(crate) fn get(&self, id: DragItemId) -> Option<&ChromeDragItem> {
        self.items.get(id.raw())
    }

    /// All registered items, in id order.
    #[cfg(test)]
    pub(crate) fn items(&self) -> &[ChromeDragItem] {
        &self.items
    }
}

/// **Shared** allocator + map from each hint target's opaque [`HintTargetId`] to the
/// [`InteractionIntent`](crate::app::interaction::InteractionIntent) the host fires when
/// its picker letter is chosen. Lives on [`AppState`](crate::app_state::AppState) so it
/// spans **every** retained tree that carries hint targets — the chrome tree AND each
/// per-pane header tree — which rebuild on independent cadences.
///
/// Ids come from a **monotonic** counter (never reused), so ids from different trees can
/// never collide even though the trees rebuild at different times. Each tree records the
/// contiguous [`Range`](std::ops::Range) of ids it registered and calls
/// [`remove_range`](HintTargetRegistry::remove_range) when it is rebuilt or pruned, so the
/// map only ever holds live targets. Parallel to [`DragItemRegistry`]; the grid-ui hint
/// framework is domain-neutral (ids are opaque) and this app-side map gives them meaning.
#[derive(Default, Clone)]
pub(crate) struct HintTargetRegistry {
    /// Next id to hand out (monotonic; never reset, so ids are globally unique).
    next: usize,
    /// Live targets only — stale ids are removed on their tree's rebuild/prune.
    intents:
        std::collections::HashMap<heca_grid_ui::HintTargetId, crate::app::interaction::InteractionIntent>,
}

impl HintTargetRegistry {
    /// Register an actionable target's intent and return its freshly-allocated id.
    pub(crate) fn register(
        &mut self,
        intent: crate::app::interaction::InteractionIntent,
    ) -> heca_grid_ui::HintTargetId {
        let id = heca_grid_ui::HintTargetId::new(self.next);
        self.next += 1;
        self.intents.insert(id, intent);
        id
    }

    /// The id the next [`register`](HintTargetRegistry::register) will hand out — take a
    /// checkpoint before and after building a tree to capture the contiguous id range it
    /// registered.
    pub(crate) fn checkpoint(&self) -> usize {
        self.next
    }

    /// Drop every id in `range` (a tree's previously-registered block) — called when that
    /// tree is rebuilt or pruned so the map never accumulates stale targets.
    pub(crate) fn remove_range(&mut self, range: std::ops::Range<usize>) {
        for i in range {
            self.intents.remove(&heca_grid_ui::HintTargetId::new(i));
        }
    }

    /// The intent for `id` (`None` if it is not a live target).
    pub(crate) fn get(
        &self,
        id: heca_grid_ui::HintTargetId,
    ) -> Option<&crate::app::interaction::InteractionIntent> {
        self.intents.get(&id)
    }
}

/// Handles to the retained chrome tree's **value** signals — the state that changes
/// without a structural change (pane/column selection + status text). Collected
/// during [`build_chrome_root`] and pushed each frame by [`sync_chrome_signals`], so
/// these values are **not** in [`chrome_signature`] and focus changes no longer
/// rebuild the tree. Rebuilt with the tree on structural change.
#[derive(Default)]
pub(crate) struct ChromeSignals {
    /// Each pane card's `active` signal, keyed by pane id.
    pub(crate) pane_active: Vec<(PaneId, Signal<bool>)>,
    /// Each column [`MarkerGroup`]'s `active` signal + the pane ids it holds (active
    /// iff it contains the active pane).
    pub(crate) col_active: Vec<(Vec<PaneId>, Signal<bool>)>,
    /// Each workspace [`DockFrame`]'s `active` signal + the pane ids it holds (active
    /// iff it contains the active pane). Drives the active-workspace accent wash in
    /// place, mirroring [`col_active`](ChromeSignals::col_active).
    pub(crate) ws_active: Vec<(Vec<PaneId>, Signal<bool>)>,
    /// Sidebar-nav cursor signals, mirroring the `*_active` families: each pane
    /// card's nav-outline signal (by pane id), each column's (by `(ws_idx, col_idx)`),
    /// each workspace's (by `ws_idx`). Driven from `nav_selection()` — the cursor
    /// highlight is distinct from `active_pane`.
    pub(crate) pane_nav: Vec<(PaneId, Signal<bool>)>,
    pub(crate) ws_nav: Vec<(usize, Signal<bool>)>,
    /// Each pane card's [`KeyHint`] pick-letter signal, keyed by pane id. Driven each
    /// frame from the active [`InputMode`](crate::app_state::InputMode) candidates
    /// (move/swap/take pick): `Some(letter)` while the pane is a candidate, else
    /// `None`. This is the move/swap/take **targeting overlay** — the keyboard logic
    /// (candidates + key consumption) already lives in the action/input layer; this
    /// only projects it into the retained Dock.
    pub(crate) pane_hint: Vec<(PaneId, Signal<Option<String>>)>,
    /// Each workspace [`DockFrame`]'s [`KeyHint`] pick-letter signal, keyed by `ws_idx`.
    /// Driven each frame from the active "move column/pane to workspace" pick candidates
    /// (mirrors [`pane_hint`](ChromeSignals::pane_hint)); the keycap is tinted differently
    /// (theme `warning`) so a workspace target reads distinctly from a pane target.
    pub(crate) ws_hint: Vec<(usize, Signal<Option<String>>)>,
    /// Each column [`MarkerGroup`]'s [`KeyHint`] pick-letter signal, keyed by
    /// `(ws_idx, col_idx)`. Driven from the active "move pane to column" pick candidates
    /// (only the active workspace's columns light up); tinted `success` to read distinctly
    /// from pane (accent) and workspace (warning) picks.
    pub(crate) col_hint: Vec<(usize, usize, Signal<Option<String>>)>,
    /// Per-pane runtime display signals for the fixed pane-info rows.
    pub(crate) pane_info: Vec<(PaneId, PaneInfoSignals)>,
    /// The status-bar label's text signal.
    pub(crate) status: Option<Signal<String>>,
}

/// The move/swap/take pick keycap for `pane` — `Some(letter)` while it is a pick
/// candidate, else `None`. The currently focused pane is never a target, so it shows
/// no keycap. Pure projection of the active `InputMode` candidates; mirrors the
/// legacy hand-drawn sidebar's `candidate_char` so the overlay reads identically.
fn pick_keycap(
    pane: PaneId,
    active: Option<PaneId>,
    candidates: Option<&[(char, PaneId)]>,
) -> Option<String> {
    if active == Some(pane) {
        return None;
    }
    candidates?
        .iter()
        .find(|(_, p)| *p == pane)
        .map(|(ch, _)| ch.to_string())
}

/// Find a pane's sidebar entry across all workspaces (tiled + floating).
fn find_pane_entry(tree: &SidebarTree, pane_id: PaneId) -> Option<&SidebarPaneEntry> {
    tree.workspaces
        .iter()
        .flat_map(|ws| {
            ws.columns
                .iter()
                .flat_map(|col| col.panes.iter())
                .chain(ws.floating_panes.iter())
        })
        .find(|pane| pane.pane_id == pane_id)
}

fn pane_fallback_name(tree: &SidebarTree, pane_id: PaneId) -> &str {
    find_pane_entry(tree, pane_id)
        .map(|pane| pane.name.as_str())
        .unwrap_or_else(|| unreachable!("pane {pane_id:?} must exist in sidebar tree"))
}

/// The pane's user-set override name (from rename), if any — wins over the process
/// title. `None` while the pane tracks its process.
fn pane_custom_name(tree: &SidebarTree, pane_id: PaneId) -> Option<&str> {
    find_pane_entry(tree, pane_id).and_then(|pane| pane.custom_name.as_deref())
}

fn sync_pane_runtime_state(
    session: &heca_core::layout::Session,
    workspaces: &WorkspacesContainerState,
) -> bool {
    use std::collections::HashSet;
    // TODO(pane-runtime-tasks:F3): this is a per-frame full-sync push of every pane's runtime
    // state into the store — the same push model Phase 0 is moving away from. It
    // is change-guarded (the setters only `.set()`/emit on real change, so there
    // are no spurious events or repaints), but it still borrows `panes` once per
    // pane per frame. Fold this into the reactive damage-path work (Phase 0's last
    // task) so runtime changes flow core→signal→paint without a per-frame scan.
    let mut live_panes = HashSet::new();
    let mut changed = false;
    for ws in &session.workspaces {
        for col in &ws.scrolling.columns {
            for pane in &col.panes {
                live_panes.insert(pane.id);
                changed |= workspaces.set_pane_runtime(
                    pane.id,
                    &pane.runtime,
                    pane.custom_name.as_deref(),
                );
            }
        }
        for float in &ws.floating_panes {
            live_panes.insert(float.pane.id);
            changed |= workspaces.set_pane_runtime(
                float.pane.id,
                &float.pane.runtime,
                float.pane.custom_name.as_deref(),
            );
        }
    }
    workspaces.retain_panes(&live_panes);
    changed
}

/// Project a borrowed sidebar-tree `SidebarItem` into the `Copy`
/// [`SidebarSelection`] mirrored in the chrome store (`None` stays `None`).
fn sidebar_selection_from_item(
    item: Option<&crate::sidebar::SidebarItem>,
) -> Option<SidebarSelection> {
    use crate::sidebar::SidebarItem;
    item.map(|it| match it {
        SidebarItem::Workspace { ws_idx } => SidebarSelection::Workspace { ws_idx: *ws_idx },
        SidebarItem::Column { ws_idx, col_idx } => SidebarSelection::Column {
            ws_idx: *ws_idx,
            col_idx: *col_idx,
        },
        SidebarItem::Pane { pane_id } => SidebarSelection::Pane { pane_id: *pane_id },
        SidebarItem::FloatingPane { pane_id, ws_idx } => SidebarSelection::FloatingPane {
            pane_id: *pane_id,
            ws_idx: *ws_idx,
        },
    })
}

/// Mirror canonical app/runtime state into the shared chrome store before the
/// retained tree reads it. `InputMode` remains the source of truth for keyboard
/// pick flows; the store is the reactive UI mirror.
pub(crate) fn sync_chrome_state(state: &mut crate::app_state::AppState) -> bool {
    state
        .chrome_state
        .workspaces
        .set_active_pane(state.focused_pane);
    state
        .chrome_state
        .workspaces
        .set_pane_renamed_add_process_name(state.pane_renamed_add_process_name);
    state
        .chrome_state
        .workspaces
        .set_pane_show_cwd(state.pane_show_cwd);
    // Project the sidebar-nav cursor selection into the store — while actually
    // navigating (`SidebarNav`) *or* while a context menu opened from the sidebar is up
    // (`sidebar_nav_active`), so the nav-cursor highlight shows during navigation, stays
    // on the target row while its menu is open, and clears on exit. Selection-driven:
    // this does NOT move the real focus (`active_pane`); the expanded sidebar renders
    // both, distinctly. The setter is a change-guarded chokepoint, so calling it every
    // frame is cheap.
    let nav_selection = if state.sidebar_nav_active() {
        sidebar_selection_from_item(state.sidebar_tree.current_item())
    } else {
        None
    };
    state
        .chrome_state
        .workspaces
        .set_nav_selection(nav_selection);
    let next_candidates = state
        .input_mode
        .candidates()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if next_candidates.is_empty() {
        state.chrome_state.workspaces.clear_pick_candidates();
    } else {
        state
            .chrome_state
            .workspaces
            .set_pick_candidates(next_candidates);
    }
    let next_ws_candidates = state
        .input_mode
        .ws_candidates()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if next_ws_candidates.is_empty() {
        state.chrome_state.workspaces.clear_ws_pick_candidates();
    } else {
        state
            .chrome_state
            .workspaces
            .set_ws_pick_candidates(next_ws_candidates);
    }
    let next_col_candidates = state
        .input_mode
        .col_candidates()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if next_col_candidates.is_empty() {
        state.chrome_state.workspaces.clear_col_pick_candidates();
    } else {
        state
            .chrome_state
            .workspaces
            .set_col_pick_candidates(next_col_candidates);
    }
    // Mirror the in-progress pick (its kind + prompt) into the store so components and
    // plugins can react to the pending action (e.g. a custom prompt overlay).
    let pending_pick = state.input_mode.pending_pick(&state.action_catalog);
    state.chrome_state.workspaces.set_pending_pick(pending_pick);
    // Phase 2: bridge backend-detected runtime → canonical `Pane.runtime` + emit
    // `pane.exited{code}` BEFORE mirroring `Pane.runtime` into the store.
    crate::app::process_monitor::sync_pane_runtime_from_backends(state);
    crate::app::git_monitor::sync_pane_git_from_cwds(state);
    sync_pane_runtime_state(&state.session, &state.chrome_state.workspaces)
}

/// Push the chrome's value-state (selection + status text) into the retained tree's
/// bound signals. Guarded — writes only on change, so unchanged frames cause no
/// signal churn. Called each frame before paint; this is what lets focus changes
/// update the highlight + status **without** rebuilding the tree.
pub(crate) fn sync_chrome_signals(state: &crate::app_state::AppState) -> bool {
    let Some(retained) = state.chrome_tree.as_ref() else {
        return false;
    };
    let mut changed = false;
    let active = state.chrome_state.workspaces.active_pane();
    for (pid, sig) in &retained.signals.pane_active {
        let v = active == Some(*pid);
        if sig.get_untracked() != v {
            sig.set(v);
            changed = true;
        }
    }
    for (pids, sig) in &retained.signals.col_active {
        let v = active.is_some_and(|a| pids.contains(&a));
        if sig.get_untracked() != v {
            sig.set(v);
            changed = true;
        }
    }
    for (pids, sig) in &retained.signals.ws_active {
        let v = active.is_some_and(|a| pids.contains(&a));
        if sig.get_untracked() != v {
            sig.set(v);
            changed = true;
        }
    }
    // Project the sidebar-nav cursor selection onto each row's nav-cursor signal —
    // distinct from `active` above, so the expanded sidebar shows both the real
    // focus and the nav cursor while navigating.
    let nav = state.chrome_state.workspaces.nav_selection();
    for (pid, sig) in &retained.signals.pane_nav {
        let v = matches!(nav, Some(SidebarSelection::Pane { pane_id }) if pane_id == *pid)
            || matches!(nav, Some(SidebarSelection::FloatingPane { pane_id, .. }) if pane_id == *pid);
        if sig.get_untracked() != v {
            sig.set(v);
            changed = true;
        }
    }
    for (ws_idx, sig) in &retained.signals.ws_nav {
        let v = matches!(nav, Some(SidebarSelection::Workspace { ws_idx: w }) if w == *ws_idx);
        if sig.get_untracked() != v {
            sig.set(v);
            changed = true;
        }
    }
    // Project the active move/swap/take pick candidates onto each pane's KeyHint
    // keycap. The candidates (char→PaneId) and the key-press consumption already
    // live in the `InputMode` / action layer (`app/input.rs`); this only mirrors the
    // letters into the retained Dock. The currently focused pane is never a target,
    // so it shows no keycap (matches the legacy hand-drawn sidebar's behavior).
    let candidates = state
        .chrome_state
        .workspaces
        .with_pick_candidates(|c| c.to_vec());
    for (pid, sig) in &retained.signals.pane_hint {
        let next = pick_keycap(*pid, active, Some(&candidates));
        if sig.get_untracked() != next {
            sig.set(next);
            changed = true;
        }
    }
    // Project the active "move to workspace" pick candidates onto each workspace's
    // KeyHint keycap (letter → `ws_idx`). Same mechanism as the pane hints above.
    let ws_candidates = state
        .chrome_state
        .workspaces
        .with_ws_pick_candidates(|c| c.to_vec());
    for (ws_idx, sig) in &retained.signals.ws_hint {
        let next = ws_candidates
            .iter()
            .find(|(_, w)| w == ws_idx)
            .map(|(ch, _)| ch.to_string());
        if sig.get_untracked() != next {
            sig.set(next);
            changed = true;
        }
    }
    // Project the active "move pane to column" candidates onto every column's KeyHint
    // (letter → `(ws_idx, col_idx)`), across all workspaces.
    let col_candidates = state
        .chrome_state
        .workspaces
        .with_col_pick_candidates(|c| c.to_vec());
    for (ws_idx, col_idx, sig) in &retained.signals.col_hint {
        let next = col_candidates
            .iter()
            .find(|(_, w, c)| w == ws_idx && c == col_idx)
            .map(|(ch, _, _)| ch.to_string());
        if sig.get_untracked() != next {
            sig.set(next);
            changed = true;
        }
    }
    for (pid, sigs) in &retained.signals.pane_info {
        let runtime = runtime_snapshot(&state.chrome_state.workspaces, *pid);
        let next = pane_info_view(
            &state.programs,
            pane_fallback_name(&state.sidebar_tree, *pid),
            pane_custom_name(&state.sidebar_tree, *pid),
            runtime.as_ref(),
            // Drive the sidebar card's `(process)` suffix live: compute the hint with the
            // real flag so a rename toggles it without a tree rebuild. Only `process_hint`
            // depends on this; icon/title/status/git are unaffected.
            state
                .chrome_state
                .workspaces
                .pane_renamed_add_process_name(),
        );
        let pane_active = active == Some(*pid);
        if sigs.icon.get_untracked() != next.icon {
            sigs.icon.set(next.icon);
            changed = true;
        }
        if sigs.title_active.get_untracked() != next.title {
            sigs.title_active.set(next.title.clone());
            changed = true;
        }
        if sigs.title_inactive.get_untracked() != next.title {
            sigs.title_inactive.set(next.title.clone());
            changed = true;
        }
        let hint_text = next
            .process_hint
            .as_deref()
            .map(|program| format!("({program})"))
            .unwrap_or_default();
        if sigs.process_hint.get_untracked() != hint_text {
            sigs.process_hint.set(hint_text);
            changed = true;
        }
        let hint_visible = next.process_hint.is_some();
        if sigs.process_hint_visible.get_untracked() != hint_visible {
            sigs.process_hint_visible.set(hint_visible);
            changed = true;
        }
        // Cwd row: text follows the live cwd, visibility follows the setting + presence.
        let cwd_path = runtime.as_ref().and_then(|rt| rt.cwd.clone());
        let cwd_text = cwd_path
            .as_deref()
            .map(home_relative_path)
            .unwrap_or_default();
        if sigs.cwd.get_untracked() != cwd_text {
            sigs.cwd.set(cwd_text);
            changed = true;
        }
        let cwd_visible =
            state.chrome_state.workspaces.pane_show_cwd() && cwd_path.is_some();
        if sigs.cwd_visible.get_untracked() != cwd_visible {
            sigs.cwd_visible.set(cwd_visible);
            changed = true;
        }
        for (signal, visible) in [
            (sigs.title_active_visible, pane_active),
            (sigs.title_inactive_visible, !pane_active),
            (sigs.status_idle_visible, next.status == ProcessStatus::Idle),
            (
                sigs.status_running_visible,
                next.status == ProcessStatus::Running,
            ),
            (
                sigs.status_success_visible,
                next.status == ProcessStatus::Success,
            ),
            (
                sigs.status_error_visible,
                next.status == ProcessStatus::Error,
            ),
        ] {
            if signal.get_untracked() != visible {
                signal.set(visible);
                changed = true;
            }
        }
        let git_visible = next.git_branch.is_some();
        if sigs.git_visible.get_untracked() != git_visible {
            sigs.git_visible.set(git_visible);
            changed = true;
        }
        let branch = next.git_branch.unwrap_or_default();
        if sigs.git_branch.get_untracked() != branch {
            sigs.git_branch.set(branch.clone());
            changed = true;
        }
        let branch_display = truncate_sidebar_git_branch(&branch);
        if sigs.git_branch_display.get_untracked() != branch_display {
            sigs.git_branch_display.set(branch_display);
            changed = true;
        }
        for (visible_signal, label_signal, value) in [
            (sigs.git_added_visible, sigs.git_added, next.git_added),
            (
                sigs.git_modified_visible,
                sigs.git_modified,
                next.git_modified,
            ),
            (sigs.git_deleted_visible, sigs.git_deleted, next.git_deleted),
        ] {
            let visible = value.is_some();
            if visible_signal.get_untracked() != visible {
                visible_signal.set(visible);
                changed = true;
            }
            let label = value.unwrap_or_default();
            if label_signal.get_untracked() != label {
                label_signal.set(label);
                changed = true;
            }
        }
    }
    if let Some(status) = &retained.signals.status {
        let next = chrome_status(state);
        if status.get_untracked() != next {
            status.set(next);
            changed = true;
        }
    }
    changed
}

/// Build the chrome root tree from app state (the expensive part — creates the
/// widget tree and its signals). Call only when [`chrome_signature`] changes.
pub(crate) fn build_chrome_root(
    state: &crate::app_state::AppState,
    chrome: ChromeConfig,
    hint_targets: &mut HintTargetRegistry,
) -> (Flex, ChromeSignals, DragItemRegistry) {
    let phys = state.window.inner_size();
    let scale = state.scale_factor as f32;
    let w = phys.width as f32 / scale;
    let h = phys.height as f32 / scale;
    let (side_bg, _sidebar_bg, fg) = chrome_colors(state);
    let theme = chrome_gui_theme(state);
    let status = chrome_status(state);
    let mut signals = ChromeSignals::default();
    let mut drag_items = DragItemRegistry::default();
    let event_proxy = state.event_proxy.clone();
    let emit_intent: ChromeIntentEmitter = Rc::new(move |intent| {
        let _ = event_proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
            source: crate::app::interaction::InteractionSource::MouseLeftSidebar,
            intent,
        });
    });

    // Expanded ⇄ Hidden: width is 0 when the region is Hidden (no icon rail — see
    // `docs/sidebar-provider-modes.md`), so a positive width means Expanded.
    // Left and right are two instances of the SAME `build_sidebar_shell` (one
    // component), differing only by width and mounted content: the left hosts the
    // `WorkspacesContainer`, the right is an empty placeholder until it gains a Provider.
    let sidebar_gap = state.appearance.effective_sidebar_gap(&state.theme);
    let border_style = state.appearance.effective_sidebar_border_style();
    let border_width = state.appearance.effective_sidebar_border_width(&state.theme);
    let border_radius = state.appearance.effective_sidebar_border_radius(&state.theme);
    let sidebar_h = (h - chrome.tab_bar_height - chrome.status_bar_height).max(0.0);

    let left_w = chrome.left_sidebar_width;
    let left_sidebar = (left_w > 0.0).then(|| {
        let content = build_workspaces_container(
            &state.sidebar_tree,
            &state.programs,
            &theme,
            &emit_intent,
            &state.chrome_state.workspaces,
            &mut signals,
            &mut drag_items,
            hint_targets,
        );
        build_sidebar_shell(
            left_w,
            sidebar_h,
            left_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
            Some(content),
        )
    });
    let right_w = chrome.right_sidebar_width;
    let right_sidebar = (right_w > 0.0).then(|| {
        build_sidebar_shell(
            right_w,
            sidebar_h,
            right_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
            None,
        )
    });

    // Top-bar collapse toggles (sidebar-fu-14): shown for each mounted sidebar so the
    // expand/collapse control is always visible (works in both expanded + collapsed).
    // The arrow flips with the state: expanded → point at the edge (collapse); collapsed
    // → point away from the edge (expand).
    let left_toggle = state.show_left_sidebar.then(|| {
        let glyph = if state.chrome_state.left_visible() {
            Glyph::ArrowLineLeft
        } else {
            Glyph::ArrowLineRight
        };
        sidebar_toggle_button(
            glyph,
            crate::input::WmAction::SidebarLeft,
            "sidebar_left",
            &state.action_shortcuts,
            &state.action_catalog,
            hint_targets,
            emit_intent.clone(),
            theme.colors.muted,
        )
    });
    let right_toggle = state.show_right_sidebar.then(|| {
        let glyph = if state.chrome_state.right_visible() {
            Glyph::ArrowLineRight
        } else {
            Glyph::ArrowLineLeft
        };
        sidebar_toggle_button(
            glyph,
            crate::input::WmAction::SidebarRight,
            "sidebar_right",
            &state.action_shortcuts,
            &state.action_catalog,
            hint_targets,
            emit_intent.clone(),
            theme.colors.muted,
        )
    });

    let root = chrome_root(
        &ChromeFrame {
            w,
            h,
            tab_bar_height: chrome.tab_bar_height,
            status_bar_height: chrome.status_bar_height,
            status: &status,
            side_bg,
            fg,
        },
        left_sidebar,
        right_sidebar,
        left_toggle,
        right_toggle,
        &mut signals,
    );
    (root, signals, drag_items)
}

/// Feed a pointer-press into the retained chrome tree so widget callbacks can route
/// sidebar intents through the app event loop. The tree is discarded afterwards so
/// incidental local widget state cannot drift away from the canonical store.
pub(crate) fn chrome_dispatch_press(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    if let Some(tree) = state.chrome_tree.as_mut() {
        tree.root.event(&Event::PointerPressed {
            pos: Point::new(pos.0 as f64, pos.1 as f64),
        });
    }
    state.chrome_tree = None;
}

/// Feed a pointer-move into the retained chrome tree so its **hover affordances**
/// update in the real app — the `MarkerGroup` grip brightening (the column's "grab
/// me" cue) and `Row` hover. The app otherwise only dispatches `PointerPressed`, so
/// these were inert in the sidebar though they work in the showcase. Unlike
/// [`chrome_dispatch_press`] this does **not** discard the tree — hover is transient
/// and must persist across moves; the caller already requests a repaint.
pub(crate) fn chrome_dispatch_move(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    if let Some(tree) = state.chrome_tree.as_mut() {
        tree.root.event(&Event::PointerMoved {
            pos: Point::new(pos.0 as f64, pos.1 as f64),
        });
    }
}

/// The pane a press at `pos` (logical window coords) would start dragging, found by
/// hit-testing the **retained** chrome tree's real laid-out bounds (F4.5) — replaces
/// the legacy fixed-row `sidebar_hit_test`. `None` off any pane card.
pub(crate) fn sidebar_drag_source(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<ChromeDragItem> {
    let tree = state.chrome_tree.as_ref()?;
    let id = heca_grid_ui::drag::source_at(&tree.root, Point::new(pos.0 as f64, pos.1 as f64))?;
    tree.drag_items.get(id).cloned()
}

/// The deepest sidebar item (pane → column → workspace) under `pos`, regardless of
/// drag semantics — used to anchor the right-click context menu on whatever the
/// cursor is over. Unlike [`sidebar_drag_source`] this accepts every registered
/// item (workspaces are drop-only, so they never appear as a drag source but must
/// still be right-clickable). Resolves against the **expanded** grid sidebar's
/// retained tree; the hand-drawn collapsed rail is not covered (it moves onto grid
/// widgets in the collapsed-rail migration, `app-task-21`).
pub(crate) fn sidebar_item_at(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
) -> Option<ChromeDragItem> {
    let tree = state.chrome_tree.as_ref()?;
    let hit = heca_grid_ui::drag::resolve_at_filtered(
        &tree.root,
        Point::new(pos.0 as f64, pos.1 as f64),
        &|_| true,
    )?;
    tree.drag_items.get(hit.id).cloned()
}

/// The kind of thing being dragged — passed **explicitly** by the caller so drop
/// resolution never depends on the live drag payload, which is already wiped to
/// `Idle` by the time the release handler runs (`mouse.rs` `mem::replace`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DragSourceKind {
    /// A pane is being dragged.
    Pane,
    /// A column is being dragged.
    Column,
}

/// Which drop-target kinds a given drag source may land on (F4.5 scope C). A pane
/// drag targets panes **and workspaces** — a workspace is only the resolved target
/// when the cursor is over its header/empty area (a pane card under the cursor is the
/// deeper hit and wins), which is the one way to move a pane into an *empty* workspace
/// (empty columns can't exist, so columns need no pane-drop target). A column drag
/// targets columns + workspaces — **never** the nested pane cards, or the deepest hit
/// would always be a pane and a column could never be dropped on another column.
fn target_accepted_by(source: DragSourceKind, item: &ChromeDragItem) -> bool {
    match source {
        DragSourceKind::Pane => {
            matches!(
                item,
                ChromeDragItem::Pane(_) | ChromeDragItem::Workspace { .. }
            )
        }
        DragSourceKind::Column => {
            matches!(
                item,
                ChromeDragItem::Column { .. } | ChromeDragItem::Workspace { .. }
            )
        }
    }
}

/// Resolve the drop a drag of `source` kind would land on at `pos`, filtered to the
/// target kinds it accepts (see [`target_accepted_by`]). `None` when no acceptable
/// target is under the cursor.
fn resolve_sidebar_drop(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
    source: DragSourceKind,
) -> Option<(ChromeDragItem, heca_grid_ui::drag::DropHit)> {
    let tree = state.chrome_tree.as_ref()?;
    let accept = |id| {
        tree.drag_items
            .get(id)
            .is_some_and(|it| target_accepted_by(source, it))
    };
    let hit = heca_grid_ui::drag::resolve_at_filtered(
        &tree.root,
        Point::new(pos.0 as f64, pos.1 as f64),
        &accept,
    )?;
    let item = tree.drag_items.get(hit.id).cloned()?;
    Some((item, hit))
}

/// The drop target + [`DropSide`](heca_grid_ui::drag::DropSide) a drag of `source`
/// kind at `pos` lands on, source-aware (see [`resolve_sidebar_drop`]). `None` off
/// any acceptable item.
pub(crate) fn sidebar_drop_target(
    state: &crate::app_state::AppState,
    pos: (f32, f32),
    source: DragSourceKind,
) -> Option<(ChromeDragItem, heca_grid_ui::drag::DropSide)> {
    resolve_sidebar_drop(state, pos, source).map(|(item, hit)| (item, hit.side))
}

/// Hash of everything the chrome tree displays (window size, theme, status text,
/// sidebar content). When it changes, the retained tree is rebuilt; otherwise the
/// existing tree is reused (re-laid-out + painted only).
pub(crate) fn chrome_signature(state: &crate::app_state::AppState, chrome: ChromeConfig) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hsh = std::collections::hash_map::DefaultHasher::new();
    let phys = state.window.inner_size();
    phys.width.hash(&mut hsh);
    phys.height.hash(&mut hsh);
    state.scale_factor.to_bits().hash(&mut hsh);
    // App-wide font zoom scales the chrome font, so a change must rebuild the tree.
    state.app_font_zoom.to_bits().hash(&mut hsh);
    chrome.left_sidebar_width.to_bits().hash(&mut hsh);
    chrome.right_sidebar_width.to_bits().hash(&mut hsh);
    chrome.sidebar_gap.to_bits().hash(&mut hsh);
    state.theme.name.hash(&mut hsh);
    for c in [
        state.theme.accent,
        state.theme.foreground,
        state.theme.border,
    ] {
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    state.theme.border_radius.to_bits().hash(&mut hsh);
    state.appearance.chrome_opacity().to_bits().hash(&mut hsh);
    state.appearance.opacity().to_bits().hash(&mut hsh);
    // The sidebar frame STYLE is a build-time structural choice (it picks the Pane
    // frame), so a config reload that changes it must rebuild the retained tree.
    // (The border WIDTH is read at paint via `chrome_gui_theme`, so it live-reloads
    // without a rebuild.)
    state.appearance.effective_sidebar_border_style().hash(&mut hsh);
    // The sidebar border WIDTH/RADIUS and background are baked into the retained
    // tree at build time (per-widget Pane overrides + the shell fill), so a config
    // reload that changes them must rebuild the tree.
    state
        .appearance
        .effective_sidebar_border_width(&state.theme)
        .to_bits()
        .hash(&mut hsh);
    state
        .appearance
        .effective_sidebar_border_radius(&state.theme)
        .to_bits()
        .hash(&mut hsh);
    {
        let c = left_sidebar_shell_background_color(state);
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
        let c = right_sidebar_shell_background_color(state);
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    // The active workspace (which gets the accent wash + count badge) is structural
    // enough to rebuild on a workspace SWITCH — but pane-to-pane focus *within* a
    // workspace must NOT rebuild: pane/column `active` + the status text are bound
    // signals (`sync_chrome_signals`), deliberately excluded from this signature.
    state.session.active_workspace_idx.hash(&mut hsh);
    for ws in &state.sidebar_tree.workspaces {
        ws.ws_idx.hash(&mut hsh);
        ws.name.hash(&mut hsh);
        ws.collapsed.hash(&mut hsh);
        for c in &ws.columns {
            c.col_idx.hash(&mut hsh);
            c.collapsed.hash(&mut hsh);
            for p in &c.panes {
                p.pane_id.0.hash(&mut hsh);
                p.name.hash(&mut hsh);
                state
                    .chrome_state
                    .workspaces
                    .with_pane_runtime(p.pane_id, |runtime| {
                        runtime.and_then(|rt| rt.git.get_untracked()).is_some()
                    })
                    .hash(&mut hsh);
            }
            u8::MAX.hash(&mut hsh); // column separator in the hash stream
        }
        for p in &ws.floating_panes {
            p.pane_id.0.hash(&mut hsh);
            p.name.hash(&mut hsh);
            state
                .chrome_state
                .workspaces
                .with_pane_runtime(p.pane_id, |runtime| {
                    runtime.and_then(|rt| rt.git.get_untracked()).is_some()
                })
                .hash(&mut hsh);
        }
        u64::MAX.hash(&mut hsh); // workspace separator
    }
    hsh.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_config::programs::ProgramsConfig;
    use heca_core::layout::{LayoutOptions, Session, SessionId};
    use heca_core::runtime::{ContentKind, GitInfo, PaneRuntime, ProcessStatus};
    use std::path::PathBuf;

    #[test]
    fn pick_keycap_projects_candidates() {
        let p1 = PaneId(1);
        let p2 = PaneId(2);
        let p3 = PaneId(3);
        let cands = [('a', p1), ('s', p2)];
        // No pick active → no keycap.
        assert_eq!(pick_keycap(p1, Some(p3), None), None);
        // Candidate pane → its letter.
        assert_eq!(
            pick_keycap(p1, Some(p3), Some(&cands)),
            Some("a".to_string())
        );
        assert_eq!(
            pick_keycap(p2, Some(p3), Some(&cands)),
            Some("s".to_string())
        );
        // Non-candidate pane → none.
        assert_eq!(pick_keycap(p3, Some(p1), Some(&cands)), None);
        // The focused pane is never a target, even if listed as a candidate.
        assert_eq!(pick_keycap(p1, Some(p1), Some(&cands)), None);
    }

    #[test]
    fn test_content_rect_full() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.loc.x, 200.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 880.0);
        assert_eq!(r.size.h, 744.0);
    }

    #[test]
    fn test_content_rect_clamping_when_sidebars_exceed_window() {
        // Both sidebars together exceed the window width.
        // Left sidebar is NOT clamped for x-position, but IS clamped for width calculation.
        // Right sidebar is clamped to remaining space after left sidebar.
        // Width must never go negative.
        let c = ChromeConfig {
            tab_bar_height: 20.0,
            status_bar_height: 10.0,
            left_sidebar_width: 300.0,
            right_sidebar_width: 300.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(500.0, 600.0);
        // x = 300, y = 20
        // left clamped: min(300, 500) = 300
        // right clamped: min(300, 500-300) = min(300, 200) = 200
        // w = 500 - 300 - 200 = 0  (not negative)
        // h = 600 - 20 - 10 = 570
        assert_eq!(r.loc.x, 300.0);
        assert_eq!(r.loc.y, 20.0);
        assert_eq!(r.size.w, 0.0);
        assert_eq!(r.size.h, 570.0);
    }

    #[test]
    fn test_content_rect_no_sidebars() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 0.0,
            right_sidebar_width: 0.0,
            sidebar_gap: 0.0,
        };
        let r = c.content_rect(1024.0, 768.0);
        assert_eq!(r.loc.x, 0.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 1024.0);
        assert_eq!(r.size.h, 712.0);
    }

    #[test]
    fn test_content_rect_reserves_sidebar_gap_between_sidebars_and_content() {
        let c = ChromeConfig {
            tab_bar_height: 32.0,
            status_bar_height: 24.0,
            left_sidebar_width: 200.0,
            right_sidebar_width: 200.0,
            sidebar_gap: 12.0,
        };
        let r = c.content_rect(1280.0, 800.0);
        assert_eq!(r.loc.x, 212.0);
        assert_eq!(r.loc.y, 32.0);
        assert_eq!(r.size.w, 856.0);
        assert_eq!(r.size.h, 744.0);
    }

    #[test]
    fn app_theme_to_gui_theme_preserves_loaded_palette_tokens() {
        let theme = heca_config::theme::load("mocha");
        let font_config = heca_config::font::FontConfig::default();
        let gui = app_theme_to_gui_theme(&theme, &font_config);

        assert_eq!(gui.colors.name, theme.name);
        assert_eq!(gui.colors.background, theme.background);
        assert_eq!(gui.colors.surface, theme.surface);
        assert_eq!(gui.colors.muted, theme.muted);
        assert_eq!(gui.colors.border, theme.border);
        assert_eq!(gui.colors.accent, theme.accent);
        assert_eq!(gui.colors.glow, theme.glow);
        assert_eq!(gui.colors.danger, theme.danger);
        assert_eq!(gui.colors.success, theme.success);
        assert_eq!(gui.colors.warning, theme.warning);
        assert_eq!(gui.font_family, font_config.family.ui_normal());
        assert_eq!(gui.font_size, font_config.size.ui);
        assert_eq!(gui.colors.border_radius, theme.border_radius);
        assert_eq!(gui.colors.border_width, theme.border_width);
        assert_eq!(gui.colors.glow_size, heca_grid_ui::theme::GlowLevel::None);
        assert_eq!(gui.colors.intensity, heca_grid_ui::theme::Intensity::Off);
        assert!(gui.colors.show_focus_border);
    }

    #[test]
    fn chrome_background_and_surface_colors_come_from_theme_tokens() {
        let tron = heca_config::theme::load("grid_tron");
        let latte = heca_config::theme::load("latte");

        assert_eq!(
            top_bottom_pane_background_color(&tron),
            tron.effective_top_bottom_pane_background()
        );
        assert_eq!(chrome_surface_color(&tron), tron.surface);
        assert_eq!(
            chrome_surface_color(&latte),
            latte.surface
        );
        assert_ne!(
            latte.effective_left_sidebar_background(),
            chrome_surface_color(&latte)
        );
    }

    #[test]
    fn sidebar_shell_background_stays_distinct_from_bar_tint_when_transparent() {
        let theme = heca_config::theme::load("latte");
        let appearance = heca_config::appearance::AppearanceConfig {
            transparency: 10,
            ..Default::default()
        };

        let left_bg = theme.effective_left_sidebar_background();
        assert_ne!(
            chrome_bar_color_for(&theme, &appearance),
            sidebar_shell_background_color_for(left_bg, &appearance)
        );
        assert_eq!(
            sidebar_shell_background_color_for(left_bg, &appearance).r,
            theme.effective_left_sidebar_background().r
        );
    }

    #[test]
    fn chrome_scene_emits_status_text() {
        use heca_grid_ui::{Color, DrawCommand};
        let theme = GuiTheme::default();
        // No sidebar (collapsed) — just the status bar should produce text.
        let scene = super::chrome_scene(
            &super::ChromeFrame {
                w: 800.0,
                h: 600.0,
                tab_bar_height: 32.0,
                status_bar_height: 24.0,
                status: "2 panes | foo | NORMAL",
                side_bg: theme.colors.background,
                fg: Color::new(200, 200, 200, 255),
            },
            &theme,
            None,
            None,
        );
        assert!(!scene.is_empty(), "scene should not be empty");
        assert!(
            scene.iter().any(|cmd| matches!(cmd, DrawCommand::Text(..))),
            "scene should contain at least one Text draw command",
        );
    }

    #[test]
    fn sidebar_shell_hosts_container() {
        use crate::app_state::SidebarItemState;
        use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree, SidebarWsEntry};
        use heca_grid_ui::Component;

        let mut tree = SidebarTree::new();
        tree.workspaces.push(SidebarWsEntry {
            ws_idx: 0,
            name: "ws1".into(),
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![SidebarColEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![SidebarPaneEntry {
                    pane_id: heca_core::layout::PaneId(1),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        let theme = GuiTheme::default();
        let emit_intent: super::ChromeIntentEmitter = Rc::new(|_| {});
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        chrome
            .workspaces
            .set_active_pane(Some(heca_core::layout::PaneId(1)));
        // The shell wraps a bracketed Pane that holds just the WorkspacesContainer (the
        // collapse toggle moved to the top bar, sidebar-fu-14); the container hosts a
        // dock per workspace (so the tree's text is visible).
        let content = super::build_workspaces_container(
            &tree,
            &heca_config::programs::ProgramsConfig::default(),
            &theme,
            &emit_intent,
            &chrome.workspaces,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
            &mut super::HintTargetRegistry::default(),
        );
        let shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            Some(content),
        );
        assert_eq!(
            shell.base().children.len(),
            1,
            "sidebar shell wraps a single full-height background surface",
        );
        let surface = &shell.base().children[0];
        assert_eq!(
            surface.base().children.len(),
            1,
            "the background surface wraps a single bracketed Pane",
        );
        let pane = &surface.base().children[0];
        assert_eq!(
            pane.base().children.len(),
            1,
            "the shell Pane holds just the WorkspacesContainer body (the collapse toggle \
             moved to the top bar, sidebar-fu-14)",
        );
        let container = &pane.base().children[0];
        assert!(
            !container.base().children.is_empty(),
            "WorkspacesContainer must host a dock per workspace",
        );
    }

    #[test]
    fn bordered_sidebar_paints_border_color_at_configured_width() {
        // Regression: with `sidebar_border_style = "bordered"`, the shell must paint
        // a visible frame in the (global) border color at the configured
        // `sidebar_border_width`. Previously the bordered sidebar drew nothing /
        // ignored the color because its width was theme-locked.
        use crate::app_state::SidebarItemState;
        use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree, SidebarWsEntry};
        use heca_grid_ui::DrawCommand;

        let mut tree = SidebarTree::new();
        tree.workspaces.push(SidebarWsEntry {
            ws_idx: 0,
            name: "ws1".into(),
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![SidebarColEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![SidebarPaneEntry {
                    pane_id: heca_core::layout::PaneId(1),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        // A distinct border color so we can prove it reached the painted frame.
        let mut theme = GuiTheme::default();
        theme.colors.border = Color::new(0x40, 0xe0, 0xff, 0xff);
        let border_w = 4.0_f32;

        let emit_intent: super::ChromeIntentEmitter = Rc::new(|_| {});
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let content = super::build_workspaces_container(
            &tree,
            &heca_config::programs::ProgramsConfig::default(),
            &theme,
            &emit_intent,
            &chrome.workspaces,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
            &mut super::HintTargetRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bordered,
            border_w,
            12.0,
            Some(content),
        );

        let scene = super::paint_chrome_root(&mut shell, 280.0, 600.0, &theme);
        let found = scene.iter().any(|c| match c {
            DrawCommand::Rect(r) => r
                .border
                .is_some_and(|b| b.color == theme.colors.border && (b.width - border_w).abs() < 0.01),
            _ => false,
        });
        assert!(
            found,
            "bordered sidebar must paint a {border_w}px frame in the configured border color",
        );
    }

    #[test]
    fn drag_registry_captures_pane_column_and_workspace() {
        use crate::app_state::SidebarItemState;
        use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree, SidebarWsEntry};

        let mut tree = SidebarTree::new();
        tree.workspaces.push(SidebarWsEntry {
            ws_idx: 0,
            name: "ws1".into(),
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![SidebarColEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![SidebarPaneEntry {
                    pane_id: heca_core::layout::PaneId(7),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        let theme = GuiTheme::default();
        let emit_intent: super::ChromeIntentEmitter = Rc::new(|_| {});
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut drag = super::DragItemRegistry::default();
        // Drag items are registered by the WorkspacesContainer content (mounted into the
        // shell), so build that directly with the drag registry.
        let _ = super::build_workspaces_container(
            &tree,
            &heca_config::programs::ProgramsConfig::default(),
            &theme,
            &emit_intent,
            &chrome.workspaces,
            &mut super::ChromeSignals::default(),
            &mut drag,
            &mut super::HintTargetRegistry::default(),
        );

        let items = drag.items();
        // The drag framework decides kind by the side-map, not by raw ids — so a pane,
        // its column, and its workspace must each be registered (F4.5 step 2 scope C).
        assert!(items.contains(&super::ChromeDragItem::Pane(heca_core::layout::PaneId(7))));
        assert!(items.contains(&super::ChromeDragItem::Column { ws: 0, col: 0 }));
        assert!(items.contains(&super::ChromeDragItem::Workspace { ws: 0 }));
    }

    #[test]
    fn sync_pane_runtime_state_projects_session_runtime_into_store() {
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        {
            let ws = session
                .active_workspace_mut()
                .expect("session should create an initial workspace");
            ws.add_pane(
                heca_core::layout::Pane::new(PaneId(10), "editor"),
                None,
                true,
                ColumnWidth::Proportion(0.5),
            );
            ws.floating_panes
                .push(heca_core::layout::workspace::FloatingPane {
                    pane: heca_core::layout::Pane::new(PaneId(20), "git"),
                    position: Point::new(50.0, 50.0),
                    size: Size::new(400.0, 300.0),
                    is_active: true,
                    original_column_idx: None,
                    original_pane_idx: None,
                });
            ws.find_pane_mut(PaneId(10)).expect("tiled pane").runtime = PaneRuntime {
                program: Some("nvim".into()),
                status: ProcessStatus::Running,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: Some(0),
                git: Some(GitInfo {
                    branch: Some("main".into()),
                    ahead: 1,
                    behind: 0,
                    added: 2,
                    modified: 3,
                    deleted: 4,
                    dirty: true,
                }),
                kind: ContentKind::Terminal,
            };
            ws.find_pane_mut(PaneId(20)).expect("floating pane").runtime = PaneRuntime {
                program: Some("lazygit".into()),
                status: ProcessStatus::Idle,
                cwd: Some(PathBuf::from("/tmp/project")),
                exit_code: None,
                git: None,
                kind: ContentKind::Terminal,
            };
        }

        sync_pane_runtime_state(&session, &chrome.workspaces);

        let tiled = chrome.workspaces.with_pane_runtime(PaneId(10), |runtime| {
            runtime.expect("tiled runtime").snapshot()
        });
        let floating = chrome.workspaces.with_pane_runtime(PaneId(20), |runtime| {
            runtime.expect("floating runtime").snapshot()
        });
        assert_eq!(tiled.program.as_deref(), Some("nvim"));
        assert_eq!(tiled.status, ProcessStatus::Running);
        assert_eq!(tiled.cwd, Some(PathBuf::from("/tmp/project")));
        assert_eq!(
            tiled.git,
            Some(GitInfo {
                branch: Some("main".into()),
                ahead: 1,
                behind: 0,
                added: 2,
                modified: 3,
                deleted: 4,
                dirty: true,
            })
        );
        assert_eq!(floating.program.as_deref(), Some("lazygit"));
        assert_eq!(floating.status, ProcessStatus::Idle);
    }

    #[test]
    fn sync_pane_runtime_state_prunes_removed_panes() {
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut session = Session::new(
            SessionId(1),
            Size::new(1280.0, 800.0),
            1.0,
            LayoutOptions::default(),
        );
        {
            let ws = session
                .active_workspace_mut()
                .expect("session should create an initial workspace");
            ws.add_pane(
                heca_core::layout::Pane::new(PaneId(10), "editor"),
                None,
                true,
                ColumnWidth::Proportion(0.5),
            );
        }

        sync_pane_runtime_state(&session, &chrome.workspaces);
        assert!(
            chrome
                .workspaces
                .with_pane_runtime(PaneId(10), |runtime| runtime.is_some())
        );

        session
            .active_workspace_mut()
            .expect("session should keep its workspace")
            .scrolling
            .columns
            .clear();
        sync_pane_runtime_state(&session, &chrome.workspaces);

        assert!(
            !chrome
                .workspaces
                .with_pane_runtime(PaneId(10), |runtime| runtime.is_some())
        );
    }

    #[test]
    fn pane_info_view_resolves_program_status_and_git_segments() {
        let programs = ProgramsConfig::default();
        let runtime = PaneRuntime {
            program: Some("v".into()),
            status: ProcessStatus::Running,
            cwd: None,
            exit_code: None,
            git: Some(GitInfo {
                branch: Some("main".into()),
                ahead: 0,
                behind: 0,
                added: 2,
                modified: 3,
                deleted: 1,
                dirty: true,
            }),
            kind: ContentKind::Terminal,
        };

        // No custom name → title is the program name, no process hint (the title IS the process).
        let view = pane_info_view(&programs, "shell", None, Some(&runtime), true);
        assert_eq!(view.title, "Neovim");
        assert_eq!(view.process_hint, None);

        // Custom name + setting on → title is the custom name, hint carries the program name.
        let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), true);
        assert_eq!(view.title, "Editor");
        assert_eq!(view.process_hint.as_deref(), Some("Neovim"));
        // Custom name + setting off → no hint.
        let view = pane_info_view(&programs, "shell", Some("Editor"), Some(&runtime), false);
        assert_eq!(view.title, "Editor");
        assert_eq!(view.process_hint, None);

        let view = pane_info_view(&programs, "shell", None, Some(&runtime), false);

        assert_eq!(view.icon, Glyph::FileCode);
        assert_eq!(view.title, "Neovim");
        assert_eq!(view.status, ProcessStatus::Running);
        assert_eq!(view.git_branch.as_deref(), Some("main"));
        assert_eq!(view.git_added.as_deref(), Some("+2"));
        assert_eq!(view.git_modified.as_deref(), Some("~3"));
        assert_eq!(view.git_deleted.as_deref(), Some("-1"));
    }

    #[test]
    fn pane_info_view_uses_shell_fallbacks_without_git() {
        let programs = ProgramsConfig::default();
        let runtime = PaneRuntime {
            program: Some("zsh".into()),
            status: ProcessStatus::Idle,
            ..PaneRuntime::default()
        };

        let view = pane_info_view(&programs, "pane", None, Some(&runtime), false);

        assert_eq!(view.icon, Glyph::Terminal);
        assert_eq!(view.title, "zsh");
        assert_eq!(view.status, ProcessStatus::Idle);
        assert_eq!(view.git_branch, None);
        assert_eq!(view.git_added, None);
        assert_eq!(view.git_modified, None);
        assert_eq!(view.git_deleted, None);
    }

    #[test]
    fn pane_name_segment_renders_the_panes_own_name() {
        use heca_config::appearance::PaneSegment;
        let programs = ProgramsConfig::default();
        let theme = GuiTheme::default();
        let runtime = PaneRuntime {
            program: Some("v".into()),
            status: ProcessStatus::Running,
            ..PaneRuntime::default()
        };

        // Renamed pane → the `pane_name` segment produces a bar (the custom name wins over
        // the program name, unlike `app_name` which always tracks the process).
        let bar = build_pane_info_bar(
            &programs,
            "shell",
            Some("Editor"),
            Some(&runtime),
            &[PaneSegment::PaneName],
            &theme,
            400.0,
            13.0,
        );
        assert!(bar.is_some());

        // Un-renamed pane → still produces a bar (falls back to the program name, never empty).
        let bar = build_pane_info_bar(
            &programs,
            "shell",
            None,
            Some(&runtime),
            &[PaneSegment::PaneName],
            &theme,
            400.0,
            13.0,
        );
        assert!(bar.is_some());
    }

    #[test]
    fn pane_action_spec_maps_kinds_to_actions() {
        use crate::input::WmAction;
        use heca_config::appearance::PaneAction;
        let pid = PaneId(7);
        let catalog = crate::actions::ActionCatalog::with_builtins();

        // Pane-parameterized actions carry the pane/column and don't need focus. Icons
        // resolve from the action catalog (close = FolderSimpleMinus; add-pane = the
        // add_pane_to_column identity → FolderSimplePlus).
        let (g, a, _, focus) = super::pane_action_spec(&catalog, PaneAction::Close, pid, 2, 3);
        assert_eq!(g, Glyph::FolderSimpleMinus);
        assert_eq!(a, WmAction::ClosePaneById { pane_id: pid });
        assert!(!focus);

        let (g, a, _, focus) = super::pane_action_spec(&catalog, PaneAction::Split, pid, 2, 3);
        assert_eq!(g, Glyph::FolderSimplePlus);
        assert_eq!(
            a,
            WmAction::AddPaneToColumn {
                ws_idx: 2,
                col_idx: 3
            }
        );
        assert!(!focus);

        // Active-targeted actions use the requested icons + need focus-first.
        let (g, a, _, focus) = super::pane_action_spec(&catalog, PaneAction::Zoom, pid, 0, 0);
        assert_eq!(g, Glyph::FrameCorners);
        assert_eq!(a, WmAction::ZoomColumn);
        assert!(focus);

        let (g, a, _, focus) = super::pane_action_spec(&catalog, PaneAction::Float, pid, 0, 0);
        assert_eq!(g, Glyph::Cards);
        assert_eq!(a, WmAction::Float);
        assert!(focus);
    }

    #[test]
    fn floating_pane_keeps_only_float_and_close() {
        use heca_config::appearance::PaneAction;
        let catalog = crate::actions::ActionCatalog::with_builtins();
        // Driven by the action policy: float/close are focused-pane-local (kept),
        // split/zoom/move are tiled-only (hidden when floating).
        assert!(super::pane_action_visible_when_floating(&catalog, PaneAction::Float));
        assert!(super::pane_action_visible_when_floating(&catalog, PaneAction::Close));
        assert!(!super::pane_action_visible_when_floating(&catalog, PaneAction::Split));
        assert!(!super::pane_action_visible_when_floating(&catalog, PaneAction::Zoom));
        assert!(!super::pane_action_visible_when_floating(&catalog, PaneAction::MoveLeft));
        assert!(!super::pane_action_visible_when_floating(&catalog, PaneAction::MoveRight));
    }

    #[test]
    fn pane_header_key_changes_on_content_and_width() {
        use heca_config::appearance::{PaneAction, PaneSegment};
        let programs = ProgramsConfig::default();
        let segments = [PaneSegment::AppName, PaneSegment::GitBranch];
        let actions = [PaneAction::Split, PaneAction::Close];
        let runtime = PaneRuntime {
            program: Some("zsh".into()),
            status: ProcessStatus::Idle,
            git: Some(GitInfo {
                branch: Some("main".into()),
                ..GitInfo::default()
            }),
            ..PaneRuntime::default()
        };
        let hints = ActionShortcuts::default();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        #[allow(clippy::too_many_arguments)]
        fn content<'a>(
            programs: &'a ProgramsConfig,
            segments: &'a [heca_config::appearance::PaneSegment],
            actions: &'a [heca_config::appearance::PaneAction],
            rt: &'a PaneRuntime,
            hints: &'a ActionShortcuts,
            catalog: &'a crate::actions::ActionCatalog,
            col_idx: usize,
        ) -> PaneHeaderContent<'a> {
            PaneHeaderContent {
                programs,
                fallback_name: "shell",
                custom_name: None,
                runtime: Some(rt),
                segments,
                actions,
                ws_idx: 0,
                col_idx,
                zoomed: false,
                floating: false,
                shortcuts: hints,
                catalog,
            }
        }
        let base = pane_header_key(
            &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
            15.0,
            300.0,
        );
        // Same inputs ⇒ same key (no needless rebuild).
        assert_eq!(
            base,
            pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                300.0
            )
        );
        // A different column ⇒ different key (re-bakes the split action's col_idx).
        assert_ne!(
            base,
            pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 1),
                15.0,
                300.0
            )
        );
        // A different branch ⇒ different key (rebuild).
        let mut other = runtime.clone();
        other.git = Some(GitInfo {
            branch: Some("dev".into()),
            ..GitInfo::default()
        });
        assert_ne!(
            base,
            pane_header_key(
                &content(&programs, &segments, &actions, &other, &hints, &catalog, 0),
                15.0,
                300.0
            )
        );
        // A large width change ⇒ different key (re-truncate); tiny jitter ⇒ same bucket.
        assert_ne!(
            base,
            pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                120.0
            )
        );
        assert_eq!(
            base,
            pane_header_key(
                &content(&programs, &segments, &actions, &runtime, &hints, &catalog, 0),
                15.0,
                295.0
            )
        );
    }

}
