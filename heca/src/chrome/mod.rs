//! Chrome metrics and central application constants.
//!
//! UI dimensions, timing defaults, layout proportions, and render parameters
//! that were previously scattered as magic numbers across the codebase.

mod state;
mod events;
pub use events::{ChromeEvent, ChromeEventBus, ChromeRegion, ChromeSubscription};
pub use state::{SharedChromeState, WorkspacesContainerState};

use heca_core::layout::types::{Point, Rectangle, Size};
use heca_core::layout::ColumnWidth;
use std::time::Duration;

// ── Chrome metrics ──

/// Default tab bar height in logical pixels.
pub const DEFAULT_TAB_BAR_HEIGHT: f32 = 32.0;
/// Default status bar height in logical pixels.
pub const DEFAULT_STATUS_BAR_HEIGHT: f32 = 24.0;
/// Default collapsed sidebar width in logical pixels.
pub const DEFAULT_COLLAPSED_SIDEBAR_WIDTH: f32 = 40.0;
/// Default expanded sidebar width in logical pixels.
pub const DEFAULT_SIDEBAR_WIDTH: f32 = 240.0;
/// Minimum sidebar width to be considered expanded (for rendering decisions).
pub const SIDEBAR_EXPANDED_THRESHOLD: f32 = 80.0;

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
        Rectangle::new(Point::new(x as f64, y as f64), Size::new(w as f64, h as f64))
    }
}

// ── Grid-UI chrome scene builder ──────────────────────────────────────────────

