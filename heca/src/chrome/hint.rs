//! **Who is lettering what** — the host's whole half of every keycap on screen (F003/P082/T427).
//!
//! # The rule: whoever offers a letter owns it until they withdraw it
//!
//! A keycap is [`Base::hint_label`](heca_grid_ui::Base), and the widget that declared the pick
//! draws it. Several things offer letters — the universal picker, a
//! [`KeyHintGroup`](heca_grid_ui::widgets::KeyHintGroup) a surface opened for itself, and the
//! move/swap/take/pick input modes — and the only rule that keeps them from fighting is
//! **ownership**: this module remembers exactly which targets *it* lettered and withdraws exactly
//! those. It never clears a label it did not set.
//!
//! **This replaces a mode check, and that is the point.** The pick modes used to be projected onto
//! four per-widget signal lists (`pane_hint`, `ws_hint`, `col_hint`, `dock_hint`) every frame,
//! which wrote `None` over any letter the picker had offered — so `prefix+/` lit nothing in the
//! sidebar while working perfectly over a layer, which is not synced from here. The first fix was
//! `if matches!(state.input_mode, HintPick { .. }) { return }`, and that names a **host mode**: a
//! plugin opening its own group is not `HintPick`, so its letters were clobbered identically and
//! the plugin author had no way to add themselves to that match. Ownership has no such list.
//!
//! # Addressing
//!
//! A mode knows *what* it is lettering (this pane, that workspace) but not where the widget drawing
//! it sits, so it offers by the target's **`key`** — the identity the widget already declares
//! and that the cursor, the right-click and the drag all read. No second addressing scheme for the
//! same rows. The universal picker keeps the path form, because it collects paths, and both write
//! the same slot.

use crate::app_state::InputMode;
use crate::providers::workspaces::{column_key, pane_key, workspace_key};
use super::{ChromeConfig, LayerId};
use heca_core::layout::{PaneId, Point, Rectangle, Size};

/// The targets this module has a letter on, by `key`. Withdrawal is exactly this set, which is
/// what makes the rule ownership rather than "clear everything and hope".
#[derive(Default, Debug)]
pub(crate) struct OfferedLetters {
    keys: Vec<String>,
}

/// What the **current input mode** wants lettered, as `(key, letter)`.
///
/// One place that knows how a mode's candidates become row identities, so a new pick mode is one
/// arm here rather than a fifth signal list and a fifth projection.
fn wanted(mode: &InputMode, active_pane: Option<heca_core::layout::PaneId>) -> Vec<(String, char)> {
    let mut out = Vec::new();
    if let Some(cands) = mode.candidates() {
        // **The focused pane is never a target**, even when the mode lists it: every one of these
        // picks means "the other one", so lettering where you already are offers a move to nowhere.
        out.extend(
            cands
                .iter()
                .filter(|(_, id)| Some(*id) != active_pane)
                .map(|(ch, id)| (pane_key(*id), *ch)),
        );
    }
    if let Some(cands) = mode.ws_candidates() {
        out.extend(cands.iter().map(|(ch, ws)| (workspace_key(*ws), *ch)));
    }
    if let Some(cands) = mode.col_candidates() {
        out.extend(
            cands
                .iter()
                .map(|(ch, ws, col)| (column_key(*ws, *col), *ch)),
        );
    }
    // A dock names itself with `scope_key` rather than `key` — a container's identity, not a
    // row's — and `offer_hint_by_key` matches either, so this is the same one line as the rest.
    if let Some(cands) = mode.dock_candidates() {
        out.extend(cands.iter().map(|(ch, id)| (id.clone(), *ch)));
    }
    out
}

