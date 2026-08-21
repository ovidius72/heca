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
    pub(super) fn new(surface: &HintSurface, path: Vec<usize>) -> Self {
        Self { surface: surface.clone(), path }
    }
}

/// The retained tree a [`HintSurface`] names, or `None` when it is gone.
pub(super) fn hint_surface_root<'a>(
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
