//! Backend creation helpers.
//!
//! The app prefers real PTY-backed terminals and falls back to the fake backend
//! only when terminal startup fails on the current machine.

use heca_config::theme::Theme;
use heca_core::backend::{
    FakeBackend, PaneBackend, ShellIntegrationAssets, TerminalBackend, TerminalBackendOptions,
    TerminalPaletteDefaults,
};
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use winit::event_loop::EventLoopProxy;

use crate::app::events::AppEvent;
use crate::app_state::AppState;

pub(crate) const FALLBACK_TERMINAL_GRID: (usize, usize) = (80, 24);
const MAX_TERMINAL_UNITS: f64 = 16_384.0;
const BASH_SNIPPET: &str = include_str!("../../assets/shell-integration/bash_init.sh");
const FISH_SNIPPET: &str = include_str!("../../assets/shell-integration/fish_init.fish");
const ZSH_RC_SNIPPET: &str = include_str!("../../assets/shell-integration/zsh/.zshrc");

pub(crate) fn estimate_terminal_grid(
    width: f64,
    height: f64,
    cell_size: (f32, f32),
) -> (usize, usize) {
    let cols = estimate_terminal_units(width, cell_size.0 as f64, FALLBACK_TERMINAL_GRID.0);
    let rows = estimate_terminal_units(height, cell_size.1 as f64, FALLBACK_TERMINAL_GRID.1);
    (cols, rows)
}

fn estimate_terminal_units(extent: f64, approx_cell: f64, fallback: usize) -> usize {
    if !extent.is_finite() || !approx_cell.is_finite() || extent <= 0.0 || approx_cell <= 0.0 {
        return fallback;
    }

    (extent / approx_cell).ceil().clamp(1.0, MAX_TERMINAL_UNITS) as usize
}

pub(crate) fn create_terminal_backend_for_state(
    state: &AppState,
    cols: usize,
    rows: usize,
) -> Box<dyn PaneBackend> {
    let options = terminal_backend_options(
        &state.theme,
        Some(&state.event_proxy),
        state.shell_integration_enabled,
        state.terminal_scrollback_lines,
        state.terminal_scroll_animations_enabled,
    );
    create_terminal_backend_with_options(cols, rows, state.terminal_cell_size, options)
}

pub(crate) fn create_command_backend_for_state(
    state: &AppState,
    cols: usize,
    rows: usize,
    command: &str,
) -> Box<dyn PaneBackend> {
    // Direct command panes spawn a concrete program inside the PTY; shell
    // integration is intentionally disabled because OSC prompt/cwd hooks are a
    // shell concern and would only add noise here.
    let options = terminal_backend_options(
        &state.theme,
        Some(&state.event_proxy),
        false,
        state.terminal_scrollback_lines,
        state.terminal_scroll_animations_enabled,
    );
    create_command_backend_with_options(cols, rows, state.terminal_cell_size, command, options)
}

pub(crate) fn terminal_grid_for_workspace(state: &AppState, ws_idx: usize) -> (usize, usize) {
    state
        .session
        .workspaces
        .get(ws_idx)
        .map(|ws| {
            estimate_terminal_grid(
                ws.scrolling.working_area.size.w,
                ws.scrolling.working_area.size.h,
                state.terminal_cell_size,
            )
        })
        .unwrap_or(FALLBACK_TERMINAL_GRID)
}

#[expect(
    clippy::too_many_arguments,
    reason = "terminal backend creation threads theme/event/shell/scrollback/animation policy explicitly; grouping is a later refactor"
)]
pub(crate) fn create_terminal_backend(
    cols: usize,
    rows: usize,
    theme: &Theme,
    cell_size: (f32, f32),
    event_proxy: Option<&EventLoopProxy<AppEvent>>,
    shell_integration_enabled: bool,
    scrollback_size: usize,
    scroll_animations: bool,
) -> Box<dyn PaneBackend> {
    let options = terminal_backend_options(
        theme,
        event_proxy,
        shell_integration_enabled,
        scrollback_size,
        scroll_animations,
    );
    create_terminal_backend_with_options(cols, rows, cell_size, options)
}

