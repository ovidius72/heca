//! **Preparing the frame**: the window painted into a scene, and the terminals' render state and
//! textures.

use heca_core::layout::PaneId;
use heca_grid_ui::{Component, Scene as GuiScene};
use heca_renderer::terminal::TerminalStyle;

use super::{FrameTerminals, FrameValues};
use crate::app::terminal_render::{
    TerminalRenderState, hyperlink_decor_from, sync_retained_terminal_layers,
    terminal_font_families_from,
};
use crate::app_state::AppState;
use crate::chrome::terminal::TerminalId;

/// **Paint the window**: the workspace, the chrome and every surface, in one walk into one scene.
///
/// This lays the whole tree out and paints it, so every terminal's room is known for this frame and
/// the host reads where each was drawn from the scene at the flush. The scene is flushed later,
/// once.
pub(in crate::app) fn paint_window(state: &mut AppState, v: &FrameValues) -> GuiScene {
    // F4.1 — retained tree: rebuild the widget tree only when the chrome signature changes;
    // otherwise re-layout + paint the kept tree (no per-frame signal churn, and a live tree to
    // dispatch events into in F4.2).
    let chrome_sig = crate::chrome::chrome_signature(state, v.chrome);
    if state.chrome_tree.as_ref().map(|t| t.sig) != Some(chrome_sig) {
        let (chrome_root, signals, drag_items, intent_source) =
            crate::chrome::build_chrome_root(state, v.chrome);
        // **Seat the chrome subtree, keep the window root.** Every surface hangs beside the
        // chrome rather than inside it, so a rebuild — a resize, a sidebar toggle, a theme
        // reload — leaves them untouched.
        crate::chrome::seat_chrome(&mut state.window_root, chrome_root);
        state.chrome_tree = Some(crate::chrome::RetainedChrome {
            sig: chrome_sig,
            signals,
            drag_items,
            intent_source,
        });
    }
    // Push value-state (selection + status) into the retained tree's bound signals so
    // focus/mode changes update in place without a rebuild (the signature excludes them).
    crate::chrome::sync_chrome_signals(state);
    // **Flush signal-driven structure before this frame is laid out.**
    //
    // Wrappers like `Visibility` apply their `hidden` flip in `tick`, so a row revealed by a
    // signal has no box until one runs. The frame pass already ticked, but that was before the
    // store was brought up to date — and a pane's runtime now reaches its row through the row's
    // own subscription, which fires during that update. Without this the reveal would land a
    // frame late, and it used to be skipped entirely whenever the sync pass reported no change.
    //
    // Unconditional, because "did anything change" is no longer a question one return value
    // can answer once rows subscribe for themselves. A tick with no time and nothing pending is
    // a walk that finds nothing.
    state.window_root.tick(0.0);
    let (w, h) = (v.w, v.h);
    let theme = crate::chrome::chrome_gui_theme(state);
    let mut scene = crate::chrome::paint_chrome_root(&mut state.window_root, w, h, &theme);
    // **A drag draws itself.** The insertion line, the swap outline and the picture of the
    // thing under the pointer are painted by the widgets the drag passes through, inside
    // `paint_child` — so nothing opts in, and a plugin's own row gets the same feedback
    // (F003/P097/T496). The host pass that drew this for the left sidebar alone is gone.
    // Follow-link keycaps (prefix+Shift+o) over the focused terminal's hyperlinks, painted
    // into the chrome scene so they sit above pane content. terminal-task-18.
    crate::chrome::paint_link_hints(state, &mut scene, w, h, &theme);
    // Visual-bell flash over the content area (fades out). terminal-task-17.
    crate::chrome::paint_bell_flash(state, &mut scene, v.pane_area, w, h, &theme);

    // The trees just built may have declared terminals: one more frame starts and draws them.
    if crate::chrome::terminal::declared_waiting() {
        state.mark_full_redraw();
    }
    scene
}

/// **Every terminal this frame draws**: the tiled panes', the floating panes', and those no pane
/// owns — a dock's, an overlay's — each with its snapshot.
pub(in crate::app) fn collect_panes(state: &mut AppState, v: &FrameValues) -> FrameTerminals {
    let pane_positions = state.layout()
        .active_workspace()
        .map(|ws| ws.scroll().panes_with_positions())
        .unwrap_or_default();
    let tiled = pane_positions
        .iter()
        .map(|(pane_id, rect)| {
            let px = v.pane_area.loc.x as f32 + v.ws_offset.0 + rect.loc.x as f32;
            let py = v.pane_area.loc.y as f32 + v.ws_offset.1 + rect.loc.y as f32;
            let id = state.server.backends.id_for(*pane_id);
            TerminalRenderState::collect(
                state,
                id,
                Some(*pane_id),
                (px, py, rect.size.w as f32, rect.size.h as f32),
            )
        })
        .collect();
    let float_boxes: Vec<(PaneId, (f32, f32, f32, f32))> = state.layout()
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
        .map(|(pane_id, at)| {
            let id = state.server.backends.id_for(pane_id);
            TerminalRenderState::collect(state, id, Some(pane_id), at)
        })
        .collect();
    // The terminals no pane owns, in a stable order: the oldest id first.
    let mut unowned: Vec<TerminalId> = state
        .terminals
        .keys()
        .copied()
        .filter(|id| state.server.backends.pane_of(*id).is_none())
        .collect();
    unowned.sort_by_key(|id| id.0);
    let docked = unowned
        .into_iter()
        .map(|id| TerminalRenderState::collect(state, id, None, (0.0, 0.0, 0.0, 0.0)))
        .collect();
    FrameTerminals {
        tiled,
        floating,
        docked,
    }
}

/// **Bring every terminal's retained texture up to date**, and show each its viewport.
pub(in crate::app) fn sync_terminal_layers(
    state: &mut AppState,
    v: &FrameValues,
    terminals: &FrameTerminals,
) {
    // The chip and the scrollbar are children of each terminal and were painted before this
    // frame's snapshots were read: a change shows on the next frame, so ask for one.
    if crate::chrome::terminal::show_viewports(
        state,
        terminals
            .all()
            .filter_map(|t| t.mount.as_ref().map(|m| (t.id, t.pane, &m.snapshot))),
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
        &terminals.tiled,
        style(v.surface_alpha),
        (v.w, v.h),
        v.phys_size,
    );
    sync_retained_terminal_layers(
        state,
        &terminals.floating,
        style(v.floating_surface_alpha),
        (v.w, v.h),
        v.phys_size,
    );
    sync_retained_terminal_layers(
        state,
        &terminals.docked,
        style(v.surface_alpha),
        (v.w, v.h),
        v.phys_size,
    );
}
