//! Grid UI showcase — runnable visual proof of the component library.
//!
//! Builds a retained `heca-grid-ui` component tree (HUD cards + interactive
//! buttons), lays it out each frame, paints it into a `Scene`, and renders via
//! the GPU backend (SDF glow + corner brackets + scanline) on the dark Grid
//! theme. Buttons respond to pointer hover and click.
//!
//! Run: `cargo run -p heca-renderer --example showcase`

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{BracketCmd, DrawCommand, Glow, ScanlineCmd};
use heca_grid_ui::{Component, Event, LayoutEngine, PaintCx, Point, Rectangle, Scene, Size};
use heca_renderer::grid::GridRenderer;
use heca_renderer::scene::enqueue_scene;
use heca_renderer::text::TextRenderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};
use winit::keyboard::{Key, NamedKey};

/// Paired `(active, error)` state signals for a showcase pane-info card.
type PaneTitleSignals = (Signal<bool>, Signal<bool>);

/// A static swatch that paints a drag drop-indicator over its bounds so the
/// catalog can *show* the visual without a live drag loop: `swap=false` → the
/// **move** insertion line (`drop_indicator`, `After`); `swap=true` → the
/// whole-item **swap** double-frame (`swap_indicator`). Both are host-painted in
/// the real app (F4.5 sidebar pane/column DnD).
struct IndicatorSwatch {
    base: heca_grid_ui::component::Base,
    swap: bool,
}

impl IndicatorSwatch {
    fn new(swap: bool) -> Self {
        Self {
            base: heca_grid_ui::component::Base::new(),
            swap,
        }
    }
}

impl Component for IndicatorSwatch {
    fn base(&self) -> &heca_grid_ui::component::Base {
        &self.base
    }
    fn base_mut(&mut self) -> &mut heca_grid_ui::component::Base {
        &mut self.base
    }
    fn paint(&self, cx: &mut PaintCx) {
        let b = self.base.bounds;
        // A faint tile so the indicator reads against a surface, like a sidebar item.
        cx.rect(b, cx.theme().surface, None, cx.theme().radius, None);
        if self.swap {
            cx.swap_indicator(b);
        } else {
            cx.drop_indicator(b, heca_grid_ui::drag::DropSide::After);
        }
    }
}

impl LayoutExt for IndicatorSwatch {}

/// Map a winit logical key onto the renderer-agnostic `GridKey`.
fn to_grid_key(key: &Key) -> Option<GridKey> {
    Some(match key {
        Key::Named(NamedKey::Tab) => GridKey::Tab,
        Key::Named(NamedKey::Enter) => GridKey::Enter,
        Key::Named(NamedKey::Space) => GridKey::Space,
        Key::Named(NamedKey::Escape) => GridKey::Escape,
        Key::Named(NamedKey::Backspace) => GridKey::Backspace,
        Key::Named(NamedKey::Delete) => GridKey::Delete,
        Key::Named(NamedKey::ArrowLeft) => GridKey::ArrowLeft,
        Key::Named(NamedKey::ArrowRight) => GridKey::ArrowRight,
        Key::Named(NamedKey::ArrowUp) => GridKey::ArrowUp,
        Key::Named(NamedKey::ArrowDown) => GridKey::ArrowDown,
        Key::Named(NamedKey::Home) => GridKey::Home,
        Key::Named(NamedKey::End) => GridKey::End,
        Key::Character(s) => GridKey::Char(s.chars().next()?),
        _ => return None,
    })
}
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Build the retained UI tree. `Flex` is layout-only; `Card`/`Button` are the
/// styled surfaces.
/// Live theme controls, backed by signals so a `Select` change is picked up by
/// the next frame's `build_scene` (which reads `self.theme`). Demonstrates that
/// border width, radius and glow are all configurable at runtime, not hardcoded.
/// Intensity options in the order shown by the INTENSITY select.
const INTENSITY_OPTS: [Intensity; 4] = [
    Intensity::Off,
    Intensity::Low,
    Intensity::Medium,
    Intensity::Heavy,
];

/// Size-select options, in dropdown order (`NORMAL`, `SMALL`, `LARGE`).
const SIZE_OPTS: [WidgetSize; 3] = [WidgetSize::Normal, WidgetSize::Small, WidgetSize::Large];

/// UI zoom as a **continuous** level in `[ZOOM_MIN, ZOOM_MAX]` (the 0–5 dial). The
/// factor is geometric — `ZOOM_RATIO^(level - ZOOM_DEFAULT)` — so level 2 == `1.0×`
/// and fractional levels (2.5, 3.2 …) interpolate smoothly. Multiplies the
/// logical→physical scale, scaling the WHOLE UI uniformly (a global zoom, distinct
/// from the per-widget `WidgetSize`). Driven by the SIZE-row select, `Ctrl +`/`-`/`0`,
/// and `Ctrl` + mouse-wheel.
const ZOOM_RATIO: f32 = 1.15;
/// Neutral (`1.0×`) level — the dial is centered here.
const ZOOM_DEFAULT: f32 = 2.0;
const ZOOM_MIN: f32 = 0.0;
const ZOOM_MAX: f32 = 5.0;
/// Level change per keypress / wheel notch.
const ZOOM_STEP: f32 = 0.25;

const APP_TITLE: &str = "heca-grid-ui showcase";

/// How the keybinding "prefix" is *displayed* (the config/parse token stays "prefix").
/// Single source of truth — change it here, not at each call site.
const PREFIX_SYMBOL: &str = "λ";

/// Render a keybinding string for **display**: the `prefix` token becomes
/// [`PREFIX_SYMBOL`]. Pass the real combo (the config/parse form, e.g. `"prefix+f"`)
/// — the stored binding is unchanged; only the rendered text substitutes the symbol.
fn display_shortcut(combo: &str) -> String {
    // The prefix is a *sequence* (press prefix, then the key), so join it with a
    // space — not "+", which would imply a simultaneous chord. Modifier chords
    // inside the key (e.g. "Shift+c") keep their own "+".
    match combo.strip_prefix("prefix+") {
        Some(rest) => format!("{PREFIX_SYMBOL} {rest}"),
        None => combo.to_string(),
    }
}

/// Theme names loadable via `heca_theme::load_theme`, in cycle order.
const THEME_NAMES: [&str; 3] = ["grid_tron", "mocha", "latte"];

/// Map a `heca_theme::Theme` onto a grid-ui `Theme`, bridging the two crates
/// until Phase 3B migrates grid-ui to re-export from `heca-theme`.
///
/// Font family/size come from the default [`heca_config::font::FontConfig`] —
/// fonts are decoupled from the color theme (see `compositor-04c`).
fn heca_theme_to_grid_ui(ht: &heca_theme::Theme) -> Theme {
    let font_config = heca_config::font::FontConfig::default();
    let shadow_color = match heca_theme::Color::from_str(&ht.shadow.color) {
        Ok(c) => Color::new(c.r, c.g, c.b, (ht.shadow.alpha * 255.0).min(255.0) as u8),
        Err(_) => Color::TRANSPARENT,
    };
    Theme {
        name: ht.name.clone(),
        background: Color::new(
            ht.background.r,
            ht.background.g,
            ht.background.b,
            ht.background.a,
        ),
        surface: Color::new(ht.surface.r, ht.surface.g, ht.surface.b, ht.surface.a),
        foreground: Color::new(
            ht.foreground.r,
            ht.foreground.g,
            ht.foreground.b,
            ht.foreground.a,
        ),
        muted: Color::new(ht.muted.r, ht.muted.g, ht.muted.b, ht.muted.a),
        border: Color::new(ht.border.r, ht.border.g, ht.border.b, ht.border.a),
        accent: Color::new(ht.accent.r, ht.accent.g, ht.accent.b, ht.accent.a),
        glow: Color::new(ht.glow.r, ht.glow.g, ht.glow.b, ht.glow.a),
        shadow: shadow_color,
        danger: Color::new(ht.danger.r, ht.danger.g, ht.danger.b, ht.danger.a),
        success: Color::new(ht.success.r, ht.success.g, ht.success.b, ht.success.a),
        warning: Color::new(ht.warning.r, ht.warning.g, ht.warning.b, ht.warning.a),
        font_family: font_config.family.ui_normal().to_string(),
        font_size: font_config.size.ui,
        radius: ht.border_radius,
        border_width: ht.border_width,
        // TODO: map from config `focus_border_width` once added to heca-theme;
        // for now the affordance outlines keep their visible default.
        focus_border_width: 1.5,
        glow_size: match ht.glow_size {
            heca_theme::GlowLevel::None => GlowLevel::None,
            heca_theme::GlowLevel::Thin => GlowLevel::Thin,
            heca_theme::GlowLevel::Medium => GlowLevel::Medium,
            heca_theme::GlowLevel::Large => GlowLevel::Large,
        },
        intensity: match ht.intensity {
            heca_theme::Intensity::Off => Intensity::Off,
            heca_theme::Intensity::Low => Intensity::Low,
            heca_theme::Intensity::Medium => Intensity::Medium,
            heca_theme::Intensity::Heavy => Intensity::Heavy,
        },
        show_focus_border: ht.show_focus_border,
        icon_secondary_alpha: ht.icon_secondary_alpha,
        active_wash_alpha: ht.active_wash_alpha,
        card_background_alpha: ht.card_background_alpha,
    }
}

fn load_grid_theme(name: &str) -> Theme {
    heca_theme_to_grid_ui(&heca_theme::load_theme(name))
}

#[derive(Clone, Copy)]
struct ThemeCtl {
    glow: Signal<GlowLevel>,
    radius: Signal<f32>,
    border: Signal<f32>,
    font: Signal<f32>,
    intensity: Signal<Intensity>,
    /// Global widget size variant, applied to the whole tree (demo of `WidgetSize`).
    size: Signal<WidgetSize>,
    /// Global UI zoom level (continuous, `ZOOM_MIN..=ZOOM_MAX`).
    zoom: Signal<f32>,
    /// Selected theme index into [`THEME_NAMES`].
    theme_idx: Signal<usize>,
}

/// Recursively set the size variant on every widget, so a global control reflects
/// across the whole showcase (in a real app you'd size widgets individually).
fn apply_size(c: &mut dyn Component, size: WidgetSize) {
    c.base_mut().style.size = size;
    for child in c.base_mut().children.iter_mut() {
        apply_size(child.as_mut(), size);
    }
}

