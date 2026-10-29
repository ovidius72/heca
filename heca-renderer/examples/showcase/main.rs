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
use std::sync::Arc;
use std::time::{Duration, Instant};

use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{DrawCommand, ScanlineCmd};
use heca_grid_ui::widgets::{ContextMenu, KeyCap, Menu, NfGlyph, NfIcon, Overlay};
use heca_grid_ui::{
    Component, Event, LayoutEngine, PaintCx, Panel, Point, RawPointer, RawPointerKind, Rectangle,
    Scene, Size,
};
use heca_renderer::grid::GridRenderer;
use heca_renderer::scene::enqueue_scene;
use heca_renderer::text::TextRenderer;
use heca_view::build;
use heca_view::{Intent, PropValue, ViewGlyph};
use heca_view_realize::{FormBindings, IntentEmitter, realize};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, StartCause, WindowEvent};

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
        cx.rect(
            b,
            cx.theme().colors.surface,
            None,
            cx.theme().colors.border_radius,
            None,
        );
        if self.swap {
            cx.swap_indicator(b);
        } else {
            cx.drop_indicator(b, heca_grid_ui::drag::DropSide::After);
        }
    }
}

impl LayoutExt for IndicatorSwatch {}

/// Map a winit logical key onto the renderer-agnostic `GridKey`.
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

/// Overlay-panel frame options, in the order shown by the OVERLAY FRAME select.
/// This mirrors the app's `[appearance] overlay_border_style` setting — which the
/// showcase cannot read (it builds its own `Theme` and never loads `config.toml`),
/// so the control writes the same `theme.colors.overlay_frame` token the config
/// path writes in the real app.
const OVERLAY_FRAME_OPTS: [FrameStyle; 3] = [
    FrameStyle::Bracketed,
    FrameStyle::Bordered,
    FrameStyle::None,
];

/// Size-select options, in dropdown order (`NORMAL`, `SMALL`, `LARGE`, `HEADER`).
const SIZE_OPTS: [WidgetSize; 4] = [
    WidgetSize::Normal,
    WidgetSize::Small,
    WidgetSize::Large,
    WidgetSize::Header,
];

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

/// Map a `heca_theme::Theme` onto a grid-ui `Theme`.
///
/// Font family/size come from the default [`heca_config::font::FontConfig`] —
/// fonts are decoupled from the color theme (see `compositor-04c`).
fn heca_theme_to_grid_ui(ht: &heca_theme::Theme) -> Theme {
    let font_config = heca_config::font::FontConfig::default();
    Theme {
        colors: ht.clone(),
        font_family: font_config.family.ui_normal().to_string(),
        font_size: font_config.size.ui,
        // Config's own default focus-outline width.
        focus_border_width: heca_config::appearance::DEFAULT_FOCUS_BORDER_WIDTH,
        // The picker's own size and colour — one of each, whatever a letter sits on.
        hint_font_size: heca_config::appearance::AppearanceConfig::default().hint_font_size,
        hint_color: ht.accent,
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
    /// Frame drawn around overlay panels (dialog / dropdown / menu / palette) —
    /// the showcase stand-in for `[appearance] overlay_border_style`.
    overlay_frame: Signal<FrameStyle>,
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
    c.base_mut().style.layout.size = size;
    for child in c.base_mut().children.iter_mut() {
        apply_size(child.as_mut(), size);
    }
}

/// An option whose content is an icon **and** a label — the thing a `Vec<String>` of options could
/// never express. `Choice` lays them out in a row with a theme-derived gap, and tints both together
/// when the option is chosen.
fn level_option(value: &str, glyph: Glyph, label: &str) -> Choice {
    Choice::new(value)
        .child(Icon::new(glyph))
        .child(Label::new(label))
}

/// The tree the **described** column renders: a titled `Panel` holding a clickable `Row`, a rule,
/// and a line whose colour is overridden by **token name** (not a hex literal, so it follows the
/// theme — press `1`/`2`/`3` and watch it change with everything else).
///
/// Pure data. No builder, no closure: the `Row`'s click carries an `Intent`, which is exactly what
/// a plugin would ship over the boundary.
struct BuiltUi {
    /// The page tree: a whole-window two-axis `ScrollRegion` whose single child is the demo column.
    ui: ScrollRegion,
    /// The overlay layer ABOVE the page scroll: viewport-anchored widgets (Dialog /
    /// CommandPalette / ContextMenu / ToastStack) live here so they center/anchor on
    /// the real window and never scroll or clip with the page (BUG B). Laid out and
    /// dispatched separately, painted after `ui` — the showcase's minimal stand-in
    /// for the app's layer stack (`LayerRegistry`).
    overlays: Flex,
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

struct GpuState {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    grid: GridRenderer,
    text: TextRenderer,
    ui_images: heca_renderer::ui_image::UiImages,
    compositor: heca_renderer::composite::Compositor,
    scale_factor: f64,
    theme: Theme,
    /// The page tree: a whole-window two-axis `ScrollRegion` wrapping the demo column.
    ui: ScrollRegion,
    /// The overlay layer above the page scroll (Dialog / palette / menu / toasts):
    /// viewport-sized, dispatched before and painted after `ui`.
    overlays: Flex,
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
    alt: bool,
    /// Whether the Cmd/Super (meta) key is held.
    meta: bool,
    /// tmux-style prefix is armed (the next key is a heca command, not the app's).
    prefix_pending: bool,
    /// In the dedicated zoom mode (entered via `prefix +`); `j`/`k` zoom, Esc exits.
    zoom_mode: bool,
    /// Last-applied theme index — when `ctl.theme_idx` differs, load a new theme.
    current_theme_idx: usize,
    ctl: ThemeCtl,
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
    focus: FocusManager,
    /// Focus/dispatch state for the overlay layer tree (`overlays`) — its own
    /// manager so overlay routing never disturbs the page tree's focus.
    focus_ov: FocusManager,
    shift: bool,
    /// The widget keymap (`widget-keys-config`) — this demo host uses the built-in defaults;
    /// the heca app builds its own from `[keys.widgets]`.
    keymap: Keymap,
}

#[derive(Default)]
struct App {
    state: Option<GpuState>,
}


mod described;
mod page;
mod scene;
mod gpu;
mod app;
use described::*;
use page::build_ui;
use scene::build_scene;

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).expect("run");
}


#[cfg(test)]
mod sweeps;
