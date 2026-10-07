//! **Seating the workspace in the window tree**, and handing it its model each frame.

use heca_grid_ui::Component;

use super::{WORKSPACE_KEY, WorkspaceSeams, gather, workspace};
use crate::app_state::AppState;

/// **Tell the workspace what the columns and the floats are this frame**, seating it first if the
/// window has none. It goes at the **start** of the window root's children, so everything after it —
/// the chrome and every surface — is drawn over it and answers the pointer before it.
pub(crate) fn seat_workspace(state: &mut AppState) {
    let headers = crate::chrome::pane_header_inputs(state);
    let info_bar = headers.is_some();
    let model = gather(state, headers);
    let at = state
        .window_root
        .base()
        .children
        .iter()
        .position(|c| c.base().key.as_deref() == Some(WORKSPACE_KEY));
    let at = match at {
        Some(at) => at,
        None => {
            let seams = WorkspaceSeams {
                column: crate::chrome::column::callbacks(state),
                pane: crate::chrome::pane::callbacks(state),
                header_env: info_bar.then(|| crate::chrome::HeaderEnv::of(state)),
                resize_column: {
                    let emit = resize_emitter(state);
                    std::rc::Rc::new(move |col_idx, delta| {
                        emit(crate::input::WmAction::ResizeColumnBy { col_idx, delta })
                    })
                },
                hovered: state.hovered_column.clone(),
                resize_pane: {
                    let emit = resize_emitter(state);
                    std::rc::Rc::new(move |col_idx, pane_idx, delta| {
                        emit(crate::input::WmAction::ResizePaneHeightBy {
                            col_idx,
                            pane_idx,
                            delta,
                        })
                    })
                },
            };
            state
                .window_root
                .base_mut()
                .children
                .insert(0, Box::new(workspace(seams)));
            0
        }
    };
    if !state.window_root.base_mut().children[at].set_props(&model) {
        crate::chrome::warn_author(
            "[heca] the workspace did not take its model, so the panes will not update".to_string(),
        );
    }
}

/// Drop the workspace, and with it every pane in it: a config reload changes what they were built
/// from (the theme, the fonts, the shortcuts), and the next frame builds them afresh.
pub(crate) fn clear_workspace(state: &mut AppState) {
    state
        .window_root
        .base_mut()
        .children
        .retain(|c| c.base().key.as_deref() != Some(WORKSPACE_KEY));
}

/// What posts a resize the way every action is posted: to the event loop, as a mouse gesture on the
/// content, through the same interaction policy and follow-up a key press gets.
fn resize_emitter(state: &AppState) -> impl Fn(crate::input::WmAction) + 'static {
    let proxy = state.event_proxy.clone();
    move |action| {
        crate::chrome::ChromeIntentEmitter::new(
            &proxy,
            crate::app::interaction::InteractionSource::MouseContent,
        )
        .fire(crate::app::interaction::InteractionIntent::ActivateAction(
            action,
        ));
    }
}
