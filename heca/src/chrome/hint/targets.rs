//! **Which targets are live right now** — the visibility rule applied to the real surface stack,
//! and then the policy question asked of each survivor.
//!
//! Two questions, deliberately separate: *can you see it* is geometry, *would it do anything* is
//! policy.

use super::surfaces::{hint_surface_root, HintSurface, HintTarget};
use super::visibility::{resolve_hint_layers, HintLayer};
use crate::chrome::ChromeConfig;
use heca_core::layout::{PaneId, Point, Rectangle, Size};

/// Build the current surface stack (front → back) and resolve the reachable hint targets
/// for the universal picker. **The one place hint visibility is decided.** The stack
/// mirrors the paint order so hints match what is visually on top; adding a new surface
/// (overlay / exposé / …) means adding a layer here, never a bespoke filter. See
/// `docs/surface-compositor.md`.
pub(crate) fn active_hint_targets(
    state: &crate::app_state::AppState,
) -> Vec<(HintTarget, Rectangle)> {
    // **A letter that would do nothing is worse than no letter** (F003/P082/T432). Candidacy is
    // read off the tree; whether the act it names is permitted *right now* is the app's judgement,
    // and it is asked here — once, for every surface, with no special case per widget.
    //
    // The case it exists for: with a floating pane active, `prefix+/` lettered every pane and every
    // sidebar row naming one, and pressing a letter did nothing, because `ActionPolicy` correctly
    // refuses a `FocusPane` that does not target the active float. It could not be fixed before the
    // declaration carried its `Intent` — an opaque closure gives a policy nothing to ask about, and
    // the only alternative was inferring intent from an identity string (`pane:7`, which a naive
    // match also finds inside `pane:70`).
    //
    // A pick is a **keyboard** gesture: it lands on nothing, so where the keyboard is *is* its
    // context — the same reasoning `chrome/pane/mod.rs` and RPC already apply.
    visible_hint_targets(state)
        .into_iter()
        .filter(|(target, _)| {
            let Some(root) = hint_surface_root(state, &target.surface) else {
                return false;
            };
            candidate_allowed(state, target, root)
        })
        .collect()
}

/// **Is this candidate worth a letter?** — judged by *what it says it does*, and by nothing else
/// (F003/P082/T432).
///
/// One question, asked of every candidate on every surface: the declaration carries an
/// [`Intent`](heca_view::Intent), the app asks its own policy about it, and a refused act is not
/// offered a letter. A picker that hands out letters which do nothing is a broken picker.
///
/// **It knows nothing about what the candidate is.** Not a pane, not a workspace, not a row — the
/// mechanism has no list of kinds, so it works for a widget nobody has written yet and for a
/// plugin's row exactly as it does for heca's own. An earlier version resolved the candidate's
/// identity back to *the pane that owns it*, which made the sidebar rows behave like their panes and
/// nothing else behave like anything: a rule with `state.panes` in it is a rule that has to be
/// extended by hand for every kind that follows.
///
/// The other half of the contract is the declaration's: **an action says what it may do** through
/// its own `ActionMeta.policy`, which is required and has no permissive default. A verb that
/// declares `Global` while it focuses a pane will be offered a letter that half works, and that is
/// the declaration lying — not this function guessing wrong.
fn candidate_allowed(
    state: &crate::app_state::AppState,
    target: &HintTarget,
    root: &dyn heca_grid_ui::Component,
) -> bool {
    match heca_grid_ui::hint_intent(root, &target.path) {
        Some(intent) => crate::app::interaction::view_intent_allowed(
            state,
            pick_source(state, &target.surface),
            &intent,
        ),
        // Nothing to ask — a declaration that is only a closure, or a widget whose letter simply
        // runs its own click. Offered, as it always was.
        None => true,
    }
}

/// **The source a pick on this surface will be dispatched with.**
///
/// Read from the surface itself, never chosen here. A judgement about a gesture is only worth
/// anything if it is the judgement the gesture will actually get, and the source decides the
/// [`Domain`](crate::app::interaction::Domain): a keyboard-driven source with a dock focused
/// resolves to `Container`, which permits acts that `Floating` refuses. Asking as `Keyboard` while
/// the chrome tree dispatches as `MouseLeftSidebar` is exactly how the picker came to offer letters
/// that execution then refused (Antonio, driving 2026-08-21).
///
/// Each surface answers from the emitter that built it, so there is nothing to keep in step:
/// the chrome tree carries its emitter's source, a layer's is its own id, and a pane's pick is the
/// keyboard gesture `chrome/pane/mod.rs` posts.
fn pick_source(
    state: &crate::app_state::AppState,
    surface: &HintSurface,
) -> crate::app::interaction::InteractionSource {
    use crate::app::interaction::InteractionSource as S;
    match surface {
        HintSurface::Chrome => state
            .chrome_tree
            .as_ref()
            .map(|t| t.intent_source)
            .unwrap_or(S::Keyboard),
        HintSurface::Layer(id) => S::Surface(*id),
        HintSurface::Pane(_) | HintSurface::PaneHeader(_) => S::Keyboard,
    }
}