/// Bring the offered letters in line with the active mode: withdraw what we lettered and no longer
/// want, offer what we want now.
///
/// Runs every frame and is cheap when nothing is picking — the common case is two empty vectors.
/// Returns whether anything changed, so the caller can decide to repaint.
pub(crate) fn sync_offered_letters(state: &crate::app_state::AppState) -> bool {
    let wanted = wanted(&state.input_mode, state.focused_pane);
    // **Which pane shells could actually show a letter.** Computed ONCE per pass, not per key: it
    // resolves the whole surface stack.
    let visible_panes = if wanted.is_empty() {
        std::collections::HashSet::new()
    } else {
        visible_pane_targets(state)
    };
    let mut changed = false;

    // Withdraw first, so a target that keeps its letter across a mode change is not briefly cleared.
    let stale: Vec<String> = state
        .offered_letters
        .borrow()
        .keys
        .iter()
        .filter(|k| !wanted.iter().any(|(want, _)| want == *k))
        .cloned()
        .collect();
    for key in &stale {
        offer_in_every_tree(state, key, None, &visible_panes);
        changed = true;
    }

    for (key, ch) in &wanted {
        if offer_in_every_tree(state, key, Some(ch.to_string()), &visible_panes) {
            changed = true;
        }
    }

    // **Say what is lettered, once, when it changes** — `hint.changed`. A plugin subscribes to this
    // to render its own prompt or highlight beside heca's keycaps (`app.on("hint.changed", …)`),
    // which is why it is emitted here rather than by whatever happened to set a mode: this is the
    // one place that knows the whole picture.
    //
    // Change-guarded, like every other chrome event: this runs every frame and the common case is
    // that nothing is picking.
    let mut offered = state.offered_letters.borrow_mut();
    let next: Vec<String> = wanted.iter().map(|(k, _)| k.clone()).collect();
    if offered.keys != next {
        state
            .chrome_state
            .events()
            .emit(crate::chrome::ChromeEvent::HintLettersChanged {
                letters: wanted.iter().map(|(k, ch)| (*ch, k.clone())).collect(),
            });
    }
    offered.keys = next;
    changed
}

/// Offer `label` to whichever retained tree declares `key`. Front to back, so a surface in
/// front shadows one behind it — the nearest declaration wins, as everywhere else.
fn offer_in_every_tree(
    state: &crate::app_state::AppState,
    key: &str,
    label: Option<String>,
    visible_panes: &std::collections::HashSet<PaneId>,
) -> bool {
    for layer in state.layers.visible_front_to_back() {
        if heca_grid_ui::offer_hint_by_key(layer.root(), key, label.clone()) {
            return true;
        }
    }
    // Below the layers there is no shadowing, because these are not competing surfaces — they are
    // **several views of one thing**. A pane and the sidebar row that names it both declare
    // `pane:7`, and both must wear the letter: the pane owns the identity, the row shows it. An
    // early return here is what made a pane listed in BOTH sidebars get lettered in only one of
    // them (F003/P082, found by tracing `nodes naming it: chrome=2` while one letter went out).
    let mut offered = false;
    if let Some(tree) = state.chrome_tree.as_ref() {
        offered |= heca_grid_ui::offer_hint_by_key(&tree.root, key, label.clone());
    }
    // **A pane scrolled behind a sidebar is skipped — the pane, not the pick.** Its keycap draws on
    // the overlay layer, so it would land on top of the very thing covering it (Antonio, driving,
    // 2026-08-19). Its sidebar row is a second view of the same pane and IS visible, so it still
    // wears the letter and the pane stays reachable — which is why this filters the VIEW rather
    // than the candidate. Visibility is asked of `resolve_hint_layers`, never re-derived here.
    for (pane_id, shell) in state.panes.iter() {
        if !visible_panes.contains(pane_id) {
            continue;
        }
        offered |= heca_grid_ui::offer_hint_by_key(&shell.root, key, label.clone());
    }
    for header in state.pane_headers.values() {
        offered |= heca_grid_ui::offer_hint_by_key(&header.root, key, label.clone());
    }
    offered
}

