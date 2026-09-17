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
/// **The halo an ACTIVE surface wears** — radius and strength.
///
/// One definition, because two things draw it: the focused pane's own frame
/// ([`shell`](self::shell)) and the card that stands for that pane in the exposé
/// (`chrome::expose::PaneCard`). Written twice they drift, and the map stops looking like the app
/// it is a picture of. Scaled by the theme's `glow_size` at the single `PaintCx` chokepoint like
/// every other glow, so `glow_size = none` removes it with the rest.
pub(crate) const ACTIVE_GLOW_RADIUS: f32 = 10.0;
pub(crate) const ACTIVE_GLOW_STRENGTH: f32 = 0.55;

/// **How much of a pane its header takes — measured, never computed.**
///
/// Read off the laid-out tree, so it is right for whatever was put in the header slot: a terminal's
/// info bar today, something else tomorrow, of any height. It used to be arithmetic on the host's
/// side — an approximation of `Tag`'s internal padding, plus a copy of the library's line-height
/// ratio, plus a margin — which meant anything else placed in that slot had to add up to the same
/// three numbers or sit wrong (Antonio, driving, 2026-09-02).
///
/// The shell builds the pane as a column of two when there is a header: the header at its natural
/// height, then the content taking the rest. One child means no header.
pub(crate) fn header_height(state: &crate::app_state::AppState, pane_id: PaneId) -> f32 {
    use heca_grid_ui::Component;
    let Some(retained) = state.panes.get(&pane_id) else {
        return 0.0;
    };
    // **The header is found by NAME.** It says which row of the pane's template it is
    // (`shell::PANE_HEADER_AREA`), so a second thing in the body cannot be mistaken for it and a
    // pane without one simply has no node carrying that name.
    //
    // It used to ask "does this pane have two children?" — which AGENTS § 0 and docs/layout.md both
    // name as never right, because the answer changes with anything else put in the body.
    fn area(n: &dyn Component, name: &str) -> Option<f32> {
        if n.base().grid_area.as_deref() == Some(name) {
            return Some(n.base().bounds.size.h as f32);
        }
        n.base().children.iter().find_map(|c| area(c.as_ref(), name))
    }
    area(&retained.root, shell::PANE_HEADER_AREA).unwrap_or(0.0)
}

pub(crate) fn clear_panes(state: &mut crate::app_state::AppState) {
    state.panes.clear();
    crate::chrome::clear_columns(state);
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

    // What each pane wants along its top. Built here and handed down as a child — the shell has no
    // idea what a terminal is, so the bar arrives as an ordinary widget like any other content.
    let mut headers = crate::chrome::build_pane_headers(state);

    let cb = callbacks(state);

    // ── Phase 2: build (only when changed), lay out, position, prune ──
    let mut seen: std::collections::HashSet<PaneId> = std::collections::HashSet::new();
    for model in &models {
        seen.insert(model.pane_id);
        let header = headers.remove(&model.pane_id);
        // ONE key for the pane and what it carries, so they rebuild together or not at all. Two
        // keys would let a bar go stale inside a pane that had no reason to rebuild.
        let key = match &header {
            Some((_, header_key, _)) => format!("{}|{header_key}", model.key()),
            None => model.key(),
        };
        let needs_build = state
            .panes
            .get(&model.pane_id)
            .map(|p| p.key != key)
            .unwrap_or(true);
        let header_texts = header.as_ref().map(|(_, _, texts)| texts.clone());
        if needs_build {
            let root = PaneShell {
                model,
                cb: &cb,
                header: header
                    .map(|(tree, _, _)| Box::new(tree) as Box<dyn heca_grid_ui::Component>),
                content: None,
            }
            .build();
            state
                .panes
                .insert(model.pane_id, RetainedPane { root, key });
        }
        // **The words are a per-frame input**, written onto the retained tree exactly as the pane's
        // rect is — so the program a pane shows, its branch and its working directory change
        // without the bar being rebuilt (F003/P097/T500). Rebuilding for them blinked the buttons
        // out and back twice per command, because a fresh widget paints nothing until the layout
        // walk has given it a box.
        if let (Some(texts), Some(retained)) = (&header_texts, state.panes.get(&model.pane_id)) {
            crate::chrome::pane_header::refresh_pane_header_text(&retained.root, texts);
        }
        if let Some(retained) = state.panes.get_mut(&model.pane_id) {
            // The pane's rect is a per-frame input, written onto the retained tree rather than
            // built into it — so a zoom, a resize or a float moves the frame without rebuilding
            // the widget and throwing away its signals.
            shell::size_to(&mut retained.root, model.w, model.h);
            // **What focus changed**, written on rather than rebuilt for — see `focus_state_to`.
            shell::focus_state_to(
                &mut retained.root,
                model.active,
                model.border_color,
                model.accent,
            );
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
    crate::chrome::sync_columns(state);
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
