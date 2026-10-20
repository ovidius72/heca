//! The open overlays: which one is on top, whether content is covered, and closing one.

use std::collections::HashMap;

use heca_view::PropMap;

use super::spec::{ModalResult, OverlayId};
use crate::actions::ActionRegistry;
use crate::app_state::AppState;
use crate::chrome::FormBindings;

/// A caller's completion, run once when the overlay resolves. It receives the live
/// [`AppState`] + [`ActionRegistry`] (so it can dispatch a follow-up action) and the result.
type OverlayCompletion = Box<dyn FnOnce(&mut AppState, &ActionRegistry, ModalResult)>;

/// The host-owned overlay stack: pending completions keyed by overlay id. The overlay's
/// *visual* stack lives in the [`LayerRegistry`](crate::chrome::LayerRegistry); this holds only the
/// result callbacks. Kept on [`AppState`].
#[derive(Default)]
pub struct OverlayHost {
    pub(super) completions: HashMap<OverlayId, OverlayCompletion>,
    /// Per-overlay form bindings — the named value widgets in the modal body, read into
    /// [`ModalResult::Action`]'s `data` when the overlay is submitted.
    pub(super) forms: HashMap<OverlayId, FormBindings>,
}

impl OverlayHost {
    /// Register what runs when overlay `id` resolves.
    ///
    /// The map itself stays private: a completion is only ever *installed* beside the layer that
    /// will resolve it, and only ever *run* by [`resolve`], which removes it. This is the door for
    /// an overlay built outside this module (the command palette).
    pub(crate) fn on_resolve(
        &mut self,
        id: OverlayId,
        completion: impl FnOnce(&mut AppState, &ActionRegistry, ModalResult) + 'static,
    ) {
        self.completions.insert(id, Box::new(completion));
    }
}

/// The front-most open **modal** overlay — the one capturing input — if any. The input path
/// routes keyboard/pointer to its layer root (a self-contained [`Dialog`](heca_grid_ui::Dialog))
/// and swallows everything else while it's up.
pub(crate) fn top_modal(state: &AppState) -> Option<OverlayId> {
    state.layers.top_modal_id(&state.window_root).map(OverlayId)
}

/// Is the tiled area covered by an overlay? The one input `Domain::Overlay` needs
/// — see [`DynamicLayer::lock`](crate::chrome::surfaces::layers::DynamicLayer).
pub(crate) fn content_covered(state: &AppState) -> bool {
    state.layers.content_covered(&state.window_root)
}

/// Read the current values of an overlay's named body fields into a [`PropMap`] — the `data`
/// handed back in [`ModalResult::Action`]. Empty if the overlay has no form (e.g. a plain
/// confirm) or is already gone.
pub(crate) fn collect_form(state: &AppState, overlay: OverlayId) -> PropMap {
    state
        .overlays
        .forms
        .get(&overlay)
        .map(FormBindings::collect)
        .unwrap_or_default()
}

/// Resolve an open overlay: pop its layer + action metadata and run its completion with
/// `result`. The single confirm/dismiss point — a button press, a KeyHint pick, and RPC all
/// funnel here via `SubmitOverlay`/`CloseOverlay` (dispatched in `dispatch_intent`). No-op if
/// the overlay is already gone (double-resolve safe).
pub(crate) fn resolve(
    state: &mut AppState,
    registry: &ActionRegistry,
    overlay: OverlayId,
    result: ModalResult,
) {
    let completion = state.overlays.completions.remove(&overlay);
    state.overlays.forms.remove(&overlay);
    state.layers.remove(&mut state.window_root, overlay.0);
    state.needs_redraw = true;
    // **No mode is restored**. A container's keyboard focus is not a mode, an
    // overlay never takes it away, and it is simply still there when the overlay closes — so there
    // is nothing to put back and no origin to record.

    if let Some(comp) = completion {
        comp(state, registry, result);
    }
}