/// Handles the host keeps after building the UI, to drive chrome interactions
/// from the keymap (the same signals an RPC layer would write).
struct BuiltUi {
    ui: Flex,
    /// Sidebar display mode (G5); `[` toggles full width ⇄ icon rail.
    sidebar_mode: Signal<RegionMode>,
    /// Per-cell pick-letter hints for the workspaces rail; `p` lights them up.
    rail_hints: Vec<Signal<Option<String>>>,
    /// Per-cell selection state for the workspaces rail (the focused pane).
    rail_states: Vec<Signal<bool>>,
    /// The letter assigned to each rail cell during a pick.
    rail_letters: Vec<char>,
    /// A pane's "needs attention" request; `n` sets it (flash + host beep).
    attention_req: Signal<bool>,
    /// Command-palette open state; `Ctrl+K` opens it.
    palette_open: Signal<bool>,
    /// Context-menu open state + anchor; right-click opens it at the cursor.
    menu_open: Signal<bool>,
    menu_anchor: Signal<Point>,
    /// Host-owned toast render list; `t` pushes one, the stack reports dismiss.
    toasts: Signal<Vec<ToastSpec>>,
}

fn build_ui(theme: &Theme, ctl: ThemeCtl) -> BuiltUi {
    // Host-owned sidebar display mode (G5): the keymap toggles it (`[`), the
    // region sizes to it, and its rail-aware Docks fold to icons when collapsed.
    let sidebar_mode = signal(RegionMode::Expanded);
    // Workspaces rail (enumerate flavor): one icon cell per pane, with a generic
    // KeyHint keycap that lights up on a move/swap/select pick (`p` in the demo).
    let rail_letters = vec!['a', 'b', 'c', 'd', 'e'];
    let rail_hints: Vec<Signal<Option<String>>> =
        rail_letters.iter().map(|_| signal(None)).collect();
    let rail_states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
    // Host-owned "needs attention" request for a pane (`n` fires it): the row
    // flashes and the host plays its own beep — grid-ui stays audio-free.
    let attention_req = signal(false);
    // Command palette (Ctrl+K): a fuzzy launcher. Type to filter (smart-case),
    // ↑/↓ or Ctrl+J/K to move, Enter to run, Esc to close. Commands just print.
    let palette = CommandPalette::new()
        .placeholder("Type a command…   (↑/↓ · Ctrl+J/K · Enter)")
        .command(
            Command::new("Split pane right", || println!("[showcase] split right"))
                .icon(Glyph::Sidebar)
                .key("⌥⌘→"),
        )
        .command(
            Command::new("Close pane", || println!("[showcase] close pane"))
                .icon(Glyph::Close)
                .key("⌘W"),
        )
        .command(
            Command::new("Toggle sidebar", || println!("[showcase] toggle sidebar"))
                .icon(Glyph::Sidebar)
                .key("⌘B"),
        )
        .command(
            Command::new("New terminal", || println!("[showcase] new terminal"))
                .icon(Glyph::Terminal),
        )
        .command(
            Command::new("Search files", || println!("[showcase] search files"))
                .icon(Glyph::Search)
                .key("⌘P"),
        )
        .command(
            Command::new("Git: commit", || println!("[showcase] git commit"))
                .icon(Glyph::GitCommit),
        )
        .command(
            Command::new("Settings", || println!("[showcase] settings"))
                .icon(Glyph::Gear)
                .key("⌘,"),
        );
    let palette_open = palette.open_signal();

    // Right-click context menu: the pointer counterpart to the keyboard picks.
    // Right-click anywhere to open it at the cursor. Entries carry an icon and a
    // quick-pick keycap (press the letter to run); ↑/↓ + Enter and click also work.
    let menu = ContextMenu::new()
        .entry(MenuEntry::new("Rename", || println!("[showcase] rename")).icon(Glyph::FileCode).key('r').shortcut(display_shortcut("prefix+$")))
        .entry(MenuEntry::new("Move to workspace", || println!("[showcase] → workspace")).icon(Glyph::ArrowRight).key('w'))
        .entry(MenuEntry::new("Move to column", || println!("[showcase] → column")).icon(Glyph::SquareSplitVertical).key('c'))
        .entry(MenuEntry::new("Duplicate", || println!("[showcase] duplicate")).icon(Glyph::Cards).key('d').enabled(false))
        .entry(MenuEntry::new("Close", || println!("[showcase] close")).icon(Glyph::XSquare).key('x').danger(true).shortcut(display_shortcut("prefix+x")));
    let menu_open = menu.open_signal();
    let menu_anchor = menu.anchor_signal();
    // Initial positions for the control selects, read from the current control
    // values — so the selects stay in sync if the tree is rebuilt (on font change).
    let radius_opts = [0.0f32, 4.0, 8.0, 16.0];
    let border_opts = [0.0f32, 1.0, 2.0, 3.0];
    let font_opts = [8.0f32, 10.0, 12.0, 15.0, 20.0, 28.0];
    let near = |arr: &[f32], v: f32, dflt: usize| {
        arr.iter()
            .position(|x| (x - v).abs() < 0.01)
            .unwrap_or(dflt)
    };
    let glow_idx = GlowLevel::ALL
        .iter()
        .position(|g| *g == ctl.glow.get_untracked())
        .unwrap_or(2);
    let radius_idx = near(&radius_opts, ctl.radius.get_untracked(), 2);
    let border_idx = near(&border_opts, ctl.border.get_untracked(), 1);
    let font_idx = near(&font_opts, ctl.font.get_untracked(), 3);
    let intensity_idx = INTENSITY_OPTS
        .iter()
        .position(|x| *x == ctl.intensity.get_untracked())
        .unwrap_or(2);
    let size_idx = SIZE_OPTS
        .iter()
        .position(|x| *x == ctl.size.get_untracked())
        .unwrap_or(0);
    let zoom_idx = (ctl.zoom.get_untracked().round() as usize).min(ZOOM_MAX as usize);

    let card = |title: &str, value: &str| {
        Card::new(title)
            .width(Length::Px(220.0))
            .height(Length::Px(140.0))
            .background(theme.surface)
            .border(theme.border, theme.border_width)
            .glow(theme.glow)
            .child(Label::new(value).color(theme.foreground).font_scale(2.0))
    };
    let click = |label: &str| {
        let name = label.to_string();
        move || println!("[showcase] {name} clicked")
    };
    // A toggle sitting to the left of its (state-colored) label.
    let toggle_row = |toggle: Toggle, label: &str, color: Color| {
        Flex::row()
            .gap(16.0)
            .align(Align::Center)
            .child(toggle)
            .child(Label::new(label).color(color))
    };
    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);

    // 20-entry list so the dropdown caps its height and shows a scrollbar.
    let workspaces: Vec<String> = (1..=20).map(|n| format!("WORKSPACE {n:02}")).collect();

    // ToastStack (G-overlay): the app owns the render list (`toasts`) and the
    // lifecycle; the stack just corner-anchors + animates them and reports
    // intents. `t` pushes one (see the keymap); clicking × removes it here.
    // Start empty — press `t` to push toasts (keeps them out of the dropdown's
    // corner by default; the overlapping-overlays text-bleed is a separate
    // renderer limitation to fix later).
    let toasts = signal(Vec::<ToastSpec>::new());
    let toast_stack = ToastStack::new(toasts)
        .corner(ToastCorner::TopRight)
        .on_dismiss(move |id| toasts.update(|v| v.retain(|s| s.id != id)))
        .on_action(|id| println!("[showcase] toast {id} action"));

    let ui = Flex::column()
        .padding(40.0)
        .gap(28.0)
        .align(Align::Center)
        .child(
            Flex::row()
                .gap(28.0)
                .child(card("UPLINK", "ONLINE"))
                .child(card("POWER", "98%"))
                .child(card("GRID NODES", "1024")),
        )
        // Dropdowns near the top: open downward and must overlap the rows below.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("MODE").color(theme.muted).font_scale(0.85))
                .child(Select::new(["NORMAL", "PREFIX", "PASSTHROUGH"]).on_change(report))
                .child(Label::new("WORKSPACE").color(theme.muted).font_scale(0.85))
                .child(Select::new(workspaces).selected(3).on_change(report)),
        )
        // One button per GridCN variant.
        .child(
            Flex::row()
                .gap(14.0)
                .align(Align::Center)
                .child(Button::primary("DEFAULT").on_click(click("DEFAULT")))
                .child(Button::secondary("SECONDARY"))
                .child(Button::outline("OUTLINE"))
                .child(Button::ghost("GHOST"))
                .child(Button::link("LINK"))
                .child(Button::destructive("DESTRUCTIVE").on_click(click("DESTRUCTIVE")))
                // Theme switcher: click to cycle grid_tron → mocha → latte → …
                // The label shows the current theme name; the showcase tree is
                // rebuilt on change so every widget picks up the new palette.
                .child(
                    Button::primary(format!("⇄ THEME: {}", theme.name)).on_click(move || {
                        let next = (ctl.theme_idx.get_untracked() + 1) % THEME_NAMES.len();
                        ctl.theme_idx.set(next);
                    }),
                ),
        )
        // Change widgets: Toggles across their states (on / off / disabled).
        .child(
            Flex::column()
                .gap(16.0)
                .child(toggle_row(
                    Toggle::new().on(true).on_change(report),
                    "GRID UPLINK",
                    theme.accent,
                ))
                .child(toggle_row(
                    Toggle::new().on(true).on_change(report),
                    "AUTO-SCAN",
                    theme.accent,
                ))
                // Disabled + off: stealth look, muted ("opaque") label.
                .child(toggle_row(
                    Toggle::new().disabled(true),
                    "STEALTH MODE",
                    theme.muted,
                ))
                // Disabled + on: active-but-locked, colored label.
                .child(toggle_row(
                    Toggle::new().on(true).disabled(true),
                    "LOCKED OUT",
                    theme.accent,
                )),
        )
        // Checkboxes: integrated labels (clickable), with one label on the left.
        .child(
            Flex::row()
                .gap(24.0)
                .align(Align::Center)
                .child(
                    Checkbox::new()
                        .checked(true)
                        .label("ENCRYPT")
                        .on_change(report),
                )
                .child(Checkbox::new().label("VERBOSE").on_change(report))
                .child(
                    Checkbox::new()
                        .checked(true)
                        .label("READ-ONLY")
                        .disabled(true),
                )
                .child(
                    Checkbox::new()
                        .label("LABEL LEFT")
                        .label_side(LabelSide::Left)
                        .on_change(report),
                ),
        )
        // Text inputs: empty-with-placeholder, pre-filled, and disabled.
        .child(
            Flex::row()
                .gap(20.0)
                .align(Align::Center)
                .child(Input::new().placeholder("CALLSIGN").on_change(report))
                .child(Input::new().value("GRID-7").on_change(report))
                .child(Input::new().value("LOCKED").disabled(true)),
        )
        // Tabs: segmented selector with a sliding underline.
        .child(Tabs::new(["OVERVIEW", "SIGNALS", "LOGS"]).on_change(report))
        .child(Separator::horizontal().length(440.0))
        // Display widgets: status dot + badges across variants.
        .child(
            Flex::row()
                .gap(12.0)
                .align(Align::Center)
                .child(StatusDot::online())
                .child(Badge::success("ONLINE"))
                .child(Badge::warning("DEGRADED"))
                .child(Badge::danger("OFFLINE"))
                .child(Badge::accent("v2.0"))
                .child(Badge::outline("BETA")),
        )
        // Tags (G8): metadata chips that can carry a leading icon and multiple
        // segments (a status-bar pill). Quieter than a Badge; hue configurable.
        .child(
            Flex::row()
                .gap(12.0)
                .align(Align::Center)
                // Segmented status-bar chip: path · branch · diff-stat (colored).
                .child(
                    Tag::new("~/repos/do-things")
                        .leading(Icon::new(Glyph::Folder).size(13.0).color(theme.muted))
                        .segment(
                            Flex::row()
                                .align(Align::Center)
                                .gap(6.0)
                                .child(Icon::new(Glyph::GitBranch).size(13.0).color(theme.muted))
                                .child(Label::new("main").color(theme.foreground).font_scale(0.8)),
                        )
                        .segment(
                            Flex::row()
                                .align(Align::Center)
                                .gap(6.0)
                                .child(Icon::new(Glyph::File).size(13.0).color(theme.muted))
                                .child(Label::new("5").color(theme.foreground).font_scale(0.8))
                                .child(Label::new("+152").color(theme.success).font_scale(0.8))
                                .child(Label::new("-12").color(theme.danger).font_scale(0.8)),
                        ),
                )
                .child(
                    Tag::new("feature/grid-ui")
                        .leading(Icon::new(Glyph::GitBranch).size(13.0).color(theme.accent))
                        .color(theme.accent),
                )
                .child(Tag::new("rust").color(theme.success)),
        )
        // Spinner + Alert.
        .child(
            Flex::row()
                .gap(20.0)
                .align(Align::Center)
                .child(Spinner::new())
                .child(Alert::warning("LINK UNSTABLE").body("retrying handshake...")),
        )
        // Toasts: bracket-framed notification cards. Severity-toned, with a body
        // line, an optional inline action, and a × dismiss. Presentation only —
        // here the example plays the "host" (its callbacks just print); a real app
        // owns the queue + lifecycle. Stacked like a notification list (also the
        // shape they take inline in a sidebar).
        .child(
            Flex::column()
                .gap(10.0)
                .child(Toast::success("Build succeeded").body("12 crates compiled in 4.2s"))
                .child(
                    Toast::danger("Connection lost")
                        .body("Reconnecting to the grid…")
                        .action("Retry", click("toast-retry"))
                        .on_dismiss(click("toast-dismiss")),
                )
                // A clickable card with no body — the inline "notification row" case.
                .child(Toast::info("New message from GRID-7").on_click(click("toast-open"))),
        )
        // Value displays: progress bar + energy gauge.
        .child(
            Flex::row()
                .gap(24.0)
                .align(Align::Center)
                .child(Label::new("POWER").color(theme.muted).font_scale(0.85))
                .child(ProgressBar::new().value(0.72))
                .child(Gauge::new().value(0.85)),
        )
        // Dropdown (overlay layer): opens over the content below it.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("INTENSITY").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["OFF", "LOW", "MEDIUM", "HEAVY"])
                        .selected(intensity_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.intensity.set(INTENSITY_OPTS[i.min(3)]);
                            }
                        }),
                ),
        )
        // Live theme controls — glow size, corner radius, border width. Each
        // Select writes a signal that the next frame folds into `self.theme`.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("GLOW").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(GlowLevel::ALL.map(|g| g.label()))
                        .selected(glow_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.glow
                                    .set(GlowLevel::ALL[i.min(GlowLevel::ALL.len() - 1)]);
                            }
                        }),
                )
                .child(Label::new("RADIUS").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "4", "8", "16"])
                        .selected(radius_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.radius.set(radius_opts[i.min(3)]);
                            }
                        }),
                )
                .child(Label::new("BORDER").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "1", "2", "3"])
                        .selected(border_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.border.set(border_opts[i.min(3)]);
                            }
                        }),
                ),
        )
        // Font-size control + widgets that re-measure from it: the Input, Select
        // and Label below are built at `theme.font_size`, so changing it rebuilds
        // them at a different size — you can see their boxes grow/shrink with text.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("FONT").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["8", "10", "12", "15", "20", "28"])
                        .selected(font_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.font.set(font_opts[i.min(font_opts.len() - 1)]);
                            }
                        }),
                )
                .child(Input::new().value("SIZED"))
                .child(Select::new(["ALPHA", "BETA", "GAMMA"]))
                .child(Label::new("Aa").color(theme.foreground)),
        )
        // Widget size variant — applied to the WHOLE tree so the showcase reflects
        // Small / Normal / Large globally (font + padding scale together) — and a
        // global ZOOM (0–5, centered on 2) that scales the entire UI uniformly.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("SIZE").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["NORMAL", "SMALL", "LARGE"])
                        .selected(size_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.size.set(SIZE_OPTS[i.min(SIZE_OPTS.len() - 1)]);
                            }
                        }),
                )
                .child(Label::new("ZOOM").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "1", "2", "3", "4", "5"])
                        .selected(zoom_idx)
                        .on_change(move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.zoom.set((i as f32).min(ZOOM_MAX));
                            }
                        }),
                ),
        )
        // Item rows: a single-select menu panel. Clicking a row highlights it and
        // clears the others. Immediate-mode (no reactive effects): each row's
        // `active` signal is set directly on click via a shared list.
        .child({
            let selected = signal(0usize);
            let states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
            let select = move |i: usize, item: Item| -> Item {
                states.borrow_mut().push(item.state());
                let states = states.clone();
                item.on_activate(move || {
                    selected.set(i);
                    for (j, s) in states.borrow().iter().enumerate() {
                        s.set(j == i);
                    }
                })
            };
            Pane::new()
                .width(Length::Px(320.0))
                .gap(2.0)
                .background(theme.surface)
                .child(select(
                    0,
                    Item::new("DASHBOARD")
                        .leading(StatusDot::online())
                        .active(true)
                        .marker(ActiveMarker::Bar),
                ))
                .child(select(
                    1,
                    Item::new("VIEW PROFILE")
                        .marker(ActiveMarker::Bar)
                        .trailing(Tag::new("CMD P")),
                ))
                .child(select(
                    2,
                    Item::new("SETTINGS")
                        .marker(ActiveMarker::Bar)
                        .trailing(Tag::new("CMD ,")),
                ))
                .child(
                    Item::new("SYSTEM")
                        .muted(true)
                        .trailing(Label::new(">").color(theme.muted)),
                )
        })
        // Duotone icons (G2): a strip of Phosphor glyphs from the icon font. Each
        // is two stacked layers — a dimmed secondary wash + a full primary on top,
        // same hue (secondary = primary at the theme's icon_secondary_alpha).
        .child(
            Flex::row()
                .gap(20.0)
                .align(Align::Center)
                .child(Icon::new(Glyph::Folder).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::FileCode).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::GitBranch).color(theme.success).size(34.0))
                .child(
                    Icon::new(Glyph::Terminal)
                        .color(theme.foreground)
                        .size(34.0),
                )
                .child(Icon::new(Glyph::Gear).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::Lightning).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::Warning).color(theme.warning).size(34.0))
                // Pane-action glyphs (the in-pane info bar buttons).
                .child(
                    Icon::new(Glyph::SquareSplitVertical)
                        .color(theme.foreground)
                        .size(34.0),
                )
                .child(
                    Icon::new(Glyph::ArrowLineLeft)
                        .color(theme.foreground)
                        .size(34.0),
                )
                .child(
                    Icon::new(Glyph::ArrowLineRight)
                        .color(theme.foreground)
                        .size(34.0),
                )
                .child(
                    Icon::new(Glyph::FrameCorners)
                        .color(theme.foreground)
                        .size(34.0),
                )
                .child(Icon::new(Glyph::Cards).color(theme.foreground).size(34.0))
                .child(Icon::new(Glyph::XSquare).color(theme.danger).size(34.0)),
        )
        // IconButton + Tooltip: a toolbar of compact, clickable icon affordances —
        // ghost at rest, tinted hover frame + press flash + focus ring — each
        // wrapped in a hover-revealed Tooltip label. The danger one uses `.tone()`.
        .child(
            Flex::row()
                .gap(8.0)
                .align(Align::Center)
                .child(Tooltip::new(
                    IconButton::new(Icon::new(Glyph::Search).color(theme.foreground).size(20.0))
                        .on_click(|| println!("[showcase] search")),
                    "Search",
                ))
                .child(Tooltip::new(
                    IconButton::new(Icon::new(Glyph::Gear).color(theme.foreground).size(20.0))
                        .on_click(|| println!("[showcase] settings")),
                    "Settings",
                ))
                .child(Tooltip::new(
                    IconButton::new(Icon::new(Glyph::Plus).color(theme.foreground).size(20.0))
                        .on_click(|| println!("[showcase] add")),
                    "New pane",
                ))
                // `.active(true)`: held-on (toggled) status — a persistent tone-tinted
                // frame, like the in-pane zoom/float buttons when engaged.
                .child(Tooltip::new(
                    IconButton::new(
                        Icon::new(Glyph::FrameCorners)
                            .color(theme.foreground)
                            .size(20.0),
                    )
                    .active(true)
                    .on_click(|| println!("[showcase] unzoom")),
                    "Zoom (active)",
                ))
                .child(
                    Tooltip::new(
                        IconButton::new(Icon::new(Glyph::Close).color(theme.danger).size(20.0))
                            .tone(theme.danger)
                            .on_click(|| println!("[showcase] close")),
                        "Close",
                    )
                    .side(TooltipSide::Bottom),
                ),
        )
        // Modal: a centered confirm dialog over a scrim. The destructive button
        // opens it; Esc / scrim / the dialog buttons close it (keys route to the
        // overlay while open). The Modal renders nothing until opened.
        .child({
            let modal = Modal::new("Delete pane?", "This action cannot be undone.")
                .confirm("Delete", || println!("[showcase] pane deleted"))
                .cancel("Cancel", || println!("[showcase] cancelled"))
                .danger(true)
                // Destructive → force an explicit choice: Esc / scrim won't dismiss.
                .dismissible(false);
            let open = modal.open_signal();
            Flex::row()
                .gap(12.0)
                .align(Align::Center)
                .child(Button::destructive("DELETE PANE…").on_click(move || open.set(true)))
                .child(modal)
        })
        // Pane frame variants: three Panes side-by-side showing None, Bordered,
        // and Bracketed modes. Each has a background + border so the decoration
        // is visible. Click and keyboard interactivity via the `.on_activate`
        // on each child Item.
        .child({
            Flex::row()
                .gap(8.0)
                .align(Align::Center)
                .child(
                    Pane::new()
                        .frameless()
                        .width(Length::Px(100.0))
                        .height(Length::Px(80.0))
                        .background(theme.surface)
                        .child(Label::new("None").font_size(12.0).color(theme.muted)),
                )
                .child(
                    Pane::new()
                        .bordered()
                        .width(Length::Px(100.0))
                        .height(Length::Px(80.0))
                        .background(theme.surface)
                        .border(theme.border, theme.border_width)
                        .child(Label::new("Bordered").font_size(12.0).color(theme.accent)),
                )
                .child(
                    // Bracketed: the self-contained corner reticle (driven by
                    // `theme.border_width`); no `.border()` — that would compete.
                    Pane::new()
                        .bracketed()
                        .width(Length::Px(100.0))
                        .height(Length::Px(80.0))
                        .background(theme.surface)
                        .child(Label::new("Bracketed").font_size(12.0).color(theme.accent)),
                )
                .child(
                    // Bordered with a per-pane width override: independent of the
                    // global BORDER control (`theme.border_width`). Used by the
                    // self-themed sidebar shell (`[appearance] sidebar_border_width`).
                    Pane::new()
                        .bordered()
                        .border_width(3.0)
                        .radius(10.0)
                        .width(Length::Px(100.0))
                        .height(Length::Px(80.0))
                        .background(theme.surface)
                        .border(theme.accent, 0.0)
                        .child(Label::new("Border 3px").font_size(12.0).color(theme.accent)),
                )
        })
        // In-pane info bar: a `Tag` chip composed *inside* the pane top (the app's
        // pane-info header). The `Pane` carries no built-in title — the bar is a
        // separate widget, so frame decoration and header stay independent. Left =
        // metadata segments (location · app · branch · diff-stat); the app adds the
        // action-button cluster on the right.
        .child(
            Flex::row()
                .gap(12.0)
                .align(Align::Start)
                // Idle terminal pane: location · shell name.
                .child(
                    Pane::new()
                        .bordered()
                        .width(Length::Px(220.0))
                        .height(Length::Px(90.0))
                        .padding(8.0)
                        .gap(8.0)
                        .background(theme.surface)
                        .border(theme.border, theme.border_width)
                        .child(
                            Tag::new("~/projects/heca")
                                .leading(Icon::new(Glyph::Folder).size(13.0).color(theme.muted))
                                .segment(
                                    Flex::row()
                                        .align(Align::Center)
                                        .gap(6.0)
                                        .child(
                                            Icon::new(Glyph::Terminal)
                                                .size(13.0)
                                                .color(theme.muted),
                                        )
                                        .child(
                                            Label::new("zsh")
                                                .color(theme.foreground)
                                                .font_scale(0.8),
                                        ),
                                ),
                        )
                        .child(Label::new("idle").font_size(12.0).color(theme.muted)),
                )
                // Running pane: full header = segments (left) + action buttons
                // (right). The app's pane info bar is exactly this — a segment `Tag`
                // and an `IconButton` cluster (default split + close, each a tooltip'd
                // action with its keybind) laid out space-between inside the pane top.
                .child(
                    Pane::new()
                        .bordered()
                        .width(Length::Px(360.0))
                        .height(Length::Px(90.0))
                        .padding(8.0)
                        .gap(8.0)
                        .background(theme.surface)
                        .border(theme.border, theme.border_width)
                        .child(
                            Flex::row()
                                .width(Length::Px(344.0))
                                .align(Align::Center)
                                .justify(Justify::SpaceBetween)
                                .child(
                                    Tag::new("Neovim")
                                        .leading(
                                            Icon::new(Glyph::FileCode)
                                                .size(13.0)
                                                .color(theme.accent),
                                        )
                                        .segment(
                                            Flex::row()
                                                .align(Align::Center)
                                                .gap(6.0)
                                                .child(
                                                    Icon::new(Glyph::GitBranch)
                                                        .size(13.0)
                                                        .color(theme.muted),
                                                )
                                                .child(
                                                    Label::new("…phase-7")
                                                        .color(theme.foreground)
                                                        .font_scale(0.8),
                                                ),
                                        )
                                        .color(theme.accent),
                                )
                                .child(
                                    Flex::row()
                                        .align(Align::Center)
                                        .gap(2.0)
                                        .child(
                                            Tooltip::new(
                                                IconButton::new(
                                                    Icon::new(Glyph::SquareSplitVertical)
                                                        .color(theme.foreground)
                                                        .size(15.0),
                                                )
                                                .cell(24.0),
                                                "Add pane  ⌃B V",
                                            )
                                            .side(TooltipSide::Bottom),
                                        )
                                        .child(
                                            Tooltip::new(
                                                IconButton::new(
                                                    Icon::new(Glyph::XSquare)
                                                        .color(theme.danger)
                                                        .size(15.0),
                                                )
                                                .cell(24.0)
                                                .tone(theme.danger),
                                                "Close  ⌃B X",
                                            )
                                            .side(TooltipSide::Bottom),
                                        ),
                                ),
                        )
                        .child(Label::new("running").font_size(12.0).color(theme.muted)),
                ),
        )
        // ScrollRegion (gridui-01): an embeddable vertical scroll viewport —
        // 25 rows in a 180px window → auto-scrollbar + wheel/drag. Pointer
        // coords are translated into content space so the rows stay clickable at
        // their visual position while scrolled. (The chrome sidebar itself is
        // not yet wired to it — that needs scroll-aware DnD hit-testing, a
        // follow-up.)
        .child(
            Flex::column()
                .gap(8.0)
                .child(Label::new("SCROLL REGION").color(theme.muted).font_scale(0.8))
                .child({
                    let mut list = ScrollRegion::new()
                        .height(Length::Px(180.0))
                        .width(Length::Px(300.0));
                    for i in 1..=25 {
                        list = list.child(
                            Item::new(format!("item {i:02}"))
                                .leading(Icon::new(Glyph::FileCode).color(theme.accent).size(16.0)),
                        );
                    }
                    list
                }),
        )
        // Chrome vocabulary (G1 Grid · G3 ItemGroup · G4 DockFrame · G5
        // ChromeRegion): a sidebar region hosting two DockFrames of grouped rows,
        // beside a PANES dock of composed, state-colored cards. Headers and the
        // file rows respond to click and keyboard (Tab to focus, Enter to activate).
        .child({
            // Single-select highlight shared across the explorer's file rows: the
            // clicked row goes active (accent bar), the rest clear. Immediate-mode.
            let selected = signal(0usize);
            let states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
            let file = move |name: &str, icon: Icon| -> Item {
                let item = Item::new(name).leading(icon).marker(ActiveMarker::Bar);
                let i = states.borrow().len();
                states.borrow_mut().push(item.state());
                let states = states.clone();
                item.on_activate(move || {
                    selected.set(i);
                    for (j, s) in states.borrow().iter().enumerate() {
                        s.set(j == i);
                    }
                })
            };

            // A pane "card" — the final Phase 7 pane-info shape: row 1 is a
            // centered inline `status dot + icon + title`; row 2 is a flat git
            // metadata line (`branch + optional counts`), hidden outside repos.
            let pane_sel = signal(0usize);
            let pane_states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
            let pane_titles: Rc<RefCell<Vec<PaneTitleSignals>>> = Rc::new(RefCell::new(Vec::new()));
            let pane = move |icon: Glyph,
                             title: &str,
                             git_branch: Option<&str>,
                             git_added: Option<&str>,
                             git_modified: Option<&str>,
                             git_deleted: Option<&str>,
                             status: DotStatus|
                  -> Row {
                let git_row = Visibility::new(
                    Flex::row()
                        .align(Align::Center)
                        .gap(6.0)
                        .child(Icon::new(Glyph::GitBranch).size(12.0).color(theme.warning))
                        .child(
                            Label::new(git_branch.unwrap_or_default())
                                .color(theme.foreground)
                                .font_scale(0.8),
                        )
                        .child(Visibility::new(
                            Flex::row()
                                .align(Align::Center)
                                .gap(4.0)
                                .child(Icon::new(Glyph::Plus).size(12.0).color(theme.success))
                                .child(
                                    Label::new(git_added.unwrap_or_default())
                                        .color(theme.success)
                                        .font_scale(0.8),
                                ),
                            git_added.is_some(),
                        ))
                        .child(Visibility::new(
                            Flex::row()
                                .align(Align::Center)
                                .gap(4.0)
                                .child(Icon::new(Glyph::Warning).size(12.0).color(theme.warning))
                                .child(
                                    Label::new(git_modified.unwrap_or_default())
                                        .color(theme.warning)
                                        .font_scale(0.8),
                                ),
                            git_modified.is_some(),
                        ))
                        .child(Visibility::new(
                            Flex::row()
                                .align(Align::Center)
                                .gap(4.0)
                                .child(Icon::new(Glyph::Minus).size(12.0).color(theme.danger))
                                .child(
                                    Label::new(git_deleted.unwrap_or_default())
                                        .color(theme.danger)
                                        .font_scale(0.8),
                                ),
                            git_deleted.is_some(),
                        )),
                    git_branch.is_some(),
                );
                let row = Row::new()
                    .background(theme.foreground.with_alpha(5))
                    .highlight(theme.accent)
                    .radius(theme.control_radius())
                    .padding(10.0)
                    .child({
                        let active_title = Visibility::new(
                            Label::new(title).color(theme.accent).bold(true),
                            false,
                        );
                        let active_title_signal = active_title.visible_signal();
                        let inactive_title = Visibility::new(
                            Label::new(title).color(theme.foreground).bold(true),
                            true,
                        );
                        let inactive_title_signal = inactive_title.visible_signal();
                        pane_titles
                            .borrow_mut()
                            .push((active_title_signal, inactive_title_signal));
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
                                            .child(StatusDot::new(status)),
                                    )
                                    .child(
                                        Flex::row().align(Align::Center).child(
                                            Icon::new(icon).color(theme.foreground).size(14.0),
                                        ),
                                    )
                                    .child(Flex::row().align(Align::Center).child(
                                        Flex::column().child(active_title).child(inactive_title),
                                    )),
                            )
                            .child(
                                Flex::row()
                                    .child(Flex::row().width(Length::Px(2.0)))
                                    .child(git_row),
                            )
                    });
                let i = pane_states.borrow().len();
                pane_states.borrow_mut().push(row.state());
                let pane_states = pane_states.clone();
                let pane_titles = pane_titles.clone();
                // DnD framework (universal `DragExt`): each card is a drag source
                // carrying its index as the opaque id. The app resolves a drop via
                // `drag::source_at`/`resolve_at` over the laid-out tree and paints
                // `PaintCx::drag_ghost`/`drop_indicator` — see docs/widgets.md §Drag.
                row.draggable(DragItemId::new(i)).on_activate(move || {
                    pane_sel.set(i);
                    for (j, s) in pane_states.borrow().iter().enumerate() {
                        let selected = j == i;
                        s.set(selected);
                        let (active_title, inactive_title) = pane_titles.borrow()[j];
                        active_title.set(selected);
                        inactive_title.set(!selected);
                    }
                })
            };

            // G4 DockFrame framing G3 ItemGroups; header slot carries a count
            // badge. `.rail(...)` makes it fold to a single icon when the sidebar
            // collapses to its rail (G5) — bound to the host-owned mode signal.
            let explorer = DockFrame::new("EXPLORER")
                .rail(sidebar_mode, Glyph::FolderOpen)
                // `.active(true)` paints the faint accent **wash** over the whole
                // frame (alpha = `theme.active_wash_alpha`) — the cue the app uses
                // to mark the active workspace. Signal-backed, so a host flips it in
                // place via `.active_state()` without rebuilding.
                .active(true)
                // Inset the header count badge by the Item rows' horizontal
                // padding (~14px) so it lines up vertically with the rows'
                // trailing badges instead of sitting flush at the frame edge.
                .header(
                    Flex::row()
                        .align(Align::Center)
                        .child(Badge::accent("3"))
                        .child(Flex::row().width(Length::Px(14.0))),
                )
                .child(
                    ItemGroup::new("src")
                        .child(file(
                            "main.rs",
                            Icon::new(Glyph::FileCode).color(theme.accent).size(18.0),
                        ))
                        .child(file(
                            "chrome_region.rs",
                            Icon::new(Glyph::FileCode)
                                .color(theme.foreground)
                                .size(18.0),
                        ))
                        .child(file(
                            "dock_frame.rs",
                            Icon::new(Glyph::FileCode)
                                .color(theme.foreground)
                                .size(18.0),
                        )),
                )
                .child(
                    // Collapsed group — verifies the paint fix: its rows must not
                    // bleed to the top-left while hidden.
                    ItemGroup::new("tests").expanded(false).child(file(
                        "phase_a.rs",
                        Icon::new(Glyph::FileCode).color(theme.muted).size(18.0),
                    )),
                );
            // Git status rows: a state-colored duotone icon + change-kind badge.
            let git_row =
                |icon: Icon, name: &str, tag: Badge| Item::new(name).leading(icon).trailing(tag);
            let source_control = DockFrame::new("SOURCE CONTROL")
                .rail(sidebar_mode, Glyph::GitBranch)
                .header(
                    Flex::row()
                        .align(Align::Center)
                        .child(Badge::warning("3"))
                        .child(Flex::row().width(Length::Px(14.0))),
                )
                .child(git_row(
                    Icon::new(Glyph::GitBranch).color(theme.warning).size(18.0),
                    "chrome_region.rs",
                    Badge::warning("M"),
                ))
                .child(git_row(
                    Icon::new(Glyph::Plus).color(theme.success).size(18.0),
                    "showcase.rs",
                    Badge::success("A"),
                ))
                .child(git_row(
                    Icon::new(Glyph::Minus).color(theme.danger).size(18.0),
                    "old_sidebar.rs",
                    Badge::danger("D"),
                ));

            // G5 ChromeRegion: a vertical sidebar shell hosting the docks. Bound
            // to the host-owned mode signal so `[` collapses it to the icon rail.
            let sidebar = ChromeRegion::vertical()
                .with_mode_signal(sidebar_mode)
                .expanded_size(340.0)
                .rail_size(64.0)
                .gap(14.0)
                .padding(14.0)
                .background(theme.surface)
                .dock(explorer)
                .dock(source_control);

            // PANES: Phase 7 sidebar-style pane cards. Row 1 is a centered inline
            // `status dot + icon + name`; row 2 is a flat git metadata line that
            // hides outside repos.
            // The dock is a DnD drop target (universal `DragExt`) — its cards drop here.
            let panes = DockFrame::new("PANES")
                .drop_target(DragItemId::new(usize::MAX))
                .child(
                    pane(
                        Glyph::FileCode,
                        "Neovim",
                        Some("feature/pane-runtime"),
                        Some("+2"),
                        Some("~3"),
                        None,
                        DotStatus::Online,
                    )
                    // `n` fires a "needs attention" pulse on this pane.
                    .attention(attention_req)
                    .attention_color(theme.warning),
                )
                .child(pane(
                    Glyph::FolderOpen,
                    "Yazi",
                    Some("feature/pane-runtime"),
                    None,
                    None,
                    None,
                    DotStatus::Offline,
                ))
                .child(pane(
                    Glyph::Terminal,
                    "build",
                    None,
                    None,
                    None,
                    None,
                    DotStatus::Error,
                ));
            // MarkerGroup: a column-style grouping fronted by a left marker bar
            // that lights to the accent when the group is active (here: the first).
            // Each group is also a DnD drag source + drop target (universal `DragExt`)
            // grabbed by its left grip gutter — this is how the app drags whole
            // *columns* (F4.5). A column drag accepts only column/workspace targets via
            // `drag::resolve_at_filtered`, so nested pane rows fall through to the group.
            let marker_demo = Flex::column()
                .gap(8.0)
                .child(
                    Label::new("MARKER GROUP")
                        .color(theme.muted)
                        .font_scale(0.8),
                )
                .child(
                    // Wide rows wrapped in KeyHint with `CenterRight`: the pick keycap
                    // pins to the right edge (vs the rail's `Center`) so the row label
                    // stays readable, and the transparent wrapper lets each row fill the
                    // column width. Driven by the same `p` pick as the rail.
                    MarkerGroup::new()
                        .active(true)
                        .gap(4.0)
                        .draggable(DragItemId::new(900))
                        .drop_target(DragItemId::new(900))
                        .child(
                            KeyHint::new(Row::new().padding(6.0).child(Label::new("pane A")))
                                .hint(rail_hints[0])
                                .placement(HintPlacement::CenterRight),
                        )
                        .child(
                            KeyHint::new(Row::new().padding(6.0).child(Label::new("pane B")))
                                .hint(rail_hints[1])
                                .placement(HintPlacement::CenterRight),
                        ),
                )
                .child(
                    MarkerGroup::new()
                        .gap(4.0)
                        .draggable(DragItemId::new(901))
                        .drop_target(DragItemId::new(901))
                        .child(
                            // `.color(theme.warning)` tints the keycap differently — the
                            // app uses this so a "move → workspace" pick reads distinctly
                            // from a pane pick.
                            KeyHint::new(Row::new().padding(6.0).child(Label::new("pane C")))
                                .hint(rail_hints[2])
                                .color(theme.warning)
                                .placement(HintPlacement::CenterRight),
                        ),
                )
                // The two host-painted drop visuals: a MOVE inserts (line on an edge);
                // a SWAP exchanges the whole item (double frame, no before/after).
                .child(
                    Label::new("DROP INDICATORS")
                        .color(theme.muted)
                        .font_scale(0.8),
                )
                .child(
                    Flex::row()
                        .gap(12.0)
                        .child(
                            Flex::column()
                                .gap(4.0)
                                .child(
                                    IndicatorSwatch::new(false)
                                        .width(Length::Px(120.0))
                                        .height(Length::Px(34.0)),
                                )
                                .child(Label::new("move").color(theme.muted).font_scale(0.8)),
                        )
                        .child(
                            Flex::column()
                                .gap(4.0)
                                .child(
                                    IndicatorSwatch::new(true)
                                        .width(Length::Px(120.0))
                                        .height(Length::Px(34.0)),
                                )
                                .child(
                                    Label::new("swap (Shift)")
                                        .color(theme.muted)
                                        .font_scale(0.8),
                                ),
                        ),
                );
            // Prefix-as-symbol: the keybinding "prefix" is *displayed* as λ (a plain
            // Geist Mono glyph — no icon/Nerd font needed). The config/parse token
            // stays "prefix+…"; only the rendered shortcut uses λ.
            let prefix_demo = Flex::column()
                .gap(8.0)
                .child(Label::new(format!("PREFIX AS SYMBOL — {PREFIX_SYMBOL}")).color(theme.muted).font_scale(0.82))
                .child(
                    Flex::row()
                        .align(Align::Center)
                        .gap(18.0)
                        .child(Tag::new(display_shortcut("prefix+f")))
                        .child(Tag::new(display_shortcut("prefix+q")))
                        .child(Tag::new(display_shortcut("prefix+Shift+c"))),
                )
                .child(
                    Label::new("(config token stays \"prefix+…\")")
                        .color(theme.muted)
                        .font_scale(0.74),
                );
            let panes_col = Flex::column()
                .width(Length::Px(380.0))
                .gap(16.0)
                .child(panes)
                .child(marker_demo)
                .child(prefix_demo);

            // Workspaces rail (enumerate flavor): one icon cell PER pane, so every
            // pane stays visible + addressable when collapsed — unlike a tool dock
            // that folds to a single icon. Each cell is wrapped in the generic
            // KeyHint, whose keycap lights up during a move/swap/select pick (`p`).
            let rail_icons = [
                (Glyph::Terminal, theme.success),
                (Glyph::FileCode, theme.accent),
                (Glyph::GitBranch, theme.warning),
                (Glyph::Gear, theme.foreground),
                (Glyph::Warning, theme.danger),
            ];
            let mut workspaces_rail = Flex::column().gap(8.0).align(Align::Center);
            for (i, (glyph, color)) in rail_icons.iter().enumerate() {
                let cell = RailCell::new(Icon::new(*glyph).color(*color).size(22.0))
                    .cell_size(44.0)
                    .active(i == 0);
                rail_states.borrow_mut().push(cell.state());
                let states = rail_states.clone();
                let cell = cell.on_activate(move || {
                    for (j, s) in states.borrow().iter().enumerate() {
                        s.set(j == i);
                    }
                });
                workspaces_rail = workspaces_rail.child(
                    KeyHint::new(cell)
                        .hint(rail_hints[i])
                        .placement(HintPlacement::Center),
                );
            }
            let rail_col = Flex::column()
                .gap(8.0)
                .align(Align::Center)
                .child(Label::new("WS").color(theme.muted).font_scale(0.8))
                .child(workspaces_rail);

            Flex::row()
                .gap(28.0)
                .align(Align::Start)
                .child(rail_col)
                .child(sidebar)
                .child(panes_col)
        })
        // The command palette overlays everything when open (Ctrl+K).
        .child(palette)
        // The right-click context menu overlays at the cursor when open.
        .child(menu)
        // The toast stack overlays a corner (presentation only; app owns the list).
        .child(toast_stack);
    BuiltUi {
        ui,
        sidebar_mode,
        rail_hints,
        rail_states: rail_states.borrow().clone(),
        rail_letters,
        attention_req,
        palette_open,
        menu_open,
        menu_anchor,
        toasts,
    }
}

