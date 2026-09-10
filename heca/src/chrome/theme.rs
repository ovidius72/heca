//! **Resolving the app's theme into the one the widgets read** (F003/P082/T427 split).
//!
//! `heca-config`'s `Theme` is what a user writes; `heca_grid_ui::Theme` is what a widget paints
//! from. This is the one place the first becomes the second, so a colour a widget shows always came
//! through here — and the `[appearance]` transparency, the font sizes and the per-region surface
//! tints are applied once rather than at each call site.
//!
//! Split out of a 5236-line `chrome/mod.rs` that held every kind of logic at once.

use super::*;

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
pub(crate) fn app_theme_to_gui_theme(
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
        // Overwritten at the chokepoint below from `[appearance] hint_font_size` and the app's own
        // accent; these are what a theme built outside it (tests, the showcase) gets.
        hint_font_size: 12.0,
        hint_color: theme.accent,
    }
}

pub(crate) fn chrome_surface_color(theme: &heca_config::theme::Theme) -> Color {
    theme.surface
}

pub(crate) fn top_bottom_pane_background_color(theme: &heca_config::theme::Theme) -> Color {
    theme.effective_top_bottom_pane_background()
}

pub(crate) fn chrome_bar_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    top_bottom_pane_background_color(theme).with_alpha(alpha_u8(appearance.chrome_opacity()))
}

pub(crate) fn sidebar_shell_background_color_for(
    sidebar_bg: Color,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    sidebar_bg.with_alpha(alpha_u8(appearance.opacity()))
}

pub(crate) fn chrome_shell_surface_color_for(
    theme: &heca_config::theme::Theme,
    appearance: &heca_config::appearance::AppearanceConfig,
) -> Color {
    chrome_surface_color(theme).with_alpha(alpha_u8(appearance.opacity()))
}

pub(crate) fn chrome_bar_color(state: &crate::app_state::AppState) -> Color {
    chrome_bar_color_for(&state.theme, &state.appearance)
}

pub(crate) fn left_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    let base = state.theme.effective_left_sidebar_background();
    let resolved = state.appearance.effective_sidebar_background_color(base);
    sidebar_shell_background_color_for(resolved, &state.appearance)
}

pub(crate) fn right_sidebar_shell_background_color(state: &crate::app_state::AppState) -> Color {
    let base = state.theme.effective_right_sidebar_background();
    let resolved = state.appearance.effective_sidebar_background_color(base);
    sidebar_shell_background_color_for(resolved, &state.appearance)
}

pub(crate) fn chrome_shell_surface_color(state: &crate::app_state::AppState) -> Color {
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
    // **The picker's own size and colour — one of each, app-wide.** Set here, at the single theme
    // chokepoint, and carried unchanged into a pane's derived theme (which replaces only its accent,
    // border and background), so a letter looks the same on the active pane, an inactive one, a
    // header button and a sidebar row.
    theme.hint_font_size = state.appearance.hint_font_size.clamp(6.0, 48.0);
    theme.hint_color = state.appearance.effective_hint_color(&state.theme);
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
pub(crate) fn chrome_status(state: &crate::app_state::AppState) -> String {
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
    // **A reply to the last key wins the tail of the bar.** It is there because that key could not
    // do what was asked, so showing the mode's ordinary prompt beside it would answer a question
    // nobody asked. The mode word stays: what you are in has not changed.
    let tail = match &state.status_note {
        Some(note) => format!(" — {note}"),
        None => rename_hint,
    };
    format!("{pane_count} panes | {focus_title} | {mode_str}{tail}")
}

