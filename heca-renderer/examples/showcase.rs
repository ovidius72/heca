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
use std::time::Instant;

use heca_grid_ui::prelude::*;
use heca_grid_ui::scene::{BracketCmd, DrawCommand, Glow, ScanlineCmd};
use heca_grid_ui::{Component, Event, LayoutEngine, PaintCx, Point, Rectangle, Scene, Size};
use heca_renderer::grid::GridRenderer;
use heca_renderer::scene::enqueue_scene;
use heca_renderer::text::TextRenderer;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};

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
const INTENSITY_OPTS: [Intensity; 4] =
    [Intensity::Off, Intensity::Low, Intensity::Medium, Intensity::Heavy];

#[derive(Clone, Copy)]
struct ThemeCtl {
    glow: Signal<GlowLevel>,
    radius: Signal<f32>,
    border: Signal<f32>,
    font: Signal<f32>,
    intensity: Signal<Intensity>,
}

fn build_ui(theme: &Theme, ctl: ThemeCtl) -> (Flex, Signal<RegionMode>) {
    // Host-owned sidebar display mode (G5): the keymap toggles it (`[`), the
    // region sizes to it, and its rail-aware Docks fold to icons when collapsed.
    let sidebar_mode = signal(RegionMode::Expanded);
    // Initial positions for the control selects, read from the current control
    // values — so the selects stay in sync if the tree is rebuilt (on font change).
    let radius_opts = [0.0f32, 4.0, 8.0, 16.0];
    let border_opts = [0.0f32, 1.0, 2.0, 3.0];
    let font_opts = [8.0f32, 10.0, 12.0, 15.0, 20.0, 28.0];
    let near = |arr: &[f32], v: f32, dflt: usize| {
        arr.iter().position(|x| (x - v).abs() < 0.01).unwrap_or(dflt)
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

    let card = |title: &str, value: &str| {
        Card::new(title)
            .width(Length::Px(220.0))
            .height(Length::Px(140.0))
            .background(theme.surface)
            .border(theme.accent, 1.5)
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
                .child(Button::destructive("DESTRUCTIVE").on_click(click("DESTRUCTIVE"))),
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
                    Select::new(["OFF", "LOW", "MEDIUM", "HEAVY"]).selected(intensity_idx).on_change(
                        move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.intensity.set(INTENSITY_OPTS[i.min(3)]);
                            }
                        },
                    ),
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
                                ctl.glow.set(GlowLevel::ALL[i.min(GlowLevel::ALL.len() - 1)]);
                            }
                        }),
                )
                .child(Label::new("RADIUS").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "4", "8", "16"]).selected(radius_idx).on_change(move |a| {
                        if let SignalData::Usize(i) = a.data {
                            ctl.radius.set(radius_opts[i.min(3)]);
                        }
                    }),
                )
                .child(Label::new("BORDER").color(theme.muted).font_scale(0.85))
                .child(
                    Select::new(["0", "1", "2", "3"]).selected(border_idx).on_change(move |a| {
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
                    Select::new(["8", "10", "12", "15", "20", "28"]).selected(font_idx).on_change(
                        move |a| {
                            if let SignalData::Usize(i) = a.data {
                                ctl.font.set(font_opts[i.min(font_opts.len() - 1)]);
                            }
                        },
                    ),
                )
                .child(Input::new().value("SIZED"))
                .child(Select::new(["ALPHA", "BETA", "GAMMA"]))
                .child(Label::new("Aa").color(theme.foreground)),
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
                        .trailing(Label::new("CMD P").color(theme.muted).font_scale(0.8))
                        .trailing_bordered(true),
                ))
                .child(select(
                    2,
                    Item::new("SETTINGS")
                        .marker(ActiveMarker::Bar)
                        .trailing(Label::new("CMD ,").color(theme.muted).font_scale(0.8))
                        .trailing_bordered(true),
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
                .child(Icon::new(Glyph::Terminal).color(theme.foreground).size(34.0))
                .child(Icon::new(Glyph::Gear).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::Lightning).color(theme.accent).size(34.0))
                .child(Icon::new(Glyph::Warning).color(theme.warning).size(34.0)),
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

            // A pane "card" — a clickable, single-selectable `Row` (focus + Enter
            // + hover/active highlight) carrying composed two-line content over a
            // persistent state-tinted background. Demonstrates that rich composed
            // rows, not just `Item`s, can be interactive.
            let pane_sel = signal(0usize);
            let pane_states: Rc<RefCell<Vec<Signal<bool>>>> = Rc::new(RefCell::new(Vec::new()));
            let pane = move |icon: Icon, color: Color, title: &str, branch: &str, info: &str, tag: Badge| -> Row {
                let row = Row::new()
                    .background(color.with_alpha(22))
                    .radius(theme.control_radius())
                    .padding(10.0)
                    .child(
                        Grid::new()
                            .grow(1.0)
                            .columns([Track::Px(22.0), Track::Fr(1.0), Track::Auto])
                            .rows([Track::Auto, Track::Auto])
                            // icon · title · tag share the title row; the subtitle
                            // sits under the title, the flanking cells left empty.
                            .areas(["dot title tag", ". sub ."])
                            .gap(4.0)
                            .area(icon, "dot")
                            .area(Label::new(title).color(color), "title")
                            // G8 recipe: the branch is a composed, *multi-segment*
                            // Tag — git-branch icon + branch │ change info — not
                            // plain text. Wrapped in a row so the chip hugs its
                            // content (left) instead of stretching to fill the cell.
                            .area(
                                Flex::row().child(
                                    Tag::new(branch)
                                        .leading(Icon::new(Glyph::GitBranch).size(13.0).color(color))
                                        .segment_text(info, None)
                                        .color(color),
                                ),
                                "sub",
                            )
                            .area(tag, "tag"),
                    );
                let i = pane_states.borrow().len();
                pane_states.borrow_mut().push(row.state());
                let pane_states = pane_states.clone();
                row.on_activate(move || {
                    pane_sel.set(i);
                    for (j, s) in pane_states.borrow().iter().enumerate() {
                        s.set(j == i);
                    }
                })
            };

            // G4 DockFrame framing G3 ItemGroups; header slot carries a count
            // badge. `.rail(...)` makes it fold to a single icon when the sidebar
            // collapses to its rail (G5) — bound to the host-owned mode signal.
            let explorer = DockFrame::new("EXPLORER")
                .rail(sidebar_mode, Glyph::FolderOpen)
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
                        .child(file("main.rs", Icon::new(Glyph::FileCode).color(theme.accent).size(18.0)))
                        .child(file("chrome_region.rs", Icon::new(Glyph::FileCode).color(theme.foreground).size(18.0)))
                        .child(file("dock_frame.rs", Icon::new(Glyph::FileCode).color(theme.foreground).size(18.0))),
                )
                .child(
                    // Collapsed group — verifies the paint fix: its rows must not
                    // bleed to the top-left while hidden.
                    ItemGroup::new("tests")
                        .expanded(false)
                        .child(file("phase_a.rs", Icon::new(Glyph::FileCode).color(theme.muted).size(18.0))),
                );
            // Git status rows: a state-colored duotone icon + change-kind badge.
            let git_row = |icon: Icon, name: &str, tag: Badge| {
                Item::new(name).leading(icon).trailing(tag)
            };
            let source_control = DockFrame::new("SOURCE CONTROL")
                .rail(sidebar_mode, Glyph::GitBranch)
                .header(
                    Flex::row()
                        .align(Align::Center)
                        .child(Badge::warning("3"))
                        .child(Flex::row().width(Length::Px(14.0))),
                )
                .child(git_row(Icon::new(Glyph::GitBranch).color(theme.warning).size(18.0), "chrome_region.rs", Badge::warning("M")))
                .child(git_row(Icon::new(Glyph::Plus).color(theme.success).size(18.0), "showcase.rs", Badge::success("A")))
                .child(git_row(Icon::new(Glyph::Minus).color(theme.danger).size(18.0), "old_sidebar.rs", Badge::danger("D")));

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

            // PANES: each row a state-tinted card, its tag aligned to the title line.
            let panes = DockFrame::new("PANES")
                .child(pane(
                    Icon::new(Glyph::Terminal).color(theme.success).size(20.0),
                    theme.success,
                    "Pane 1 (nvim)",
                    "my-branch",
                    "1+",
                    Badge::success("RUN"),
                ))
                .child(pane(
                    Icon::new(Glyph::GitPullRequest).color(theme.warning).size(20.0),
                    theme.warning,
                    "Review (diff)",
                    "my-branch",
                    "· 2d",
                    Badge::warning("IDLE"),
                ))
                .child(pane(
                    Icon::new(Glyph::Warning).color(theme.danger).size(20.0),
                    theme.danger,
                    "build",
                    "main",
                    "exit 1",
                    Badge::danger("STOP"),
                ));
            let panes_col = Flex::column().width(Length::Px(380.0)).child(panes);

            Flex::row().gap(28.0).align(Align::Start).child(sidebar).child(panes_col)
        });
    (ui, sidebar_mode)
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
fn build_scene(root: &dyn Component, theme: &Theme, w: f32, h: f32) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx =
            PaintCx::new(&mut scene, theme).with_viewport(Size::new(w as f64, h as f64));
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
    scale_factor: f64,
    theme: Theme,
    ui: Flex,
    /// Host-owned sidebar display mode (G5); `[` toggles expanded ⇄ icon rail.
    sidebar_mode: Signal<RegionMode>,
    ctl: ThemeCtl,
    scroll_y: f32,
    cursor: Point,
    last_frame: Instant,
    focus: FocusManager,
    shift: bool,
}