/// Shift every node's absolute bounds down by `dy` (negative scrolls the page up).
/// Used for whole-page scrolling: the window framebuffer clips the overflow.
fn offset_tree(c: &mut dyn Component, dy: f64) {
    c.base_mut().bounds.loc.y += dy;
    for child in c.base_mut().children.iter_mut() {
        offset_tree(child.as_mut(), dy);
    }
}

/// Paint the tree, then decorate bordered surfaces with corner brackets and add
/// a full-window scanline overlay.
fn build_scene(root: &dyn Component, theme: &Theme, w: f32, h: f32, show_clip_demo: bool) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        // Background fill as the first scene command (not a clear pass): scissored to
        // the damage region it clears only the changed area, so the rest of the
        // persistent scene texture is preserved for damage-region redraw.
        cx.rect(
            Rectangle::from_size(Size::new(w as f64, h as f64)),
            theme.background,
            None,
            0.0,
            None,
        );
        root.paint(&mut cx);
    }
    // Corner brackets on the cards (first row) only — not the small buttons.
    if let Some(card_row) = root.base().children.first() {
        for card in &card_row.base().children {
            scene.push(DrawCommand::Brackets(BracketCmd {
                rect: card.base().bounds,
                color: theme.accent,
                len: 14.0,
                thickness: 1.5,
                glow: Some(Glow {
                    color: theme.glow,
                    radius: 6.0,
                    intensity: 1.0,
                }),
            }));
        }
    }
    // Clip-primitive demo (task B), toggled by `c`: a centered overlay viewport
    // whose text content is taller than the box and offset by a fractional amount,
    // wrapped in `with_clip`. The renderer scissors it, so the top and bottom lines
    // are sliced cleanly at the panel edges instead of spilling out — the editor's
    // pixel-scroll foundation. Painted into the overlay layer so it sits on top.
    if show_clip_demo {
        let mut cx = PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
        let (pw, ph) = (240.0, 150.0);
        let panel = Rectangle::new(
            Point::new((w as f64 - pw) * 0.5, (h as f64 - ph) * 0.5),
            Size::new(pw, ph),
        );
        cx.with_overlay(|cx| {
            cx.rect(
                panel,
                theme.surface,
                Some(heca_grid_ui::scene::Border {
                    color: theme.accent,
                    width: theme.border_width,
                }),
                theme.radius,
                None,
            );
            // Title sits above the clipped region (not clipped).
            cx.text(
                Rectangle::new(
                    Point::new(panel.loc.x + 12.0, panel.loc.y + 8.0),
                    Size::new(panel.size.w - 24.0, 16.0),
                ),
                "CLIP VIEWPORT  (c)",
                theme.accent,
                11.0,
                TextAlign::Start,
                true,
            );
            let inner = Rectangle::new(
                Point::new(panel.loc.x + 12.0, panel.loc.y + 30.0),
                Size::new(panel.size.w - 24.0, panel.size.h - 42.0),
            );
            cx.with_clip(inner, |cx| {
                // Start ~half a line above the top edge so line 00 is sliced; the 12
                // lines overflow the bottom so the last line is sliced too.
                for i in 0..12 {
                    let y = inner.loc.y - 9.0 + i as f64 * 18.0;
                    cx.text(
                        Rectangle::new(Point::new(inner.loc.x, y), Size::new(inner.size.w, 16.0)),
                        &format!("clip line {i:02} — sliced at the edges"),
                        theme.foreground,
                        12.0,
                        TextAlign::Start,
                        false,
                    );
                }
            });
        });
    }
    scene.push(DrawCommand::Scanline(ScanlineCmd {
        rect: Rectangle::from_size(Size::new(w as f64, h as f64)),
        color: theme.accent,
        spacing: 3.0,
        opacity: theme.intensity.scanline_opacity(),
    }));
    scene
}

