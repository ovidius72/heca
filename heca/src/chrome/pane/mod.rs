//! **The pane shell surface** — one retained component per pane, whatever is running inside it.
//!
//! This file is the only one in the folder that may touch `AppState` (AGENTS.md § 0b-bis rule 4):
//! it reduces the session to [`PaneShellModel`]s and hands them down, so every component below is
//! testable headless.
//!
//! ## Why the tree is retained
//!
//! A pane's letter is drawn by the framework from `Base::hint_label`, which the host writes when
//! the picker opens and reads back one keystroke later. A tree rebuilt inside the paint function —
//! which is what `paint_terminal_pane_shell` did — is gone before either can happen, which is why
//! the pane letters used to be stamped by a host paint pass instead. Retaining the tree is what
//! makes the letter the pane's own.
//!
//! The rebuild cadence follows [`crate::chrome::sync_pane_headers`]: build only when the content
//! key changes, re-lay-out and re-position every frame, prune panes that vanished.

mod model;
mod shell;
#[cfg(test)]
pub(crate) mod testing;

pub(crate) use model::PaneShellModel;
pub(crate) use shell::{PaneCallbacks, PaneShell};

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::KeyHint;
use heca_grid_ui::{LayoutEngine, Size};

/// A retained per-pane shell tree.
///
/// Rebuilt only when [`PaneShellModel::key`] changes, so the widget signals inside survive a
/// re-layout; re-laid-out and repositioned every frame by [`sync_panes`]; painted **through
/// `heca_grid_ui::paint_child`**, which is what draws its letter.
pub(crate) struct RetainedPane {
    pub(crate) root: KeyHint,
    /// The model key the tree was built from.
    pub(crate) key: String,
}

/// Drop every retained pane shell — used when a config reload changes the theme or font baked into
/// the trees. [`sync_panes`] rebuilds them next frame.
///
/// Nothing has to be released with them: a pane declares its pick **on itself**, so a tree that is
/// gone simply has no declaration left. That is the whole reason the picker registers nothing.
/// **The halo an ACTIVE surface wears** — radius and strength.
///
/// One definition, because two things draw it: the focused pane's own frame
/// ([`shell`](self::shell)) and the card that stands for that pane in the exposé
/// (`chrome::expose::PaneCard`). Written twice they drift, and the map stops looking like the app
/// it is a picture of. Scaled by the theme's `glow_size` at the single `PaintCx` chokepoint like
/// every other glow, so `glow_size = none` removes it with the rest.
pub(crate) const ACTIVE_GLOW_RADIUS: f32 = 10.0;
pub(crate) const ACTIVE_GLOW_STRENGTH: f32 = 0.55;

pub(crate) fn clear_panes(state: &mut crate::app_state::AppState) {
    state.panes.clear();
}

/// Build, lay out and position the retained shell for every visible pane.
///
/// Runs at the **top** of `render_frame`, before the `scene_view` borrow of `state.compositor`, so
/// it can mutate `state.panes`; render then paints them read-only.
pub(crate) fn sync_panes(state: &mut crate::app_state::AppState) {
    // ── Phase 1: gather, under immutable borrows only ──
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

    let active_pane = state
        .session
        .active_workspace()
        .and_then(|ws| ws.active_pane())
        .map(|p| p.id);
    let floating: std::collections::HashSet<PaneId> = state
        .session
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
            }
        })
        .collect();

    let cb = callbacks(state);

    // ── Phase 2: build (only when changed), lay out, position, prune ──
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for model in &models {
        seen.insert(model.pane_id);
        let key = model.key();
        let needs_build = state
            .panes
            .get(&model.pane_id)
            .map(|p| p.key != key)
            .unwrap_or(true);
        if needs_build {
            let root = PaneShell { model, cb: &cb }.build();
            state.panes.insert(model.pane_id, RetainedPane { root, key });
        }
        if let Some(retained) = state.panes.get_mut(&model.pane_id) {
            // The pane's rect is a per-frame input, written onto the retained tree rather than
            // built into it — so a zoom, a resize or a float moves the frame without rebuilding
            // the widget and throwing away its signals.
            shell::size_to(&mut retained.root, model.w, model.h);
            LayoutEngine::new().compute(
                &mut retained.root,
                Size::new(model.w as f64, model.h as f64),
            );
            crate::chrome::translate_tree(&mut retained.root, model.x as f64, model.y as f64);
        }
    }
    state.panes.retain(|id, _| seen.contains(id));
}

/// The app's edges, gathered once. A test calls this to get exactly the seams and nothing else
/// (AGENTS.md § 0b-bis rule 3).
fn callbacks(state: &crate::app_state::AppState) -> PaneCallbacks {
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
