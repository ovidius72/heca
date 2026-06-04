//! Grid UI showcase — runnable visual proof of the component library.
//!
//! Builds a retained `heca-grid-ui` component tree (HUD cards + interactive
//! buttons), lays it out each frame, paints it into a `Scene`, and renders via
//! the GPU backend (SDF glow + corner brackets + scanline) on the dark Grid
//! theme. Buttons respond to pointer hover and click.
//!
//! Run: `cargo run -p heca-renderer --example showcase`

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
fn build_ui(theme: &Theme) -> Flex {
    let card = |title: &str, value: &str| {
        Card::new(title)
            .width(Length::Px(220.0))
            .height(Length::Px(140.0))
            .background(theme.surface)
            .border(theme.accent, 1.5)
            .glow(theme.glow)
            .radius(4.0)
            .child(Label::new(value).color(theme.foreground).font_size(32.0))
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
            .child(Label::new(label).color(color).font_size(15.0))
    };
    let report = |a: Action| println!("[showcase] {} -> {:?}", a.name, a.data);

    // 20-entry list so the dropdown caps its height and shows a scrollbar.
    let workspaces: Vec<String> = (1..=20).map(|n| format!("WORKSPACE {n:02}")).collect();

    Flex::column()
        .padding(40.0)
        .gap(28.0)
        .align(Align::Center)
        // Top-of-page dropdowns: open downward and must overlap the rows below.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("MODE").color(theme.muted).font_size(13.0))
                .child(Select::new(["NORMAL", "PREFIX", "PASSTHROUGH"]).on_change(report))
                .child(Label::new("WORKSPACE").color(theme.muted).font_size(13.0))
                .child(Select::new(workspaces).selected(3).on_change(report)),
        )
        .child(
            Flex::row()
                .gap(28.0)
                .child(card("UPLINK", "ONLINE"))
                .child(card("POWER", "98%"))
                .child(card("GRID NODES", "1024")),
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
                .child(Checkbox::new().checked(true).label("ENCRYPT").on_change(report))
                .child(Checkbox::new().label("VERBOSE").on_change(report))
                .child(Checkbox::new().checked(true).label("READ-ONLY").disabled(true))
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
                .child(Label::new("POWER").color(theme.muted).font_size(13.0))
                .child(ProgressBar::new().value(0.72))
                .child(Gauge::new().value(0.85)),
        )
        // Dropdown (overlay layer): opens over the content below it.
        .child(
            Flex::row()
                .gap(16.0)
                .align(Align::Center)
                .child(Label::new("INTENSITY").color(theme.muted).font_size(13.0))
                .child(Select::new(["OFF", "LOW", "MEDIUM", "HEAVY"]).selected(2).on_change(report)),
        )
}

/// Paint the tree, then decorate bordered surfaces with corner brackets and add
/// a full-window scanline overlay.
fn build_scene(root: &dyn Component, theme: &Theme, w: f32, h: f32) -> Scene {
    let mut scene = Scene::new();
    {
        let mut cx = PaintCx::new(&mut scene, theme);
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
        opacity: theme.intensity.scanline_opacity().max(0.05),
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
        let ui = build_ui(&theme);

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

        // Relayout the retained tree to fill the window.
        self.ui.base_mut().style.width = Length::Px(w);
        self.ui.base_mut().style.height = Length::Px(h);
        LayoutEngine::new().compute(&mut self.ui, Size::new(w as f64, h as f64));

        let scene = build_scene(&self.ui, &self.theme, w, h);
        enqueue_scene(&mut self.grid, &mut self.text, &scene);

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
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("showcase") });

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

        self.grid.render(&self.device, &view, &mut encoder);
        self.text.render(&self.device, &self.queue, &view, &mut encoder);
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
                state.ui.event(&Event::PointerMoved { pos: state.cursor });
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
                    state.window.request_redraw();
                }
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
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed =>
            {
                if let Some(gk) = to_grid_key(&event.logical_key) {
                    match gk {
                        // Tab / Shift+Tab move keyboard focus across buttons.
                        GridKey::Tab => state.focus.advance(&mut state.ui, !state.shift),
                        // Escape closes an open overlay first, else clears focus.
                        GridKey::Escape if state.focus.overlay_active(&mut state.ui) => {
                            state.focus.deliver_key(&mut state.ui, GridKey::Escape);
                        }
                        GridKey::Escape => state.focus.clear(&mut state.ui),
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
