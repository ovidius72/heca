//! **The pane** — a frame around whatever is running inside it, built from a [`PaneShellModel`] and
//! held by the workspace.
//!
//! This file is one of the few in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4):
//! it reduces the session to [`PaneShellModel`]s and hands them down, so every component below is
//! testable headless.
//!
//! A pane's letter is drawn by the framework from `Base::hint_label`, which the host writes when
//! the picker opens and reads back one keystroke later, so the pane is built once and kept: the
//! workspace rebuilds it only when its model's key changes and otherwise writes what changed (its
//! focus, its rect, its header's words) onto the pane that is there.

mod model;
pub(crate) mod shell;
#[cfg(test)]
pub(crate) mod testing;

pub(crate) use model::{FloatLook, PaneShellModel};
pub(crate) use shell::{PaneCallbacks, PaneShell};

use heca_core::layout::PaneId;

/// **Every visible pane, reduced to plain data** — one reading of the session, so a column and the
/// panes in it can never disagree about where anything is (AGENTS.md § 0b-bis rule 4: `mod.rs`
/// gathers, everything below takes plain data).
pub(crate) fn pane_models(state: &crate::app_state::AppState) -> Vec<PaneShellModel> {
    let frame_style = state.appearance.effective_pane_border_style();
    let border = state
        .appearance
        .effective_pane_border_color(&state.theme)
        .to_f32x4();
    let active_border = state
        .appearance
        .effective_pane_active_border_color(&state.theme)
        .to_f32x4();
    let floating_border = state
        .appearance
        .effective_pane_floating_border_color(&state.theme)
        .to_f32x4();
    let border_width = state.appearance.effective_pane_border_width(&state.theme);
    let border_radius = state.appearance.effective_pane_border_radius(&state.theme);
    let content_inset = state.appearance.effective_pane_padding(&state.theme);
    let accent = state.theme.accent.to_f32x4();
    let active_glow = (
        state.theme.active_glow_radius,
        state.theme.active_glow_strength,
    );

    // A float covers what is under it: it fills itself, or frosts what lies beneath when the
    // config asks for a see-through float with a blur.
    let float_look = crate::chrome::pane::FloatLook {
        background: state.theme.float_background.to_f32x4(),
        frost: if state.terminal_floating_surface_opacity() < 1.0 {
            state.appearance.terminal_floating_blur_radius()
        } else {
            0.0
        },
    };
    let active_pane = state.layout()
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    let floating: std::collections::HashSet<PaneId> = state.layout()
        .active_workspace()
        .map(|ws| ws.floating_panes.iter().map(|f| f.pane.id).collect())
        .unwrap_or_default();

    let models: Vec<PaneShellModel> = crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .map(|(pane_id, x, y, w, h)| {
            let is_active = Some(pane_id) == active_pane;
            let border_color = if is_active {
                active_border
            } else if floating.contains(&pane_id) {
                floating_border
            } else {
                border
            };
            PaneShellModel {
                pane_id,
                x,
                y,
                w,
                h,
                active: is_active,
                frame: frame_style,
                border_color,
                border_width,
                border_radius,
                content_inset,
                accent,
                active_glow,
                float: floating.contains(&pane_id).then_some(float_look),
                move_modifier: state.interactive_move_modifier,
            }
        })
        .collect();
    models
}

/// The app's edges, gathered once. A test calls this to get exactly the seams and nothing else
/// (AGENTS.md § 0b-bis rule 3).
pub(crate) fn callbacks(state: &crate::app_state::AppState) -> PaneCallbacks {
    let proxy = state.event_proxy.clone();
    PaneCallbacks {
        pick: std::rc::Rc::new(move |pane_id: PaneId| {
            // The pane asks; the core does. A pick focuses the pane through the same intent path a
            // click, a keybinding and RPC all use — it never touches the session itself.
            let _ = proxy.send_event(crate::app::events::AppEvent::ChromeIntent {
                // **A pick is a keyboard gesture, not a click.** It carries no pointer, so calling
                // it `MouseContent` mislabels it — and the label is load-bearing: only
                // keyboard-driven sources can resolve to `Domain::Container`, and
                // `base_context_is_dormant` reads the source too. Same reasoning the codebase
                // already applies to RPC: a click lands on something, a pick lands on nothing, so
                // where the keyboard is *is* its context.
                //
                // ⚠️ Not established by tracing: a pick and a real click on a pane emitted the same
                // source before this line, so a trace cannot tell them apart. The argument is the
                // gesture's, not the evidence's.
                source: crate::app::interaction::InteractionSource::Keyboard,
                intent: crate::app::interaction::InteractionIntent::FocusPane { pane_id },
            });
        }),
    }
}
