//! Chrome metrics and central application constants.
//!
//! UI dimensions, timing defaults, layout proportions, and render parameters
//! that were previously scattered as magic numbers across the codebase.

mod contribution;
pub(crate) mod context_menu;
mod events;
mod expose;
pub(crate) use expose::register as register_expose;

/// Re-register a **host-owned** named layer, so what it shows is current.
///
/// The one place a layer name maps to the code that rebuilds it. A layer whose content is derived
/// from app state cannot be kept fresh by signals alone — those replace a prop, never a child — so
/// it is rebuilt, and `add_named` replacing under the same name is what makes that safe.
///
/// An unknown name is a no-op: a plugin's layer is rebuilt by the plugin, not from here.
pub(crate) fn rebuild_named_layer(state: &mut crate::app_state::AppState, name: &str) {
    if Some(name) == layers::layer_name(layers::HOST_OWNER, "expose").as_deref() {
        expose::register(state);
    }
}
mod focus;
mod host;
mod layers;
mod overlay;
mod palette;
mod state;
// Registry API surface consumed by the next migration steps (ShowLayer/HideLayer, the
// confirm dialog as a layer, plugins) — some names not yet referenced in-binary.
#[allow(unused_imports)]
pub(crate) use expose::record_expose_cursor;
pub(crate) use layers::{
    LayerBackdrop, LayerBand, LayerId, LayerKind, LayerRegistry,
};
// Declarative UI model (plugin-task-ui-1); consumed by `realize` (ui-3) + Modal body (ui-4).
// It lives in the `heca-view` crate since F003/P017/T009 — a plugin depends on that crate, and it
// cannot depend on this binary. Re-exported here so the app keeps one path to the vocabulary.
#[allow(unused_imports)]
pub(crate) use heca_view::{
    Intent, PropMap, PropValue, ViewAlign, ViewNode, ViewSize, ViewVariant, WidgetKind,
};
// The mapper (plugin-task-ui-3): `ViewNode` → retained grid-ui `Component`. It lives in
// `heca-view-realize` since F003/P017/T009 — below this crate, so anything that can build widgets
// can render a described tree, the showcase included. The app supplies the two seams it takes:
// `ViewHintTargets` for pick registration, a wrapping closure for the click sink.
#[allow(unused_imports)]
pub(crate) use heca_view_realize::{realize, FormBindings, IntentEmitter};
// Host-owned overlay stack (plugin-task-ui-4, §2.7.1/§2.7.2), built on `LayerRegistry`.
// `OverlayId` is pub (carried by `WmAction`); the rest is crate-internal.
pub use overlay::OverlayId;
#[allow(unused_imports)]
pub(crate) use overlay::{
    collect_form as collect_overlay_form, open_dropdown, open_modal, resolve as resolve_overlay,
    content_covered, top_modal, DropdownItem, DropdownSpec, ModalAction, ModalResult, ModalSpec,
    OverlayHost,
};
// Context-menu resolution: ContextPath + ContextTarget + ContextMenuRegistry + the unified
// `open_context_menu_for`. Built-in providers seeded at startup; a provider attaches its own
// entries via `Provider::context_menus` (context-menu-5).
#[allow(unused_imports)]
pub(crate) use context_menu::{
    open_context_menu_for, resolve_active_context, ContextMenuProvider, ContextMenuRegistry,
    ContextPath, ContextTarget,
};
pub use context_menu::MenuBuild;
// The command palette: every registered action, searchable, dispatched through the one door
// (F003/P085/T358).
pub(crate) use palette::open_command_palette;
pub use contribution::{ContextMenuContribution, Contribution, RegionSet};
pub use events::{ChromeEvent, ChromeEventBus, ChromeSubscription, RegionId, SidebarSelection};
// Chrome keyboard focus: which dock the keyboard is aimed at (F003/P011/T020).
pub(crate) use focus::{dock_candidates, navigable_dock, placement_for, region_on_screen};
pub use host::ChromeHost;
pub use state::{SharedChromeState, WorkspacesContainerState};
// Contribution/placement API surface for the render + provider phases (plugin-03).
// These are public seam types not yet consumed by name in-binary — same rationale
// as the `#![allow(dead_code)]` carried by the modules that define them.
// `MoveError` is `ChromeHost::move_container`'s error; handlers report it via
// `Display` today and the mouse/DnD path names it in plugin-03.
#[allow(unused_imports)]
pub use contribution::{
    BuildBody, BuildCx, ContainerContribution, ContainerId, OverlaySpec, PanelContribution,
    StatusSegment, ToolbarGroup, WidgetModel,
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

use crate::providers::workspaces::{PaneEntry, WorkspaceTree};
use heca_config::programs::{ProgramIcon, ProgramsConfig};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_grid_ui::builders::{ComponentExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::drag::{DragItemId, DragPhase, DragSurfaceId};
use heca_grid_ui::reactive::{Signal, SignalGet, SignalUpdate, signal};
use heca_grid_ui::style::{Align, Justify, Length, Spacing, WidgetSize};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    BadgeButton, Flex, FocusScope, Glyph, HintPlacement, Icon, IconButton, KeyHint, Label, Pane,
    ScrollBar, Separator, Surface, Tag,
    Tooltip,
    TooltipSide,
};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PaneInfoView {
    pub(crate) icon: Glyph,
    /// Custom name if set, else the program name — the sidebar card's label.
    pub(crate) title: String,
    /// The program/application name (always the process, never the rename) — the info-bar
    /// `AppName` segment shows this, so renaming a pane doesn't hide what's running in it.
    pub(crate) app_name: String,
    /// The process/program name shown as a small dimmed label *next to* a custom name (e.g.
    /// `(nvim)`). `Some` only when the pane has a custom name and `pane_renamed_add_process_name`
    /// is on — a pane that merely tracks its process has the process name *as* its title already.
    pub(crate) process_hint: Option<String>,
    pub(crate) status: ProcessStatus,
    pub(crate) git_branch: Option<String>,
    pub(crate) git_added: Option<String>,
    pub(crate) git_modified: Option<String>,
    pub(crate) git_deleted: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneInfoSignals {
    pub(crate) icon: Signal<Glyph>,
    pub(crate) title_active: Signal<String>,
    pub(crate) title_inactive: Signal<String>,
    pub(crate) title_active_visible: Signal<bool>,
    pub(crate) title_inactive_visible: Signal<bool>,
    /// The dimmed `(process)` suffix text beside a renamed pane's name (e.g. `(nvim)`),
    /// or empty when hidden. Signal-driven so a rename toggles it live without a tree
    /// rebuild (renames update signals, they don't rebuild the sidebar card).
    pub(crate) process_hint: Signal<String>,
    pub(crate) process_hint_visible: Signal<bool>,
    /// The pane's working-directory row text (home-relative path), and its visibility
    /// (`[settings] pane_show_cwd` and the pane has a cwd). Signal-driven so a `cd` in the
    /// pane updates the path live, mirroring the git-branch row.
    pub(crate) cwd: Signal<String>,
    pub(crate) cwd_visible: Signal<bool>,
    pub(crate) status_idle_visible: Signal<bool>,
    pub(crate) status_running_visible: Signal<bool>,
    pub(crate) status_success_visible: Signal<bool>,
    pub(crate) status_error_visible: Signal<bool>,
    pub(crate) git_visible: Signal<bool>,
    pub(crate) git_branch: Signal<String>,
    pub(crate) git_branch_display: Signal<String>,
    pub(crate) git_added_visible: Signal<bool>,
    pub(crate) git_added: Signal<String>,
    pub(crate) git_modified_visible: Signal<bool>,
    pub(crate) git_modified: Signal<String>,
    pub(crate) git_deleted_visible: Signal<bool>,
    pub(crate) git_deleted: Signal<String>,
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

pub(crate) fn pane_info_view(
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
pub(crate) fn home_relative_path(path: &std::path::Path) -> String {
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
pub(crate) const CARD_META_FONT_SCALE: f32 = 0.8;

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
/// **The bindings, not a sentence** (F003/P085/T358). One entry per binding, in the index's order —
/// an action bound in two layers has two — and each stored as the **raw** config key
/// (`"prefix+Shift+e"`), spelled for display on read.
///
/// It used to store one pre-joined display string, which decided for every surface at once. That is
/// what left the command palette unable to draw keycaps: a surface that wants a line joins, a
/// surface that wants a chip per key cannot un-join — and the two want different spellings anyway
/// (a keycap draws a Nerd Font `⇧`, a tooltip has to write `Shift`).
#[derive(Clone, Default, PartialEq)]
pub(crate) struct ActionShortcuts {
    by_action: std::collections::HashMap<String, Vec<String>>,
    /// How [`get`](ActionShortcuts::get) spells a binding for a **text** surface.
    style: crate::shortcut::KeyStyle,
}

impl ActionShortcuts {
    /// Resolve a display shortcut for every action the **built keymaps** bind (F003/P086/T366).
    ///
    /// Built from [`Keymaps::by_action`](crate::keymap::Keymaps::by_action) rather than from the
    /// config file, because the file is only part of the answer: reading `[keys]` alone missed every
    /// `[[keys.mode]]` binding, would miss every `[[keys.component]]` one, and could never see a key
    /// a plugin registered at runtime. A tooltip that says nothing for a key the user can actually
    /// press is the bug this closes.
    ///
    /// An action bound in several layers keeps all of them — a component's `j` and a mode's `j` are
    /// both real, and picking the first would be a guess.
    ///
    /// `style` is how [`get`](Self::get) spells a binding for a text surface — the optional
    /// spelling switch, chosen once here rather than at each of the call sites that render a tip.
    /// Keycaps are unaffected: they read [`chords`](Self::chords) and draw glyphs.
    pub(crate) fn from_index(
        index: &crate::keymap::BindingIndex,
        style: crate::shortcut::KeyStyle,
    ) -> Self {
        let by_action = index
            .iter()
            .filter_map(|(action, bound)| {
                // **One entry per distinct key.** A config can bind the same physical key twice
                // under two spellings — the shipped default did: `cursor_up = "k,Up,ArrowUp"`, and
                // `Up` and `ArrowUp` both parse to `GridKey::ArrowUp` — and the index keeps both,
                // correctly, because it records what was written. A surface showing them draws the
                // same keycap twice. Deduped here rather than in the config, because a user's own
                // file can do it too and their config is not ours to police.
                let mut keys: Vec<String> = Vec::new();
                let mut seen: Vec<Vec<heca_grid_ui::widgets::KeyCap>> = Vec::new();
                for b in bound {
                    // Compared as **caps**, not as text: the caps come from the one key table, which
                    // maps `up` and `arrowup` (and `enter`/`return`) to a single glyph — so two
                    // spellings of one key collapse in every display style, while two genuinely
                    // different keys stay two.
                    let caps = crate::shortcut::chord_caps(&b.key);
                    if !seen.contains(&caps) {
                        seen.push(caps);
                        keys.push(b.key.clone());
                    }
                }
                (!keys.is_empty()).then(|| (action.clone(), keys))
            })
            .collect();
        Self { by_action, style }
    }

    /// Every binding of `action_name` as its **raw config key**, in the index's order — empty when
    /// unbound. What a surface drawing keycaps reads, through
    /// [`chord_caps`](crate::shortcut::chord_caps).
    pub(crate) fn chords(&self, action_name: &str) -> &[String] {
        self.by_action.get(action_name).map_or(&[], Vec::as_slice)
    }

    /// The display shortcut for `action_name` as **one line** (several joined with ` / `), or `None`
    /// when unbound — what a tooltip, which has a single line, shows.
    pub(crate) fn get(&self, action_name: &str) -> Option<String> {
        let keys = self.by_action.get(action_name)?;
        let line = keys
            .iter()
            .map(|k| {
                crate::shortcut::format_shortcut_styled(k, k.starts_with("prefix+"), self.style)
            })
            .collect::<Vec<_>>()
            .join(" / ");
        (!line.is_empty()).then_some(line)
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
    let hints: Vec<String> = content
        .actions
        .iter()
        .map(|&a| content.shortcuts.get(pane_action_name(a)).unwrap_or_default())
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
            let proxy = ctx.event_proxy.clone();
            let pane_id = ctx.pane_id;
            let wm_action = spec.wm_action.clone();
            let needs_focus = spec.needs_focus;
            // **One gesture, written once**: the click and the `prefix+/` pick both run this.
            // Active-targeted actions (zoom/float) act on the focused pane, so it focuses this pane
            // first — the events are queued and processed in order on the UI thread, so the action
            // lands on this pane.
            let fire = move || {
                use crate::app::interaction::{InteractionIntent, InteractionSource};
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
            };
            let peek = fire.clone();
            let button = IconButton::new(Icon::new(spec.glyph).color(icon_color))
                .size(WidgetSize::Header)
                .tone(tone)
                .active(spec.is_active)
                .on_click(fire);
            // **What a pick does to this button**, declared on the button itself: no id, no
            // registry, and nothing for a config-added or plugin-added button to forget — the
            // picker collects the declaration out of the laid-out tree.
            let button = KeyHint::new(button).on_peek(peek);
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
            widgets.scrollbar.base_mut().style.layout.width =
                heca_grid_ui::style::Length::Px(8.0);
            widgets.scrollbar.base_mut().style.layout.height =
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

/// Drop every retained pane header. Used when headers are globally invalidated: the info-bar is
/// turned off, or a config reload changes the theme/font baked into the trees (see `reload_config`,
/// which mirrors this alongside `terminal_layers.clear()`). `sync_pane_headers` rebuilds them from
/// scratch next frame.
///
/// Nothing has to be released with them: a header's pickable buttons declare what a pick does
/// **on themselves** ([`PeekTarget`]), so a tree that is gone simply has no declarations left.
pub(crate) fn clear_pane_headers(state: &mut crate::app_state::AppState) {
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
            let ctx = PaneHeaderCtx {
                pane_id: input.pane_id,
                ws_idx: input.ws_idx,
                col_idx: input.col_idx,
                event_proxy: state.event_proxy.clone(),
            };
            match build_pane_header(&content, &theme, font, input.avail_w, ctx) {
                Some(root) => {
                    state
                        .pane_headers
                        .insert(input.pane_id, RetainedPaneHeader { root, key });
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
    // Prune vanished panes.
    state.pane_headers.retain(|id, _| seen.contains(id));
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

/// Deliver a pointer event to the open modal layer — **the whole set, in one place**.
///
/// Returns `true` when a modal owns the pointer, so every caller stops there and nothing leaks to
/// the page behind. There is deliberately **one** function rather than a branch per winit event:
/// the input this tree needs is not a per-caller choice.
///
/// That choice is what kept breaking. A widget with a *gesture* needs the whole set or it fails in
/// a way nothing catches — a [`ScrollRegion`](heca_grid_ui::ScrollRegion) that never receives the
/// release leaves its thumb welded to the cursor, and one that never receives the wheel simply
/// does not scroll. Both are silent: it lays out, paints and hit-tests perfectly. The modal path
/// used to forward the move and the press only, so both happened, and the fix had already been
/// written twice elsewhere without the hole here being visible from either.
///
/// Since F004/P084/T394 there is only one pointer event to forward — [`Event::Raw`] — and the
/// framework resolves it into whatever it meant. The set cannot go missing a kind because there
/// are no longer kinds to choose between.
///
/// So: a caller says *a pointer event happened*, not *which kinds this surface deigns to forward*.
pub(crate) fn dispatch_modal_pointer(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    debug_assert!(
        matches!(ev, Event::Raw(_)),
        "dispatch_modal_pointer is the pointer path; keys go through the keymap",
    );
    if top_modal(state).is_none() {
        return false;
    }
    if let Some(root) = state.layers.top_modal_root_mut() {
        let _ = heca_grid_ui::dispatch(root, ev);
    }
    state.mark_full_redraw();
    true
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
        heca_grid_ui::dispatch(&mut header.root, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
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
        let _ = heca_grid_ui::dispatch(&mut header.root, &Event::pointer_moved(point));
        if rect_contains(header.root.base().bounds, point) {
            over = true;
        }
    }
    over
}

/// Feed a pointer release into the retained pane headers, so a gesture that started on one can end.
///
/// The header seam had a press and a move and no release — the last of the four surfaces to be
/// missing a kind. Nothing there grabs the pointer *today*, which is exactly why it went unnoticed:
/// the first widget mounted here that does would have been broken on arrival, the same way a scroll
/// region was in three other places. Not hit-tested, deliberately: a release ends the gesture
/// wherever the cursor drifted to.
pub(crate) fn dispatch_pane_header_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left));
    }
}

/// Feed the wheel into the retained pane headers. Returns `true` when one consumed it.
///
/// Nothing in a header scrolls today. It is wired anyway, because "no widget here needs it yet" is
/// the reasoning that produced every other missing kind.
pub(crate) fn dispatch_pane_header_wheel(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    let mut handled = false;
    for header in state.pane_headers.values_mut() {
        handled |= heca_grid_ui::dispatch(&mut header.root, ev) == heca_grid_ui::Handled::Yes;
    }
    handled
}

/// Feed a pointer press into the retained terminal viewport widgets. Returns
/// `true` when any widget consumed the press (badge click or scrollbar drag).
pub(crate) fn dispatch_pane_viewport_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    let point = Point::new(pos.0 as f64, pos.1 as f64);
    for widgets in state.pane_viewport_widgets.values_mut() {
        if heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes
            || heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_pressed(point, heca_grid_ui::PointerButton::Left))
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
        let badge_handled = heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_moved(point))
            == heca_grid_ui::Handled::Yes;
        let scrollbar_handled = heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_moved(point))
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
        handled |= heca_grid_ui::dispatch(&mut widgets.badge, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
        handled |= heca_grid_ui::dispatch(&mut widgets.scrollbar, &Event::pointer_released(point, heca_grid_ui::PointerButton::Left))
            == heca_grid_ui::Handled::Yes;
    }
    handled
}

fn rect_contains(r: Rectangle, p: Point) -> bool {
    p.x >= r.loc.x && p.x <= r.loc.x + r.size.w && p.y >= r.loc.y && p.y <= r.loc.y + r.size.h
}

pub(crate) fn runtime_snapshot(state: &WorkspacesContainerState, pane_id: PaneId) -> Option<PaneRuntime> {
    state.pane_runtime(pane_id)
}

// Kept short so the branch + git counts fit the sidebar card width without
// overflowing (the row isn't width-clipped). The full branch is on hover.
const SIDEBAR_GIT_BRANCH_MAX_CHARS: usize = 22;

/// Truncate a branch for the sidebar, keeping the **tail** (the meaningful end,
/// e.g. `…security-upgrade`) rather than the boilerplate `feature/` prefix.
pub(crate) fn truncate_sidebar_git_branch(branch: &str) -> String {
    truncate_path_left(branch, SIDEBAR_GIT_BRANCH_MAX_CHARS)
}

/// How a chrome widget reports a user action back to the app: it emits an
/// [`InteractionIntent`](crate::app::interaction::InteractionIntent), never a direct
/// state mutation. Handed to container providers through
/// [`ChromeCtx::emit_intent`](crate::providers::ChromeCtx::emit_intent).
pub(crate) type ChromeIntentEmitter = Rc<dyn Fn(crate::app::interaction::InteractionIntent)>;

/// Transparent wrapper that marks only its own bounds dirty when the host bumps
/// `request`. This lets retained chrome updates damage the specific card/marker/label
/// instead of the entire chrome root.
pub(crate) struct RepaintWatch {
    base: heca_grid_ui::Base,
    request: Signal<u64>,
    seen: u64,
}

impl RepaintWatch {
    pub(crate) fn new(child: impl Component + 'static) -> (Self, Signal<u64>) {
        let mut base = heca_grid_ui::Base::new();
        base.style.layout.width = Length::Auto;
        base.style.layout.height = Length::Auto;
        base.style.layout.direction = heca_grid_ui::Direction::Column;
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
            if child.base().style.layout.hidden {
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

/// Make a node take the box its parent gives it instead of the size of its own content.
///
/// A wrapper that hugs its child measures the child's *content*, and a flex item's automatic minimum
/// size then stops it shrinking back — so a dock 1214px tall keeps all 1214px inside the 296px slot
/// its share won and overflows the frame. The fix is CSS's `flex: 1 1 0`: a zero base size plus
/// permission to shrink, so the parent's box is what there is to divide.
///
/// **Do not copy this trio into new code.** It is the same debt [`with_share`] carries and for the
/// same reason — `Layout` has no `flex_basis`, so a zero base size has to be written as a height,
/// which is a fixed measure standing in for a proportion. P052(F004)/T350 replaces both with one
/// `share(n)` setter in the library; this exists so a *transparent* wrapper stays transparent until
/// then, rather than each caller rediscovering the combination.
fn pass_box_down(node: &mut dyn Component) {
    let layout = &mut node.base_mut().style.layout;
    layout.flex_grow = 1.0;
    layout.height = heca_grid_ui::Length::Px(0.0);
    layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
    layout.flex_shrink = Some(1.0);
}

/// Give a container body its declared share of the region's **main axis**, as a flex grow factor
/// (F003/P011/T021) — height in a sidebar, width in a bar, one number either way.
///
/// Applied to every container however many are seated, so the rule needs no special case: alone it
/// takes the whole region, two equal shares take half each, `2.0` beside `1.0` takes two thirds,
/// and `0.0` is content-sized.
///
/// Set by the region rather than by the container, because a share only means anything relative to
/// its siblings — which a container cannot see and should not have to.
fn with_share(mut body: WidgetModel, grow: f32) -> WidgetModel {
    let layout = &mut body.base_mut().style.layout;
    layout.flex_grow = grow;
    if grow > 0.0 {
        // A share has to be **of the region**, not of what is left over after the content.
        //
        // `flex_grow` alone distributes only *positive* free space, and a container's content is
        // routinely taller than the sidebar — so two containers measured 1214px each inside a 600px
        // body, overflowed the frame, and got no share at all. In CSS this is `flex: 1 1 0`; there
        // is no `flex_basis` in this vocabulary, so the equivalent is a **zero base size** plus
        // permission to shrink. Then the free space is the whole region and the shares divide it:
        // 296px each, measured.
        //
        // Safe because a shared container is expected to scroll its own content — it nests its own
        // scroll area, so being handed less height than its content is the normal case, not a
        // squeeze. A container that asked for `0.0` is saying "size me to my content" and keeps its
        // natural height.
        layout.height = heca_grid_ui::Length::Px(0.0);
        layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
        layout.flex_shrink = Some(1.0);
    }
    body
}

/// Wrap a container body in the two things the **host** owns about it: whether it holds chrome
/// keyboard focus, and its letter while a dock pick is open (F003/P011/T020).
///
/// Both are host state, not container state — a container cannot know that it is the focused one, or
/// which letter it was given among its siblings — so they are applied here rather than left to each
/// provider to remember. Both wrappers are transparent: they hug the body and route events, focus and
/// drag straight through, so the container behaves exactly as it does unwrapped.
///
/// The focus signal is the **same one** the container's own scroll area binds as its keyboard target
/// (`StateView::container_keyboard_target`), so the ring and the keys can never disagree about which
/// dock has focus.
fn focus_and_pick(
    mut body: WidgetModel,
    container: &str,
    share: f32,
    ctx: &crate::providers::ChromeCtx<'_>,
    signals: &mut ChromeSignals,
) -> WidgetModel {
    let hint = signal::<Option<String>>(None);
    signals.dock_hint.push((container.to_string(), hint));
    // The share lands on the outermost node, so every level below it has to pass the box down or the
    // dock keeps its content height inside the slot its share won — measured 1214px inside 296px, the
    // same failure F003/P011/T021 fixed one level up.
    //
    // Only when there *is* a share to pass: a container that asked for `0.0` is saying "size me to my
    // content", and a zero base size inside a content-sized parent would collapse it to nothing.
    if share > 0.0 {
        pass_box_down(body.as_mut());
    }
    let mut picked = KeyHint::new_boxed(body)
        .hint(hint)
        // Top-centre over a tall dock. Outside a render pass there is no theme to tint it with, and
        // there is no keycap to draw either (no pick is open while a host reads metadata), so the
        // default accent stands.
        .placement(HintPlacement::TopCenter);
    if let Some(theme) = ctx.theme() {
        // The theme's `warning` tone, so a dock letter reads distinctly from a pane pick (accent)
        // and a column pick (success).
        picked = picked.color(theme.colors.warning);
    }
    if share > 0.0 {
        pass_box_down(&mut picked);
    }
    // Stamp the placement id on the outermost wrapper, so a press anywhere inside — including on a
    // widget that consumes it — resolves back to this container (`nav::scope_at`,
    // F003/P086/T365). It goes here because this is the one place the host already wraps every
    // mount, so a container gets click-to-focus with nothing declared, a plugin's included.
    Box::new(
        FocusScope::new(picked)
            .focus(ctx.state().container_keyboard_target(container))
            .scope_key(container),
    )
}

/// Build the body of a chrome **region** from whatever the [`ChromeHost`] has seated in
/// it — the render half of the pluggable-chrome contract.
///
/// For each mounted container, in the host's order: ask its provider for a
/// [`Contribution`] and call the container's `build` seam. The other contribution kinds
/// belong to other hosts (bars take `ToolbarGroup`/`StatusSegment`, overlays go to the
/// overlay host, §3.1.1), so a region ignores them rather than guessing.
///
/// `None` — not an empty widget — when nothing is mounted, so the shell can tell "no
/// provider here" from "a provider that built an empty body".
///
/// Takes the three registries rather than a ready-made [`BuildCx`] because the context is **per
/// container**, not per region: each build hook is told which mount it is building, so it can ask
/// for state that is per mount (its own scroll offset). One `BuildCx` for a whole region could not
/// carry that (F003/P011/T021).
fn build_region_content(
    host: &ChromeHost,
    region: RegionId,
    ctx: &crate::providers::ChromeCtx<'_>,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> Option<WidgetModel> {
    let mut bodies = host
        .contributions(region)
        .iter()
        .filter_map(|mounted| match mounted.provider().build_contribution(ctx) {
            Contribution::Container(c) => {
                let mut bx = BuildCx::new(&c.id, signals, drag);
                let body = (c.build)(ctx, &mut bx);
                // The share goes on the OUTERMOST node, so it has to be applied after the wrappers:
                // a share set on the body would leave the wrapper content-sized and divide nothing
                // (F003/P011/T021's lesson, one level up).
                Some(with_share(
                    focus_and_pick(body, &c.id, c.grow, ctx, signals),
                    c.grow,
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    match bodies.len() {
        0 => None,
        // One container owns the region: its own share already makes it fill the region, so
        // there is nothing to wrap it in.
        1 => bodies.pop(),
        // Several containers share a region: stack them in the host's order (the order
        // `reorder`/`move_container` maintain), each keeping its own body and its own share. The
        // stack itself must be allowed to shrink to the region, or it takes its content's height
        // and overflows before the shares are ever divided.
        _ => {
            let mut stack = Flex::column().gap(8.0).grow(1.0);
            {
                let layout = &mut stack.base_mut().style.layout;
                layout.min_height = Some(heca_grid_ui::Length::Px(0.0));
                layout.flex_shrink = Some(1.0);
            }
            // A rule between containers, so two of them read as two things rather than one long
            // list. It takes no share: a `Separator` is a leaf with its own height, and `with_share`
            // only touches the containers, so the rule keeps its natural 1px and the shares divide
            // what is left. Its colour comes from the theme's border token, so it follows a reload.
            let count = bodies.len();
            Some(Box::new(bodies.into_iter().enumerate().fold(
                stack,
                |col, (i, body)| {
                    let col = if i > 0 && i < count {
                        col.child(Separator::horizontal())
                    } else {
                        col
                    };
                    col.child_boxed(body)
                },
            )))
        }
    }
}

/// Build a sidebar **SHELL** — a full-height, bracket-framed, frosted panel filling a
/// sidebar column. This is **one component, two instances**: the left and right
/// sidebars are the same shell differing only by width/position and the `content`
/// mounted inside. Per the chrome plan (`pluggable-chrome-plugin-plan.md` §2.1 / §2.8,
/// `docs/sidebar-provider-modes.md`) the sidebar is a *shell* that hosts a Provider's
/// content — and that is now literally true: `content` is whatever
/// [`ChromeHost::contributions`] seats in the region (see [`build_region_content`]), so
/// the shell knows nothing about workspaces. A region with no provider mounted passes
/// `None` and renders as an empty frame.
///
/// The body is a [`Box<dyn Component>`](heca_grid_ui::Component) — a provider's render
/// seam returns a built subtree, not a concrete widget type — which is why it is mounted
/// with [`Pane::child_boxed`] rather than `Parent::child`.
#[allow(clippy::too_many_arguments)]
fn build_sidebar_shell(
    region_w: f32,
    sidebar_h: f32,
    shell_bg: Color,
    sidebar_gap: f32,
    border_style: heca_config::appearance::BorderStyle,
    border_width: f32,
    border_radius: f32,
    content: Option<WidgetModel>,
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
        // Mounted directly: **the shell does not scroll** (F003/P011/T021).
        //
        // Scrolling is per container. Each one nests its own scroll area and scrolls its own
        // content, which is what makes two of them in a sidebar independent. A scroll viewport
        // around the whole stack would defeat that twice over: it takes the wheel for the sidebar
        // instead of the container under the cursor, and — because a viewport measures its content
        // at its natural height, which is the whole point of one — it leaves the containers
        // content-sized, so a fractional share has no height to divide and they bunch at the top.
        //
        // The shell's job is to give containers bounds. It hands them the region's height, they
        // take their shares of it, and each scrolls inside what it got.
        body = body.child_boxed(content);
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
    emit: ChromeIntentEmitter,
    color: Color,
) -> Tooltip {
    use crate::app::interaction::InteractionIntent;
    // Label from the action descriptor (catalog-owned), never re-spelled here.
    let label = catalog.label(action_name).unwrap_or(action_name);
    // One gesture: a click and a `prefix+/` pick both fire the button's action.
    let fire = {
        let emit = emit.clone();
        let action = action.clone();
        move || emit(InteractionIntent::ActivateAction(action.clone()))
    };
    let peek = fire.clone();
    // Just pick the size variant — the widget derives icon px + padding from the
    // theme font internally (`Icon` with no explicit px uses the variant-scaled font,
    // `IconButton` scales its padding). No caller-side size math.
    let button = IconButton::new(Icon::new(glyph).color(color))
        .size(WidgetSize::Small)
        .on_click(fire);
    action_tooltip(KeyHint::new(button).on_peek(peek), action_name, label, shortcuts)
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
    targets: Vec<(PeekTarget, Rectangle)>,
    occluders: Vec<Rectangle>,
    modal: bool,
}

/// **Which retained tree a peek declaration was collected from.** The app keeps several trees that
/// rebuild on independent cadences — the chrome, one per pane header, one per registered layer —
/// so a path alone does not say where to run it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum PeekSurface {
    /// The chrome tree (top bar, sidebars and everything a container contributed).
    Chrome,
    /// One pane's header tree.
    PaneHeader(PaneId),
    /// A dynamically registered layer (an exposé, a modal, a plugin panel).
    Layer(super::chrome::layers::LayerId),
}

/// **One pickable region**: the tree it lives in and its path from that tree's root.
///
/// This is the whole of what a `prefix+/` candidate carries. It is deliberately *not* an id from a
/// registry: nothing is registered, so nothing has to be un-registered when a tree is rebuilt or
/// pruned, and there is no allocator whose ranges have to be kept in step with three rebuild
/// cadences. A path is only meaningful for the tree it was read from, which is exactly the lifetime
/// a pick has — the letters go up and one is chosen in the same breath. A tree rebuilt in between
/// simply answers "nothing there" (`heca_grid_ui::fire_peek` returns `false`), which is not an
/// error.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct PeekTarget {
    pub(crate) surface: PeekSurface,
    /// Child indices from the surface's root down to the declaring widget.
    pub(crate) path: Vec<usize>,
}

impl PeekTarget {
    fn new(surface: &PeekSurface, path: Vec<usize>) -> Self {
        Self { surface: surface.clone(), path }
    }
}

/// Run what the widget behind `target` said a pick does to it. `false` when its tree is gone or was
/// rebuilt under the letters.
pub(crate) fn fire_peek(state: &crate::app_state::AppState, target: &PeekTarget) -> bool {
    let root: Option<&dyn heca_grid_ui::Component> = match &target.surface {
        PeekSurface::Chrome => state.chrome_tree.as_ref().map(|t| &t.root as &dyn heca_grid_ui::Component),
        PeekSurface::PaneHeader(pane_id) => state
            .pane_headers
            .get(pane_id)
            .map(|h| &h.root as &dyn heca_grid_ui::Component),
        PeekSurface::Layer(id) => state.layers.get(*id).map(|l| l.root()),
    };
    root.is_some_and(|root| heca_grid_ui::fire_peek(root, &target.path))
}

/// **Every peek declaration on screen, with its live bounds** — across all three kinds of retained
/// tree, with no eligibility filter. What [`paint_peek_letters`] re-reads each frame so the keycaps
/// track live layout; which targets are *eligible* was decided once, at pick time, by
/// [`active_peek_targets`].
fn collect_peek_targets(state: &crate::app_state::AppState) -> Vec<(PeekTarget, Rectangle)> {
    let mut out = Vec::new();
    if let Some(tree) = state.chrome_tree.as_ref() {
        out.extend(peeks_of(&PeekSurface::Chrome, &tree.root));
    }
    for (&pane_id, header) in state.pane_headers.iter() {
        out.extend(peeks_of(&PeekSurface::PaneHeader(pane_id), &header.root));
    }
    for layer in state.layers.visible_front_to_back() {
        out.extend(peeks_of(&PeekSurface::Layer(layer.id), layer.root()));
    }
    out
}

/// The single visibility rule (`docs/surface-compositor.md` §3): walk the layers
/// **front → back**; a target is eligible iff it lies in `viewport` and its **centre** is
/// not covered by any higher layer's occluder; a **modal** layer cuts off everything
/// beneath it. This one rule subsumes every case — off-screen, hidden behind the sidebar,
/// a zoomed/floating pane drawn over another, a modal over the whole app — and extends to
/// new surfaces for free.
fn resolve_hint_layers(
    layers: Vec<HintLayer>,
    viewport: Rectangle,
) -> Vec<(PeekTarget, Rectangle)> {
    let covers = |r: &Rectangle, x: f64, y: f64| {
        x >= r.loc.x && x < r.loc.x + r.size.w && y >= r.loc.y && y < r.loc.y + r.size.h
    };
    let vr = viewport.loc.x + viewport.size.w;
    let vb = viewport.loc.y + viewport.size.h;
    let mut kept = Vec::new();
    let mut occluders: Vec<Rectangle> = Vec::new();
    for layer in layers {
        for (target, b) in layer.targets {
            let in_view = b.loc.x < vr
                && b.loc.x + b.size.w > viewport.loc.x
                && b.loc.y < vb
                && b.loc.y + b.size.h > viewport.loc.y;
            let cx = b.loc.x + b.size.w / 2.0;
            let cy = b.loc.y + b.size.h / 2.0;
            if in_view && !occluders.iter().any(|o| covers(o, cx, cy)) {
                kept.push((target, b));
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
pub(crate) fn active_peek_targets(
    state: &crate::app_state::AppState,
) -> Vec<(PeekTarget, Rectangle)> {
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
            targets: peeks_of(&PeekSurface::Chrome, &tree.root),
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
            targets: peeks_of(&PeekSurface::PaneHeader(pane_id), &header.root),
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
        let bounds = layer.root().base().bounds;
        layers.push(HintLayer {
            band: layer.band,
            targets: peeks_of(&PeekSurface::Layer(layer.id), layer.root()),
            occluders: vec![bounds],
            modal: layer.modal,
        });
    }

    // Order the whole stack front → back by band (Modal in front … Background at the back),
    // stable so within-band order — built-ins before dynamics, and the draw order among
    // panes (zoomed/float on top) — is preserved.
    layers.sort_by_key(|l| std::cmp::Reverse(l.band.rank()));

    resolve_hint_layers(layers, viewport)
}

/// One surface's peek declarations, addressed by [`PeekTarget`].
fn peeks_of(
    surface: &PeekSurface,
    root: &dyn heca_grid_ui::Component,
) -> Vec<(PeekTarget, Rectangle)> {
    heca_grid_ui::collect_peeks(root)
        .into_iter()
        .map(|(path, bounds)| (PeekTarget::new(surface, path), bounds))
        .collect()
}

/// Paint the universal picker overlay (`prefix+/`): a glowing keycap over every pickable region,
/// driven by `InputMode::HintPick`. Bounds are re-collected from the retained trees each frame and
/// matched to the mode's candidates by [`PeekTarget`], so the keycaps track layout. Mirrors
/// [`paint_link_hints`]. No-op when the picker isn't active.
pub(crate) fn paint_peek_letters(
    state: &crate::app_state::AppState,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let crate::app_state::InputMode::HintPick { candidates } = &state.input_mode else {
        return;
    };
    // Look up the current bounds of every candidate. Collected from the SAME surfaces the picker
    // resolves over (`collect_peek_targets`) — chrome, per-pane headers and every visible layer —
    // and re-read each frame so the keycaps track live layout. Which targets are *eligible* was
    // already decided at pick time; here we only need where they are now.
    let bounds_by_target: std::collections::HashMap<PeekTarget, Rectangle> =
        collect_peek_targets(state).into_iter().collect();
    if bounds_by_target.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for (label, target) in candidates {
        let Some(bounds) = bounds_by_target.get(target) else {
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
        // A layer on its way out paints at falling opacity rather than vanishing between two
        // frames. `PaintCx` scales every command it emits, so the layer's own widgets know nothing
        // about it — a fade is something done *to* a surface.
        cx.with_opacity(layer.opacity(), |cx| layer.root().paint(cx));
    }
}

/// Peak alpha for a non-current search-match highlight; the current match is bolder.
const SEARCH_HL_ALPHA: u8 = 64;
const SEARCH_HL_CURRENT_ALPHA: u8 = 150;

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
    if state.searches.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    // Every pane that has a search draws its own highlights and bar. They are
    // independent, so a search in one pane never disturbs another's.
    for (&pane_id, search) in &state.searches {
        paint_pane_search(state, &mut cx, pane_id, search, theme, Size::new(w as f64, h as f64));
    }
}

/// Match highlights + query bar for one pane's search.
fn paint_pane_search(
    state: &crate::app_state::AppState,
    cx: &mut PaintCx,
    pane_id: PaneId,
    search: &crate::app_state::SearchState,
    theme: &GuiTheme,
    viewport: Size,
) {
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
        // A match highlight tracks terminal cells, not chrome, so it stays a painted
        // rect rather than a widget — but its corner still comes from the theme.
        cx.rect(
            rect,
            theme.colors.accent.with_alpha(alpha),
            None,
            theme.colors.control_radius(),
            None,
        );
    }

    paint_search_bar(state, cx, pane_id, search, theme, viewport);
}

/// Build the search bar's widget tree, positioned at `pane`'s bottom-right corner.
///
/// `field` is the size the query [`Input`] measured to — the tree reserves a slot of
/// exactly that size and the caller paints the retained field into it. The size is
/// measured by the layout engine, never derived from a character count.
///
/// Pure so it can be tested without a GPU or an `AppState`, which is how its
/// placement is covered.
fn search_bar_tree(
    field: Size,
    count: Option<String>,
    pane: Rectangle,
    theme: &GuiTheme,
) -> Flex {
    // The query slot, then the match position as a separate chip so it reads as
    // distinct information rather than as part of what was typed.
    let mut row = Flex::row()
        .align(Align::Center)
        .gap_spacing(Spacing::Sm)
        .child(
            Flex::row()
                .width(Length::Px(field.w as f32))
                .height(Length::Px(field.h as f32)),
        );
    if let Some(count) = count {
        row = row.child(Tag::new(count).color(theme.colors.accent));
    }

    // A box the size of the pane, offset to the pane's origin, with the bar pushed
    // into its bottom-right corner. The engine does the positioning; nothing here
    // measures text or computes a coordinate.
    Flex::row()
        .justify(Justify::End)
        .align(Align::End)
        .width(Length::Px(pane.size.w as f32))
        .height(Length::Px(pane.size.h as f32))
        .margin_left(pane.loc.x as f32)
        .margin_top(pane.loc.y as f32)
        .padding(Spacing::Sm.scale() * theme.font_size)
        .child(row)
}

/// The query field + match counter at the searched pane's bottom-right corner.
///
/// The query is a real [`Input`], so its caret, selection and the whole editing model
/// are the library's rather than reimplemented here. It is retained in [`SearchState`]
/// (a field must keep its caret across frames) and therefore cannot be moved into the
/// per-frame tree — so the tree reserves a slot and the field is painted into it, the
/// same arrangement [`CommandPalette`](heca_grid_ui::widgets::CommandPalette) uses for
/// its own query line.
fn paint_search_bar(
    state: &crate::app_state::AppState,
    cx: &mut PaintCx,
    pane_id: PaneId,
    search: &crate::app_state::SearchState,
    theme: &GuiTheme,
    viewport: Size,
) {
    let Some((_, px, py, pw, ph)) = crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .find(|(id, ..)| *id == pane_id)
    else {
        return;
    };

    let query = search.input.borrow().value_str();
    let count = (!query.is_empty()).then(|| {
        if search.matches.is_empty() {
            "no matches".to_string()
        } else {
            let pos = search.current.map(|i| i + 1).unwrap_or(0);
            format!("{}/{}", pos, search.matches.len())
        }
    });

    // Measure the field on its own first: the engine sizes it, so the bar reserves
    // exactly what it needs without anyone estimating a width from the query length.
    let field_size = {
        let mut field = search.input.borrow_mut();
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut *field, viewport);
        field.base().bounds.size
    };

    let pane = Rectangle::new(
        Point::new(px as f64, py as f64),
        Size::new(pw as f64, ph as f64),
    );
    let mut root = search_bar_tree(field_size, count, pane, theme);
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(&mut root, viewport);
    root.paint(cx);

    // Draw the retained field into the slot the tree reserved for it.
    let Some(slot) = search_field_slot(&root) else {
        return;
    };
    let mut field = search.input.borrow_mut();
    field.base_mut().bounds = slot;
    field.base_mut().font = theme.font_size;
    field.paint(cx);
}

/// Bounds of the slot [`search_bar_tree`] reserved for the query field:
/// pane box → row → first child.
fn search_field_slot(root: &Flex) -> Option<Rectangle> {
    let row = root.base().children.first()?;
    Some(row.base().children.first()?.base().bounds)
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
pub(crate) fn alpha_u8(a: f32) -> u8 {
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
    // Focus-outline visibility (config `show_focus_border`, theme fallback) — the
    // app-wide focus-ring kill switch; rings additionally show only on keyboard
    // focus (focus-visible), never on click.
    theme.colors.show_focus_border = state
        .appearance
        .effective_show_focus_border(&state.theme);
    // Overlay-panel frame style (config `overlay_border_style`, theme fallback) —
    // bracket reticle / plain edge / none for dialogs, dropdowns, context menus and
    // the command palette. Read at paint time, so it live-reloads like the rest.
    theme.colors.overlay_frame = state
        .appearance
        .effective_overlay_border_style(&state.theme);
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
    pub(crate) fn register(&mut self, item: ChromeDragItem) -> DragItemId {
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

/// **Fire a named gesture**: the closure that emits `intent` through this surface's chrome sink.
///
/// This is `realize`'s `emit(intent)` for native code — deliberately the same one line, so the two
/// authoring paths produce the same wiring:
///
/// ```ignore
/// // declarative (heca-view-realize):     native (here):
/// row.on_press(intent)                    row.on_activate(fires(mount, intent, emit))
/// row.on_peek(intent)                     KeyHint::new(row).on_peek(fires(mount, intent, emit))
/// ```
///
/// **Why a name and not a closure.** A closure is reachable from exactly one place: the gesture
/// that captured it. An [`Intent`] is an action id plus arguments, so the same declaration answers
/// a click, a menu entry, a keybinding and an RPC call, is routed by the action's own policy, and
/// passes the destructive-confirm gate — none of which a closure can be. It is also the only form
/// that crosses a plugin boundary, so a native row and a WASM row declare their gestures the same
/// way.
///
/// **`mount` is not ceremony — it is which element this is.** A widget is built inside one mounted
/// container and the gesture it declares carries that seating
/// ([`SEAT_ARG`](crate::providers::SEAT_ARG)), so the same container seated twice has two rows that
/// each answer for themselves. It is a parameter rather than something a caller may add, because a
/// rule a caller has to remember is a rule that gets forgotten: without it the host had to resolve
/// the call back to an instance by guessing (focused seat, else last focused, else the first one
/// that declares the name), and a right-sidebar row was performed by the left-sidebar copy. Every
/// call site already holds its mount — it is what `BuildCx::container_id` hands a component.
///
/// It replaced `named_press`, which handed back **one** intent for both the click and the
/// `prefix+/` pick. Those are different gestures — a click on a sidebar row means *go there and
/// leave*, a pick means *look at that one* — and serving both from one declaration is what made
/// `prefix+/` walk out of the sidebar (F004/P084/T399).
pub(crate) fn fires(
    mount: &str,
    mut intent: Intent,
    emit: &ChromeIntentEmitter,
) -> impl Fn() + 'static {
    intent.args.insert(
        crate::providers::SEAT_ARG.to_string(),
        heca_view::PropValue::Text(mount.to_string()),
    );
    let emit = emit.clone();
    move || {
        emit(crate::app::interaction::InteractionIntent::View(
            intent.clone(),
        ))
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
    /// Every navigable row's cursor-outline signal, as `(mount, nav_key, signal)` — driven from
    /// that **mount's** cursor (`container_cursor`), which is why the mount is part of the key.
    ///
    /// Generic since F003/P085/T354: this used to be two domain-keyed families (`pane_nav` by
    /// `PaneId`, `ws_nav` by `ws_idx`), which no component outside the workspace tree could join. A
    /// row now declares one identity (`Base::nav_key`) and this is the highlight half of it; the
    /// cursor highlight stays distinct from `active_pane`, as it always was.
    pub(crate) row_nav: Vec<(String, String, Signal<bool>)>,
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
    /// Each mounted container's [`KeyHint`] pick-letter signal, keyed by its **mount id**. Driven
    /// from the active dock pick (`focus_dock` with no dock named), so the letters that appear over
    /// the docks are a projection of the same candidates the keypress is resolved against
    /// (F003/P011/T020).
    pub(crate) dock_hint: Vec<(String, Signal<Option<String>>)>,
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

/// The dock pick keycap for `container` — `Some(letter)` while it is a candidate, else `None`.
///
/// Pure projection of the active dock-pick candidates, keyed by **mount id**: two placements of one
/// container are two candidates with two letters, so this cannot match on the container's kind.
fn dock_keycap(container: &str, candidates: &[(char, String)]) -> Option<String> {
    candidates
        .iter()
        .find(|(_, id)| id == container)
        .map(|(ch, _)| ch.to_string())
}

/// Find a pane's sidebar entry across all workspaces (tiled + floating).
fn find_pane_entry(tree: &WorkspaceTree, pane_id: PaneId) -> Option<&PaneEntry> {
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

fn pane_fallback_name(tree: &WorkspaceTree, pane_id: PaneId) -> &str {
    find_pane_entry(tree, pane_id)
        .map(|pane| pane.name.as_str())
        .unwrap_or_else(|| unreachable!("pane {pane_id:?} must exist in sidebar tree"))
}

/// The pane's user-set override name (from rename), if any — wins over the process
/// title. `None` while the pane tracks its process.
fn pane_custom_name(tree: &WorkspaceTree, pane_id: PaneId) -> Option<&str> {
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
    // The program catalog is this component's, not the shared context's (F003/P086/T367). Mirrored
    // here like the display flags above, so `prefix+Shift+r` reaches it; guarded on `Rc` identity,
    // which is exactly what a reload replaces.
    state
        .chrome_state
        .workspaces
        .set_programs(state.programs.clone());
    // Sidebar-nav selection: the **store owns it**. The nav handlers
    // publish into it (`publish_sidebar_selection`), and here it is projected back onto
    // the tree's positional `cursor` — which `sync_from_session` rebuilds from scratch, so
    // it cannot be the truth. That direction is also what lets an RPC or a plugin *drive*
    // the selection: whatever they write into the store moves the cursor on the next
    // frame.
    //
    // The selection lives exactly as long as a container holds the keyboard
    // (`container_cursor_visible`) — including while a menu opened on one of its rows is up, since
    // an overlay does not take chrome focus away. Selection-driven: it does NOT move the real
    // focus (`active_pane`); the container renders both, distinctly. The setter is a
    // change-guarded chokepoint, so calling it every frame is cheap.
    if state.container_cursor_visible() {
        let selection = state.chrome_state.workspaces.nav_selection();
        state.chrome_state.workspaces.tree_mut().apply_nav_selection(selection);
    } else {
        state.chrome_state.workspaces.set_nav_selection(None);
    }
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
    let next_dock_candidates = state
        .input_mode
        .dock_candidates()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if next_dock_candidates.is_empty() {
        state.chrome_state.clear_dock_pick_candidates();
    } else {
        state
            .chrome_state
            .set_dock_pick_candidates(next_dock_candidates);
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
    // Project each mount's cursor onto its rows' cursor signals — distinct from `active` above, so
    // the expanded sidebar shows both the real focus and the cursor. Keyed by `(mount, nav_key)`,
    // so two placements of one container light **different** rows (F003/P085/T354).
    for (mount, key, sig) in &retained.signals.row_nav {
        let v = state
            .chrome_state
            .container_cursor(mount)
            .get_untracked()
            .as_deref()
            == Some(key.as_str());
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
    // Project the active dock pick onto each mounted container's KeyHint (letter → container id).
    // Same mechanism as the pane/workspace/column hints above, one level up: these targets are the
    // shell's containers rather than anything inside one.
    let dock_candidates = state
        .chrome_state
        .with_dock_pick_candidates(|c| c.to_vec());
    for (container, sig) in &retained.signals.dock_hint {
        let next = dock_keycap(container, &dock_candidates);
        if sig.get_untracked() != next {
            sig.set(next);
            changed = true;
        }
    }
    for (pid, sigs) in &retained.signals.pane_info {
        let runtime = runtime_snapshot(&state.chrome_state.workspaces, *pid);
        let next = pane_info_view(
            &state.programs,
            pane_fallback_name(&state.chrome_state.workspaces.tree(), *pid),
            pane_custom_name(&state.chrome_state.workspaces.tree(), *pid),
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

    // The region body is whatever the `ChromeHost` has seated in that region — the app
    // no longer knows that the left sidebar happens to hold the workspace tree. Moving
    // the `workspaces` container to the right region (`ChromeHost::move_container`) moves
    // its UI with it, with no change here.
    //
    // The context carries only what every component needs — the frame's theme and the intent sink
    // (F003/P086/T367). A component's own model is its own to read, so the host no longer borrows
    // one component's tree here on everybody's behalf.
    let ctx = crate::providers::ChromeCtx::for_build(
        crate::host::App::new(&state.chrome_state),
        &theme,
        &emit_intent,
        &state.action_catalog,
    );

    let left_w = chrome.left_sidebar_width;
    let left_sidebar = (left_w > 0.0).then(|| {
        let content = build_region_content(
            &state.chrome_host,
            RegionId::LeftSidebar,
            &ctx,
            &mut signals,
            &mut drag_items,
        );
        build_sidebar_shell(
            left_w,
            sidebar_h,
            left_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
            content,
        )
    });
    let right_w = chrome.right_sidebar_width;
    let right_sidebar = (right_w > 0.0).then(|| {
        let content = build_region_content(
            &state.chrome_host,
            RegionId::RightSidebar,
            &ctx,
            &mut signals,
            &mut drag_items,
        );
        build_sidebar_shell(
            right_w,
            sidebar_h,
            right_sidebar_shell_background_color(state),
            sidebar_gap,
            border_style,
            border_width,
            border_radius,
            content,
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
/// sidebar intents through the app event loop.
///
/// **The tree is kept.** It used to be dropped here (`chrome_tree = None`) to stop incidental
/// widget-local state drifting from the canonical store — but that is a rebuild used as a reset,
/// and it takes everything else with it. Nothing that spans two events can survive: a scrollbar
/// grab, a scroll position, a hover. A scroll region in the sidebar was impossible for exactly this
/// reason, not for any reason to do with scrolling.
///
/// The drift it guarded against is already handled properly, twice over: the tree is rebuilt
/// whenever `chrome_signature` changes (which is what a press that alters canonical state does),
/// and `sync_chrome_signals` pushes value-state into the tree's bound signals every frame. State
/// that must not drift belongs in one of those — in the store, read through a signal — which is the
/// read-via-signals/write-via-actions rule this codebase already runs on.
/// Returns `true` when a widget consumed the press — the caller must then treat it as spoken for
/// and not also resolve it by geometry. That gate is what stops a press on the sidebar's scrollbar
/// thumb being read as a press on the pane card behind it.
pub(crate) fn chrome_dispatch_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) -> bool {
    chrome_dispatch_button_press(state, pos, heca_grid_ui::PointerButton::Left)
}

/// The same, for a **named button** — so a right-press reaches the tree instead of being read off
/// it from the outside.
///
/// A press carries its button now, which is what makes a widget able to answer a right-click at
/// all. The app still has its own menu path behind this (F004/P084/T395 is what removes it); this
/// is the door that lets a widget claim the press before any of that runs.
pub(crate) fn chrome_dispatch_button_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
    button: heca_grid_ui::PointerButton,
) -> bool {
    state
        .chrome_tree
        .as_mut()
        .map(|tree| {
            heca_grid_ui::dispatch(
                &mut tree.root,
                &Event::pointer_pressed(Point::new(pos.0 as f64, pos.1 as f64), button),
            ) == heca_grid_ui::Handled::Yes
        })
        .unwrap_or(false)
}

/// Deliver a **button release** to the chrome tree.
///
/// The other half of [`chrome_dispatch_button_press`], and not optional: the framework synthesises
/// `Click` / `RightClick` from a press **and** a release on the same widget, so a host that
/// delivers only presses produces no clicks at all — a declared context menu would never open, and
/// a widget that captured the press would never learn the gesture ended.
pub(crate) fn chrome_dispatch_button_release(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
    button: heca_grid_ui::PointerButton,
) -> bool {
    state
        .chrome_tree
        .as_mut()
        .map(|tree| {
            heca_grid_ui::dispatch(
                &mut tree.root,
                &Event::pointer_released(Point::new(pos.0 as f64, pos.1 as f64), button),
            ) == heca_grid_ui::Handled::Yes
        })
        .unwrap_or(false)
}

/// **Open the menu declared nearest the focused widget**, bubbling outwards — the keyboard
/// counterpart of an unclaimed right-click (F004/P084/T395).
///
/// Returns whether anything declared one. The action that calls this used to mean "the focused
/// pane's menu"; it now means "the focused widget's", which is what makes one binding work for a
/// pane, a column, a workspace and a plugin's own row without the host knowing any of them exist.
pub(crate) fn open_declared_menu_for_focus(state: &mut crate::app_state::AppState) -> bool {
    // **Where the keyboard is, in a chrome surface, is the focused container's cursor** — the
    // `nav_key` of the row it sits on. That is the half only the host knows, so it is the half the
    // host passes; `open_for_keyboard` owns the rest, including falling back to a genuinely focused
    // widget (a plugin's input) when the cursor names nothing.
    let cursor = {
        use heca_grid_ui::reactive::SignalGet as _;
        state
            .chrome_state
            .focused_container()
            .and_then(|mount| state.chrome_state.container_cursor(&mount).get())
    };
    let Some(tree) = state.chrome_tree.as_ref() else {
        return false;
    };
    heca_grid_ui::open_for_keyboard(&tree.root, cursor.as_deref())
}

/// Mount every menu a widget declared and asked to open since the last frame.
///
/// The other half of the sink installed at startup: the widget layer cannot reach a layer, so it
/// queues, and the host drains. One place, called once a frame, so a menu opened from a click, from
/// a key, or from a widget's own timer all arrive the same way.
pub(crate) fn drain_pending_menus(state: &mut crate::app_state::AppState) {
    let queued: Vec<_> = state.pending_menus.borrow_mut().drain(..).collect();
    for (menu, anchor) in queued {
        overlay::present_menu(state, menu, anchor);
    }
}

/// Tell every retained tree the pointer is gone: hover clears, any capture or drag ends.
///
/// One call per tree the host mounts, because each keeps its own hover — that is the point of the
/// state living on the widgets rather than in one router the host would have to own.
pub(crate) fn chrome_dispatch_cancelled(state: &mut crate::app_state::AppState, ev: &Event) {
    if let Some(tree) = state.chrome_tree.as_mut() {
        let _ = heca_grid_ui::dispatch(&mut tree.root, ev);
    }
    for header in state.pane_headers.values_mut() {
        let _ = heca_grid_ui::dispatch(&mut header.root, ev);
    }
}

/// Which container a point is inside, or `None` when it is outside every one (F003/P086/T365).
///
/// Read off the **retained tree's real laid-out bounds**, so it costs nothing to keep in step with
/// what is on screen and works for any container — a plugin's included — without the host knowing
/// anything about it.
pub(crate) fn container_at(state: &crate::app_state::AppState, pos: (f32, f32)) -> Option<String> {
    let tree = state.chrome_tree.as_ref()?;
    heca_grid_ui::nav::scope_at(&tree.root, Point::new(pos.0 as f64, pos.1 as f64))
}

/// Which **row** a point is on, by the nav key its component gave it — `None` when the point is on
/// no row (F003/P086/T365).
///
/// Read off the retained tree's real laid-out bounds, the same walk the right-click target and the
/// drag source use, so all three agree about what a point is pointing at.
pub(crate) fn nav_key_at(state: &crate::app_state::AppState, pos: (f32, f32)) -> Option<String> {
    let tree = state.chrome_tree.as_ref()?;
    heca_grid_ui::nav_key_at(&tree.root, Point::new(pos.0 as f64, pos.1 as f64))
}

/// Feed a pointer-release into the retained chrome tree, so a gesture that started there can end.
///
/// Without it a scrollbar thumb grabbed in the sidebar stays welded to the cursor — the widget is
/// still waiting for the end of a gesture nobody told it about. Deliberately not hit-tested: a
/// release ends the gesture wherever the cursor drifted to.
pub(crate) fn chrome_dispatch_release(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    if let Some(tree) = state.chrome_tree.as_mut() {
        heca_grid_ui::dispatch(&mut tree.root, &Event::pointer_released(Point::new(pos.0 as f64, pos.1 as f64), heca_grid_ui::PointerButton::Left));
    }
}

/// Feed the wheel into the retained chrome tree. Returns `true` when it was consumed — a hovered
/// scroll region took it — so the caller leaves the terminal alone.
pub(crate) fn chrome_dispatch_wheel(
    state: &mut crate::app_state::AppState,
    ev: &Event,
) -> bool {
    state
        .chrome_tree
        .as_mut()
        .map(|tree| heca_grid_ui::dispatch(&mut tree.root, ev) == heca_grid_ui::Handled::Yes)
        .unwrap_or(false)
}

/// Feed a semantic [`WidgetIntent`](heca_grid_ui::WidgetIntent) into the retained chrome tree.
///
/// One intent goes to the **root**, not to a container the host picked: every mounted container sits
/// inside a [`FocusScope`](heca_grid_ui::FocusScope), and only the focused one lets a
/// `Event::Widget` into its subtree (F003/P085/T351). So the host says *what*, and the tree decides
/// *where* — which is what keeps this working when a second dock is mounted, or the same dock is
/// placed twice.
///
/// Returns `true` when something acted on it. A `false` is a legitimate answer, not a failure: a
/// container with nothing scrollable **declines**, and the caller must not then fall through to the
/// pane — the pane is not an outer scroll area of the sidebar.
pub(crate) fn chrome_dispatch_widget(
    state: &mut crate::app_state::AppState,
    intent: heca_grid_ui::WidgetIntent,
) -> bool {
    state
        .chrome_tree
        .as_mut()
        .map(|tree| {
            heca_grid_ui::dispatch(&mut tree.root, &Event::Widget(intent)) == heca_grid_ui::Handled::Yes
        })
        .unwrap_or(false)
}

/// Feed a pointer-move into the retained chrome tree so its **hover affordances**
/// update in the real app — the `MarkerGroup` grip brightening (the column's "grab
/// me" cue) and `Row` hover. The app otherwise only dispatches `PointerPressed`, so
/// these were inert in the sidebar though they work in the showcase. Unlike
/// [`chrome_dispatch_press`] this does **not** discard the tree — hover is transient
/// and must persist across moves; the caller already requests a repaint.
pub(crate) fn chrome_dispatch_move(state: &mut crate::app_state::AppState, pos: (f32, f32)) {
    if let Some(tree) = state.chrome_tree.as_mut() {
        heca_grid_ui::dispatch(&mut tree.root, &Event::pointer_moved(Point::new(pos.0 as f64, pos.1 as f64)));
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
    for ws in &state.chrome_state.workspaces.tree().workspaces {
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

    /// A rule sits between containers, and takes no share of the height.
    ///
    /// It has to be a leaf with its natural height, or it would be handed a share of its own and the
    /// containers would each lose height to a 1px line.
    #[test]
    fn a_rule_separates_containers_without_taking_a_share() {
        use heca_grid_ui::LayoutEngine;
        use heca_core::layout::Size as CoreSize;

        let body = || -> WidgetModel { Box::new(Flex::column().height(Length::Px(40.0))) };
        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        // Two containers with a rule between them, as `build_region_content` assembles them.
        stack.base_mut().children.push(with_share(body(), 1.0));
        stack = stack.child(Separator::horizontal());
        stack.base_mut().children.push(with_share(body(), 1.0));

        let mut root = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut root, CoreSize::new(300.0, 600.0));

        let kids = &root.base().children[0].base().children;
        assert_eq!(kids.len(), 3, "container, rule, container");
        assert_eq!(kids[1].base().style.layout.flex_grow, 0.0, "the rule takes no share");
        assert!(
            kids[1].base().bounds.size.h < 10.0,
            "the rule keeps its own thin height: {:?}",
            kids[1].base().bounds.size.h,
        );
        // Within a pixel: an odd leftover after the rule and the gaps has to land somewhere, so
        // equal shares of an odd number of pixels differ by one. Measured 292 / 1 / 291.
        assert!(
            (kids[0].base().bounds.size.h - kids[2].base().bounds.size.h).abs() <= 1.0,
            "and the containers still share equally around it: {:?}",
            kids.iter().map(|k| k.base().bounds.size.h).collect::<Vec<_>>(),
        );
    }

    /// Shares divide the region even when the content is taller than it.
    ///
    /// This is the part that looked done and was not. `flex_grow` distributes only *positive* free
    /// space, and a container's content is routinely taller than a sidebar — so two containers
    /// measured 1214px each inside a 600px body, overflowed the frame, and divided nothing. The fix
    /// is a zero base size plus permission to shrink (CSS `flex: 1 1 0`; this vocabulary has no
    /// `flex_basis`), which makes the free space the whole region.
    ///
    /// The numbers are asserted rather than the flags, because the flags were "right" while the
    /// layout was wrong.
    #[test]
    fn shares_divide_the_region_even_with_content_taller_than_it() {
        use heca_grid_ui::LayoutEngine;
        use heca_core::layout::Size as CoreSize;

        // A body far shorter than the content it holds, as a sidebar is.
        let tall = || -> WidgetModel {
            let mut inner = Flex::column();
            for _ in 0..20 {
                inner = inner.child(Flex::column().height(Length::Px(60.0)));
            }
            Box::new(heca_grid_ui::ScrollRegion::new().child(inner))
        };

        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        for _ in 0..2 {
            stack.base_mut().children.push(with_share(tall(), 1.0));
        }
        let mut body = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

        let stack = &body.base().children[0];
        assert!(
            stack.base().bounds.size.h <= 600.0,
            "the stack fits the region instead of overflowing it: {:?}",
            stack.base().bounds.size.h,
        );
        let heights: Vec<f64> = stack
            .base()
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        assert_eq!(heights.len(), 2);
        assert!(
            (heights[0] - heights[1]).abs() < 1.0,
            "equal shares are equal: {heights:?}",
        );
        assert!(
            heights[0] > 250.0 && heights[0] < 300.0,
            "each takes about half the 600px region, less the gap: {heights:?}",
        );
    }

    /// A container's declared share reaches the widget, and saying nothing means an equal share.
    ///
    /// The share is a flex grow factor, so it is the region's **main axis** — height in a sidebar,
    /// width in a bar — and one number covers both. What this pins is the wiring: the number on the
    /// contribution has to land on the body that gets laid out, and it would be silently dropped if
    /// anything rebuilt or rewrapped the body afterwards (F003/P011/T021).
    ///
    /// Whether two containers then *look* right side by side is layout, and the user judges that in
    /// the app — a test asserting taffy divides 200px into 100 and 100 would be testing taffy.
    #[test]
    fn a_containers_declared_share_reaches_its_body() {
        let body = || -> WidgetModel { Box::new(Flex::column()) };

        // The trait default, which is what a provider that says nothing gets.
        assert_eq!(
            crate::providers::Provider::grow(&crate::providers::WorkspacesContainerProvider::new()),
            1.0,
            "saying nothing means one equal share",
        );

        assert_eq!(with_share(body(), 1.0).base().style.layout.flex_grow, 1.0);
        assert_eq!(
            with_share(body(), 2.0).base().style.layout.flex_grow,
            2.0,
            "twice the share of a 1.0 beside it",
        );
        assert_eq!(
            with_share(body(), 0.0).base().style.layout.flex_grow,
            0.0,
            "content-sized: no share of the leftover",
        );
    }

    /// Wrapping a container in the focus ring + pick keycap must not disturb the shares.
    ///
    /// The wrappers are transparent, so the share has to be applied to the **outermost** node: on the
    /// body it would leave the wrapper content-sized, and two containers would divide nothing —
    /// exactly the failure F003/P011/T021 measured (1214px each inside a 600px body). Pixels are
    /// asserted, not flags, because the flags were right while the layout was broken.
    #[test]
    fn the_focus_wrappers_do_not_disturb_the_shares() {
        use heca_core::layout::Size as CoreSize;
        use heca_grid_ui::LayoutEngine;
        use heca_grid_ui::widgets::{FocusScope, KeyHint};

        // A body far taller than the region it is given, as a real dock is.
        let tall = || -> WidgetModel {
            let mut inner = Flex::column();
            for _ in 0..20 {
                inner = inner.child(Flex::column().height(Length::Px(60.0)));
            }
            Box::new(heca_grid_ui::ScrollRegion::new().grow(1.0).child(inner))
        };
        // The wrapping `focus_and_pick` applies, without needing a render context for it.
        let wrapped = || -> WidgetModel {
            let mut body = tall();
            pass_box_down(body.as_mut());
            let mut picked = KeyHint::new_boxed(body);
            pass_box_down(&mut picked);
            Box::new(FocusScope::new(picked).focus(signal(true)))
        };

        let mut stack = Flex::column().gap(8.0).grow(1.0);
        {
            let layout = &mut stack.base_mut().style.layout;
            layout.min_height = Some(Length::Px(0.0));
            layout.flex_shrink = Some(1.0);
        }
        for _ in 0..2 {
            stack.base_mut().children.push(with_share(wrapped(), 1.0));
        }
        let mut body = Flex::column().height(Length::Px(600.0)).child(stack);
        LayoutEngine::new().compute(&mut body, CoreSize::new(300.0, 600.0));

        let stack = &body.base().children[0];
        let heights: Vec<f64> = stack
            .base()
            .children
            .iter()
            .map(|c| c.base().bounds.size.h)
            .collect();
        assert_eq!(heights.len(), 2);
        assert!(
            (heights[0] - heights[1]).abs() < 1.0,
            "equal shares are still equal through the wrappers: {heights:?}",
        );
        assert!(
            heights[0] > 250.0 && heights[0] < 300.0,
            "each still takes about half the 600px region, less the gap: {heights:?}",
        );
        // And the wrappers pass the height straight down — a ring around a box half the size of the
        // box would be worse than no ring.
        let ring = &stack.base().children[0];
        let hinted = &ring.base().children[0];
        let dock = &hinted.base().children[0];
        assert_eq!(
            (ring.base().bounds.size.h, dock.base().bounds.size.h),
            (heights[0], heights[0]),
            "ring, keycap wrapper and dock all measure the same box",
        );
    }

    /// A container that asked for **no** share keeps its content height through the wrappers.
    ///
    /// `grow = 0.0` means "size me to my content", and a zero base size inside a content-sized parent
    /// would collapse the whole thing to nothing — which is why the pass-down is conditional.
    #[test]
    fn a_content_sized_container_keeps_its_height_through_the_wrappers() {
        use heca_core::layout::Size as CoreSize;
        use heca_grid_ui::LayoutEngine;
        use heca_grid_ui::widgets::{FocusScope, KeyHint};

        let body: WidgetModel = Box::new(Flex::column().height(Length::Px(120.0)));
        // What `focus_and_pick` does with `share = 0.0`: wrap, and touch no layout.
        let wrapped: WidgetModel =
            Box::new(FocusScope::new(KeyHint::new_boxed(body)).focus(signal(false)));
        let mut region = Flex::column()
            .height(Length::Px(600.0))
            .child_boxed(with_share(wrapped, 0.0));
        LayoutEngine::new().compute(&mut region, CoreSize::new(300.0, 600.0));

        let ring = &region.base().children[0];
        assert_eq!(
            ring.base().bounds.size.h, 120.0,
            "content-sized means the content's height, not zero and not the region's",
        );
    }

    #[test]
    fn dock_keycap_projects_the_pick_by_mount_id() {
        let candidates = [
            ('a', "workspaces".to_string()),
            ('b', "workspaces.2".to_string()),
        ];
        assert_eq!(dock_keycap("workspaces", &candidates), Some("a".to_string()));
        assert_eq!(
            dock_keycap("workspaces.2", &candidates),
            Some("b".to_string()),
            "a second placement of the same container is its own candidate",
        );
        assert_eq!(dock_keycap("docker", &candidates), None);
        assert_eq!(dock_keycap("workspaces", &[]), None, "no pick, no keycap");
    }

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

    /// A one-workspace / one-column / one-pane tree — the smallest projection that still
    /// has every level.
    fn one_pane_tree(pane_id: u64) -> crate::providers::workspaces::WorkspaceTree {
        use crate::app_state::SidebarItemState;
        use crate::providers::workspaces::{ColumnEntry, PaneEntry, WorkspaceTree, WorkspaceEntry};

        let mut tree = WorkspaceTree::new();
        tree.workspaces.push(WorkspaceEntry {
            ws_idx: 0,
            name: "ws1".into(),
            custom_name: None,
            collapsed: false,
            state: SidebarItemState::Active,
            columns: vec![ColumnEntry {
                col_idx: 0,
                collapsed: false,
                panes: vec![PaneEntry {
                    pane_id: heca_core::layout::PaneId(pane_id),
                    name: "pane1".into(),
                    custom_name: None,
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });
        tree
    }

    /// Build a region's body the way the app does since the provider cutover: through the
    /// `ChromeHost`, by asking the mounted provider for its contribution and calling its
    /// `build` seam. These tests deliberately do **not** reach into the container's own
    /// builder — that would test a path the app no longer takes.
    fn region_body(
        tree: &crate::providers::workspaces::WorkspaceTree,
        theme: &GuiTheme,
        chrome: &SharedChromeState,
        signals: &mut super::ChromeSignals,
        drag: &mut super::DragItemRegistry,
    ) -> Option<super::WidgetModel> {
        let mut host = super::ChromeHost::new(chrome.events());
        host.register(Box::new(crate::providers::WorkspacesContainerProvider::new()));
        let emit: super::ChromeIntentEmitter = Rc::new(|_| {});
        // The component reads its model from its own state, so the fixture puts it there rather
        // than handing it to the context (F003/P086/T367).
        *chrome.workspaces.tree_mut() = tree.clone();
        let catalog = crate::actions::ActionCatalog::with_builtins();
        let ctx = crate::providers::ChromeCtx::for_build(
            crate::host::App::new(chrome),
            theme,
            &emit,
            &catalog,
        );
        super::build_region_content(&host, super::RegionId::LeftSidebar, &ctx, signals, drag)
    }

    #[test]
    fn sidebar_shell_hosts_the_provider_mounted_container() {
        use heca_grid_ui::Component;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        chrome
            .workspaces
            .set_active_pane(Some(heca_core::layout::PaneId(1)));
        // The shell wraps a bracketed Pane that holds just the container the region's
        // provider built (the collapse toggle moved to the top bar, sidebar-fu-14); the
        // container hosts a dock per workspace (so the tree's text is visible).
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        assert!(
            content.is_some(),
            "the workspaces provider is mounted in the left region, so it must contribute a body",
        );
        let shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
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
            "the shell Pane holds just the mounted container's body (the collapse toggle \
             moved to the top bar, sidebar-fu-14)",
        );
        let container = &pane.base().children[0];
        assert!(
            !container.base().children.is_empty(),
            "the WorkspacesContainer must host a dock per workspace",
        );
    }

    #[test]
    fn bordered_sidebar_paints_border_color_at_configured_width() {
        // Regression: with `sidebar_border_style = "bordered"`, the shell must paint
        // a visible frame in the (global) border color at the configured
        // `sidebar_border_width`. Previously the bordered sidebar drew nothing /
        // ignored the color because its width was theme-locked.
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);

        // A distinct border color so we can prove it reached the painted frame.
        let mut theme = GuiTheme::default();
        theme.colors.border = Color::new(0x40, 0xe0, 0xff, 0xff);
        let border_w = 4.0_f32;

        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bordered,
            border_w,
            12.0,
            content,
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

    /// The focused dock is **outlined**, and an unfocused one is not (F003/P011/T020).
    ///
    /// Focus with nothing to show for it tells the user nothing, so this asserts the drawn ring
    /// rather than a flag — and asserts that it follows the store *without a rebuild*, because the
    /// host flips a signal rather than rebuilding the chrome tree.
    ///
    /// Whether the ring reads well at that width and colour is a visual judgement, and the user makes
    /// it in the app. What is pinned here is that it is drawn at all, on the right dock.
    #[test]
    fn the_focused_dock_is_outlined_and_follows_the_store() {
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
        );

        let ring = theme.colors.effective_focus_ring();
        let outlined = |shell: &mut Flex| {
            super::paint_chrome_root(shell, 280.0, 600.0, &theme)
                .iter()
                .any(|c| match c {
                    DrawCommand::Rect(r) => r.border.is_some_and(|b| b.color == ring),
                    _ => false,
                })
        };

        assert!(
            !outlined(&mut shell),
            "no dock has focus yet, so nothing is outlined",
        );
        chrome.set_focused_container(Some("workspaces".into()));
        assert!(
            outlined(&mut shell),
            "the focused dock is outlined in the theme's focus colour — same tree, one signal",
        );
        chrome.set_focused_container(Some("something-else".into()));
        assert!(
            !outlined(&mut shell),
            "and focus elsewhere takes the outline away",
        );
    }

    /// A dock's pick letter reaches the tree: the host registers the signal, and setting it stamps a
    /// keycap over that dock. The letters themselves come from the pick candidates
    /// (`dock_keycap`), projected each frame by `sync_chrome_signals`.
    #[test]
    fn a_docks_pick_letter_is_stamped_over_it() {
        use heca_grid_ui::DrawCommand;

        let tree = one_pane_tree(1);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut signals = super::ChromeSignals::default();
        let content = region_body(
            &tree,
            &theme,
            &chrome,
            &mut signals,
            &mut super::DragItemRegistry::default(),
        );
        let mut shell = super::build_sidebar_shell(
            280.0,
            600.0,
            theme.colors.background,
            8.0,
            heca_config::appearance::BorderStyle::Bracketed,
            1.0,
            12.0,
            content,
        );

        assert_eq!(
            signals.dock_hint.len(),
            1,
            "one mounted container ⇒ one dock-letter signal, keyed by its mount id",
        );
        let (container, hint) = &signals.dock_hint[0];
        assert_eq!(container, "workspaces");

        let letter_drawn = |shell: &mut Flex| {
            super::paint_chrome_root(shell, 280.0, 600.0, &theme)
                .iter()
                .any(|c| matches!(c, DrawCommand::Text(t) if t.text == "a"))
        };
        assert!(!letter_drawn(&mut shell), "no pick open, no keycap");
        hint.set(Some("a".to_string()));
        assert!(
            letter_drawn(&mut shell),
            "the dock's letter is stamped over it while the pick is open",
        );
    }

    #[test]
    fn drag_registry_captures_pane_column_and_workspace() {
        let tree = one_pane_tree(7);
        let theme = GuiTheme::default();
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut drag = super::DragItemRegistry::default();
        // Drag items are registered while the region's mounted container builds its body,
        // so go through the host path and inspect what landed in the registry.
        let _ = region_body(
            &tree,
            &theme,
            &chrome,
            &mut super::ChromeSignals::default(),
            &mut drag,
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

#[cfg(test)]
mod search_bar_tests {
    use super::*;

    /// A pane deliberately far from the window origin: a bar placed relative to the
    /// window instead of the pane lands nowhere near this rect.
    fn pane() -> Rectangle {
        Rectangle::new(Point::new(600.0, 100.0), Size::new(640.0, 1400.0))
    }

    fn viewport() -> Size {
        Size::new(1900.0, 1600.0)
    }

    /// Lay the bar out for a query field of `field` size and return the bar's own
    /// bounds — the row holding the field slot and the counter.
    fn laid_out_bar_bounds(field: Size, count: Option<String>) -> Rectangle {
        let theme = GuiTheme::default();
        let mut root = search_bar_tree(field, count, pane(), &theme);
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut root, viewport());
        // root (pane box) -> row
        root.base().children[0].base().bounds
    }

    /// A representative measured field size.
    fn field(w: f64) -> Size {
        Size::new(w, 22.0)
    }

    /// **Regression guard.** The bar is positioned purely by margins on its box, so
    /// it must land inside the pane it belongs to. This caught the engine silently
    /// dropping a root's margin, which drew the bar over the *sidebar* — a whole pane
    /// away from the terminal it described.
    #[test]
    fn the_search_bar_lands_inside_its_pane() {
        let b = laid_out_bar_bounds(field(120.0), Some("29/36".into()));
        let p = pane();
        assert!(
            b.loc.x >= p.loc.x
                && b.loc.y >= p.loc.y
                && b.loc.x + b.size.w <= p.loc.x + p.size.w
                && b.loc.y + b.size.h <= p.loc.y + p.size.h,
            "bar at {:?} escaped its pane {p:?}",
            b
        );
    }

    /// It is anchored to the bottom-right specifically, not merely somewhere inside.
    #[test]
    fn the_search_bar_hugs_the_bottom_right_corner() {
        let b = laid_out_bar_bounds(field(120.0), Some("29/36".into()));
        let p = pane();
        let right_gap = (p.loc.x + p.size.w) - (b.loc.x + b.size.w);
        let bottom_gap = (p.loc.y + p.size.h) - (b.loc.y + b.size.h);
        assert!(
            right_gap < p.size.w / 2.0 && bottom_gap < p.size.h / 2.0,
            "expected bottom-right; gaps were right={right_gap} bottom={bottom_gap}"
        );
    }

    /// The bar is sized by the field the engine measured, not by arithmetic — the old
    /// version clamped width to a hand-picked 120..480 range computed from a hardcoded
    /// glyph advance ratio.
    #[test]
    fn a_wider_field_makes_a_wider_bar() {
        let narrow = laid_out_bar_bounds(field(80.0), None).size.w;
        let wide = laid_out_bar_bounds(field(300.0), None).size.w;
        assert!(wide > narrow, "wide bar {wide} should exceed narrow {narrow}");
    }

    /// **The field must land in the slot the tree reserved**, or the caret and the
    /// text would draw somewhere other than the bar the user sees.
    #[test]
    fn the_reserved_slot_matches_the_field_size() {
        let theme = GuiTheme::default();
        let f = field(140.0);
        let mut root = search_bar_tree(f, Some("1/3".into()), pane(), &theme);
        LayoutEngine::new()
            .base_font(theme.font_size)
            .compute(&mut root, viewport());
        let slot = search_field_slot(&root).expect("the tree always reserves a slot");
        assert_eq!((slot.size.w, slot.size.h), (f.w, f.h));
    }
}
