//! **Opening a menu** — the one path the mouse and the keyboard share — and turning the identity a
//! menu's declarer published into the target its providers read.

use heca_core::layout::{PaneId, Point};

use super::registry::plugin_providers_for;
use super::{ContextPath, ContextTarget};
use crate::app::interaction::InteractionSource;
use crate::app_state::AppState;
use crate::chrome::{DropdownSpec, open_dropdown};
use crate::providers::ChromeCtx;

/// Open the context menu for a resolved `(path, target)` pair: registry lookup → merged items
/// → [`open_dropdown`] (the host-owned overlay). Both the mouse right-click and the keyboard
/// `OpenContextMenu` route here.
///
/// **No mode is restored when it closes**: chrome focus is not a mode, an overlay
/// does not take it away, and it is simply still there afterwards — so there is nothing to record
/// and no `origin` parameter to pass.
pub(crate) fn open_context_menu_for(
    state: &mut AppState,
    path: &str,
    target: ContextTarget,
    anchor: Point,
    source: InteractionSource,
) {
    // Providers — built-in and plugin alike — build against the host facade, never `AppState`. An
    // observe-only context is right here: building a menu reads state, it never renders.
    let ctx = ChromeCtx::new(&state.chrome_state);

    // Plugin entries are collected from the MOUNTED providers at open time, rather than
    // pre-registered at activation: a menu is then always built from what is mounted *now*, so an
    // unmounted provider's entries cannot linger and there is no registration to keep in sync.
    let plugin = plugin_providers_for(&state.chrome_host, &ctx, path);
    let items = state
        .context_menu_registry
        .items_for(&ctx, path, &target, plugin);
    // A keyboard/RPC-opened menu is centered on its anchor (window center); a mouse-opened menu
    // is placed down-right of the click point.
    let centered = matches!(source, InteractionSource::Keyboard);
    open_dropdown(
        state,
        DropdownSpec {
            anchor,
            items,
            source,
            centered,
        },
    );
}

/// Resolve the active context for a keyboard-opened context menu (`OpenContextMenu` / `prefix+>`):
/// **the focused content pane**, and nothing else.
///
/// It used to also resolve the focused container's cursor row, by asking the component which menu
/// path described that key. That whole route is gone: a row **declares its own menu**, so the keyboard
/// trigger finds it by walking the tree from the container's cursor (`chrome::open_declared_menu_for_focus`)
/// and never asks the host to name a menu. What is left here is the fallback for a content pane, which is not
/// a widget and cannot carry a declaration.
///
/// Returns `None` when no pane is focused.
pub(crate) fn resolve_active_context(state: &AppState) -> Option<(ContextPath, ContextTarget)> {
    Some((
        ContextPath(ContextPath::PANE.to_string()),
        ContextTarget::Pane {
            pane_id: crate::app::interaction::focused_pane_id(state)?,
            hyperlink: None,
        },
    ))
}

/// **Turn the identity a menu's declarer published into the target its providers read.**
///
/// A pane declares `pane:<id>` about itself already — the same declaration the keyboard cursor, the
/// right-click target and a drag all read — so a menu declared on a pane arrives here saying what
/// it is about, and nothing has to hit-test to find out.
///
/// The hyperlink is resolved here rather than by the pane, and that is the one thing this path
/// still owes to the terminal: whether the clicked cell holds a link is a question about the
/// terminal's own contents, and until the terminal is a component the only thing that can answer is
/// the host. A keyboard-opened menu passes no point and so never gets a link entry, which is right
/// — there is no cell under a keystroke.
pub(crate) fn target_from_subject(
    state: &AppState,
    subject: Option<&str>,
    at: Option<heca_core::layout::Point>,
) -> ContextTarget {
    let Some(pane_id) = subject.and_then(pane_id_from_key) else {
        return ContextTarget::Contribution;
    };
    let hyperlink = at.and_then(|p| {
        crate::chrome::panes::terminal::of_pane(state, pane_id)?.link_at((p.x as f32, p.y as f32))
    });
    ContextTarget::Pane { pane_id, hyperlink }
}

/// `pane:<id>` → the pane. The identity a pane publishes about itself; anything else is not a pane.
pub(super) fn pane_id_from_key(key: &str) -> Option<PaneId> {
    key.strip_prefix("pane:")?.parse().ok().map(PaneId)
}
