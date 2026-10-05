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
pub(crate) mod shell;
#[cfg(test)]
pub(crate) mod testing;

pub(crate) use model::PaneShellModel;
pub(crate) use shell::{PaneCallbacks, PaneShell};

use heca_core::layout::PaneId;
use heca_grid_ui::widgets::Pane as UiPane;
use heca_grid_ui::{LayoutEngine, Size};

/// A retained per-pane shell tree.
///
/// Rebuilt only when [`PaneShellModel::key`] changes, so the widget signals inside survive a
/// re-layout; re-laid-out and repositioned every frame by [`sync_panes`]; painted **through
/// `heca_grid_ui::paint_child`**, which is what draws its letter.
pub(crate) struct RetainedPane {
    pub(crate) root: UiPane,
    /// The model key the tree was built from.
    pub(crate) key: String,
}

/// Drop every retained pane shell — used when a config reload changes the theme or font baked into
/// the trees. [`sync_panes`] rebuilds them next frame.
///
/// Nothing has to be released with them: a pane declares its pick **on itself**, so a tree that is
/// gone simply has no declaration left. That is the whole reason the picker registers nothing.
pub(crate) fn clear_panes(state: &mut crate::app_state::AppState) {
    state.panes.clear();
    crate::chrome::clear_columns(state);
}

/// **Every visible pane, reduced to plain data** — the gather half of [`sync_panes`], on its own
/// so the column surface can build the panes it holds from the same models rather than a second
/// reading of the session (AGENTS.md § 0b-bis rule 4: `mod.rs` gathers, everything below takes
/// plain data).
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
                active_glow,
            }
        })
        .collect();
    models
}

/// Build, lay out and position the retained shell for every visible pane.
///
/// Runs at the **top** of `render_frame`, before the `scene_view` borrow of `state.compositor`, so
/// it can mutate `state.panes`; render then paints them read-only.
pub(crate) fn sync_panes(
    state: &mut crate::app_state::AppState,
    headers: Option<
        &std::collections::HashMap<PaneId, crate::chrome::pane_header::PaneHeaderInput>,
    >,
) {
    // **Only the panes no column holds.** A tiled pane is a child of its column now
    // (`chrome::column`), built, placed, focused and painted there — so building a second tree for
    // it here would be two widgets answering to one name, and two of everything the pane carries.
    // A floating pane belongs to no column, so it is still the host's to place.
    let tiled: std::collections::HashSet<PaneId> = state
        .session
        .active_workspace()
        .map(|ws| {
            ws.scrolling
                .panes_with_positions()
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        })
        .unwrap_or_default();
    let all = pane_models(state);
    let models: Vec<PaneShellModel> = all
        .into_iter()
        .filter(|m| !tiled.contains(&m.pane_id))
        .collect();

    let cb = callbacks(state);
    // What a header is built from, taken the first time a pane is built this frame.
    let mut env: Option<std::rc::Rc<crate::chrome::HeaderEnv>> = None;

    // ── Phase 2: build (only when changed), lay out, position, prune ──
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for model in &models {
        seen.insert(model.pane_id);
        // The pane is rebuilt only for what the pane is: its header builds its own tree when its
        // shape changes, so a pane never rebuilds because of it.
        let key = model.key();
        let needs_build = state
            .panes
            .get(&model.pane_id)
            .map(|p| p.key != key)
            .unwrap_or(true);
        if needs_build {
            let content = Box::new(crate::chrome::terminal::view_of(state, model.pane_id))
                as Box<dyn heca_grid_ui::Component>;
            // What each pane wants along its top, placed as an ordinary child — the shell has no
            // idea what a terminal is, so the bar arrives like any other content.
            let header = headers.map(|_| {
                let env = env.get_or_insert_with(|| crate::chrome::HeaderEnv::of(state));
                crate::chrome::PaneHeader::new(env.clone())
            });
            let root = PaneShell {
                model,
                cb: &cb,
                header,
                content: Some(content),
            }
            .build();
            state
                .panes
                .insert(model.pane_id, RetainedPane { root, key });
        }
        if let Some(retained) = state.panes.get_mut(&model.pane_id) {
            // Tell the pane what is true of it now: its header builds only if its shape changed,
            // and writes its own words, so nothing here touches the tree.
            if let Some(input) = headers.and_then(|h| h.get(&model.pane_id)) {
                crate::chrome::give_header_facts(&mut retained.root, input);
            }
            // The pane's rect is a per-frame input, written onto the retained tree rather than
            // built into it — so a zoom, a resize or a float moves the frame without rebuilding
            // the widget and throwing away its signals.
            shell::size_to(&mut retained.root, model.w, model.h);
            // **What focus changed**, written on rather than rebuilt for — see `focus_state_to`.
            shell::focus_state_to(&mut retained.root, model);
            LayoutEngine::new().compute(
                &mut retained.root,
                Size::new(model.w as f64, model.h as f64),
            );
            heca_grid_ui::shift_subtree(&mut retained.root, model.x as f64, model.y as f64);
        }
    }
    state.panes.retain(|id, _| seen.contains(id));
    // The columns are placed in the same pass, from the same geometry — so a column and the panes
    // in it can never be one frame out of step.
    crate::chrome::sync_columns(state, headers);
    // A terminal is kept only for a pane that is shown.
    crate::chrome::terminal::retain_owned(state);
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