use crate::sidebar::{SidebarColEntry, SidebarPaneEntry, SidebarTree};
use heca_core::layout::PaneId;
use heca_core::runtime::{PaneRuntime, ProcessStatus};
use heca_config::programs::{ProgramIcon, ProgramsConfig};
use heca_grid_ui::builders::{DragExt, LayoutExt, Parent, StyleExt};
use heca_grid_ui::drag::{DragItemId, DragPhase, DragSurfaceId};
use heca_grid_ui::style::{Align, Justify, Length};
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::widgets::{
    ActiveMarker, Badge, DockFrame, Flex, Glyph, HintPlacement, Icon, IconButton,
    KeyHint, Label, MarkerGroup, Pane, Row, StatusDot, Surface, Tag, Tooltip, TooltipSide,
    Visibility,
};
use heca_grid_ui::reactive::{signal, Signal, SignalGet, SignalUpdate};
use heca_grid_ui::{Color, Component, Event, LayoutEngine, PaintCx, Scene};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PaneInfoView {
    icon: Glyph,
    title: String,
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
    runtime: Option<&PaneRuntime>,
) -> PaneInfoView {
    let raw = runtime
        .and_then(|pane| pane.program.as_deref())
        .filter(|raw| !raw.is_empty())
        .unwrap_or(fallback_name);
    let program = programs.resolve(raw);
    let git = runtime.and_then(|pane| pane.git.as_ref());
    PaneInfoView {
        icon: program_glyph(program.icon),
        title: program.name.into_owned(),
        status: runtime
            .map(|pane| pane.status.clone())
            .unwrap_or(ProcessStatus::Idle),
        git_branch: git
            .map(|info| info.branch.clone().unwrap_or_else(|| "detached".to_string())),
        git_added: git.and_then(|info| (info.added > 0).then(|| format!("+{}", info.added))),
        git_modified: git
            .and_then(|info| (info.modified > 0).then(|| format!("~{}", info.modified))),
        git_deleted: git
            .and_then(|info| (info.deleted > 0).then(|| format!("-{}", info.deleted))),
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
pub(crate) fn build_pane_info_bar(
    programs: &ProgramsConfig,
    fallback_name: &str,
    runtime: Option<&PaneRuntime>,
    segments: &[heca_config::appearance::PaneSegment],
    theme: &GuiTheme,
    max_width: f32,
    font: f32,
) -> Option<Tag> {
    use heca_config::appearance::PaneSegment;

    let view = pane_info_view(programs, fallback_name, runtime);
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
            PaneSegment::AppName => (view.icon, view.title.clone()),
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
        let leading = Icon::new(glyph).color(theme.foreground);
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

/// Glyph size of a header action button at the bar `font` — a touch larger than the
/// body font so the icons are clearly legible/clickable.
fn header_icon_size(font: f32) -> f32 {
    (font * 1.25).max(15.0)
}

/// Square cell size of a header action button — the icon plus snug padding (keeps
/// the inter-button spacing small while the buttons stay comfortably tappable).
fn header_button_cell(font: f32) -> f32 {
    header_icon_size(font) + 6.0
}

/// Total width the action-button cluster occupies (0 when there are no actions).
pub(crate) fn header_buttons_width(actions: &[heca_config::appearance::PaneAction], font: f32) -> f32 {
    if actions.is_empty() {
        return 0.0;
    }
    let n = actions.len() as f32;
    n * header_button_cell(font) + (n - 1.0).max(0.0) * HEADER_BUTTON_GAP
}

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

/// Tooltip keybind hints for the pane-action buttons, formatted from the user's
/// config (so they track rebinds) with the leader shown as the symbolized prefix
/// combo. Empty string ⇒ unbound (tooltip then shows the label only). Built at
/// config load/reload (`PaneActionHints::from_keys`), stored on `AppState`.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct PaneActionHints {
    pub(crate) split: String,
    pub(crate) move_left: String,
    pub(crate) move_right: String,
    pub(crate) close: String,
    pub(crate) zoom: String,
    pub(crate) float: String,
}

impl PaneActionHints {
    /// Resolve each button's display keybind from the config bindings.
    pub(crate) fn from_keys(
        keys: &heca_config::keys::KeysConfig,
        prefix: &crate::keymap::KeyCombo,
    ) -> Self {
        let hint = |action: &str| {
            keys.bindings
                .get(action)
                .and_then(|b| b.keys().first().copied())
                .map(|s| format_binding(s, prefix))
                .unwrap_or_default()
        };
        Self {
            split: hint("split_vertical"),
            move_left: hint("move_pane_left"),
            move_right: hint("move_pane_right"),
            close: hint("close"),
            zoom: hint("zoom_column"),
            float: hint("float"),
        }
    }

    fn for_action(&self, action: heca_config::appearance::PaneAction) -> &str {
        use heca_config::appearance::PaneAction;
        match action {
            PaneAction::Split => &self.split,
            PaneAction::MoveLeft => &self.move_left,
            PaneAction::MoveRight => &self.move_right,
            PaneAction::Close => &self.close,
            PaneAction::Zoom => &self.zoom,
            PaneAction::Float => &self.float,
        }
    }
}

/// macOS-style symbols for a [`KeyCombo`] (the prefix): `⌃⌥⇧⌘` + the key.
fn symbolize_combo(c: &crate::keymap::KeyCombo) -> String {
    let mut s = String::new();
    if c.ctrl {
        s.push('⌃');
    }
    if c.alt {
        s.push('⌥');
    }
    if c.shift {
        s.push('⇧');
    }
    if c.super_ {
        s.push('⌘');
    }
    s.push_str(&symbolize_key(&c.key));
    s
}

/// A single key token, prettified (named keys → glyphs; letters upper-cased).
fn symbolize_key(k: &str) -> String {
    match k.to_ascii_lowercase().as_str() {
        "enter" | "return" => "↩".into(),
        "space" => "␣".into(),
        "arrowleft" | "left" => "←".into(),
        "arrowright" | "right" => "→".into(),
        "arrowup" | "up" => "↑".into(),
        "arrowdown" | "down" => "↓".into(),
        "escape" | "esc" => "⎋".into(),
        "tab" => "⇥".into(),
        _ if k.chars().count() == 1 => k.to_uppercase(),
        _ => k.to_string(),
    }
}

/// Format a config binding string (`"prefix+v"`, `"Alt+Enter"`) for a tooltip: the
/// leading `prefix` token becomes the symbolized prefix combo, modifiers become
/// symbols. Empty input ⇒ empty output.
fn format_binding(s: &str, prefix: &crate::keymap::KeyCombo) -> String {
    let mut out = String::new();
    for tok in s.split('+').map(str::trim).filter(|t| !t.is_empty()) {
        match tok.to_ascii_lowercase().as_str() {
            "prefix" => {
                out.push_str(&symbolize_combo(prefix));
                out.push(' ');
            }
            "ctrl" | "control" => out.push('⌃'),
            "shift" => out.push('⇧'),
            "alt" | "option" => out.push('⌥'),
            "super" | "cmd" | "command" | "meta" => out.push('⌘'),
            _ => out.push_str(&symbolize_key(tok)),
        }
    }
    out.trim().to_string()
}

/// Map a configured [`PaneAction`] to its `(icon, WM action, label, needs_focus)`.
///
/// `needs_focus` is `true` for **active-targeted** unit actions (`ZoomColumn`/
/// `Float`) — the button must focus its owning pane before dispatching so the
/// action lands on the clicked pane, not whatever happened to be active. The other
/// buttons carry the pane/column in the action itself, so they don't steal focus.
fn pane_action_spec(
    action: heca_config::appearance::PaneAction,
    pane_id: PaneId,
    ws_idx: usize,
    col_idx: usize,
) -> (Glyph, crate::input::WmAction, &'static str, bool) {
    use crate::input::WmAction;
    use heca_config::appearance::PaneAction;
    match action {
        PaneAction::Split => (
            Glyph::SquareSplitVertical,
            WmAction::AddPaneToColumn { ws_idx, col_idx },
            "Add pane",
            false,
        ),
        PaneAction::MoveLeft => (
            Glyph::ArrowLineLeft,
            WmAction::MovePaneLeft { pane_id: Some(pane_id) },
            "Move left",
            false,
        ),
        PaneAction::MoveRight => (
            Glyph::ArrowLineRight,
            WmAction::MovePaneRight { pane_id: Some(pane_id) },
            "Move right",
            false,
        ),
        PaneAction::Close => (
            Glyph::XSquare,
            WmAction::ClosePaneById { pane_id },
            "Close",
            false,
        ),
        PaneAction::Zoom => (Glyph::FrameCorners, WmAction::ZoomColumn, "Zoom", true),
        PaneAction::Float => (Glyph::Cards, WmAction::Float, "Float", true),
    }
}

/// What a pane header *renders* — the projection inputs shared by the rebuild key
/// and the tree builder (groups args so neither fn explodes).
pub(crate) struct PaneHeaderContent<'a> {
    pub(crate) programs: &'a ProgramsConfig,
    pub(crate) fallback_name: &'a str,
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
    pub(crate) hints: &'a PaneActionHints,
}

/// Whether a pane-action button stays visible when its pane is **floating**, driven
/// by the shared [`action_policy`](crate::app::interaction) classification (split /
/// zoom / move are tiled-only ⇒ hidden; float / close are focused-pane-local ⇒ kept).
fn pane_action_visible_when_floating(action: heca_config::appearance::PaneAction) -> bool {
    // Policy ignores the concrete ids, so dummy ids are fine here.
    let (_, wm, _, _) = pane_action_spec(action, PaneId(0), 0, 0);
    crate::app::interaction::action_allowed_when_floating(&wm)
}

/// A content key identifying everything the header *renders* — used to decide when
/// the retained tree must be rebuilt (vs. just re-laid-out). Cheap per-frame string
/// build (≤20 panes); avoids deriving `Hash` on the projection enums.
pub(crate) fn pane_header_key(content: &PaneHeaderContent, font: f32, avail_w: f32) -> String {
    let view = pane_info_view(content.programs, content.fallback_name, content.runtime);
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
        .map(|&a| content.hints.for_action(a))
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
    // When the pane is floating, only buttons whose action is allowed in the
    // floating domain stay (split/zoom/move are tiled-only → dropped; float/close
    // remain) — driven by the shared action policy, not a hardcoded list.
    let visible_actions: Vec<heca_config::appearance::PaneAction> = content
        .actions
        .iter()
        .copied()
        .filter(|&a| !content.floating || pane_action_visible_when_floating(a))
        .collect();
    let buttons_w = header_buttons_width(&visible_actions, font);
    // The bar yields width to the button cluster first.
    let bar_max = (avail_w - buttons_w - if buttons_w > 0.0 { HEADER_BUTTON_GAP } else { 0.0 }).max(0.0);
    let bar = build_pane_info_bar(
        content.programs,
        content.fallback_name,
        content.runtime,
        content.segments,
        theme,
        bar_max,
        font,
    );

    let buttons = if visible_actions.is_empty() {
        None
    } else {
        let cell = header_button_cell(font);
        let mut row = Flex::row().align(Align::Center).gap(HEADER_BUTTON_GAP);
        for &action in &visible_actions {
            let (glyph, wm_action, label, needs_focus) =
                pane_action_spec(action, ctx.pane_id, ctx.ws_idx, ctx.col_idx);
            // Held-on status: zoom is active while the column is zoomed/full-width,
            // float while the pane is floating (so the icon reads as toggled-on).
            let is_active = match action {
                heca_config::appearance::PaneAction::Zoom => content.zoomed,
                heca_config::appearance::PaneAction::Float => content.floating,
                _ => false,
            };
            // Don't focus-first when floating: the floating pane is already the active
            // one, and a `FocusPane` from MouseContent is blocked in the floating domain
            // (logs a spurious "blocked intent"). Unfloat/close act on it directly.
            let needs_focus = needs_focus && !content.floating;
            // Tooltip = label + the user's configured keybind (prefix symbolized).
            let key = content.hints.for_action(action);
            let tip = if key.is_empty() {
                label.to_string()
            } else {
                format!("{label}  {key}")
            };
            // Close is destructive → its glyph + hover/press use the theme danger
            // hue; the rest use the foreground glyph with an accent hover. The danger
            // glyph is softened toward the header surface so the red reads as a cue,
            // not an alarm (full-intensity danger was too vibrant).
            let is_close = matches!(action, heca_config::appearance::PaneAction::Close);
            let (icon_color, tone) = if is_close {
                (theme.danger.lerp(theme.surface, 0.25), theme.danger)
            } else {
                (theme.foreground, theme.accent)
            };
            let proxy = ctx.event_proxy.clone();
            let pane_id = ctx.pane_id;
            let button = IconButton::new(Icon::new(glyph).color(icon_color).size(header_icon_size(font)))
                .cell(cell)
                .tone(tone)
                .active(is_active)
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
            row = row.child(Tooltip::new(button, tip).side(TooltipSide::Bottom));
        }
        Some(row)
    };

    let root = Flex::row().width(Length::Px(avail_w)).align(Align::Center);
    let root = match (bar, buttons) {
        (Some(bar), Some(buttons)) => root.justify(Justify::SpaceBetween).child(bar).child(buttons),
        (Some(bar), None) => root.justify(Justify::Start).child(bar),
        (None, Some(buttons)) => root.justify(Justify::End).child(buttons),
        (None, None) => return None,
    };
    Some(root)
}

/// Build/position the retained per-pane info-bar headers for every visible pane.
/// Runs at the **top** of `render_frame` (before the `scene_view` borrow of
/// `state.compositor`) so it can mutate `state.pane_headers`; render then paints
/// them read-only and `mouse.rs` dispatches pointer events into them. Rebuilds a
/// pane's tree only when its content key changes; re-lays-out + repositions every
/// frame; prunes panes that disappeared.
pub(crate) fn sync_pane_headers(state: &mut crate::app_state::AppState) {
    let segments = state.appearance.pane_title_segments.clone();
    let actions = state.appearance.pane_title_actions.clone();
    if segments.is_empty() && actions.is_empty() {
        state.pane_headers.clear();
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
        let (name, runtime) = state
            .session
            .active_workspace()
            .and_then(|ws| ws.find_pane(pane_id))
            .map(|p| (p.title.clone(), Some(p.runtime.clone())))
            .unwrap_or_else(|| (String::new(), None));
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
            runtime: input.runtime.as_ref(),
            segments: &segments,
            actions: &actions,
            ws_idx: input.ws_idx,
            col_idx: input.col_idx,
            zoomed: input.zoomed,
            floating: input.floating,
            hints: &state.pane_action_hints,
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
            LayoutEngine::new()
                .base_font(font)
                .compute(&mut header.root, Size::new(input.avail_w as f64, band as f64));
            let bar_h = header.root.base().bounds.size.h as f32;
            let bar_y = input.y + f64::from(((band - bar_h) / 2.0).max(0.0)) as f32;
            translate_tree(&mut header.root, (input.x + HEADER_MARGIN) as f64, bar_y as f64);
        }
    }
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
    let consumed = header.root.event(&Event::PointerPressed { pos: point }) == heca_grid_ui::Handled::Yes;
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
        (Self { base, request, seen: 0 }, request)
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
) -> RepaintWatch {
    let active = active_pane == Some(pane.pane_id);
    let pane_id = pane.pane_id;
    let runtime = runtime_snapshot(ws_state, pane_id);
    let info = pane_info_view(programs, &pane.name, runtime.as_ref());
    // A constant theme-driven card; the *selected* look (accent pill + border + bar)
    // is drawn by `Row` from its `active` signal, not baked into the background. This
    // keeps styling fully signal-driven (active flips in place via `sync_chrome_signals`,
    // no tree rebuild) and theme-driven (no ad-hoc per-state alphas).
    // The card is both a drag source and a drop target (F4.5); its opaque DragItemId
    // is assigned by the registry (which records that it's this pane) so the kind
    // round-trips through `drag::source_at`/`resolve_at` without trusting raw ids.
    let drag_id = drag.register(ChromeDragItem::Pane(pane_id));
    let icon_widget = Icon::new(info.icon).size(14.0).color(theme.foreground);
    let icon_signal = icon_widget.glyph_signal();
    let active_title_label = Label::new(info.title.clone()).color(theme.accent).bold(true);
    let active_title_signal = active_title_label.text_signal();
    let active_title = Visibility::new(active_title_label, active);
    let active_title_visible = active_title.visible_signal();
    let inactive_title_label =
        Label::new(info.title.clone()).color(theme.foreground).bold(true);
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
    .color(theme.foreground)
    .font_scale(0.8);
    let branch_display_signal = branch_label_widget.text_signal();
    let branch_signal = signal(info.git_branch.clone().unwrap_or_default());
    let add_label_widget =
        Label::new(info.git_added.clone().unwrap_or_default()).color(theme.success).font_scale(0.8);
    let add_label = add_label_widget.text_signal();
    let add_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Plus).size(12.0).color(theme.success))
            .child(add_label_widget),
        info.git_added.is_some(),
    );
    let add_text_visible_signal = add_segment.visible_signal();
    let modified_label_widget = Label::new(info.git_modified.clone().unwrap_or_default())
        .color(theme.warning)
        .font_scale(0.8);
    let modified_label = modified_label_widget.text_signal();
    let modified_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Warning).size(12.0).color(theme.warning))
            .child(modified_label_widget),
        info.git_modified.is_some(),
    );
    let modified_text_visible_signal = modified_segment.visible_signal();
    let deleted_label_widget =
        Label::new(info.git_deleted.clone().unwrap_or_default()).color(theme.danger).font_scale(0.8);
    let deleted_label = deleted_label_widget.text_signal();
    let deleted_segment = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(4.0)
            .child(Icon::new(Glyph::Minus).size(12.0).color(theme.danger))
            .child(deleted_label_widget),
        info.git_deleted.is_some(),
    );
    let deleted_text_visible_signal = deleted_segment.visible_signal();
    let git_row = Visibility::new(
        Flex::row()
            .align(Align::Center)
            .gap(6.0)
            .child(Icon::new(Glyph::GitBranch).size(12.0).color(theme.warning))
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
    let emit = emit_intent.clone();
    let content = if info.git_branch.is_some() {
        Flex::column()
            .gap(4.0)
            .grow(1.0)
            .child(
                Flex::row()
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
                    .child(
                        Flex::row()
                            .align(Align::Center)
                            .child(Flex::column().child(active_title).child(inactive_title)),
                    ),
            )
            .child(
                Flex::row()
                    .child(Flex::row().width(Length::Px(2.0)))
                    .child(git_row),
            )
    } else {
        Flex::row()
            .align(Align::Center)
            .grow(1.0)
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
            .child(
                Flex::row()
                    .align(Align::Center)
                    .child(Flex::column().child(active_title).child(inactive_title)),
            )
    };
    let card = Row::new()
        .background(theme.foreground.with_alpha(5))
        .highlight(theme.accent)
        .radius(theme.control_radius())
        .padding(6.0)
        .marker(ActiveMarker::Bar)
        .active(active)
        .draggable(drag_id)
        .drop_target(drag_id)
        // On click/Enter the card records its pane id in the host sink; the app reads
        // it after dispatch and focuses that pane (read-via-signal / write-via-action).
        .on_activate(move || {
            emit(crate::app::interaction::InteractionIntent::FocusPane { pane_id });
        })
        .child(content);
    // Bind the card's active signal so focus changes update it without a rebuild.
    signals.pane_active.push((pane_id, card.state()));
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
    let (watch, _repaint) =
        RepaintWatch::new(KeyHint::new(card).hint(hint).placement(HintPlacement::CenterRight));
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
) -> RepaintWatch {
    let active = c.panes.iter().any(|p| active_pane == Some(p.pane_id));
    // The MarkerGroup is a column drag source + drop target (F4.5 step 2). Its grip
    // gutter is the only surface not covered by a child pane card, so innermost-first
    // hit-testing routes a grip press → column and a card press → pane, for free.
    let drag_id = drag.register(ChromeDragItem::Column { ws: ws_idx, col: c.col_idx });
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
        ));
    }
    // Bind the column bar's active signal (lit iff it holds the active pane).
    let pane_ids = c.panes.iter().map(|p| p.pane_id).collect::<Vec<_>>();
    signals.col_active.push((pane_ids, col.state()));
    let (watch, _repaint) = RepaintWatch::new(col);
    watch
}