/// The candidates the **one visibility rule** leaves — context activation, then geometric occlusion
/// (`docs/surface-compositor.md` § 3), and nothing about policy.
///
/// Split from [`active_hint_targets`] so occlusion is still decided in exactly one place while the
/// two questions stay separate: *can you see it* is geometry, *would it do anything* is policy.
/// [`visible_pane_targets`] wants the first alone — a pane-select mode letters a pane for reasons of
/// its own, and answering it with the `prefix+/` picker's policy would be one surface's judgement
/// applied to another's.
fn visible_hint_targets(
    state: &crate::app_state::AppState,
) -> Vec<(HintTarget, Rectangle)> {
    let (vw, vh) = {
        let phys = state.window.inner_size();
        let s = state.scale_factor;
        (phys.width as f64 / s, phys.height as f64 / s)
    };
    let viewport = Rectangle::new(Point::new(0.0, 0.0), Size::new(vw, vh));
    let content = ChromeConfig {
        tab_bar_height: state.tab_bar_height(),
        status_bar_height: state.status_bar_height(),
        left_sidebar_width: state.left_sidebar_width(),
        right_sidebar_width: state.right_sidebar_width(),
        sidebar_gap: state.appearance.effective_sidebar_gap(&state.theme),
    }
    .content_rect(vw as f32, vh as f32);

    let mut layers: Vec<HintLayer> = Vec::new();

    // The stack is assembled **front → back, in paint order reversed** — layers, then the chrome
    // shell, then the panes. That is literally how the frame is drawn (`render.rs`: chrome scene,
    // then `paint_layers` into a scene flushed after it), so building the stack this way makes the
    // two agree by construction instead of by a second ordering rule that has to be kept in step.
    //
    // 1. Dynamically registered layers (the exposé, the context menu, the confirm dialog, a plugin
    //    panel), front → back among themselves. Their targets and occluder come from their
    //    laid-out tree, never a constant. The first **modal** one among them is the active context
    //    and `resolve_hint_layers` stops there — which is what suppresses the chrome and the panes
    //    beneath an exposé.
    for layer in state.layers.visible_front_to_back() {
        let bounds = layer.root().base().bounds;
        layers.push(HintLayer {
            targets: hints_of(&HintSurface::Layer(layer.id), layer.root()),
            occluders: vec![bounds],
            modal: layer.modal,
        });
    }

    // 2. Chrome (top bar + sidebars), drawn on top of all pane content. Its own targets
    //    are eligible; the chrome frame AROUND the content (bars + sidebars) occludes pane
    //    targets beneath it. Occluders come from `content_rect`, not constants.
    if let Some(tree) = state.chrome_tree.as_ref() {
        let (cl, ct) = (content.loc.x, content.loc.y);
        let (cr, cb) = (content.loc.x + content.size.w, content.loc.y + content.size.h);
        layers.push(HintLayer {
            targets: hints_of(&HintSurface::Chrome, &tree.root),
            occluders: vec![
                Rectangle::new(Point::new(0.0, 0.0), Size::new(vw, ct)), // top bar
                Rectangle::new(Point::new(0.0, 0.0), Size::new(cl, vh)), // left sidebar
                Rectangle::new(Point::new(cr, 0.0), Size::new(vw - cr, vh)), // right sidebar
                Rectangle::new(Point::new(0.0, cb), Size::new(vw, vh - cb)), // status bar
            ],
            modal: false,
        });
    }

    // 3. Panes, front (topmost draw) → back. `pane_outer_frames` is in draw order (tiled
    //    then floats; last = on top), so reversing it yields front→back: floats over tiled,
    //    and a zoomed pane (a later sibling) over the panes behind it. Each pane occludes
    //    the ones beneath by its own frame.
    for (pane_id, x, y, w, h) in crate::app::terminal_host::pane_outer_frames(state)
        .into_iter()
        .rev()
    {
        // The pane's own shell first — it OWNS the pane (its identity, its letter); the header is
        // a view of what runs inside it. Both are real declarations, so both wear a letter, and
        // both are hidden by the same occluder because they are one pane.
        let mut targets = match state.panes.get(&pane_id) {
            Some(shell) => hints_of(&HintSurface::Pane(pane_id), &shell.root),
            None => Vec::new(),
        };
        if let Some(header) = state.pane_headers.get(&pane_id) {
            targets.extend(hints_of(&HintSurface::PaneHeader(pane_id), &header.root));
        }
        if targets.is_empty() {
            continue;
        }
        layers.push(HintLayer {
            targets,
            occluders: vec![Rectangle::new(
                Point::new(x as f64, y as f64),
                Size::new(w as f64, h as f64),
            )],
            modal: false,
        });
    }

    resolve_hint_layers(layers, viewport)
}

/// **Which panes could actually show a letter**, by the one visibility rule.
///
/// A pane scrolled behind a sidebar is still a pane, and a pick mode reading the SESSION happily
/// letters it — so its keycap draws on top of the sidebar covering it (Antonio, driving,
/// 2026-08-19). `prefix+/` never had this problem because [`active_hint_targets`] resolves the
/// surface stack first: a target whose centre is covered by something in front is dropped.
///
/// So this asks that same function rather than testing rects again here. Occlusion is decided in
/// exactly one place — `resolve_hint_layers` — and a second copy would be one more thing to keep in
/// step with the layer stack. It collapses entirely when the pick modes become the one picker
/// (F011/P094/T457).
pub(crate) fn visible_pane_targets(
    state: &crate::app_state::AppState,
) -> std::collections::HashSet<PaneId> {
    visible_hint_targets(state)
        .into_iter()
        .filter_map(|(target, _)| match target.surface {
            HintSurface::Pane(id) => Some(id),
            _ => None,
        })
        .collect()
}

/// One surface's hint declarations, addressed by [`HintTarget`].
fn hints_of(
    surface: &HintSurface,
    root: &dyn heca_grid_ui::Component,
) -> Vec<(HintTarget, Rectangle)> {
    heca_grid_ui::collect_hints(root)
        .into_iter()
        .map(|(path, bounds)| (HintTarget::new(surface, path), bounds))
        .collect()
}