struct GpuState {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    grid: GridRenderer,
    text: TextRenderer,
    compositor: heca_renderer::composite::Compositor,
    scale_factor: f64,
    theme: Theme,
    ui: Flex,
    /// Host-owned sidebar display mode (G5); `[` toggles expanded ⇄ icon rail.
    sidebar_mode: Signal<RegionMode>,
    /// Workspaces-rail pick state: `p` lights the keycaps, a letter selects, Esc cancels.
    rail_hints: Vec<Signal<Option<String>>>,
    rail_states: Vec<Signal<bool>>,
    rail_letters: Vec<char>,
    rail_pick: bool,
    /// Toggles the centered clip-viewport demo (task B); `c` flips it.
    clip_demo: bool,
    /// A pane's "needs attention" request; `n` fires the pulse + a host beep.
    attention_req: Signal<bool>,
    /// Command-palette open state; `Ctrl+K` opens it.
    palette_open: Signal<bool>,
    /// Context-menu open state + anchor; right-click opens it at the cursor.
    menu_open: Signal<bool>,
    menu_anchor: Signal<Point>,
    /// Host-owned toast render list; `t` pushes one.
    toasts: Signal<Vec<ToastSpec>>,
    /// Whether Ctrl is currently held (for chord shortcuts like Ctrl+K).
    ctrl: bool,
    /// Whether the Cmd/Super (meta) key is held.
    meta: bool,
    /// tmux-style prefix is armed (the next key is a heca command, not the app's).
    prefix_pending: bool,
    /// In the dedicated zoom mode (entered via `prefix +`); `j`/`k` zoom, Esc exits.
    zoom_mode: bool,
    /// Last-applied theme index — when `ctl.theme_idx` differs, load a new theme.
    current_theme_idx: usize,
    ctl: ThemeCtl,
    scroll_y: f32,
    cursor: Point,
    last_frame: Instant,
    /// Seconds until the next scheduled frame: `Some(0.0)` ≈ continuous animation
    /// (capped to ~30fps), `Some(t)` a timed wake (e.g. caret blink), `None` idle.
    next_frame_in: Option<f32>,
    /// When set, the next frame repaints in full (an input event / resize / layout
    /// or scroll change can alter unknown regions). Cleared after the frame; pure
    /// animation frames instead repaint just the widgets that flagged themselves.
    force_full: bool,
    /// Layout is recomputed only when a layout input changed (resize/font/content
    /// event) — not on pure-animation frames.
    layout_dirty: bool,
    /// The size variant last applied to the whole tree (the global SIZE control).
    applied_size: WidgetSize,
    /// The zoom level last applied (relayout when it changes).
    applied_zoom: f32,
    content_h: f32,
    applied_scroll: f32,
    focus: FocusManager,
    shift: bool,
}

