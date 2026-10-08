//! Render and viewport helpers.
//!
//! These helpers keep low-level pane rendering and viewport synchronization out
//! of `main.rs` while preserving the current render pipeline behavior.

use crate::app::frame;
use crate::app_state::{AppState, InputMode};
use crate::chrome::ChromeConfig;

/// Human-readable status mode label and suffix for the status bar.
///
/// Chrome focus deliberately says **nothing** here (user, 2026-07-30): this bar is temporary and is
/// being replaced, and a dock's name sitting where an input mode's word goes reads as a mode when it
/// is not one. The affordance for "the keys are going there" is the **focus ring**, which is on the
/// thing itself rather than in a corner.
pub(crate) fn status_mode_parts(
    input_mode: &InputMode,
    catalog: &crate::actions::ActionCatalog,
) -> (&'static str, String) {
    // For pick modes the prompt suffix is sourced from the action's `ActionDescriptor`
    // (via `pending_pick`) so the text lives in one place — the action catalog.
    let pick_suffix = || {
        input_mode
            .pending_pick(catalog)
            .map(|p| format!(" — {}", p.prompt))
            .unwrap_or_default()
    };
    match input_mode {
        InputMode::Normal => ("NORMAL", String::new()),
        InputMode::Prefix => ("PREFIX", String::new()),
        InputMode::PaneSelect { .. } => ("SELECT", pick_suffix()),
        InputMode::FollowLink { .. } => {
            ("FOLLOW", " — press a letter to open the link".to_string())
        }
        InputMode::HintPick { .. } => {
            ("HINT", " — press a letter to activate a target".to_string())
        }
        InputMode::Search { .. } => (
            "SEARCH",
            " — type to search, Enter to keep, Esc to cancel".to_string(),
        ),
        InputMode::PaneSwap { focus_after, .. } => (
            if *focus_after { "SWAP+FOCUS" } else { "SWAP" },
            pick_suffix(),
        ),
        InputMode::Chord { sequence } => ("CHORD", format!(" w→{}", sequence.join("→"))),
        InputMode::Mode { name } => ("MODE", format!(" {} → ?", name)),
        // The confirm now lives entirely in the Modal dialog; the status bar only
        // shows the mode word, no duplicated prompt.
        InputMode::ConfirmDelete => ("CONFIRM", String::new()),
        InputMode::PaneTake { focus_after, .. } => {
            (if *focus_after { "TAKE+" } else { "TAKE" }, pick_suffix())
        }
        InputMode::WorkspacePick { target, .. } => {
            let label = match target {
                crate::app_state::WorkspacePickTarget::Column { .. } => "MOVE COL",
                crate::app_state::WorkspacePickTarget::Pane(_) => "MOVE PANE",
            };
            (label, pick_suffix())
        }
        InputMode::ColumnPick { .. } => ("MOVE PANE", pick_suffix()),
        InputMode::DockPick { .. } => ("FOCUS DOCK", pick_suffix()),
        InputMode::Selection => ("SELECTION", String::new()),
    }
}

/// Render the full frame for the current app state: the phases of [`frame`], in the order they are
/// drawn.
pub(crate) fn render_frame(state: &mut AppState) {
    let Some(v) = frame::begin(state) else {
        return;
    };
    let scene = frame::paint_window(state, &v);
    let terminals = frame::collect_panes(state, &v);
    frame::sync_terminal_layers(state, &v, &terminals);
    let Some(mut frame) = frame::Frame::open(state, v) else {
        return;
    };
    frame.clear_and_background(state);
    frame.write_mask(state, terminals.tiled.iter().chain(&terminals.floating));
    frame.flush_window(state, &scene, &terminals);
    frame.finish(state, &terminals);
}

/// Update session viewport to match current chrome/content area size.
pub(crate) fn update_session_viewport(state: &mut AppState) {
    let pane_area = ChromeConfig::of(state).content_rect();
    let new_size = heca_core::layout::types::Size::new(pane_area.size.w, pane_area.size.h);
    state.layout_mut().update_viewport(new_size);
}

#[cfg(test)]
mod tests {
    use super::status_mode_parts;
    use crate::actions::ActionCatalog;
    use crate::app_state::InputMode;
    use heca_core::layout::PaneId;

    #[test]
    fn status_mode_parts_formats_take_and_confirm() {
        let catalog = ActionCatalog::with_builtins();
        assert_eq!(
            status_mode_parts(
                &InputMode::PaneTake {
                    candidates: vec![("a".chars().next().expect("candidate label"), PaneId(1))],
                    focus_after: true,
                },
                &catalog
            ),
            (
                "TAKE+",
                " — Select a pane to pull into the active column, then focus it.".to_string()
            )
        );

        assert_eq!(
            status_mode_parts(&InputMode::ConfirmDelete, &catalog),
            ("CONFIRM", String::new()),
            "the prompt lives in the Modal now — the status bar shows only the mode word"
        );
    }
}