/// Build the **WorkspacesContainer** content — the workspace tree mounted inside the
/// sidebar shell (see `heca-sidebar-design-spec`). Each workspace is a `.frameless()`
/// [`DockFrame`] (header count [`Badge`] = total panes); its columns are compact
/// [`column_view`]s (left marker bar + pane cards, no "Col N" header rows — those ate
/// the sidebar for no user value). Pure projection of the [`SidebarTree`].
fn build_workspaces_container(
    tree: &SidebarTree,
    programs: &ProgramsConfig,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    ws_state: &WorkspacesContainerState,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
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
                emit(crate::app::interaction::InteractionIntent::ToggleWorkspaceCollapsed {
                    ws_idx,
                });
            })
            .header(
                Flex::row()
                    .align(Align::Center)
                    .child(badge)
                    .child(Flex::row().width(Length::Px(6.0))),
            );
        if active_ws {
            // Light accent wash over the whole active workspace area (+ the accent
            // count badge) makes the active workspace clearly prominent.
            dock = dock.background(theme.accent.with_alpha(28));
        }
        // The whole workspace is a column drop target (F4.5 step 2 scope C): dropping a
        // column anywhere on it that isn't a deeper column/pane target moves the column
        // into this workspace. Innermost-first hit-testing lets columns/panes override.
        dock = dock.drop_target(drag.register(ChromeDragItem::Workspace { ws: ws_idx }));
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
            ));
        }
        dock = dock.child(cols);
        col = col.child(dock);
    }
    col
}