fn terminal_backend_options(
    theme: &Theme,
    event_proxy: Option<&EventLoopProxy<AppEvent>>,
    shell_integration_enabled: bool,
    scrollback_size: usize,
    scroll_animations: bool,
) -> TerminalBackendOptions {
    let wake_on_output = event_proxy.map(|proxy| {
        let proxy = proxy.clone();
        Arc::new(move || {
            let _ = proxy.send_event(AppEvent::BackendWake);
        }) as Arc<dyn Fn() + Send + Sync>
    });
    let shell_integration = if shell_integration_enabled {
        shell_integration_assets()
    } else {
        None
    };
    let palette_defaults = TerminalPaletteDefaults {
        foreground: theme
            .terminal_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        background: theme
            .terminal_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_fg: theme
            .terminal_cursor_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_bg: theme
            .terminal_cursor_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        cursor_border: theme
            .terminal_cursor_border
            .map(|color| [color.r, color.g, color.b, color.a]),
        selection_fg: theme
            .terminal_selection_foreground
            .map(|color| [color.r, color.g, color.b, color.a]),
        selection_bg: theme
            .terminal_selection_background
            .map(|color| [color.r, color.g, color.b, color.a]),
        ansi: theme
            .terminal_ansi
            .map(|colors| colors.map(|color| [color.r, color.g, color.b, color.a])),
        brights: theme
            .terminal_brights
            .map(|colors| colors.map(|color| [color.r, color.g, color.b, color.a])),
    };
    TerminalBackendOptions {
        palette_defaults: if palette_defaults.foreground.is_some()
            || palette_defaults.background.is_some()
            || palette_defaults.cursor_fg.is_some()
            || palette_defaults.cursor_bg.is_some()
            || palette_defaults.cursor_border.is_some()
            || palette_defaults.selection_fg.is_some()
            || palette_defaults.selection_bg.is_some()
            || palette_defaults.ansi.is_some()
            || palette_defaults.brights.is_some()
        {
            Some(palette_defaults)
        } else {
            None
        },
        wake_on_output,
        shell_integration,
        scrollback_size,
        scroll_animations,
    }
}

fn create_terminal_backend_with_options(
    cols: usize,
    rows: usize,
    cell_size: (f32, f32),
    options: TerminalBackendOptions,
) -> Box<dyn PaneBackend> {
    let (cell_w, cell_h) = cell_size;
    match TerminalBackend::with_options(cols, rows, cell_w, cell_h, options) {
        Ok(backend) => Box::new(backend),
        Err(_err) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to create TerminalBackend ({_err}); falling back to FakeBackend"
            );
            Box::new(FakeBackend::with_cell_size(cols, rows, cell_w, cell_h))
        }
    }
}

fn create_command_backend_with_options(
    cols: usize,
    rows: usize,
    cell_size: (f32, f32),
    command: &str,
    options: TerminalBackendOptions,
) -> Box<dyn PaneBackend> {
    let (cell_w, cell_h) = cell_size;
    match TerminalBackend::with_command(cols, rows, cell_w, cell_h, command, options) {
        Ok(backend) => Box::new(backend),
        Err(_err) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to create command TerminalBackend ({_err}); falling back to FakeBackend"
            );
            Box::new(FakeBackend::with_cell_size(cols, rows, cell_w, cell_h))
        }
    }
}

fn shell_integration_assets() -> Option<ShellIntegrationAssets> {
    static SHELL_ASSETS: OnceLock<Result<ShellIntegrationAssets, String>> = OnceLock::new();
    match SHELL_ASSETS.get_or_init(materialize_shell_integration_assets) {
        Ok(assets) => Some(assets.clone()),
        Err(err) => {
            #[cfg(debug_assertions)]
            eprintln!(
                "[heca] warning: failed to materialize shell integration assets ({err}); spawning bare shells"
            );
            None
        }
    }
}

fn materialize_shell_integration_assets() -> Result<ShellIntegrationAssets, String> {
    let root = heca_config::loader::config_dir()
        .join("runtime")
        .join("shell-integration");
    let zsh_dir = root.join("zsh");
    fs::create_dir_all(&zsh_dir).map_err(|err| err.to_string())?;

    let bash_init = root.join("bash_init.sh");
    let fish_init = root.join("fish_init.fish");
    let zsh_rc = zsh_dir.join(".zshrc");

    write_if_changed(&bash_init, BASH_SNIPPET).map_err(|err| err.to_string())?;
    write_if_changed(&fish_init, FISH_SNIPPET).map_err(|err| err.to_string())?;
    write_if_changed(&zsh_rc, ZSH_RC_SNIPPET).map_err(|err| err.to_string())?;

    Ok(ShellIntegrationAssets {
        bash_init,
        fish_init,
        zsh_zdotdir: zsh_dir,
    })
}

fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    if fs::read_to_string(path).ok().as_deref() == Some(content) {
        return Ok(());
    }
    fs::write(path, content)
}
