//! **The trees a letter can land in**, and how one is addressed inside them.
//!
//! A target is a surface plus a **path**, never an id from a registry: nothing is registered, so
//! nothing has to be un-registered when a tree is rebuilt or pruned. A path is meaningful only for
//! the tree it was read from, which is exactly the lifetime a pick has.

use heca_core::layout::PaneId;
use heca_grid_ui::Component;

/// **Which retained tree a hint declaration was collected from** — and there are only two kinds
/// left.
///
/// ⚠️ **This is not a list of surfaces, and must not become one again** (F003/P097/T499). It once
/// had a `Layer(LayerId)` arm beside `Chrome`, from the days when a layer owned a tree of its own.
/// Since `T494` seated every surface in the window root, a layer *is* a child of the chrome's own
/// tree: `Layer(id)` resolved to `chrome::surface_node(&state.window_root, id)`, a node the
/// `Chrome` walk had already descended into. So the two arms named one tree, every registry-owned
/// surface's widgets were collected **twice** — once at their path from the root, once at their
/// path from the layer — and one button carried two candidates. Measured, not inferred: a root
/// holding a seated surface answers `collect_hints` with `[[0], [1, 0]]`, and the surface node
/// alone answers `[[0]]` for that same widget.
///
/// It also could not name what the tree actually holds. A surface that has left the registry —
/// the toast stack, placed as `heca.notifications` — is a child of the window root like any other
/// and appears in no registry listing, so its targets were silently attributed to `Chrome` and
/// judged by the chrome's occluders rather than its own.
///
/// **Where a target sits is its path**, and which surface owns it is read from that path. Adding a
/// surface adds a child; it adds nothing here.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum HintSurface {
    /// The window root — the chrome subtree and **every surface seated beside it**: an overlay, a
    /// modal, a context menu, the exposé, the toast stack, a plugin's panel.
    Window,
    /// One pane's own tree — the frame that owns the pane's identity and its letter, and whatever
    /// sits in its header slot. There used to be a second variant for the info bar; the bar is a
    /// child of its pane now, so it is the same tree (F003/P097/T497).
    ///
    /// Still separate because a pane's shell is still a retained tree of its own; it becomes an
    /// ordinary node in `P094(F011)/T449`, and this enum goes with it.
    Pane(PaneId),
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
    /// Child indices from the surface's root down to the declaring widget — the address **this
    /// frame**. See [`resolve`]: it is the shortcut, not the address.
    pub(crate) path: Vec<usize>,
    /// **What the widget is called** (`heca_grid_ui::identity_of`), taken when the target was
    /// collected. `None` for a widget with no key and no name to derive one from.
    ///
    /// This is the durable half. A path is child indices, so a rebuilt tree does not merely
    /// invalidate it — it makes it name *something else*, which is worse. Everything that has to
    /// find this widget again goes through the identity first (F003/P082/T438).
    pub(crate) identity: Option<String>,
}

impl HintTarget {
    pub(super) fn new(
        surface: &HintSurface,
        root: &dyn heca_grid_ui::Component,
        path: Vec<usize>,
    ) -> Self {
        Self {
            identity: heca_grid_ui::identity_of(root, &path),
            surface: surface.clone(),
            path,
        }
    }
}

/// **Where this target is right now** — its surface's root and its current path in it.
///
/// The identity first, the stored path only when the widget has no identity at all. A path is the
/// address *this frame*: the tree behind a surface is rebuilt whenever its content changes — a
/// window resize rebuilds the exposé — and the same indices then name a different widget. That is
/// how a letter offered before a resize came back pointing at the wrong card, and how the keystroke
/// it was waiting for ran nothing.
///
/// One resolver, so **offering a letter and running the pick agree by construction**. They are the
/// two halves of one gesture and they used to resolve the target separately.
pub(crate) fn resolve<'a>(
    state: &'a crate::app_state::AppState,
    target: &HintTarget,
) -> Option<(&'a dyn heca_grid_ui::Component, Vec<Vec<usize>>)> {
    let root = hint_surface_root(state, &target.surface)?;
    let paths = match &target.identity {
        // **Every place it is shown**, not the first: one pane is listed in both sidebars, and both
        // views wear its letter (F003/P082/T431 had this exact bug from an `.any()`).
        Some(id) => heca_grid_ui::hint_targets_of(root, id),
        None => vec![target.path.clone()],
    };
    (!paths.is_empty()).then_some((root, paths))
}