/// Build the LEFT sidebar **SHELL**: a full-height, bracket-framed, frosted panel
/// occupying the *whole* left column — a header (collapse toggle) over the body. Per
/// the chrome plan (F5, `pluggable-chrome-plugin-plan.md` §2.1) the sidebar is a
/// *shell*; the [`build_workspaces_container`] tree is mounted into the body as the
/// first container (display-only until the F4 shared-state layer wires interaction).
#[expect(
    clippy::too_many_arguments,
    reason = "chrome shell assembly still threads retained-tree state explicitly during the Phase 0 migration"
)]
fn build_sidebar_shell(
    tree: &SidebarTree,
    programs: &ProgramsConfig,
    left_w: f32,
    sidebar_h: f32,
    shell_bg: Color,
    theme: &GuiTheme,
    emit_intent: &ChromeIntentEmitter,
    ws_state: &WorkspacesContainerState,
    sidebar_gap: f32,
    signals: &mut ChromeSignals,
    drag: &mut DragItemRegistry,
) -> Flex {
    let inner_w = (left_w - sidebar_gap * 2.0).max(0.0);
    let inner_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
    // Header: a sidebar glyph + a collapse toggle pushed to the right. The toggle is
    // inert until interaction is wired (it becomes an action against shared state).
    let header = Flex::row()
        .align(Align::Center)
        .gap(6.0)
        .padding_xy(2.0, 2.0)
        .child(Icon::new(Glyph::Sidebar).size(16.0).color(theme.muted))
        .child(Flex::row().grow(1.0))
        .child(IconButton::new(
            Icon::new(Glyph::CaretRight).size(14.0).color(theme.muted),
        ));

    Flex::column()
        .width(Length::Px(left_w))
        .height(Length::Px(sidebar_h))
        .child(
            Surface::column()
                .width(Length::Px(left_w))
                .height(Length::Px(sidebar_h))
                .background(shell_bg)
                .padding(sidebar_gap)
                .child(
                    Pane::new()
                        .bracketed()
                        .width(Length::Px(inner_w))
                        .height(Length::Px(inner_h))
                        .padding(10.0)
                        .gap(8.0)
                        .background(shell_bg)
                        .border(theme.border, theme.border_width)
                        .child(header)
                        .child(build_workspaces_container(
                            tree,
                            programs,
                            theme,
                            emit_intent,
                            ws_state,
                            signals,
                            drag,
                        )),
                ),
        )
}