impl GpuState {
    async fn new(event_loop: &ActiveEventLoop) -> Self {
        let attrs = Window::default_attributes()
            .with_title(APP_TITLE)
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 420.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        // Bridge widget self-invalidation to the event loop: a widget that marks
        // itself needs-paint wakes the renderer through this (the retained-render
        // foundation — repaint what changed instead of every frame).
        {
            let w = window.clone();
            heca_grid_ui::install_frame_request(move || w.request_redraw());
        }
        let scale_factor = window.scale_factor();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("showcase_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let phys = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: phys.width.max(1),
            height: phys.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        let theme = load_grid_theme("grid_tron");
        let compositor =
            heca_renderer::composite::Compositor::new(&device, format, config.width, config.height);
        let mut grid = GridRenderer::new(&device, format);
        let mut text = TextRenderer::new(&device, format);
        // Both renderers need the scale + physical framebuffer size to map logical
        // clip rects to scissor pixels (clamped in-bounds).
        grid.set_scale_factor(scale_factor);
        grid.set_target_size(config.width, config.height);
        text.set_scale_factor(scale_factor);
        text.set_target_size(config.width, config.height);
        text.set_font_family(&theme.font_family);
        let ctl = ThemeCtl {
            glow: signal(theme.glow_size),
            radius: signal(theme.radius),
            border: signal(theme.border_width),
            font: signal(theme.font_size),
            intensity: signal(theme.intensity),
            size: signal(WidgetSize::Normal),
            zoom: signal(ZOOM_DEFAULT),
            theme_idx: signal(0),
        };
        let built = build_ui(&theme, ctl);
        let BuiltUi {
            ui,
            sidebar_mode,
            rail_hints,
            rail_states,
            rail_letters,
            attention_req,
            palette_open,
            menu_open,
            menu_anchor,
            toasts,
        } = built;

        Self {
            window,
            device,
            queue,
            surface,
            config,
            grid,
            text,
            compositor,
            scale_factor,
            theme,
            ui,
            sidebar_mode,
            rail_hints,
            rail_states,
            rail_letters,
            rail_pick: false,
            clip_demo: false,
            attention_req,
            palette_open,
            menu_open,
            menu_anchor,
            toasts,
            ctrl: false,
            meta: false,
            prefix_pending: false,
            zoom_mode: false,
            current_theme_idx: 0,
            ctl,
            scroll_y: 0.0,
            cursor: Point::new(-1.0, -1.0),
            last_frame: Instant::now(),
            next_frame_in: None,
            force_full: true, // first frame paints everything
            layout_dirty: true,
            applied_size: WidgetSize::Normal,
            applied_zoom: ZOOM_DEFAULT,
            content_h: 0.0,
            applied_scroll: 0.0,
            focus: FocusManager::new(),
            shift: false,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        // Keep the renderers' scissor-clamp size + the persistent scene texture in
        // sync with the framebuffer.
        self.grid
            .set_target_size(self.config.width, self.config.height);
        self.text
            .set_target_size(self.config.width, self.config.height);
        self.compositor
            .resize(&self.device, self.config.width, self.config.height);
        self.layout_dirty = true; // window size feeds layout
        self.window.request_redraw();
    }

    /// Light up (or clear) the workspaces-rail pick keycaps — the move/swap/select
    /// prefix. The host writes the per-cell hint signals; a real app would also
    /// reach this from RPC, hence the signal-driven path (input parity).
    fn set_rail_pick(&mut self, on: bool) {
        self.rail_pick = on;
        for (i, h) in self.rail_hints.iter().enumerate() {
            h.set(on.then(|| self.rail_letters[i].to_string()));
        }
    }

    /// Resolve a pressed letter to its rail cell: select that pane + end the pick.
    fn rail_pick_select(&mut self, c: char) {
        if let Some(i) = self.rail_letters.iter().position(|&l| l == c) {
            for (j, s) in self.rail_states.iter().enumerate() {
                s.set(j == i);
            }
            self.set_rail_pick(false);
        }
    }

    /// The logical→physical scale actually used: the window's HiDPI factor times the
    /// global UI zoom. Everything (layout viewport, glyph rasterization, scissor +
    /// cursor mapping) goes through this, so changing zoom scales the whole UI.
    fn effective_scale(&self) -> f64 {
        let level = self.ctl.zoom.get_untracked();
        self.scale_factor * ZOOM_RATIO.powf(level - ZOOM_DEFAULT) as f64
    }

    /// The platform "accelerator" modifier: Cmd (⌘) on macOS, Ctrl elsewhere — so
    /// zoom uses the native chord (⌘ +/-/0 on macOS, Ctrl +/-/0 on Windows/Linux).
    fn accel(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.meta
        } else {
            self.ctrl
        }
    }

    /// Nudge the zoom level (keys / accel+wheel), clamped to the dial range.
    fn nudge_zoom(&self, delta: f32) {
        let level = (self.ctl.zoom.get_untracked() + delta).clamp(ZOOM_MIN, ZOOM_MAX);
        self.ctl.zoom.set(level);
        self.window.request_redraw();
    }

    /// Reset zoom to the neutral level.
    fn reset_zoom(&self) {
        self.ctl.zoom.set(ZOOM_DEFAULT);
        self.window.request_redraw();
    }

    fn enter_zoom_mode(&mut self) {
        self.zoom_mode = true;
        self.window.set_title(&format!(
            "{APP_TITLE} — ZOOM  (k/+ in · j/- out · 0 reset · Esc exit)"
        ));
        self.window.request_redraw();
    }

    fn exit_zoom_mode(&mut self) {
        self.zoom_mode = false;
        self.window.set_title(APP_TITLE);
        self.window.request_redraw();
    }

    /// A key pressed while in zoom mode. The mode stays active (so you can keep
    /// pressing j/k) until Esc/Enter/q.
    fn zoom_mode_key(&mut self, gk: GridKey) {
        match gk {
            GridKey::Char('k' | '+' | '=') => self.nudge_zoom(ZOOM_STEP),
            GridKey::Char('j' | '-' | '_') => self.nudge_zoom(-ZOOM_STEP),
            GridKey::Char('0') => self.reset_zoom(),
            GridKey::Escape | GridKey::Enter | GridKey::Char('q') => self.exit_zoom_mode(),
            _ => {} // swallow other keys while the mode is held
        }
    }

    /// The key after the tmux-style prefix: a heca command (the showcase only wires
    /// the zoom mode; the real app dispatches its full keymap here).
    fn prefix_command(&mut self, gk: GridKey) {
        self.prefix_pending = false;
        if matches!(gk, GridKey::Char('+' | '=')) {
            self.enter_zoom_mode();
        }
    }

    fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        let animating = self.ui.tick(dt);

        // Zoom-aware scale: a bigger effective scale shrinks the logical viewport, so
        // the same widgets render larger (a global zoom). Feed it to both renderers
        // for crisp rasterization at the zoomed size.
        let phys = self.window.inner_size();
        let eff = self.effective_scale();
        self.grid.set_scale_factor(eff);
        self.text.set_scale_factor(eff);
        let scale = eff as f32;
        let (w, h) = (phys.width as f32 / scale, phys.height as f32 / scale);

        self.grid.set_screen_size(&self.queue, w, h);
        self.text.set_screen_size(&self.queue, w, h);

        // Theme switching: when the theme selection changes, load the new theme
        // and rebuild the showcase tree so every widget picks up the new palette.
        let theme_idx = self.ctl.theme_idx.get_untracked();
        if theme_idx != self.current_theme_idx {
            self.current_theme_idx = theme_idx;
            self.theme = load_grid_theme(THEME_NAMES[theme_idx]);
            // Reset control signals to match the new theme's baked-in values.
            self.ctl.glow.set(self.theme.glow_size);
            self.ctl.radius.set(self.theme.radius);
            self.ctl.border.set(self.theme.border_width);
            self.ctl.font.set(self.theme.font_size);
            self.ctl.intensity.set(self.theme.intensity);

            let built = build_ui(&self.theme, self.ctl);
            let BuiltUi {
                ui,
                sidebar_mode,
                rail_hints,
                rail_states,
                rail_letters,
                attention_req,
                palette_open,
                menu_open,
                menu_anchor,
                toasts,
            } = built;
            self.ui = ui;
            self.sidebar_mode = sidebar_mode;
            self.rail_hints = rail_hints;
            self.rail_states = rail_states;
            self.rail_letters = rail_letters;
            self.attention_req = attention_req;
            self.palette_open = palette_open;
            self.menu_open = menu_open;
            self.menu_anchor = menu_anchor;
            self.toasts = toasts;
            self.focus.clear(&mut self.ui);
            self.scroll_y = 0.0;
            self.applied_scroll = 0.0;
            self.layout_dirty = true;
            self.force_full = true;
            self.window
                .set_title(&format!("{} — {}", APP_TITLE, self.theme.name));
        }

        // Fold live theme controls in (paint-only except font, which reflows layout).
        self.theme.glow_size = self.ctl.glow.get_untracked();
        self.theme.radius = self.ctl.radius.get_untracked();
        self.theme.border_width = self.ctl.border.get_untracked();
        self.theme.intensity = self.ctl.intensity.get_untracked();
        let font = self.ctl.font.get_untracked();
        if (font - self.theme.font_size).abs() > f32::EPSILON {
            self.theme.font_size = font;
            self.layout_dirty = true;
        }
        // Global SIZE control: push the chosen variant onto every widget + reflow.
        let size = self.ctl.size.get_untracked();
        if size != self.applied_size {
            apply_size(&mut self.ui, size);
            self.applied_size = size;
            self.layout_dirty = true;
        }
        // Global ZOOM control: the effective scale changed the logical viewport, so
        // relayout against the new (w, h).
        let zoom = self.ctl.zoom.get_untracked();
        if (zoom - self.applied_zoom).abs() > f32::EPSILON {
            self.applied_zoom = zoom;
            self.layout_dirty = true;
        }

        // Recompute layout ONLY when an input changed it (resize / font / content
        // event) — never on pure-animation frames. The full per-frame taffy
        // relayout was the bulk of the render() CPU.
        if self.layout_dirty {
            self.ui.base_mut().style.width = Length::Px(w);
            self.ui.base_mut().style.height = Length::Auto;
            LayoutEngine::new()
                .base_font(self.theme.font_size)
                .compute(&mut self.ui, Size::new(w as f64, 100_000.0));
            self.content_h = self.ui.base().bounds.size.h as f32;
            self.applied_scroll = 0.0;
            self.layout_dirty = false;
            self.force_full = true; // relayout moves everything → repaint in full
        }
        // Whole-page scroll: shift the cached tree by only the delta since the
        // offset already baked into its bounds (window edges clip).
        self.scroll_y = self.scroll_y.clamp(0.0, (self.content_h - h).max(0.0));
        let scroll_delta = (self.applied_scroll - self.scroll_y) as f64;
        if scroll_delta != 0.0 {
            offset_tree(&mut self.ui, scroll_delta);
            self.applied_scroll = self.scroll_y;
            self.force_full = true; // the whole page shifted
        }

        let scene = build_scene(&self.ui, &self.theme, w, h, self.clip_demo);

        // Damage region for this frame: full on an input/layout/scroll change (it can
        // alter unknown regions), otherwise just the widgets that flagged themselves
        // needs-paint (a spinner, the caret) — so an animation repaints only its rect.
        // `collect_damage` also clears the flags.
        let dirty = heca_grid_ui::collect_damage(&self.ui);
        let damage: Option<[f32; 4]> = if std::mem::take(&mut self.force_full) {
            None // whole frame
        } else {
            match dirty {
                Some(r) => Some([
                    r.loc.x as f32,
                    r.loc.y as f32,
                    r.size.w as f32,
                    r.size.h as f32,
                ]),
                // Animated but reported no damage (an un-wired continuous animation)
                // → repaint in full for correctness; nothing dirty → empty rect
                // (render nothing; the blit shows the preserved scene texture).
                None if animating => None,
                None => Some([0.0, 0.0, 0.0, 0.0]),
            }
        };
        self.grid.set_damage(damage);
        self.text.set_damage(damage);

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("showcase"),
            });

        // Render the UI into the compositor's persistent scene texture (not directly
        // to the swapchain), then blit it to screen. The persistent texture is what
        // makes damage-region redraw possible — unchanged pixels survive between
        // frames, so a frame can re-render only the damaged region.
        let scene_view = self.compositor.scene_view();

        // Each pass is a rects-then-text flush (the renderer draws all queued rects,
        // then all queued text). Base first, then each overlay as its *own* pass — so
        // overlay content occludes base text (not just base rects), AND a later overlay
        // occludes an earlier one. Flushing every overlay in a single pass would draw
        // all overlay rects then all overlay text, letting a lower overlay's text bleed
        // over a higher overlay's panel (the overlapping-overlay text-bleed bug).
        // Reset the renderers' per-frame buffer offsets so the base + overlay passes
        // append to the persistent buffers (no per-frame staging allocation).
        self.grid.begin_frame();
        self.text.begin_frame();
        let glow_alpha_scale =
            heca_renderer::scene::glow_alpha_scale_for_background(self.theme.background.to_f32x4());
        enqueue_scene(
            &mut self.grid,
            &mut self.text,
            &scene.base_layer(),
            glow_alpha_scale,
        );
        self.grid.render(&self.queue, scene_view, &mut encoder);
        self.text
            .render(&self.queue, scene_view, &mut encoder, None);
        for overlay in scene.overlay_segments() {
            enqueue_scene(&mut self.grid, &mut self.text, &overlay, glow_alpha_scale);
            self.grid.render(&self.queue, scene_view, &mut encoder);
            self.text
                .render(&self.queue, scene_view, &mut encoder, None);
        }
        // Blit the composited scene onto the swapchain.
        self.compositor.blit(&view, &mut encoder);
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        // Decide when the next frame is needed: a continuous animation (spinner,
        // slide) runs at the ~30fps cap; otherwise sleep until the soonest timed
        // redraw a widget asks for (e.g. a focused caret's next blink) — or, if
        // nothing is pending, wait for input. This keeps a focused idle Input from
        // pegging a core at the full frame rate just to blink twice a second.
        self.next_frame_in = if animating {
            Some(0.0)
        } else {
            self.ui.next_redraw()
        };
    }
}