/// [`wanted`] for a test in another module — the mapping is the interesting part and belongs to
/// this file, so its assertions live wherever the case is clearest rather than being re-derived.
#[cfg(test)]
pub(crate) fn wanted_for_tests(mode: &InputMode) -> Vec<(String, char)> {
    wanted(mode, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_core::layout::PaneId;

    /// **A mode's candidates become row identities, in one place.** A new pick mode is an arm in
    /// `wanted`, not a fifth signal list, a fifth projection and a fifth thing to remember.
    #[test]
    fn a_modes_candidates_become_the_rows_own_identities() {
        let mode = InputMode::PaneSelect {
            candidates: vec![('a', PaneId(7)), ('b', PaneId(9))],
        };
        assert_eq!(
            wanted(&mode, None),
            vec![
                (pane_key(PaneId(7)), 'a'),
                (pane_key(PaneId(9)), 'b'),
            ],
        );
    }

    /// **Nothing picking, nothing wanted** — so nothing is withdrawn from anyone else. This is the
    /// assertion the old per-frame projection failed: it wrote `None` to every target it knew about
    /// whenever its own candidates were empty, which is how it erased the universal picker's
    /// letters and would have erased a plugin's.
    #[test]
    fn a_mode_that_is_not_picking_wants_nothing() {
        assert!(wanted(&InputMode::Normal, None).is_empty());
    }

    /// **The focused pane is never a target**, even when the mode lists it as a candidate: every
    /// one of these picks means "the other one", so a letter where you already are offers a move to
    /// nowhere. The rule came from the host projection this replaced and had to travel with it.
    #[test]
    fn the_pane_you_are_on_gets_no_letter() {
        let mode = InputMode::PaneSelect {
            candidates: vec![('a', PaneId(1)), ('s', PaneId(2))],
        };
        assert_eq!(
            wanted(&mode, Some(PaneId(1))),
            vec![(pane_key(PaneId(2)), 's')],
        );
    }
    /// **`hint.changed` fires when the lettering changes, and not otherwise** — the event a plugin
    /// subscribes to (`app.on("hint.changed", …)`) to render its own prompt beside heca's keycaps.
    ///
    /// It replaced `pane.pick.changed` and `dock.pick.changed`, which were emitted by four per-frame
    /// projections that no longer exist. One mechanism, one event: a pane, a workspace, a column and
    /// a dock are all lettered through the same door now.
    #[test]
    fn the_lettering_is_announced_once_per_real_change() {
        use crate::chrome::SharedChromeState;
        use std::cell::RefCell;
        use std::rc::Rc;

        let store = SharedChromeState::new(300.0, true, 300.0, false);
        let seen: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let _sub = store
            .events()
            .subscribe("hint.changed", move |e| log.borrow_mut().push(e.name().to_string()));

        store
            .events()
            .emit(crate::chrome::ChromeEvent::HintLettersChanged {
                letters: vec![('a', pane_key(PaneId(1)))],
            });

        assert_eq!(seen.borrow().len(), 1, "the name a plugin filters on is `hint.changed`");
    }

}

// ── Which targets exist, and which of them are reachable ─────────────────────

/// One layer of the on-screen surface stack (front → back) for resolving which hint
/// targets are reachable. See `docs/surface-compositor.md`: a surface owns its targets,
/// the opaque region(s) it paints over lower layers (from real layout — never hardcoded),
/// and whether it is `modal` (a blocking context that suppresses everything beneath it).
struct HintLayer {
    targets: Vec<(HintTarget, Rectangle)>,
    occluders: Vec<Rectangle>,
    modal: bool,
}

/// **Which retained tree a hint declaration was collected from.** The app keeps several trees that
/// rebuild on independent cadences — the chrome, one per pane header, one per registered layer —
/// so a path alone does not say where to run it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum HintSurface {
    /// The chrome tree (top bar, sidebars and everything a container contributed).
    Chrome,
    /// One pane's own shell tree — the frame that owns the pane's identity and its letter.
    Pane(PaneId),
    /// One pane's header tree.
    PaneHeader(PaneId),
    /// A dynamically registered layer (an exposé, a modal, a plugin panel).
    Layer(LayerId),
}

/// **One pickable region**: the tree it lives in and its path from that tree's root.
///
/// This is the whole of what a `prefix+/` candidate carries. It is deliberately *not* an id from a
/// registry: nothing is registered, so nothing has to be un-registered when a tree is rebuilt or
/// pruned, and there is no allocator whose ranges have to be kept in step with three rebuild
/// cadences. A path is only meaningful for the tree it was read from, which is exactly the lifetime
/// a pick has — the letters go up and one is chosen in the same breath. A tree rebuilt in between
/// simply answers "nothing there" (`heca_grid_ui::fire_hint` returns `false`), which is not an
/// error.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct HintTarget {
    pub(crate) surface: HintSurface,
    /// Child indices from the surface's root down to the declaring widget.
    pub(crate) path: Vec<usize>,
}