fn build_right_sidebar_shell(
    right_w: f32,
    sidebar_h: f32,
    shell_bg: Color,
    theme: &GuiTheme,
    sidebar_gap: f32,
) -> Flex {
    let inner_w = (right_w - sidebar_gap * 2.0).max(0.0);
    let inner_h = (sidebar_h - sidebar_gap * 2.0).max(0.0);
    let header = Flex::row()
        .align(Align::Center)
        .gap(6.0)
        .padding_xy(2.0, 2.0)
        .child(Label::new("Details").color(theme.foreground))
        .child(Flex::row().grow(1.0))
        .child(IconButton::new(
            Icon::new(Glyph::CaretRight).size(14.0).color(theme.muted),
        ));

    Flex::column()
        .width(Length::Px(right_w))
        .height(Length::Px(sidebar_h))
        .child(
            Surface::column()
                .width(Length::Px(right_w))
                .height(Length::Px(sidebar_h))
                .background(shell_bg)
                .padding(sidebar_gap)
                .child(
                    Pane::new()
                        .bracketed()
                        .width(Length::Px(inner_w))
                        .height(Length::Px(inner_h))
                        .padding(10.0)
                        .gap(8.0)
                        .background(shell_bg)
                        .border(theme.border, theme.border_width)
                        .child(header),
                ),
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

/// Assemble the chrome root widget tree (no layout/paint): a transparent tab band,
/// a middle row hosting the (optional) full-height sidebar shell + a transparent
/// content spacer, and the opaque status bar at the bottom. Returns the concrete
/// [`Flex`] so it can be **retained** across frames (see [`RetainedChrome`]).
fn chrome_root(
    frame: &ChromeFrame,
    left_sidebar: Option<Flex>,
    right_sidebar: Option<Flex>,
    signals: &mut ChromeSignals,
) -> Flex {
    let ChromeFrame { w, h, tab_bar_height, status_bar_height, status, side_bg, fg } = *frame;
    let middle_h = (h - tab_bar_height - status_bar_height).max(0.0);
    // The status label's text is bound so mode/focus changes update it in place.
    let status_label = Label::new(status).font_size(CHROME_TEXT_SIZE).color(fg);
    let status_signal = status_label.text_signal();
    let (status_watch, _status_repaint) = RepaintWatch::new(status_label);
    signals.status = Some(status_signal);

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

    Flex::column()
        .width(Length::Px(w))
        .height(Length::Px(h))
        // Transparent tab band — the hand-drawn tab bar paints underneath.
        .child(
            Flex::row()
                .width(Length::Px(w))
                .height(Length::Px(tab_bar_height)),
        )
        .child(middle)
        .child(
            Surface::row()
                .width(Length::Px(w))
                .height(Length::Px(status_bar_height))
                .background(side_bg)
                .radius(0.0)
                .align(Align::Center)
                .padding_xy(8.0, 0.0)
                .child(status_watch),
        )
}

/// Layout + paint a (retained) chrome root tree into a [`Scene`] at the window size.
/// Re-run every frame; cheap and creates no signals (those live in the retained tree).
pub(crate) fn paint_chrome_root(root: &mut Flex, w: f32, h: f32, theme: &GuiTheme) -> Scene {
    let mut scene = Scene::new();
    LayoutEngine::new()
        .base_font(theme.font_size)
        .compute(root, Size::new(w as f64, h as f64));
    {
        let mut cx =
            PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
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
            crate::app_state::AppDragPayload::Column { swap, .. } => (DragSourceKind::Column, *swap),
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
            Point::new((label.x + 10.0) as f64, (label.y - label.height / 2.0) as f64),
            Size::new(label.width as f64, label.height as f64),
        );
        cx.drag_ghost(rect, &label.text, swap);
    }
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
    let mut root = chrome_root(frame, left_sidebar, right_sidebar, &mut signals);
    paint_chrome_root(&mut root, frame.w, frame.h, theme)
}

/// Converts an app theme color token into the grid-ui color type.
fn app_color_to_gui(color: heca_config::theme::Color) -> Color {
    Color::new(color.r, color.g, color.b, color.a)
}

/// Converts a serialized app shadow token into the grid-ui shadow color.
///
/// The grid-ui theme stores shadow as a concrete RGBA color, while the app
/// config/theme model stores the base color string and alpha separately.
fn shadow_to_gui(shadow: &heca_config::theme::Shadow) -> Color {
    match shadow.color.parse::<heca_config::theme::Color>() {
        Ok(color) => Color::new(
            color.r,
            color.g,
            color.b,
            (shadow.alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        ),
        Err(_) => Color::TRANSPARENT,
    }
}

fn glow_level_to_gui(level: heca_config::theme::GlowLevel) -> heca_grid_ui::theme::GlowLevel {
    match level {
        heca_config::theme::GlowLevel::None => heca_grid_ui::theme::GlowLevel::None,
        heca_config::theme::GlowLevel::Thin => heca_grid_ui::theme::GlowLevel::Thin,
        heca_config::theme::GlowLevel::Medium => heca_grid_ui::theme::GlowLevel::Medium,
        heca_config::theme::GlowLevel::Large => heca_grid_ui::theme::GlowLevel::Large,
    }
}

fn intensity_to_gui(level: heca_config::theme::Intensity) -> heca_grid_ui::theme::Intensity {
    match level {
        heca_config::theme::Intensity::Off => heca_grid_ui::theme::Intensity::Off,
        heca_config::theme::Intensity::Low => heca_grid_ui::theme::Intensity::Low,
        heca_config::theme::Intensity::Medium => heca_grid_ui::theme::Intensity::Medium,
        heca_config::theme::Intensity::Heavy => heca_grid_ui::theme::Intensity::Heavy,
    }
}

/// Projects the loaded app theme into the grid-ui widget theme contract.
///
/// This keeps chrome widgets visually aligned with the runtime app palette and
/// effect tokens until the theme model is fully unified across crates.
fn app_theme_to_gui_theme(theme: &heca_config::theme::Theme) -> GuiTheme {
    GuiTheme {
        name: theme.name.clone(),
        background: app_color_to_gui(theme.background),
        surface: app_color_to_gui(theme.surface),
        foreground: app_color_to_gui(theme.foreground),
        muted: app_color_to_gui(theme.muted),
        border: app_color_to_gui(theme.border),
        accent: app_color_to_gui(theme.accent),
        glow: app_color_to_gui(theme.glow),
        shadow: shadow_to_gui(&theme.shadow),
        danger: app_color_to_gui(theme.danger),
        success: app_color_to_gui(theme.success),
        warning: app_color_to_gui(theme.warning),
        font_family: theme.font_family.clone(),
        font_size: theme.font_size,
        radius: theme.border_radius,
        border_width: theme.border_width,
        glow_size: glow_level_to_gui(theme.glow_size),
        intensity: intensity_to_gui(theme.intensity),
        show_focus_border: theme.show_focus_border,
        icon_secondary_alpha: theme.icon_secondary_alpha,
    }
}

fn chrome_surface_color(theme: &heca_config::theme::Theme) -> Color {
    app_color_to_gui(theme.surface)
}

fn top_bottom_pane_background_color(theme: &heca_config::theme::Theme) -> Color {
    app_color_to_gui(theme.effective_top_bottom_pane_background())
}

fn left_sidebar_background_color(theme: &heca_config::theme::Theme) -> Color {
    app_color_to_gui(theme.effective_left_sidebar_background())
}

fn right_sidebar_background_color(theme: &heca_config::theme::Theme) -> Color {
    app_color_to_gui(theme.effective_right_sidebar_background())
}

fn chrome_bar_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    top_bottom_pane_background_color(theme)
        .with_alpha((appearance.chrome_opacity().clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn sidebar_shell_background_color_for(
    sidebar_bg: Color,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    sidebar_bg.with_alpha((appearance.opacity().clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn chrome_shell_surface_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    chrome_surface_color(theme)
        .with_alpha((appearance.opacity().clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn chrome_bar_color(state: &crate::app_state::AppState) -> Color {
    chrome_bar_color_for(&state.theme, &state.appearance)
}

fn left_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    sidebar_shell_background_color_for(left_sidebar_background_color(&state.theme), &state.appearance)
}

fn right_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    sidebar_shell_background_color_for(right_sidebar_background_color(&state.theme), &state.appearance)
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
    let fg = app_color_to_gui(state.theme.foreground);
    (bar_bg, sidebar_bg, fg)
}

/// Bridge the loaded app theme into a GuiTheme for chrome widgets.
pub(crate) fn chrome_gui_theme(state: &crate::app_state::AppState) -> GuiTheme {
    let (_, sidebar_bg, _) = chrome_colors(state);
    let mut theme = app_theme_to_gui_theme(&state.theme);
    theme.background = app_color_to_gui(state.theme.background);
    theme.surface = sidebar_bg;
    // `[appearance]` effect-token overrides take precedence over the theme.
    // `glow_size` owns glow (presence + radius + strength); `intensity` owns
    // scanline/CRT overlay opacity only. Unset → the theme value already set
    // above by `app_theme_to_gui_theme` wins.
    theme.glow_size = glow_level_to_gui(state.appearance.effective_glow_size(&state.theme));
    theme.intensity = intensity_to_gui(state.appearance.effective_intensity(&state.theme));
    theme
}

/// The status-bar text projection (`N panes | focus | MODE…`).
fn chrome_status(state: &crate::app_state::AppState) -> String {
    let pane_count = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.columns.iter().map(|c| c.panes.len()).sum::<usize>())
        .unwrap_or(0);
    let focus_title = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.title.as_str())
        .unwrap_or("—");
    let (mode_str, rename_hint) = crate::app::render::status_mode_parts(&state.input_mode);
    format!("{} panes | {} | {}{}", pane_count, focus_title, mode_str, rename_hint)
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
    /// Each pane card's [`KeyHint`] pick-letter signal, keyed by pane id. Driven each
    /// frame from the active [`InputMode`](crate::app_state::InputMode) candidates
    /// (move/swap/take pick): `Some(letter)` while the pane is a candidate, else
    /// `None`. This is the move/swap/take **targeting overlay** — the keyboard logic
    /// (candidates + key consumption) already lives in the action/input layer; this
    /// only projects it into the retained Dock.
    pub(crate) pane_hint: Vec<(PaneId, Signal<Option<String>>)>,
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

fn pane_fallback_name(tree: &SidebarTree, pane_id: PaneId) -> &str {
    tree.workspaces
        .iter()
        .flat_map(|ws| {
            ws.columns
                .iter()
                .flat_map(|col| col.panes.iter())
                .chain(ws.floating_panes.iter())
        })
        .find(|pane| pane.pane_id == pane_id)
        .map(|pane| pane.name.as_str())
        .unwrap_or_else(|| unreachable!("pane {pane_id:?} must exist in sidebar tree"))
}

fn sync_pane_runtime_state(
    session: &heca_core::layout::Session,
    workspaces: &WorkspacesContainerState,
) -> bool {
    use std::collections::HashSet;
    // TODO(reactivity): this is a per-frame full-sync push of every pane's runtime
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
                changed |= workspaces.set_pane_runtime(pane.id, &pane.runtime);
            }
        }
        for float in &ws.floating_panes {
            live_panes.insert(float.pane.id);
            changed |= workspaces.set_pane_runtime(float.pane.id, &float.pane.runtime);
        }
    }
    workspaces.retain_panes(&live_panes);
    changed
}

/// Mirror canonical app/runtime state into the shared chrome store before the
/// retained tree reads it. `InputMode` remains the source of truth for keyboard
/// pick flows; the store is the reactive UI mirror.
pub(crate) fn sync_chrome_state(state: &mut crate::app_state::AppState) -> bool {
    state.chrome_state.workspaces.set_active_pane(state.focused_pane);
    let next_candidates = state
        .input_mode
        .candidates()
        .map(|c| c.to_vec())
        .unwrap_or_default();
    if next_candidates.is_empty() {
        state.chrome_state.workspaces.clear_pick_candidates();
    } else {
        state.chrome_state.workspaces.set_pick_candidates(next_candidates);
    }
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
    // Project the active move/swap/take pick candidates onto each pane's KeyHint
    // keycap. The candidates (char→PaneId) and the key-press consumption already
    // live in the `InputMode` / action layer (`app/input.rs`); this only mirrors the
    // letters into the retained Dock. The currently focused pane is never a target,
    // so it shows no keycap (matches the legacy hand-drawn sidebar's behavior).
    let candidates = state.chrome_state.workspaces.with_pick_candidates(|c| c.to_vec());
    for (pid, sig) in &retained.signals.pane_hint {
        let next = pick_keycap(*pid, active, Some(&candidates));
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
            runtime.as_ref(),
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
        for (signal, visible) in [
            (sigs.title_active_visible, pane_active),
            (sigs.title_inactive_visible, !pane_active),
            (sigs.status_idle_visible, next.status == ProcessStatus::Idle),
            (sigs.status_running_visible, next.status == ProcessStatus::Running),
            (sigs.status_success_visible, next.status == ProcessStatus::Success),
            (sigs.status_error_visible, next.status == ProcessStatus::Error),
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
            (sigs.git_modified_visible, sigs.git_modified, next.git_modified),
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

    let left_w = chrome.left_sidebar_width;
    let left_sidebar = if left_w >= SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_h = (h - DEFAULT_TAB_BAR_HEIGHT - DEFAULT_STATUS_BAR_HEIGHT).max(0.0);
        Some(build_sidebar_shell(
            &state.sidebar_tree,
            &state.programs,
            left_w,
            sidebar_h,
            left_sidebar_shell_background_color(state),
            &theme,
            &emit_intent,
            &state.chrome_state.workspaces,
            state.appearance.effective_sidebar_gap(&state.theme),
            &mut signals,
            &mut drag_items,
        ))
    } else {
        None
    };
    let right_w = chrome.right_sidebar_width;
    let right_sidebar = if right_w >= SIDEBAR_EXPANDED_THRESHOLD {
        let sidebar_h = (h - DEFAULT_TAB_BAR_HEIGHT - DEFAULT_STATUS_BAR_HEIGHT).max(0.0);
        Some(build_right_sidebar_shell(
            right_w,
            sidebar_h,
            right_sidebar_shell_background_color(state),
            &theme,
            state.appearance.effective_sidebar_gap(&state.theme),
        ))
    } else {
        None
    };

    let root = chrome_root(
        &ChromeFrame {
            w,
            h,
            tab_bar_height: DEFAULT_TAB_BAR_HEIGHT,
            status_bar_height: DEFAULT_STATUS_BAR_HEIGHT,
            status: &status,
            side_bg,
            fg,
        },
        left_sidebar,
        right_sidebar,
        &mut signals,
    );
    (root, signals, drag_items)
}

/// Feed a pointer-press into the retained chrome tree so widget callbacks can route
/// sidebar intents through the app event loop. The tree is discarded afterwards so
/// incidental local widget state cannot drift away from the canonical store.
pub(crate) fn chrome_dispatch_press(
    state: &mut crate::app_state::AppState,
    pos: (f32, f32),
) {
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
/// drag targets panes; a column drag targets columns + workspaces — **never** the
/// nested pane cards, or the deepest hit would always be a pane and a column could
/// never be dropped on another column.
fn target_accepted_by(source: DragSourceKind, item: &ChromeDragItem) -> bool {
    match source {
        DragSourceKind::Pane => matches!(item, ChromeDragItem::Pane(_)),
        DragSourceKind::Column => {
            matches!(item, ChromeDragItem::Column { .. } | ChromeDragItem::Workspace { .. })
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
    let accept = |id| tree.drag_items.get(id).is_some_and(|it| target_accepted_by(source, it));
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
    chrome.left_sidebar_width.to_bits().hash(&mut hsh);
    chrome.right_sidebar_width.to_bits().hash(&mut hsh);
    chrome.sidebar_gap.to_bits().hash(&mut hsh);
    state.theme.name.hash(&mut hsh);
    for c in [state.theme.accent, state.theme.foreground, state.theme.border] {
        (c.r, c.g, c.b, c.a).hash(&mut hsh);
    }
    state.theme.border_radius.to_bits().hash(&mut hsh);
    state.appearance.chrome_opacity().to_bits().hash(&mut hsh);
    state.appearance.opacity().to_bits().hash(&mut hsh);
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
        assert_eq!(pick_keycap(p1, Some(p3), Some(&cands)), Some("a".to_string()));
        assert_eq!(pick_keycap(p2, Some(p3), Some(&cands)), Some("s".to_string()));
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
        let gui = app_theme_to_gui_theme(&theme);

        assert_eq!(gui.name, theme.name);
        assert_eq!(gui.background, app_color_to_gui(theme.background));
        assert_eq!(gui.surface, app_color_to_gui(theme.surface));
        assert_eq!(gui.muted, app_color_to_gui(theme.muted));
        assert_eq!(gui.border, app_color_to_gui(theme.border));
        assert_eq!(gui.accent, app_color_to_gui(theme.accent));
        assert_eq!(gui.glow, app_color_to_gui(theme.glow));
        assert_eq!(gui.danger, app_color_to_gui(theme.danger));
        assert_eq!(gui.success, app_color_to_gui(theme.success));
        assert_eq!(gui.warning, app_color_to_gui(theme.warning));
        assert_eq!(gui.font_family, theme.font_family);
        assert_eq!(gui.font_size, theme.font_size);
        assert_eq!(gui.radius, theme.border_radius);
        assert_eq!(gui.border_width, theme.border_width);
        assert_eq!(gui.glow_size, heca_grid_ui::theme::GlowLevel::None);
        assert_eq!(gui.intensity, heca_grid_ui::theme::Intensity::Off);
        assert!(gui.show_focus_border);
    }

    #[test]
    fn chrome_background_and_surface_colors_come_from_theme_tokens() {
        let tron = heca_config::theme::load("grid_tron");
        let latte = heca_config::theme::load("latte");

        assert_eq!(
            top_bottom_pane_background_color(&tron),
            app_color_to_gui(tron.effective_top_bottom_pane_background())
        );
        assert_eq!(chrome_surface_color(&tron), app_color_to_gui(tron.surface));
        assert_eq!(
            left_sidebar_background_color(&latte),
            app_color_to_gui(latte.effective_left_sidebar_background())
        );
        assert_eq!(
            right_sidebar_background_color(&latte),
            app_color_to_gui(latte.effective_right_sidebar_background())
        );
        assert_eq!(chrome_surface_color(&latte), app_color_to_gui(latte.surface));
        assert_ne!(left_sidebar_background_color(&latte), chrome_surface_color(&latte));
    }

    #[test]
    fn sidebar_shell_background_stays_distinct_from_bar_tint_when_transparent() {
        let theme = heca_config::theme::load("latte");
        let appearance = heca_config::appearance::AppearanceConfig {
            transparency: 10,
            ..Default::default()
        };

        assert_ne!(
            chrome_bar_color_for(&theme, &appearance),
            sidebar_shell_background_color_for(left_sidebar_background_color(&theme), &appearance)
        );
        assert_eq!(
            sidebar_shell_background_color_for(left_sidebar_background_color(&theme), &appearance).r,
            theme.effective_left_sidebar_background().r
        );
    }

    #[test]
    fn chrome_scene_emits_status_text() {
        use heca_grid_ui::{Color, DrawCommand};
        let theme = GuiTheme::grid_tron();
        // No sidebar (collapsed) — just the status bar should produce text.
        let scene = super::chrome_scene(
            &super::ChromeFrame {
                w: 800.0,
                h: 600.0,
                tab_bar_height: 32.0,
                status_bar_height: 24.0,
                status: "2 panes | foo | NORMAL",
                side_bg: Color::new(17, 17, 27, 255),
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
    fn sidebar_shell_hosts_header_and_container() {
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
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        let theme = GuiTheme::grid_tron();
        let emit_intent: super::ChromeIntentEmitter = Rc::new(|_| {});
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        chrome.workspaces.set_active_pane(Some(heca_core::layout::PaneId(1)));
        // The shell wraps a bracketed Pane that holds [header, WorkspacesContainer];
        // the container hosts a dock per workspace (so the tree's text is visible).
        let shell = super::build_sidebar_shell(
            &tree,
            &heca_config::programs::ProgramsConfig::default(),
            280.0,
            600.0,
            theme.background,
            &theme,
            &emit_intent,
            &chrome.workspaces,
            8.0,
            &mut super::ChromeSignals::default(),
            &mut super::DragItemRegistry::default(),
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
            2,
            "the shell Pane holds [header, WorkspacesContainer body]",
        );
        let container = &pane.base().children[1];
        assert!(
            !container.base().children.is_empty(),
            "WorkspacesContainer must host a dock per workspace",
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
                    state: SidebarItemState::Active,
                }],
            }],
            floating_panes: Vec::new(),
        });

        let theme = GuiTheme::grid_tron();
        let emit_intent: super::ChromeIntentEmitter = Rc::new(|_| {});
        let chrome = SharedChromeState::new(280.0, true, 260.0, false);
        let mut drag = super::DragItemRegistry::default();
        let _ = super::build_sidebar_shell(
            &tree,
            &heca_config::programs::ProgramsConfig::default(),
            280.0,
            600.0,
            theme.background,
            &theme,
            &emit_intent,
            &chrome.workspaces,
            8.0,
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
            ws.floating_panes.push(heca_core::layout::workspace::FloatingPane {
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

        let tiled = chrome
            .workspaces
            .with_pane_runtime(PaneId(10), |runtime| runtime.expect("tiled runtime").snapshot());
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

        let view = pane_info_view(&programs, "shell", Some(&runtime));

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

        let view = pane_info_view(&programs, "pane", Some(&runtime));

        assert_eq!(view.icon, Glyph::Terminal);
        assert_eq!(view.title, "zsh");
        assert_eq!(view.status, ProcessStatus::Idle);
        assert_eq!(view.git_branch, None);
        assert_eq!(view.git_added, None);
        assert_eq!(view.git_modified, None);
        assert_eq!(view.git_deleted, None);
    }

    #[test]
    fn header_buttons_width_scales_with_action_count() {
        use heca_config::appearance::PaneAction;
        assert_eq!(super::header_buttons_width(&[], 15.0), 0.0);
        let one = super::header_buttons_width(&[PaneAction::Close], 15.0);
        let three = super::header_buttons_width(
            &[PaneAction::Split, PaneAction::MoveLeft, PaneAction::Close],
            15.0,
        );
        assert!(one > 0.0);
        assert!(three > one, "more buttons ⇒ wider cluster");
    }

    #[test]
    fn pane_action_spec_maps_kinds_to_actions() {
        use crate::input::WmAction;
        use heca_config::appearance::PaneAction;
        let pid = PaneId(7);

        // Pane-parameterized actions carry the pane/column and don't need focus.
        let (g, a, _, focus) = super::pane_action_spec(PaneAction::Close, pid, 2, 3);
        assert_eq!(g, Glyph::XSquare);
        assert_eq!(a, WmAction::ClosePaneById { pane_id: pid });
        assert!(!focus);

        let (g, a, _, focus) = super::pane_action_spec(PaneAction::Split, pid, 2, 3);
        assert_eq!(g, Glyph::SquareSplitVertical);
        assert_eq!(a, WmAction::AddPaneToColumn { ws_idx: 2, col_idx: 3 });
        assert!(!focus);

        // Active-targeted actions use the requested icons + need focus-first.
        let (g, a, _, focus) = super::pane_action_spec(PaneAction::Zoom, pid, 0, 0);
        assert_eq!(g, Glyph::FrameCorners);
        assert_eq!(a, WmAction::ZoomColumn);
        assert!(focus);

        let (g, a, _, focus) = super::pane_action_spec(PaneAction::Float, pid, 0, 0);
        assert_eq!(g, Glyph::Cards);
        assert_eq!(a, WmAction::Float);
        assert!(focus);
    }

    #[test]
    fn floating_pane_keeps_only_float_and_close() {
        use heca_config::appearance::PaneAction;
        // Driven by the action policy: float/close are focused-pane-local (kept),
        // split/zoom/move are tiled-only (hidden when floating).
        assert!(super::pane_action_visible_when_floating(PaneAction::Float));
        assert!(super::pane_action_visible_when_floating(PaneAction::Close));
        assert!(!super::pane_action_visible_when_floating(PaneAction::Split));
        assert!(!super::pane_action_visible_when_floating(PaneAction::Zoom));
        assert!(!super::pane_action_visible_when_floating(PaneAction::MoveLeft));
        assert!(!super::pane_action_visible_when_floating(PaneAction::MoveRight));
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
        let hints = PaneActionHints::default();
        #[allow(clippy::too_many_arguments)]
        fn content<'a>(
            programs: &'a ProgramsConfig,
            segments: &'a [heca_config::appearance::PaneSegment],
            actions: &'a [heca_config::appearance::PaneAction],
            rt: &'a PaneRuntime,
            hints: &'a PaneActionHints,
            col_idx: usize,
        ) -> PaneHeaderContent<'a> {
            PaneHeaderContent {
                programs,
                fallback_name: "shell",
                runtime: Some(rt),
                segments,
                actions,
                ws_idx: 0,
                col_idx,
                zoomed: false,
                floating: false,
                hints,
            }
        }
        let base = pane_header_key(&content(&programs, &segments, &actions, &runtime, &hints, 0), 15.0, 300.0);
        // Same inputs ⇒ same key (no needless rebuild).
        assert_eq!(
            base,
            pane_header_key(&content(&programs, &segments, &actions, &runtime, &hints, 0), 15.0, 300.0)
        );
        // A different column ⇒ different key (re-bakes the split action's col_idx).
        assert_ne!(
            base,
            pane_header_key(&content(&programs, &segments, &actions, &runtime, &hints, 1), 15.0, 300.0)
        );
        // A different branch ⇒ different key (rebuild).
        let mut other = runtime.clone();
        other.git = Some(GitInfo { branch: Some("dev".into()), ..GitInfo::default() });
        assert_ne!(
            base,
            pane_header_key(&content(&programs, &segments, &actions, &other, &hints, 0), 15.0, 300.0)
        );
        // A large width change ⇒ different key (re-truncate); tiny jitter ⇒ same bucket.
        assert_ne!(
            base,
            pane_header_key(&content(&programs, &segments, &actions, &runtime, &hints, 0), 15.0, 120.0)
        );
        assert_eq!(
            base,
            pane_header_key(&content(&programs, &segments, &actions, &runtime, &hints, 0), 15.0, 295.0)
        );
    }

    #[test]
    fn format_binding_symbolizes_prefix_and_modifiers() {
        let prefix = crate::keymap::KeyCombo::parse("Ctrl+b");
        // The `prefix` token becomes the symbolized prefix combo; the key upper-cases.
        assert_eq!(super::format_binding("prefix+v", &prefix), "⌃B V");
        assert_eq!(super::format_binding("prefix+x", &prefix), "⌃B X");
        // Modifiers within a chord symbolize; named keys map to glyphs.
        assert_eq!(super::format_binding("Alt+Enter", &prefix), "⌥↩");
        assert_eq!(super::format_binding("prefix+Shift+g", &prefix), "⌃B ⇧G");
        assert_eq!(super::format_binding("", &prefix), "");
    }
}
