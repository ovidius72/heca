//! **Preparing the panes**: their scenes, their render state and their terminals' textures.

use heca_core::layout::{PaneId, Rectangle};
use heca_grid_ui::{
    Point as GuiPoint, Rectangle as GuiRectangle, Scene as GuiScene, Size as GuiSize,
};
use heca_renderer::terminal::TerminalStyle;

use super::{FrameValues, PaneScenes};
use crate::app::terminal_render::{
    PaneRenderState, hyperlink_decor_from, paint_pane_frame, sync_retained_terminal_layers,
    terminal_font_families_from,
};
use crate::app_state::AppState;

/// A scene that paints inside `area`: whatever `paint` draws is clipped to it.
fn scene_clipped_to(area: Rectangle, paint: impl FnOnce(&mut GuiScene)) -> GuiScene {
    let mut scene = GuiScene::new();
    scene.push(heca_grid_ui::scene::DrawCommand::PushClip(
        GuiRectangle::new(
            GuiPoint::new(area.loc.x, area.loc.y),
            GuiSize::new(area.size.w, area.size.h),
        ),
    ));
    paint(&mut scene);
    scene.push(heca_grid_ui::scene::DrawCommand::PopClip);
    scene
}

/// **Paint the columns, and each float's frame, before anything is measured against them.** A
/// terminal paints one surface request at its own box, so these scenes are where every terminal's
/// real position is — below its header, scrolled, clipped — and the host reads it from there
/// instead of working it out from the pane's rect. The same scenes are flushed later; each is
/// painted once.
pub(in crate::app) fn paint_scenes(state: &mut AppState, v: &FrameValues) -> PaneScenes {
    let columns = scene_clipped_to(v.pane_area, |scene| {
        // **The columns' own letters ride in this scene too.** Through `paint_child`, never
        // `paint`: `paint_child` is what draws a widget's hint letter after painting it.
        let column_theme = crate::chrome::chrome_gui_theme(state);
        let mut cx = heca_grid_ui::PaintCx::new(scene, &column_theme);
        for column in state.columns.values() {
            heca_grid_ui::paint_child(&column.root, &mut cx);
        }
    });
    // **A floating pane's frame is painted early for the same reason**: its terminal says where it
    // is in the scene its own frame paints into. Each is flushed in its own pass, between its
    // backdrop and what goes over it.
    let float_ids: Vec<PaneId> = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.floating_panes
                .iter()
                .map(|float| float.pane.id)
                .collect()
        })
        .unwrap_or_default();
    let floats = float_ids
        .into_iter()
        .map(|id| {
            let frame = scene_clipped_to(v.pane_area, |scene| paint_pane_frame(state, scene, id));
            (id, frame)
        })
        .collect();
    let scenes = PaneScenes { columns, floats };
    let painted: Vec<&GuiScene> = std::iter::once(&scenes.columns)
        .chain(scenes.floats.values())
        .collect();
    crate::chrome::terminal::place_from(state, &painted);
    scenes
}

/// The tiled panes and the floating ones as this frame draws them, each with its terminal's
/// snapshot.
pub(in crate::app) fn collect_panes(
    state: &mut AppState,
    v: &FrameValues,
) -> (Vec<PaneRenderState>, Vec<PaneRenderState>) {
    let pane_positions = state
        .session
        .active_workspace()
        .map(|ws| ws.scrolling.panes_with_positions())
        .unwrap_or_default();
    let tiled = pane_positions
        .iter()
        .map(|(pane_id, rect)| {
            let px = v.pane_area.loc.x as f32 + v.ws_offset.0 + rect.loc.x as f32;
            let py = v.pane_area.loc.y as f32 + v.ws_offset.1 + rect.loc.y as f32;
            PaneRenderState::collect(
                state,
                *pane_id,
                (px, py, rect.size.w as f32, rect.size.h as f32),
            )
        })
        .collect();
    let float_boxes: Vec<(PaneId, (f32, f32, f32, f32))> = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.floating_panes
                .iter()
                .map(|float| {
                    let x = float.position.x as f32 + v.pane_area.loc.x as f32 + v.ws_offset.0;
                    let y = float.position.y as f32 + v.pane_area.loc.y as f32 + v.ws_offset.1;
                    (
                        float.pane.id,
                        (x, y, float.size.w as f32, float.size.h as f32),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let floating = float_boxes
        .into_iter()
        .map(|(pane_id, at)| PaneRenderState::collect(state, pane_id, at))
        .collect();
    (tiled, floating)
}

/// **Bring every terminal's retained texture up to date**, and show each its viewport.
pub(in crate::app) fn sync_terminal_layers(
    state: &mut AppState,
    v: &FrameValues,
    tiled: &[PaneRenderState],
    floating: &[PaneRenderState],
) {
    // The chip and the scrollbar are children of each terminal and were painted before this
    // frame's snapshots were read: a change shows on the next frame, so ask for one.
    if crate::chrome::terminal::show_viewports(
        state,
        tiled
            .iter()
            .chain(floating.iter())
            .filter_map(|p| p.mount.as_ref().map(|m| (p.pane_id, &m.snapshot))),
    ) {
        state.mark_full_redraw();
    }

    let font_config = state.font_config.clone();
    // Effective global terminal size (config + global zoom). Per-pane offsets are applied inside
    // `sync_retained_terminal_layers`; this is the base/fallback.
    let app_font_size = state.app_font_size();
    let hyperlink_style = hyperlink_decor_from(state.appearance.terminal.hyperlink_style);
    // Link color: config override → theme accent.
    let hyperlink_color = state
        .appearance
        .terminal
        .hyperlink_color
        .unwrap_or(state.theme.accent)
        .to_f32x4();
    let ligatures = state.appearance.terminal.ligatures;
    let style = |surface_alpha: f32| TerminalStyle {
        font_size: app_font_size,
        families: terminal_font_families_from(&font_config.family),
        surface_alpha,
        ligatures,
        hyperlink_style,
        hyperlink_color,
    };
    // Recomputed each frame by the two calls below (tiled + floating), which OR into it. Reset once
    // here first.
    state.has_animated_images = false;
    sync_retained_terminal_layers(
        state,
        tiled,
        style(v.surface_alpha),
        (v.w, v.h),
        v.phys_size,
    );
    sync_retained_terminal_layers(
        state,
        floating,
        style(v.floating_surface_alpha),
        (v.w, v.h),
        v.phys_size,
    );
}