impl HintTarget {
    fn new(surface: &HintSurface, path: Vec<usize>) -> Self {
        Self { surface: surface.clone(), path }
    }
}

/// The retained tree a [`HintSurface`] names, or `None` when it is gone.
fn hint_surface_root<'a>(
    state: &'a crate::app_state::AppState,
    surface: &HintSurface,
) -> Option<&'a dyn heca_grid_ui::Component> {
    match surface {
        HintSurface::Chrome => state
            .chrome_tree
            .as_ref()
            .map(|t| &t.root as &dyn heca_grid_ui::Component),
        HintSurface::Pane(pane_id) => state
            .panes
            .get(pane_id)
            .map(|p| &p.root as &dyn heca_grid_ui::Component),
        HintSurface::PaneHeader(pane_id) => state
            .pane_headers
            .get(pane_id)
            .map(|h| &h.root as &dyn heca_grid_ui::Component),
        HintSurface::Layer(id) => state.layers.get(*id).map(|l| l.root()),
    }
}

/// The retained tree a [`HintSurface`] names, **mutably** — what running a pick needs, since a pick
/// that is not an explicit declaration acts on the widget through its own handlers, and handlers are
/// `FnMut` (F003/P082/T441).
fn hint_surface_root_mut<'a>(
    state: &'a mut crate::app_state::AppState,
    surface: &HintSurface,
) -> Option<&'a mut (dyn heca_grid_ui::Component + 'static)> {
    match surface {
        HintSurface::Chrome => state
            .chrome_tree
            .as_mut()
            .map(|t| &mut t.root as &mut dyn heca_grid_ui::Component),
        HintSurface::Pane(pane_id) => state
            .panes
            .get_mut(pane_id)
            .map(|p| &mut p.root as &mut dyn heca_grid_ui::Component),
        HintSurface::PaneHeader(pane_id) => state
            .pane_headers
            .get_mut(pane_id)
            .map(|h| &mut h.root as &mut dyn heca_grid_ui::Component),
        HintSurface::Layer(id) => state.layers.get_mut(*id).map(|l| l.root_mut().as_mut()),
    }
}

/// **What to call this target, so its letter can find it again next time** (F003/P082/T445).
///
/// A target is addressed by a **path**, which lives exactly as long as the frame it was collected
/// in — that is what makes the picker need no registry, and also what makes a letter forget which
/// widget it belonged to. The identity is the durable half: the widget's own `key` where it has one,
/// derived from its name and scope where it has not (`heca_grid_ui::identity_of`).
///
/// Prefixed by the surface, because two surfaces may each hold a `pane:7` and they are not the same
/// pickable thing.
pub(crate) fn target_identity(
    state: &crate::app_state::AppState,
    target: &HintTarget,
) -> Option<String> {
    let root = hint_surface_root(state, &target.surface)?;
    let within = heca_grid_ui::identity_of(root, &target.path)?;
    let surface = match &target.surface {
        HintSurface::Chrome => "chrome".to_string(),
        HintSurface::Pane(id) => format!("pane:{}", id.0),
        HintSurface::PaneHeader(id) => format!("pane-header:{}", id.0),
        HintSurface::Layer(id) => format!("layer:{id:?}"),
    };
    Some(format!("{surface}/{within}"))
}

/// Run what a pick does to the widget behind `target`. `false` when its tree is gone or was rebuilt
/// under the letters.
pub(crate) fn fire_hint(state: &mut crate::app_state::AppState, target: &HintTarget) -> bool {
    let path = target.path.clone();
    hint_surface_root_mut(state, &target.surface)
        .is_some_and(|root| heca_grid_ui::fire_hint(root, &path))
}