#[derive(Default)]
struct App {
    state: Option<GpuState>,
}

impl ApplicationHandler for App {
    /// When the capped-frame timer (set via `WaitUntil`) fires, request the next
    /// animation frame.
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::ResumeTimeReached { .. } = cause
            && let Some(state) = &self.state
            && state.next_frame_in.is_some()
        {
            state.window.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let state = pollster::block_on(GpuState::new(event_loop));
            state.window.request_redraw();
            self.state = Some(state);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        // Any non-redraw event (input, resize, …) can change unknown regions, so the
        // frame it triggers repaints in full. Animation/timed frames arrive as a bare
        // RedrawRequested and instead repaint only the widgets that flagged themselves.
        if !matches!(event, WindowEvent::RedrawRequested) {
            state.force_full = true;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                // Map through the zoom-aware scale so hit-testing matches the zoomed
                // layout (else clicks land on the wrong widgets when zoomed).
                let eff = state.effective_scale();
                state.cursor = Point::new(position.x / eff, position.y / eff);
                // The OS manages the cursor (arrow in content, resize at the
                // decorated window's edges) — don't override it.
                // Route hover through grid-ui: an open overlay gets first dibs but
                // only swallows it if it consumes it (an input-grabbing Modal/dropdown
                // returns Yes so items behind don't hover; the ToastStack returns No so
                // buttons behind it still hover/animate while toasts show), otherwise
                // it falls to the widget under the cursor.
                state
                    .focus
                    .dispatch(&mut state.ui, &Event::PointerMoved { pos: state.cursor });
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Route the click through grid-ui: an open overlay (e.g. a Select
                // dropdown) gets first dibs so it can capture clicks on rows outside
                // its layout bounds; otherwise dispatch focuses the clicked widget
                // (clearing focus on a miss) and delivers the press.
                state
                    .focus
                    .dispatch(&mut state.ui, &Event::PointerPressed { pos: state.cursor });
                state.layout_dirty = true; // a click can change content/size
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                // Right-click opens the context menu at the cursor (the host decides
                // *where* and *when*; the widget just renders + captures input).
                state.menu_anchor.set(state.cursor);
                state.menu_open.set(true);
                state.layout_dirty = true;
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                // Deliver the release so a grabbed widget (e.g. a ScrollRegion
                // thumb drag) can end its grab — without this the drag never stops.
                state
                    .focus
                    .dispatch(&mut state.ui, &Event::PointerReleased { pos: state.cursor });
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Lines to scroll the open dropdown (positive = down the list).
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 20.0,
                };
                if state.zoom_mode || state.accel() {
                    // Wheel zooms the whole UI while in zoom mode, or with the
                    // accelerator held (scroll up = zoom in).
                    state.nudge_zoom(-lines * ZOOM_STEP);
                } else {
                    // Route scroll through grid-ui: an open overlay (Select dropdown /
                    // Modal) gets it first but only swallows it if it consumes it — a
                    // non-scrolling overlay like the ToastStack lets it fall through.
                    // When nothing in the tree consumes it, scroll the whole page.
                    if state
                        .focus
                        .dispatch(&mut state.ui, &Event::Scroll { delta: lines })
                        == Handled::No
                    {
                        state.scroll_y += lines * 40.0;
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                state.shift = s.shift_key();
                state.ctrl = s.control_key();
                state.meta = s.super_key();
                // Broadcast to the tree so text widgets can do word-wise editing
                // (and the command palette can track Ctrl for Ctrl+J/K nav).
                state.ui.event(&Event::ModifiersChanged(Modifiers {
                    ctrl: s.control_key(),
                    alt: s.alt_key(),
                    shift: s.shift_key(),
                    meta: s.super_key(),
                }));
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Some(gk) = to_grid_key(&event.logical_key) {
                    match gk {
                        // ── tmux-style prefix + modes (checked FIRST) ──
                        // heca hosts other apps, so our chords go through a prefix
                        // (default Ctrl+B); bare keys fall through to the hosted app.
                        // Zoom mode owns j/k/+/-/0 until Esc — see `prefix +`.
                        _ if state.zoom_mode => state.zoom_mode_key(gk),
                        _ if state.prefix_pending => state.prefix_command(gk),
                        GridKey::Char('b') if state.ctrl => state.prefix_pending = true,
                        // An open overlay gets first dibs on keys, but only swallows
                        // the ones it actually consumes: a Modal/palette eats every
                        // key (Esc/Enter/typing), while the ToastStack eats none — so
                        // global keys (`t`, `[`, …) still work while toasts show.
                        gk if state.focus.offer_to_overlay(
                            &mut state.ui,
                            &Event::Key {
                                key: gk,
                                pressed: true,
                            },
                        ) == Handled::Yes => {}
                        // Ctrl+K opens the command palette (a host-bound chord).
                        GridKey::Char('k') if state.ctrl => {
                            state.palette_open.set(true);
                        }
                        // Tab / Shift+Tab move keyboard focus across buttons.
                        GridKey::Tab => state.focus.advance(&mut state.ui, !state.shift),
                        // Escape clears focus and cancels an open rail pick.
                        GridKey::Escape => {
                            state.set_rail_pick(false);
                            state.focus.clear(&mut state.ui);
                        }
                        // `[` collapses the sidebar to its icon rail (G5) and back,
                        // but only when nothing is focused — so it still types into
                        // a focused Input.
                        GridKey::Char('[') if state.focus.focused().is_none() => {
                            let next = match state.sidebar_mode.get_untracked() {
                                RegionMode::CollapsedRail => RegionMode::Expanded,
                                _ => RegionMode::CollapsedRail,
                            };
                            state.sidebar_mode.set(next);
                        }
                        // `p` opens/closes a workspaces-rail pick: every pane cell
                        // shows its letter keycap (the generic KeyHint overlay).
                        GridKey::Char('p') if state.focus.focused().is_none() => {
                            let on = !state.rail_pick;
                            state.set_rail_pick(on);
                        }
                        // While a pick is open, a letter selects its pane cell.
                        GridKey::Char(c) if state.rail_pick => state.rail_pick_select(c),
                        // `c` toggles the centered clip-viewport demo (task B).
                        GridKey::Char('c') if state.focus.focused().is_none() => {
                            state.clip_demo = !state.clip_demo;
                        }
                        // `n` fires a "needs attention" pulse on a pane. The widget
                        // flashes; the *host* plays the sound (grid-ui is audio-free)
                        // — here, the terminal bell.
                        GridKey::Char('n') if state.focus.focused().is_none() => {
                            state.attention_req.set(true);
                            print!("\x07");
                            use std::io::Write;
                            let _ = std::io::stdout().flush();
                        }
                        // `t` pushes a new toast onto the host-owned list; the
                        // ToastStack slides it in, and × dismisses (removes the id).
                        GridKey::Char('t') if state.focus.focused().is_none() => {
                            state.toasts.update(|v| {
                                let id = v.iter().map(|s| s.id).max().unwrap_or(0) + 1;
                                v.push(
                                    ToastSpec::new(id, format!("Event #{id}"))
                                        .severity(ToastSeverity::Info)
                                        .body("Pushed with the `t` key")
                                        .action("View"),
                                );
                            });
                            state.window.request_redraw();
                        }
                        // Space/Enter (and others) go to the focused widget.
                        other => {
                            state.focus.deliver_key(&mut state.ui, other);
                        }
                    }
                    state.layout_dirty = true; // a key can change content/size
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                // Throttle to the ~30fps cap. A widget that invalidates itself (the
                // spinner) requests a redraw immediately via `mark_needs_paint`, which
                // would otherwise render at full vsync. Input/resize frames (force_full)
                // still render at once for responsiveness.
                let min = Duration::from_millis(33);
                let elapsed = Instant::now().saturating_duration_since(state.last_frame);
                if !state.force_full && elapsed < min {
                    event_loop.set_control_flow(ControlFlow::WaitUntil(state.last_frame + min));
                    return;
                }
                state.render();
                // Schedule the next frame: a continuous animation runs at the ~30fps
                // cap, a timed wake (e.g. caret blink) sleeps until its next change,
                // and a fully idle UI waits for input.
                match state.next_frame_in {
                    Some(secs) => {
                        let delay = Duration::from_secs_f32(secs.max(0.033));
                        event_loop
                            .set_control_flow(ControlFlow::WaitUntil(state.last_frame + delay));
                    }
                    None => event_loop.set_control_flow(ControlFlow::Wait),
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run");
}
