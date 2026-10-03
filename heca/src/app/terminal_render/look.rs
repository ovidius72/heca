//! **The pane's look**: the theme its contents are drawn with, the box it clips to, and its frame.

use crate::app_state::AppState;
use heca_core::layout::PaneId;
use heca_grid_ui::theme::Theme as GuiTheme;
use heca_grid_ui::{Color as GuiColor, PaintCx, Scene as GuiScene};

// The pane's frame colour is **not** carried here any more. The retained shell holds it — as its
// border, its glow, and the hue it publishes to everything it contains — so passing it to the
// paint call was handing over a fact the widget already owned.

pub(crate) fn pane_scissor_rect(
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    scale_factor: f64,
    physical_size: winit::dpi::PhysicalSize<u32>,
) -> Option<(u32, u32, u32, u32)> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let scale = scale_factor as f32;
    let left = (x.max(0.0) * scale).floor() as u32;
    let top = (y.max(0.0) * scale).floor() as u32;
    let right = ((x + w).max(0.0) * scale).ceil() as u32;
    let bottom = ((y + h).max(0.0) * scale).ceil() as u32;

    let clipped_left = left.min(physical_size.width);
    let clipped_top = top.min(physical_size.height);
    let clipped_right = right.min(physical_size.width);
    let clipped_bottom = bottom.min(physical_size.height);
    let clipped_width = clipped_right.saturating_sub(clipped_left);
    let clipped_height = clipped_bottom.saturating_sub(clipped_top);

    if clipped_width == 0 || clipped_height == 0 {
        return None;
    }

    Some((clipped_left, clipped_top, clipped_width, clipped_height))
}

/// **A floating pane's frame** — its retained shell, painted into `scene`.
///
/// The frame is the pane's **retained** shell (`chrome::sync_panes`), not a tree built here and
/// thrown away: the picker writes a letter into it when it opens and reads it back a keystroke
/// later, so a tree that does not outlive the frame cannot carry one (F011/P094/T451).
///
/// **Painted through `paint_child`, never `paint`** — `paint_child` is what draws a widget's hint
/// letter after painting it. A direct `.paint(cx)` is exactly why a pane could never show its own
/// letter.
///
/// The info bar is **not painted separately.** It is a child of the pane shell, so `paint_child`
/// on that tree draws it — clipped, letters and all — with everything else the pane carries.
/// So is the terminal, which paints the one surface request that says where it is.
pub(crate) fn paint_pane_frame(state: &AppState, scene: &mut GuiScene, pane_id: PaneId) {
    let theme = terminal_pane_gui_theme(state);
    if let Some(retained) = state.panes.get(&pane_id) {
        let mut cx = PaintCx::new(scene, &theme);
        heca_grid_ui::paint_child(&retained.root, &mut cx);
    }
}

fn to_gui_color(color: [f32; 4]) -> GuiColor {
    GuiColor::new(
        (color[0].clamp(0.0, 1.0) * 255.0) as u8,
        (color[1].clamp(0.0, 1.0) * 255.0) as u8,
        (color[2].clamp(0.0, 1.0) * 255.0) as u8,
        (color[3].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn terminal_pane_gui_theme(state: &AppState) -> GuiTheme {
    pane_gui_theme(
        crate::chrome::chrome_gui_theme(state),
        to_gui_color(state.theme.background.to_f32x4()),
    )
}

/// **What a pane changes about the theme its contents are drawn with** — and, just as much, what it
/// leaves alone.
///
/// Takes the chrome theme rather than the app, so the rule can be stated as a test instead of only
/// as a comment (`&AppState` needs a window, and a function that takes one is a function nobody can
/// check).
///
/// - **`accent` is no longer touched here.** An active pane really does mean to re-tint what it
///   holds — that is its identity — but it says so by **publishing its hue on its own shell**
///   (`chrome::pane::shell::focus_state_to` sets `tone`), which reaches the same widgets through
///   the ordinary paint walk. Swapping it into the theme meant the host had to paint each pane
///   under its own theme, and therefore that no container could ever paint one of its children.
/// - **`background`** becomes the real window backdrop, which is what sits behind a pane's reserved
///   title strip; the `Cut` title style matches against it.
/// - **`border`, `border_width` and `border_radius` are left exactly as they are.** They used to be
///   overwritten with the frame's, and they are the tokens *every control inside the pane* reads for
///   its own chrome — so each pane handed its frame's look to everything it contained. A frame is a
///   strong accent on the active pane and nearly the background on the rest, which is why a header
///   button drew a border on hover in the active pane and none anywhere else (Antonio, driving,
///   2026-09-04). Nothing is lost: the shell states its frame as its own style
///   (`chrome::pane::shell`), which is where a widget's own look belongs.
fn pane_gui_theme(mut theme: GuiTheme, window_background: GuiColor) -> GuiTheme {
    theme.colors.background = window_background;
    theme
}

#[cfg(test)]
mod tests;