/// **Hand each picked region its letter.** The whole of the host's drawing half — there is no
/// drawing half.
///
/// The widget that declared the pick draws its own keycap, which is the only way the letter lands
/// where the region it names is: a host painting the caps itself has to guess which scene, and
/// which half of it, that widget painted into, and a `Scene` defers overlay segments to a
/// frame-final band ordered by nesting depth. Caps painted into the base draw under every overlay;
/// caps painted at depth 1 draw under anything nested deeper. Both look exactly like "the picker
/// does nothing", and no test can see either — which is what made the exposé's letters invisible
/// while every other part of the picker was correct (F003/P082/T427).
///
/// A plugin's surface therefore needs nothing from here: it declares a hint, gets offered a letter,
/// and its own paint puts that letter over its own widget, at whatever depth it lives.
pub(crate) fn offer_hint_letters(
    state: &crate::app_state::AppState,
    candidates: &[(char, HintTarget)],
) {
    for (label, target) in candidates {
        if let Some(root) = hint_surface_root(state, &target.surface) {
            heca_grid_ui::offer_hint(root, &target.path, Some(label.to_string()));
        }
    }
}

/// **Run an action a widget on screen declares**, if any does (F003/P082/T427).
///
/// Front to back, so a surface in front shadows one behind it with the same name — the nearest
/// declaration wins, the same rule keys and menus already follow. Only **visible** trees are
/// walked, which is the whole of the gate: a verb whose surface is not on screen resolves to
/// nothing, exactly as an unmounted provider's does.
pub(crate) fn fire_widget_action(state: &crate::app_state::AppState, name: &str) -> bool {
    for layer in state.layers.visible_front_to_back() {
        if heca_grid_ui::fire_action(layer.root(), name) {
            return true;
        }
    }
    if let Some(tree) = state.chrome_tree.as_ref()
        && heca_grid_ui::fire_action(&tree.root, name)
    {
        return true;
    }
    state
        .pane_headers
        .values()
        .any(|h| heca_grid_ui::fire_action(&h.root, name))
}

/// **Withdraw every letter**, from every retained tree — what closing the picker means.
///
/// Walks the trees rather than the paths that were offered, so a target that moved while the
/// letters were up still loses its keycap. A stale letter left over a card is the failure this
/// exists to stop.
pub(crate) fn clear_hint_letters(state: &crate::app_state::AppState) {
    if let Some(tree) = state.chrome_tree.as_ref() {
        heca_grid_ui::clear_hints(&tree.root);
    }
    for shell in state.panes.values() {
        heca_grid_ui::clear_hints(&shell.root);
    }
    for header in state.pane_headers.values() {
        heca_grid_ui::clear_hints(&header.root);
    }
    for layer in state.layers.visible_front_to_back() {
        heca_grid_ui::clear_hints(layer.root());
    }
}


/// The single visibility rule (`docs/surface-compositor.md` §3), both halves of it, in the order
/// §2 keeps them: **coarse first, then fine.**
///
/// - **Coarse — context activation.** Walking front → back, the first **modal** layer is the active
///   context, and everything beneath it is dormant: the walk stops there. This is what makes
///   `prefix+/` under the exposé offer the map's cards and nothing else.
/// - **Fine — geometric occlusion.** *Within* what the coarse rule left eligible, a target survives
///   iff it lies in `viewport` and its **centre** is not covered by a higher surface's occluder.
///
/// The fine rule subsumes every case — off-screen, hidden behind the sidebar, a zoomed/floating
/// pane drawn over another — and extends to new surfaces for free. The caller must hand `layers`
/// in true front → back order; there is no sort here, because a second ordering is a second answer
/// to "what is in front", and the two drifted once already (2026-08-11: input and painting
/// disagreed about which layer was frontmost).
fn resolve_hint_layers(
    layers: Vec<HintLayer>,
    viewport: Rectangle,
) -> Vec<(HintTarget, Rectangle)> {
    let covers = |r: &Rectangle, x: f64, y: f64| {
        x >= r.loc.x && x < r.loc.x + r.size.w && y >= r.loc.y && y < r.loc.y + r.size.h
    };
    let vr = viewport.loc.x + viewport.size.w;
    let vb = viewport.loc.y + viewport.size.h;
    let mut kept = Vec::new();
    let mut occluders: Vec<Rectangle> = Vec::new();
    for layer in layers {
        for (target, b) in layer.targets {
            let in_view = b.loc.x < vr
                && b.loc.x + b.size.w > viewport.loc.x
                && b.loc.y < vb
                && b.loc.y + b.size.h > viewport.loc.y;
            let cx = b.loc.x + b.size.w / 2.0;
            let cy = b.loc.y + b.size.h / 2.0;
            if in_view && !occluders.iter().any(|o| covers(o, cx, cy)) {
                kept.push((target, b));
            }
        }
        occluders.extend(layer.occluders.iter().copied());
        if layer.modal {
            break;
        }
    }
    kept
}

