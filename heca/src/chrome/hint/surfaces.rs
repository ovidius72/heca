//! **The trees a letter can land in**, and how one is addressed inside them.
//!
//! A target is a surface plus a **path**, never an id from a registry: nothing is registered, so
//! nothing has to be un-registered when a tree is rebuilt or pruned. A path is meaningful only for
//! the tree it was read from, which is exactly the lifetime a pick has.

use crate::chrome::LayerId;
use heca_core::layout::PaneId;


/// **Which retained tree a hint declaration was collected from.** The app keeps several trees that
/// rebuild on independent cadences — the chrome, one per pane header, one per registered layer —
/// so a path alone does not say where to run it.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) enum HintSurface {
    /// The chrome tree (top bar, sidebars and everything a container contributed).
    Chrome,
    /// One pane's own tree — the frame that owns the pane's identity and its letter, and whatever
    /// sits in its header slot. There used to be a second variant for the info bar; the bar is a
    /// child of its pane now, so it is the same tree (F003/P097/T497).
    Pane(PaneId),
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
    pub(super) fn new(surface: &HintSurface, root: &dyn heca_grid_ui::Component, path: Vec<usize>) -> Self {
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
pub(super) fn hint_surface_root<'a>(
    state: &'a crate::app_state::AppState,
    surface: &HintSurface,
) -> Option<&'a dyn heca_grid_ui::Component> {
    match surface {
        HintSurface::Chrome => Some(&state.window_root as &dyn heca_grid_ui::Component),
        HintSurface::Pane(pane_id) => state
            .panes
            .get(pane_id)
            .map(|p| &p.root as &dyn heca_grid_ui::Component),
        HintSurface::Layer(id) => crate::chrome::surface_node(&state.window_root, *id),
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
        HintSurface::Chrome => Some(&mut state.window_root as &mut dyn heca_grid_ui::Component),
        HintSurface::Pane(pane_id) => state
            .panes
            .get_mut(pane_id)
            .map(|p| &mut p.root as &mut dyn heca_grid_ui::Component),
        HintSurface::Layer(id) => {
            crate::chrome::surface_node_mut(&mut state.window_root, *id).map(|n| n.as_mut())
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
/// Prefixed by the surface, because two surfaces may each hold a `pane:7` and they are not the same
/// pickable thing.
pub(crate) fn target_identity(
    _state: &crate::app_state::AppState,
    target: &HintTarget,
) -> Option<String> {
    let within = target.identity.clone()?;
    let surface = match &target.surface {
        HintSurface::Chrome => "chrome".to_string(),
        HintSurface::Pane(id) => format!("pane:{}", id.0),
        HintSurface::Layer(id) => format!("layer:{id:?}"),
    };
    Some(format!("{surface}/{within}"))
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
    for layer in state.layers.visible_front_to_back() {
        let Some(node) = crate::chrome::surface_node(&state.window_root, layer.id) else {
            continue;
        };
        if heca_grid_ui::fire_action(node, name) {
            return true;
        }
    }
    if heca_grid_ui::fire_action(&state.window_root, name) {
        return true;
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
pub(crate) fn clear_hint_letters(state: &crate::app_state::AppState) {
    heca_grid_ui::clear_hints(&state.window_root);
    for shell in state.panes.values() {
        heca_grid_ui::clear_hints(&shell.root);
    }
    for layer in state.layers.visible_front_to_back() {
        if let Some(node) = crate::chrome::surface_node(&state.window_root, layer.id) {
            heca_grid_ui::clear_hints(node);
        }
    }
}