/// The retained tree a [`HintSurface`] names, or `None` when it is gone.
pub(crate) fn hint_surface_root<'a>(
    state: &'a crate::app_state::AppState,
    surface: &HintSurface,
) -> Option<&'a dyn heca_grid_ui::Component> {
    match surface {
        HintSurface::Window => Some(&state.window_root as &dyn heca_grid_ui::Component),
        // **Wherever that pane's tree lives.** A floating pane is the host's own; a tiled one is a
        // child of its column, so the surface resolves to the pane's own node inside it. One
        // function, read by every side — offering a letter, running the pick and collecting the
        // targets all ask here, so none of them can disagree about where a pane is.
        HintSurface::Pane(pane_id) => state
            .panes
            .get(pane_id)
            .map(|p| &p.root as &dyn heca_grid_ui::Component)
            .or_else(|| {
                let key = crate::chrome::pane_key(*pane_id);
                state
                    .columns
                    .values()
                    .find_map(|c| heca_grid_ui::node_with_key(&c.root, &key))
            }),
    }
}

/// The retained tree a [`HintSurface`] names, **mutably** — what running a pick needs, since a pick
/// that is not an explicit declaration acts on the widget through its own handlers, and handlers are
/// `FnMut` (F003/P082/T441).
fn hint_surface_root_mut<'a>(
    state: &'a mut crate::app_state::AppState,
    surface: &HintSurface,
) -> Option<&'a mut dyn heca_grid_ui::Component> {
    match surface {
        HintSurface::Window => Some(&mut state.window_root as &mut dyn heca_grid_ui::Component),
        HintSurface::Pane(pane_id) => {
            let key = crate::chrome::pane_key(*pane_id);
            if state.panes.contains_key(pane_id) {
                return state
                    .panes
                    .get_mut(pane_id)
                    .map(|p| &mut p.root as &mut dyn heca_grid_ui::Component);
            }
            state
                .columns
                .values_mut()
                .find_map(|c| heca_grid_ui::node_with_key_mut(&mut c.root, &key))
        }
    }
}

/// **What to call this target, so its letter can find it again next time** (F003/P082/T445).
///
/// A target is addressed by a **path**, which lives exactly as long as the frame it was collected
/// in — that is what makes the picker need no registry, and also what makes a letter forget which
/// widget it belonged to. The identity is the durable half: the widget's own `key` where it has one,
/// derived from its name and scope where it has not (`heca_grid_ui::identity_of`).
///
/// **A thing that declared a key is named by that key alone, wherever it is shown.** A pane in the
/// scrolling area and the sidebar row for that pane are two views of one pane — T447's rule — so
/// they must get ONE letter, not one each. Prefixing by the tree gave them two, and two letters for
/// one thing is also what exhausts the alphabet and forces uppercase.
///
/// Offering already handles this: [`resolve`] hands back *every* place a name is shown and all of
/// them wear the letter. Only the naming split them.
///
/// Anything that declared no key keeps the prefix, because then the name is derived from content
/// and is only unique within its own tree — two docks holding the same rows is exactly the case
/// the scope exists for.
///
/// ⚠️ **There is no per-layer prefix, and adding one back is a bug** (F003/P097/T499). While
/// `Chrome` and `Layer` were separate arms, the same widget seen from the root and from its layer
/// produced `chrome/pane:7` and `layer:LayerId(3)/pane:7` — two names for one thing, each drawing
/// its own letter. Within one tree the answer is the identity itself, and every node declaring it
/// is a **view** of that one thing, which is exactly what [`resolve`] hands back.
pub(crate) fn target_identity(
    _state: &crate::app_state::AppState,
    target: &HintTarget,
) -> Option<String> {
    let within = target.identity.clone()?;
    // **A key names the thing, so it is the name.** A pane keyed `col:3-pane:7` is that pane
    // wherever it is drawn, and its zoom button is `col:3-pane:7-zoom` — unique because it says
    // which pane's zoom it is, not because of the tree it was found in.
    if let Some(key) = declared_key(_state, target) {
        return Some(key);
    }
    let surface = match &target.surface {
        HintSurface::Window => "window".to_string(),
        HintSurface::Pane(id) => format!("pane:{}", id.0),
    };
    Some(format!("{surface}/{within}"))
}