/// The coarse half of §3 — the half that was missing, and the whole of F003/P082/T416's picker
/// defect. Held here rather than in a behaviour test because the rule is a pure function of the
/// stack, and because the failure it guards was invisible on screen: the letters were painted
/// *under* the exposé while the targets they named answered normally.
#[cfg(test)]
mod hint_visibility {
    use super::*;

    /// A target somewhere harmless, named by a path so two of them are never equal.
    fn target(path: usize, x: f64) -> (HintTarget, Rectangle) {
        (
            HintTarget { surface: HintSurface::Chrome, path: vec![path] },
            Rectangle::new(Point::new(x, 10.0), Size::new(20.0, 20.0)),
        )
    }

    fn viewport() -> Rectangle {
        Rectangle::new(Point::new(0.0, 0.0), Size::new(1000.0, 800.0))
    }

    /// **The defect this task exists for.** With the exposé up, `prefix+/` offered the sidebar's
    /// rows — the letters were invisible under the map, so a keystroke drove a surface the user
    /// could not see (Antonio, 2026-08-12). A modal layer is the active context: everything
    /// beneath it is dormant, and dormant surfaces are not pickable.
    #[test]
    fn a_modal_layer_is_the_active_context_and_nothing_beneath_it_is_pickable() {
        let stack = vec![
            HintLayer { targets: vec![target(0, 0.0)], occluders: vec![], modal: true },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: false },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(kept.len(), 1, "only the active context's own targets survive");
        assert_eq!(kept[0].0.path, vec![0]);
    }

    /// The counterpart, so the rule above cannot be satisfied by suppressing everything: a layer
    /// that does **not** take the keyboard leaves the surfaces beneath it live. This is what keeps
    /// the sidebar hintable while a pane is zoomed or a toast is up.
    #[test]
    fn a_non_modal_layer_leaves_what_is_beneath_it_pickable() {
        let stack = vec![
            HintLayer { targets: vec![target(0, 0.0)], occluders: vec![], modal: false },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: false },
        ];
        assert_eq!(resolve_hint_layers(stack, viewport()).len(), 2);
    }

    /// Coarse first, then fine — both, not one. Within the active context a target is still
    /// dropped when a surface in front of it covers its centre.
    #[test]
    fn occlusion_still_applies_inside_the_active_context() {
        let stack = vec![
            HintLayer {
                targets: vec![target(0, 0.0)],
                occluders: vec![Rectangle::new(Point::new(90.0, 0.0), Size::new(200.0, 100.0))],
                modal: false,
            },
            HintLayer { targets: vec![target(1, 100.0)], occluders: vec![], modal: true },
        ];
        let kept = resolve_hint_layers(stack, viewport());
        assert_eq!(kept.len(), 1, "the covered target is dropped, the covering one kept");
        assert_eq!(kept[0].0.path, vec![0]);
    }

    /// A target scrolled off the window is not pickable however live its surface is.
    #[test]
    fn a_target_outside_the_viewport_is_dropped() {
        let stack = vec![HintLayer {
            targets: vec![target(0, 5_000.0)],
            occluders: vec![],
            modal: false,
        }];
        assert!(resolve_hint_layers(stack, viewport()).is_empty());
    }
}

/// Build the current surface stack (front → back) and resolve the reachable hint targets
/// for the universal picker. **The one place hint visibility is decided.** The stack
/// mirrors the paint order so hints match what is visually on top; adding a new surface
/// (overlay / exposé / …) means adding a layer here, never a bespoke filter. See
/// `docs/surface-compositor.md`.
pub(crate) fn active_hint_targets(
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
    active_hint_targets(state)
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