impl GpuState {
    async fn new(event_loop: &ActiveEventLoop) -> Self {
        let attrs = Window::default_attributes()
            .with_title("heca-grid-ui showcase")
            .with_inner_size(winit::dpi::LogicalSize::new(900.0, 420.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
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

        let theme = Theme::grid_tron();
        let grid = GridRenderer::new(&device, format);
        let mut text = TextRenderer::new(&device, format);
        text.set_scale_factor(scale_factor);
        text.set_font_family(&theme.font_family);
        let ctl = ThemeCtl {
            glow: signal(theme.glow_size),
            radius: signal(theme.radius),
            border: signal(theme.border_width),
            font: signal(theme.font_size),
            intensity: signal(theme.intensity),
        };
        let (ui, sidebar_mode) = build_ui(&theme, ctl);

        Self {
            window,
            device,
            queue,
            surface,
            config,
            grid,
            text,
            scale_factor,
            theme,
            ui,
            sidebar_mode,
            ctl,
            scroll_y: 0.0,
            cursor: Point::new(-1.0, -1.0),
            last_frame: Instant::now(),
            focus: FocusManager::new(),
            shift: false,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.window.request_redraw();
    }

    fn render(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        let animating = self.ui.tick(dt);

        let phys = self.window.inner_size();
        let scale = self.scale_factor as f32;
        let (w, h) = (phys.width as f32 / scale, phys.height as f32 / scale);

        self.grid.set_screen_size(&self.queue, w, h);
        self.text.set_screen_size(&self.queue, w, h);

        // Fold live theme controls in before painting.
        self.theme.glow_size = self.ctl.glow.get_untracked();
        self.theme.radius = self.ctl.radius.get_untracked();
        self.theme.border_width = self.ctl.border.get_untracked();
        self.theme.intensity = self.ctl.intensity.get_untracked();
        self.theme.font_size = self.ctl.font.get_untracked();

        // Lay the tree out at its natural (content) height — which may exceed the
        // window — then translate it up by the scroll offset. The window edges do
        // the clipping (the renderer has no scissor yet), so this is a whole-page
        // scroll, not an embedded scroll region. The engine resolves every widget's
        // font from `base_font` (= theme.font_size), so a font change reflows the
        // whole tree live — no per-widget wiring, no rebuild.
        self.ui.base_mut().style.width = Length::Px(w);
        self.ui.base_mut().style.height = Length::Auto;
        LayoutEngine::new()
            .base_font(self.theme.font_size)
            .compute(&mut self.ui, Size::new(w as f64, 100_000.0));
        let content_h = self.ui.base().bounds.size.h as f32;
        self.scroll_y = self.scroll_y.clamp(0.0, (content_h - h).max(0.0));
        offset_tree(&mut self.ui, -(self.scroll_y as f64));

        let scene = build_scene(&self.ui, &self.theme, w, h);

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

        let bg = self.theme.background.to_f32x4();
        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg[0] as f64,
                            g: bg[1] as f64,
                            b: bg[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }

        // Two layers, each a rects-then-text pass: base first, then the overlay
        // (dropdowns) on top — so overlay content occludes base *text* too, not
        // just base rects (the renderer draws all rects then all text per pass).
        enqueue_scene(&mut self.grid, &mut self.text, &scene.base_layer());
        self.grid.render(&self.device, &view, &mut encoder);
        self.text
            .render(&self.device, &self.queue, &view, &mut encoder);
        if scene.has_overlay() {
            enqueue_scene(&mut self.grid, &mut self.text, &scene.overlay_layer());
            self.grid.render(&self.device, &view, &mut encoder);
            self.text
                .render(&self.device, &self.queue, &view, &mut encoder);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        // Keep redrawing while a hover animation is in flight.
        if animating {
            self.window.request_redraw();
        }
    }
}

#[derive(Default)]
struct App {
    state: Option<GpuState>,
}

impl ApplicationHandler for App {
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
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size.width, size.height),
            WindowEvent::CursorMoved { position, .. } => {
                state.cursor = Point::new(
                    position.x / state.scale_factor,
                    position.y / state.scale_factor,
                );
                // The OS manages the cursor (arrow in content, resize at the
                // decorated window's edges) — don't override it.
                let moved = Event::PointerMoved { pos: state.cursor };
                // When an overlay is open, route hover only to it — items behind
                // the panel must not receive hover events.
                if state.focus.overlay_active(&mut state.ui) {
                    state.focus.deliver_to_overlay(&mut state.ui, &moved);
                } else {
                    state.ui.event(&moved);
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let pos = state.cursor;
                let press = Event::PointerPressed { pos };
                // An open overlay (e.g. a Select dropdown) gets first dibs so it
                // can capture clicks on rows outside its layout bounds.
                let consumed = state.focus.overlay_active(&mut state.ui)
                    && state.focus.deliver_to_overlay(&mut state.ui, &press) == Handled::Yes;
                if !consumed {
                    // A click focuses the clicked widget (clears focus if it misses).
                    state.focus.focus_at(&mut state.ui, pos);
                    state.ui.event(&press);
                }
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                // Lines to scroll the open dropdown (positive = down the list).
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y,
                    MouseScrollDelta::PixelDelta(p) => -(p.y as f32) / 20.0,
                };
                if state.focus.overlay_active(&mut state.ui) {
                    state
                        .focus
                        .deliver_to_overlay(&mut state.ui, &Event::Scroll { delta: lines });
                } else {
                    // No overlay open → scroll the whole page (clamped in render).
                    state.scroll_y += lines * 40.0;
                }
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => {
                let s = m.state();
                state.shift = s.shift_key();
                // Broadcast to the tree so text widgets can do word-wise editing.
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
                        // Tab / Shift+Tab move keyboard focus across buttons.
                        GridKey::Tab => state.focus.advance(&mut state.ui, !state.shift),
                        // Escape closes an open overlay first, else clears focus.
                        GridKey::Escape if state.focus.overlay_active(&mut state.ui) => {
                            state.focus.deliver_key(&mut state.ui, GridKey::Escape);
                        }
                        GridKey::Escape => state.focus.clear(&mut state.ui),
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
                        // Space/Enter (and others) go to the focused widget.
                        other => {
                            state.focus.deliver_key(&mut state.ui, other);
                        }
                    }
                    state.window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => state.render(),
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