/// The `key` the widget behind `target` declared about itself, or `None` when it declared none and
/// its name is guessed from the text inside it.
fn declared_key(state: &crate::app_state::AppState, target: &HintTarget) -> Option<String> {
    let (root, paths) = resolve(state, target)?;
    let mut node = root;
    for step in paths.first()? {
        node = node.base().children.get(*step)?.as_ref();
    }
    node.base().key.clone()
}

/// Run what a pick does to the widget behind `target`. `false` when its tree is gone or was rebuilt
/// under the letters.
pub(crate) fn fire_hint(state: &mut crate::app_state::AppState, target: &HintTarget) -> bool {
    // Resolved the same way the letter was offered — through [`resolve`], so the keystroke runs the
    // widget the letter is actually sitting on even if the tree was rebuilt under it.
    // Running it acts on one widget — the first view, since they are views of one thing — while
    // offering the letter reaches all of them.
    let Some((_, paths)) = resolve(state, target) else {
        return false;
    };
    let Some(path) = paths.into_iter().next() else {
        return false;
    };
    hint_surface_root_mut(state, &target.surface)
        .is_some_and(|root| heca_grid_ui::fire_hint(root, &path))
}

/// **Run an action a widget on screen declares**, if any does (F003/P082/T427).
///
/// Front to back, so a surface in front shadows one behind it with the same name — the nearest
/// declaration wins, the same rule keys and menus already follow. Only **visible** trees are
/// walked, which is the whole of the gate: a verb whose surface is not on screen resolves to
/// nothing, exactly as an unmounted provider's does.
pub(crate) fn fire_widget_action(state: &crate::app_state::AppState, name: &str) -> bool {
    // **Front to back is the child order, reversed** — a later sibling is drawn above an earlier
    // one, so the last child is the front-most surface and the chrome (child 0) is behind them all.
    // This used to ask the registry for its visible layers and then walk the whole window root, and
    // that is two answers to "what is in front": the registry lists only what it owns, so a surface
    // placed without registering — the toast stack — was reachable solely through the root walk,
    // which descends in document order and would have let the chrome shadow it.
    for child in state.window_root.base().children.iter().rev() {
        if heca_grid_ui::fire_action(child.as_ref(), name) {
            return true;
        }
    }
    state
        .panes
        .values()
        .any(|p| heca_grid_ui::fire_action(&p.root, name))
}

/// **Withdraw every letter**, from every retained tree — what closing the picker means.
///
/// Walks the trees rather than the paths that were offered, so a target that moved while the
/// letters were up still loses its keycap. A stale letter left over a card is the failure this
/// exists to stop.
/// The window root's walk reaches every surface seated beside the chrome, so there is no per-layer
/// sweep: the loop that used to follow this one re-cleared nodes the root walk had already cleared,
/// and reached only the surfaces the registry owned — never one placed without registering.
pub(crate) fn clear_hint_letters(state: &crate::app_state::AppState) {
    heca_grid_ui::clear_hints(&state.window_root);
    // Every tree that holds panes — a floating pane's own, and each column's, which carries the
    // tiled ones as children.
    for root in crate::chrome::pane_roots(state) {
        heca_grid_ui::clear_hints(root);
    }
}
