//! The window's state: [`AppState`] itself.

use super::*;

/// **A menu a widget asked to open, waiting for the host to mount it.**
///
/// The menu itself, where it goes, and **who it is about** — the declaring widget's own identity,
/// carried through so the host's providers can build its entries without a hit test of their own.
/// `None` when the declarer publishes no identity, which is a contribution: entries that act on app
/// state rather than on a particular thing.
pub type PendingMenu = (
    heca_grid_ui::widgets::ContextMenu,
    heca_grid_ui::widgets::MenuAnchor,
    Option<String>,
);


/// Central application runtime state.
///
/// Holds the winit window, GPU resources, session layout, backends,
/// input mode, sidebar model, theme, and all per-frame bookkeeping.
///
/// # Invariants
///
/// - `focused_pane` always matches the session's active pane ID (kept in sync
///   by `sync_focus` after every mutation).
/// - `backends` contains an entry for every pane in the session that has a
///   backend. Removing a pane from the session must also remove its backend
///   via `BackendStore::remove_for_pane`.
/// - `input_mode` is `Normal` unless an explicit mode transition happened
///   (prefix key, sidebar entry, rename, etc.). Mode transitions always go
///   through the input dispatch, never by direct field mutation.
/// - the workspaces model is rebuilt via `sync_from_session()` after any layout
///   or focus change that affects the sidebar projection.
pub struct AppState {
    pub window: Arc<Window>,
    pub event_proxy: EventLoopProxy<AppEvent>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub primitive_renderer: PrimitiveRenderer,
    pub text_renderer: TextRenderer,
    /// Inline terminal-image renderer (Sixel / iTerm2 / Kitty graphics).
    pub image_renderer: ImageRenderer,
    pub grid_renderer: GridRenderer,
    pub compositor: Compositor,
    pub terminal_layers: HashMap<PaneId, RetainedTerminalLayer>,
    pub terminal_layer_scratch: RetainedTerminalScratch,
    /// In-app frosted-blur primitive (shared, compositor-owned).
    /// Produces a blurred copy of the scene texture once per frame, then many
    /// `Backdrop::draw` calls stamp it into pane surface rects.
    pub blur: Blur,
    /// Backdrop sampler (shared, stateless pipeline). Draws blurred scene regions
    /// into arbitrary on-screen rects with alpha blending.
    pub backdrop: Backdrop,
    /// z=0 heca-owned frosted gradient background layer (the bottom-most layer
    /// panes composite translucently over). Cached; recomputes only on resize
    /// or gradient/blur param change. See `heca-renderer/src/background.rs`.
    pub background: BackgroundLayer,
    /// **What every window shares and none needs a window for** — the terminal processes, the
    /// notification store, the git facts kept for panes, the program catalog. See
    /// [`crate::server`]; the rest of this struct is the window and what it draws.
    pub server: crate::server::ServerState,
    /// The workspaces, columns and panes — the content every window on this session shares.
    pub session: Session,
    /// What **this window** sees of the session: the workspace shown, the scroll of each
    /// workspace, the window's size. Reach both through [`layout`](Self::layout) and
    /// [`layout_mut`](Self::layout_mut).
    pub(crate) view: WindowView,
    pub theme: Theme,
    /// Appearance contract (transparency/blur/vibrancy) — read-only, copied from config.
    pub appearance: AppearanceConfig,
    /// How roomy the command palette is (`[settings] command_palette_size`). Re-read on reload, so
    /// changing it and pressing reload resizes the next palette.
    pub command_palette_size: heca_config::settings::PaletteSize,
    /// What the user has searched for and chosen — the command palette's memory, ranked into its
    /// order and recalled by its history keys.
    ///
    /// **Host-owned**, because the palette is rebuilt from scratch every time it opens (see
    /// `chrome::palette::open_command_palette`) and a memory that died with the widget would
    /// remember nothing. In-memory for now; F004/P092/T386 gives it a file.
    pub search_store: std::rc::Rc<std::cell::RefCell<heca_grid_ui::search::SearchStore>>,
    /// How a search query's case is treated (`[settings] search_case`). Re-read on reload.
    pub search_case: heca_config::settings::SearchCase,
    /// Whether the search memory is written to disk at all (`[settings] search_history`). Turned
    /// off, the store is neither read nor written — a shared or recorded machine is a real reason,
    /// and a user who turns it off expects the existing file to stop being consulted too.
    pub search_history: bool,
    /// The store revision last written to disk, so a save happens exactly when something new was
    /// recorded — see [`crate::search_state::persist_if_changed`].
    pub search_saved_revision: u64,
    /// What this instance last agreed the history file said. A save applies **our delta** on top of
    /// whatever is on disk now, so another heca window's runs are merged rather than discarded —
    /// and that delta is only computable against this.
    pub search_baseline: crate::search_state::Baseline,
    /// Structured font configuration (families + sizes), decoupled from the color
    /// theme. Read at the same choke points that previously read `theme.font_*`.
    /// Refreshed on `prefix+Shift+r` reload.
    pub font_config: FontConfig,
    pub terminal_cell_size: (f32, f32),
    /// Whether any visible pane is currently showing an animated inline image
    /// (GIF/APNG). Set each frame by `sync_retained_terminal_layers`; the redraw
    /// loop keeps requesting frames while true so the animation plays.
    pub has_animated_images: bool,
    /// App-wide font-zoom **offset in points**, applied on top of BOTH the
    /// configured chrome/UI font (`font_config.size.ui`) and the terminal font
    /// (`font_config.size.terminal`), so the whole app scales together. Driven by
    /// the app-wide zoom action (the `app-03` base) and `Ctrl`/`Meta`+wheel over
    /// chrome. `0.0` = no zoom. Reset returns it to `0.0`; a config reload leaves
    /// it intact.
    pub app_font_zoom: f32,
    /// Per-pane terminal font-zoom **offset in points**, applied on top of the
    /// global base (`font_config.size.terminal + app_font_zoom`) for
    /// that pane only. Absent = follows the global size. The effective per-pane
    /// size is clamped to `[TERMINAL_FONT_SIZE_MIN, TERMINAL_FONT_SIZE_MAX]`.
    pub pane_font_zoom: HashMap<PaneId, f32>,
    /// Resolved base cell size for panes with a non-zero `pane_font_zoom` offset,
    /// recomputed whenever that pane's effective font size changes. Panes absent
    /// here use the global `terminal_cell_size`. Feeds the base cell that
    /// `prepare_terminal_mount` fits the PTY grid to.
    pub pane_cell_override: HashMap<PaneId, (f32, f32)>,
    pub scale_factor: f64,
    pub needs_redraw: bool,
    /// **One line of feedback in the bottom bar**, shown until the next keypress.
    ///
    /// What a key did when it could not do the thing you asked — a pick with nothing to offer is
    /// the case it exists for. It goes here rather than into a toast because it is a reply to the
    /// key you just pressed, not an event: the bar is already where the pick's own prompt appears,
    /// so the answer and the question share a surface, and nothing covers the work to say it.
    ///
    /// Cleared by the next key rather than by a timer. A stale line in a status bar costs nothing —
    /// unlike a toast, which is why this is not one — and the next thing you do is what makes it
    /// irrelevant.
    pub status_note: Option<String>,
    pub focused_pane: Option<PaneId>,
    pub input_mode: InputMode,
    /// **The window root — the one retained tree** (`docs/surface-compositor.md` § 0.8).
    ///
    /// Everything on screen hangs from here: the chrome subtree is child 0, and a surface — an
    /// overlay, a modal, a toast stack, a plugin's panel — is a positioned child beside it. One root
    /// means one walk for layout, paint, input and hints, which is what lets a surface receive
    /// pointer events by *being placed* rather than by being enrolled somewhere.
    ///
    /// **It is not an `Option`, and the chrome is a child rather than the root itself**, for the
    /// same reason: it has to outlive the chrome. The chrome subtree is discarded and rebuilt
    /// whenever its signature changes — window size, scale, sidebar widths, theme — and
    /// `reload_config` drops the build outright. A surface parented to any of that would be
    /// destroyed by a resize, a sidebar toggle or a theme reload, losing its open state, its
    /// half-played arrival and its focus. Here it survives all of them, and a surface can be placed
    /// before the first chrome has ever been built.
    pub window_root: heca_grid_ui::Flex,
    /// What the last chrome **build** produced — its signature and the handles that came with it.
    /// The tree it built lives in [`window_root`](Self::window_root) as child 0; this is the
    /// bookkeeping about it, so dropping it forces a rebuild without taking any surface with it.
    /// See `chrome::RetainedChrome` (F4.1).
    pub chrome_tree: Option<crate::chrome::RetainedChrome>,
    /// **Retained per-pane shells**, keyed by pane — the frame around whatever app runs inside,
    /// and the widget that carries the pane's identity and its pick letter. Built/positioned each
    /// frame by `chrome::sync_panes`, painted through `heca_grid_ui::paint_child` (which is what
    /// draws the letter).
    ///
    /// Retained rather than rebuilt in paint: the picker writes a letter into the tree when it
    /// opens and reads it back a keystroke later, so a tree that does not outlive the frame cannot
    /// carry one — which is why the pane letters used to be stamped by a host paint pass
    /// (F011/P094/T451).
    pub panes: HashMap<PaneId, crate::chrome::RetainedPane>,
    /// **Retained per-column trees**, keyed by the column's own id — the box a column occupies in
    /// the scrolling area, and the identity a pick addresses it by.
    ///
    /// The column is what OWNS `col:<id>`; the workspaces dock shows a view of it. Built and placed
    /// each frame by `chrome::sync_columns` (F003/P082/T474).
    pub columns: HashMap<heca_core::layout::ColumnId, crate::chrome::RetainedColumn>,
    /// The terminal each pane shows, by pane — the client's view of a running terminal process:
    /// what a window draws, and how much room it was given. Client state, like the retained trees.
    pub(crate) terminals: HashMap<heca_core::layout::PaneId, crate::chrome::terminal::Terminal>,
    /// The buttons whose press a terminal's program has heard and whose release it has not — so a
    /// release is passed on only where its press was.
    pub(crate) terminal_presses: crate::app::terminal_host::HeardPresses,
    /// Dynamically registered overlay/panel layers (an on-demand exposé, a plugin panel).
    /// The built-in surfaces (panes, sidebar, current overlays) are derived from their own
    /// trees; this holds runtime-added layers that join the same surface stack. See
    /// [`crate::chrome::LayerRegistry`] and `docs/surface-compositor.md` §9.
    pub layers: crate::chrome::LayerRegistry,
    /// How to build each layer another crate added with an extension's `.layer(..)`, kept so it is built afresh
    /// each time it is asked for. Client state: it is about what a window draws.
    pub(crate) added_layers: crate::entry::AddedLayers,
    /// Every pane header button there is, by name — heca's own and the ones other crates added.
    /// Client state: it is about what a window draws.
    pub(crate) pane_buttons: crate::chrome::PaneButtons,
    /// Every pane header chip there is, by name. Client state, like the buttons.
    pub(crate) pane_chips: crate::chrome::PaneChips,
    /// Every sidebar pane-row line there is, by name. Client state, like the buttons and chips.
    pub(crate) pane_lines: crate::chrome::PaneRowLines,
    /// Host-owned overlay stack: pending modal completions + action metadata keyed by
    /// [`OverlayId`](crate::chrome::OverlayId). The overlays' *visual* trees live in
    /// [`layers`](AppState::layers) (Modal band); this holds only the result callbacks the
    /// overlay-control actions (`SubmitOverlay`/`CloseOverlay`) resolve. See `chrome::overlay`
    /// and `pluggable-chrome-plugin-plan.md` §2.7.1/§2.7.2.
    pub overlays: crate::chrome::OverlayHost,
    /// Display shortcuts for every bound action, keyed by config name, resolved from
    /// config at load/reload (so tooltips/hints show the user's real, rebindable
    /// keys — never the defaults when overridden). Any chrome button looks its own
    /// shortcut up by the action it triggers; see [`crate::chrome::ActionShortcuts`].
    pub action_shortcuts: crate::chrome::ActionShortcuts,
    /// Runtime catalog of action metadata (labels/icons/…; later confirmation specs), seeded from
    /// the built-in descriptors and — with the plugin action API — extended by plugins. The single
    /// runtime home every UI surface resolves action metadata through (see
    /// [`crate::actions::ActionCatalog`]).
    pub action_catalog: crate::actions::ActionCatalog,
    /// Context-menu registry: the content-pane built-in seeded
    /// at startup; plugins attach via `Contribution::ContextMenu` (context-menu-5). Both the
    /// mouse right-click and the keyboard `OpenContextMenu` resolve through it via
    /// `chrome::context_menu::open_context_menu_for`.
    pub context_menu_registry: crate::chrome::ContextMenuRegistry,
    /// Shared, signal-backed chrome/UI state (read-via-signals / write-via-actions).
    /// Owns region visibility/width (migrated from the old `SidebarState`); collapse,
    /// selection, targeting candidates, and scroll migrate onto it next.
    pub chrome_state: crate::chrome::SharedChromeState,
    /// Host runtime for pluggable chrome regions (contract §3.1.1): owns container
    /// placement/ordering per region and host-level moves. plugin-02 wires it
    /// empty; first-party providers register in plugin-03.
    pub chrome_host: crate::chrome::ChromeHost,
    pub mouse: MouseState,
    pub modifiers: ModifiersState,
    /// Host-owned shared selection state, reusable across pane/backend types.
    pub selection: SelectionState,
    /// Deadline of an active **visual bell** flash (`None` = not flashing). Set on a
    /// terminal bell when `[appearance.terminal] bell_visual` is on; the render pass
    /// draws a fading content-area overlay until `Instant::now()` reaches it.
    pub bell_flash_until: Option<std::time::Instant>,
    /// **When a widget asked to be drawn again**, with no event coming to prompt it.
    ///
    /// Some behaviour is due at a *time*: a tooltip revealing once the pointer has rested, a caret
    /// blinking. A resting pointer produces no events, so the frame that would draw it never
    /// happens on its own. The widget says how long it needs (`Component::next_redraw`, folded down
    /// a whole tree), the loop sleeps until then, and **this is what makes the loop actually draw
    /// when it gets there** — without it, waking finds every reason-to-draw false and goes straight
    /// back to sleep, which is a tooltip that appears only when you nudge the mouse (Antonio,
    /// driving, 2026-09-04).
    ///
    /// Same shape as [`bell_flash_until`](Self::bell_flash_until): a deadline the frame loop reads,
    /// never a per-widget timer the host would have to keep in step.
    pub widget_frame_due: Option<std::time::Instant>,
    /// Active scrollback searches, **one per pane**. Drives each pane's query bar,
    /// its match highlights, and `n`/`N` navigation. terminal-task-19.
    ///
    /// Per-pane rather than a single global search: with one shared slot, starting a
    /// search in a second pane silently destroyed the first pane's — its bar and
    /// highlights vanished and there was no way to get them back. Every pane now
    /// keeps its own, and they all render at once.
    ///
    /// A `BTreeMap` so iteration order is stable — panes draw in a deterministic
    /// order frame to frame rather than wandering with hash seeding.
    ///
    /// Reach it through [`search_for`](Self::search_for) /
    /// [`focused_search`](Self::focused_search) and friends rather than indexing, so
    /// "the search the keyboard is driving" has exactly one definition.
    pub searches: std::collections::BTreeMap<PaneId, SearchState>,
    // (The right-click context menu + the destructive-confirm prompt are now host-owned overlay
    // layers in `chrome::overlay` — `open_dropdown` / `open_modal` — not bespoke fields here.)
    /// Most recently focused pane (for "go back" behavior).
    pub last_focused: Option<PaneId>,
    /// The last visited workspace index (for dim highlight in sidebar).
    pub last_visited_ws_idx: Option<usize>,
    /// Per-workspace last-visited pane IDs (for dim highlight and Prefix+i toggle).
    pub last_visited_pane_per_ws: Vec<Option<PaneId>>,
    /// **Where the exposé's highlight is, per workspace** — the map's own cursor, kept here rather
    /// than inside the widget because the map is rebuilt from scratch every time it opens.
    ///
    /// Two things depend on it, and both are things a freshly built tree cannot know:
    /// - the map opens on the pane you came from, not the first card in the session;
    /// - moving to another workspace and back returns the highlight to where you left it, instead
    ///   of restarting at that row's first pane.
    ///
    /// Deliberately **not** [`last_visited_pane_per_ws`](Self::last_visited_pane_per_ws), which is
    /// where the *app's* focus has been. Moving a highlight around a map is looking, not going: it
    /// must not rewrite the history that `prefix+i` and the sidebar's dim highlight read.
    ///
    /// Indexed by workspace, grown with the session like its neighbour above.
    pub expose_cursor_per_ws: Vec<Option<PaneId>>,
    /// **Which row the map's cursor is currently on** — the workspace, not the pane.
    ///
    /// The map is rebuilt whenever the session changes under it, and a rebuild has to put the
    /// cursor back where it was. Its per-workspace memory above cannot answer that on its own: it
    /// is read at the *active* workspace's index, so a rebuild while the cursor sat in another
    /// row moved the highlight to the active row — which reads as the map jumping to a different
    /// workspace the moment you delete a pane (Antonio, driving, 2026-08-11).
    ///
    /// Only consulted while the map is **already up**. Opening it fresh still starts at the
    /// workspace you are standing in, which is what the memory above is for.
    pub expose_cursor_ws: Option<usize>,
    /// **Which targets the host currently has a keycap on**, by the identity they declare
    /// (F003/P082/T427).
    ///
    /// The whole of the letter-ownership rule: `chrome::hint` withdraws exactly what it offered and
    /// never clears a label somebody else set, so the universal picker, a surface's own
    /// `KeyHintGroup` and the move/swap/take modes cannot erase one another. It replaced four
    /// per-widget signal lists projected every frame, which wrote `None` over every offered letter
    /// and needed a host-mode check to stop — a check a plugin could never add itself to.
    pub offered_letters: std::cell::RefCell<crate::chrome::hint::OfferedLetters>,
    /// **Which letter each pick target wore last time** — so it wears the same one again
    /// (F003/P082/T445).
    ///
    /// Keyed by the target's identity rather than its path, because a path lives one frame. Rebuilt
    /// on every pick from what is actually on screen, so a target that has gone releases its letter
    /// instead of holding one nobody can reach.
    ///
    /// Antonio, driving, 2026-08-17: *"I want to expand a pane, prefix+/ and `k` appears on that
    /// icon… then I want to collapse. prefix+/ and `j` appears on that button, while I was expecting
    /// `k`."* The letter was the target's **index**, so anything appearing earlier in the tree
    /// shifted every letter after it.
    pub remembered_letters: std::collections::HashMap<String, char>,
    /// Whether mouse interactions are enabled.
    pub mouse_enabled: bool,
    /// Whether auto edge scroll is enabled.
    pub auto_scroll_edge: bool,
    /// `[settings] edge_scroll_distance`: how close (px) a dragged pane must come to the content
    /// edge for the view to scroll.
    pub edge_scroll_distance: f32,
    /// Whether newly spawned terminal panes should auto-inject shell integration.
    pub shell_integration_enabled: bool,
    /// Append the process/program name (small, dimmed) next to a renamed pane's custom name
    /// (`[settings] pane_renamed_add_process_name`). Threaded here so `sync_pane_headers` reads
    /// it each frame and a reload rebuilds the headers.
    pub pane_renamed_add_process_name: bool,
    /// Show each pane's working directory as its own row in the sidebar pane card
    /// (`[appearance.expose] show_cwd`). Projected into the chrome store each sync so the card
    /// reads it via `ws_state`; a reload updates it live.
    pub expose_show_cwd: bool,
    /// Host terminal scrollback capacity (rows) threaded from
    /// `SettingsConfig::terminal_scrollback_lines`; used when spawning terminal
    /// backends so the engine retains the configured amount of history.
    pub terminal_scrollback_lines: usize,
    /// Terminal-only mouse support for host scrollback wheel routing.
    /// When true, wheel scrolls the host scrollback viewport unless the terminal
    /// app has grabbed the mouse. Does not affect `mouse_enabled` (chrome).
    pub terminal_mouse_enabled: bool,
    /// Number of scrollback rows per wheel notch.
    pub terminal_wheel_scroll_lines: usize,
    /// Points added/removed per terminal font-zoom step, from
    /// `[settings] terminal_font_zoom_step`. Threaded here so the zoom handlers
    /// don't reach back into config each dispatch.
    pub terminal_font_zoom_step: f32,
    /// Whether `Ctrl`/`Meta`+wheel changes the font size, from
    /// `[settings] mouse_wheel_change_font_size`. When false the wheel gesture is
    /// skipped and the modified wheel is forwarded normally.
    pub mouse_wheel_change_font_size: bool,
    /// Whether backend-side discrete terminal viewport animations are enabled.
    pub terminal_scroll_animations_enabled: bool,
    /// The `[confirm]` table — per-action confirmation toggles (keyed by confirm name:
    /// `delete_pane` / `delete_column` / `delete_workspace`, or any plugin action). Read by the central
    /// confirm gate via [`ConfirmConfig::enabled`](heca_config::confirm::ConfirmConfig::enabled),
    /// which falls back to the action's `ConfirmSpec::default_enabled` when unset.
    pub confirm: heca_config::confirm::ConfirmConfig,
    /// Modifier key for interactive pane drag.
    pub interactive_move_modifier: heca_config::theme::ModifierKey,
    /// When the user entered Prefix mode (for auto-timeout).
    pub prefix_entered_at: Option<std::time::Instant>,
    /// The configured prefix key combo (e.g. Ctrl+b).
    pub prefix_combo: crate::keymap::KeyCombo,
    /// **How long prefix mode waits for the next key** — `[keys] prefix_timeout_ms`.
    pub prefix_timeout_ms: u64,
    /// Widget keymap (`widget-keys-config`): the single host-owned `[keys.widgets]`-derived map
    /// from a key chord to the semantic [`WidgetIntent`](heca_grid_ui::WidgetIntent)s it triggers.
    /// Every interactive widget/overlay (context menu, palette, `Select`, `Tabs`, `Dialog`,
    /// `Input`) consults it via [`Keymap::dispatch`](heca_grid_ui::Keymap::dispatch); consumed
    /// only while such a widget/overlay is focused (never hijacks normal input). Rebuilt on config
    /// reload alongside the keymap.
    pub widget_keymap: heca_grid_ui::Keymap,
    /// **Menus a widget declared and asked for**, waiting to become layers (F004/P084/T395).
    ///
    /// A widget builds its own menu and the framework picks the anchor, but only the host can put
    /// one *above everything* — so `install_menu_sink` drops it here and the event loop drains it.
    /// A queue rather than a direct call because the sink is a plain `Fn` installed once at
    /// startup, and inserting a layer needs `&mut AppState`; and rather than an `AppEvent` because
    /// a menu carries closures and a winit user event must be `Send`.
    pub pending_menus: std::rc::Rc<std::cell::RefCell<Vec<PendingMenu>>>,
    /// **Drops no widget took**, waiting to become moves (F003/P097/T496).
    ///
    /// The twin of [`pending_menus`](Self::pending_menus), for the same reason: a row owns the
    /// gesture and the framework resolves what landed on what, but moving a pane between
    /// workspaces needs `&mut AppState`, which a sink has not. Drained each frame.
    ///
    /// What arrives is two **names** — the host decides what its own names mean, and a name it does
    /// not recognise is simply dropped. That is what lets a plugin's rows use the same gesture.
    pub pending_drops: std::rc::Rc<std::cell::RefCell<Vec<heca_grid_ui::drag::Dropped>>>,
    /// Set to true when the user requests a config reload (e.g. via keybinding).
    /// The app checks this in about_to_wait and rebuilds keymaps/settings.
    pub pending_reload: bool,
    /// Whether the application window is currently focused.
    pub window_focused: bool,
    /// The OS cursor currently set on the window. Tracked so the cursor policy only
    /// calls `Window::set_cursor` when the icon actually changes (cursor-moved fires
    /// very often). See `mouse::update_cursor`.
    pub current_cursor: winit::window::CursorIcon,
    /// **Is the pointer resting on a toast card**, written by
    /// [`ToastStack::hovered_signal`](heca_grid_ui::widgets::ToastStack::hovered_signal).
    ///
    /// A signal rather than a callback because it is a *state*: true for as long as the pointer
    /// stays. The stack reports the fact and nothing more — it has no clock and does not know what
    /// a card's lifetime is; `NotificationRuntime::set_hovered` decides what it costs, which is
    /// that a card being read must not retire under the cursor reaching for its button.
    pub notification_hovered: heca_grid_ui::reactive::Signal<bool>,
    /// **The cards the toast stack draws** — this window's signal over the server's list of
    /// notifications on show, refreshed when the server reports a change
    /// ([`sync_toasts`](Self::sync_toasts)).
    pub toasts: heca_grid_ui::reactive::Signal<Vec<heca_grid_ui::widgets::ToastSpec>>,
    /// **The most lines a notification card's title and body may each take** —
    /// `[settings.notification_system] max_lines`, re-applied on reload. Read by the mounted
    /// `ToastStack` when it builds each card.
    pub notification_max_lines: heca_grid_ui::reactive::Signal<usize>,
    /// Whether the scoped `notification.pick` picker is open. `KeyHintGroup::open_when` on the
    /// toast surface reads this directly — F009/T492. In addition to global `prefix+/`, not
    /// instead.
    pub notification_pick_open: heca_grid_ui::reactive::Signal<bool>,
}


impl AppState {
    pub fn mark_full_redraw(&mut self) {
        self.needs_redraw = true;
    }

    /// Whether a container's cursor should be shown — **its keyboard focus**, which an overlay
    /// does not take away (context-menu-3, F003/P086/T365).
    ///
    /// The row stays highlighted for the duration of a menu opened on it, instead of losing the
    /// highlight the moment the overlay appears. It used to ask whether the app was in a mode; a
    /// container holding the keyboard is the thing that was always meant.
    pub fn container_cursor_visible(&self) -> bool {
        self.chrome_state.focused_container().is_some()
    }

    /// A first-party [`host`](crate::host) API handle (`app.on` / `app.state`) over
    /// the shared chrome store. The seam first-party providers (and the future WASM
    /// bridge) use to observe events + read state without touching internal signals.
    #[allow(
        dead_code,
        reason = "host API seam — first-party providers land in a later phase"
    )]
    pub fn host(&self) -> crate::host::App {
        crate::host::App::new(&self.chrome_state)
    }
}
