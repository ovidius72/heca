//! **What owns the screen right now** — the domain a request is judged in.

use super::types::{InteractionSource, SurfaceKey};
use crate::app_state::AppState;
use heca_core::layout::{FocusDomain, Layout};

/// **What owns the screen right now** — the axis a policy is judged against.
///
/// The other half of the pair: [`ActionPolicy`] says *what kind of act this is*, `Domain` says
/// *what the app is currently doing*. Both are needed, and only the policy half was modelled:
/// `FocusDomain` is `Tiled | Floating`, read off the session, so the six policy values could only
/// ever answer "is a pane floating?". A focused container lives in `chrome_state` and an overlay in
/// `state.layers`, so neither was visible to policy at all — they were handled by ad-hoc checks
/// beside it (a blunt "block everything while a modal is up") or not at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Domain {
    /// A pane has the keyboard — the ordinary case.
    Tiled,
    /// A floating pane is active.
    Floating,
    /// A **dock** has the keyboard.
    Container,
    /// Something **covers the tiled area**: a modal, or a plugin overlay that declared it obscures
    /// the panes. Acting on panes you cannot see is the thing this exists to stop. `terminal_keys`
    /// says a terminal shown in front holds the keyboard, so its own keys still reach it.
    Overlay { terminal_keys: bool },
}

/// Which domain the app is in for an interaction from `source`.
///
/// **Coverage first, then the keyboard, then the session.** Decided explicitly rather than left to
/// match order:
///
/// - an overlay that covers the tiled area wins over everything — whatever else is true, the panes
///   are not what the user is looking at;
/// - a container holding the keyboard decides for **keyboard-driven** interactions (a key, a
///   component's own `perform`, or a script), even with a floating pane active: the user is driving
///   the dock. A mouse click lands *on* something, so it is judged by what it landed on, not by
///   where the keyboard happens to be — and a script lands on nothing, which is precisely why it
///   belongs with the keys rather than with the clicks. Without this, a component's
///   `ContainerFocused` verb would be unreachable over RPC *even with its dock focused*;
/// - otherwise the session's own `Tiled | Floating`.
pub(crate) fn domain_for(state: &AppState, source: InteractionSource) -> Domain {
    if base_context_is_dormant(
        state
            .layers
            .top_modal_id(&state.window_root)
            .map(|id| state.layers.surface_key(id)),
        crate::chrome::content_covered(state),
        source,
    ) {
        // A terminal shown in what covers the panes, holding the keyboard — not a pane's terminal
        // behind it, which may still hold it under something that did not take it.
        let terminal_keys = state.window_keyboard_terminal().is_some()
            && heca_grid_ui::keyboard_owner(&state.window_root).is_some_and(|o| o.in_surface);
        return Domain::Overlay { terminal_keys };
    }
    let keyboard_driven = matches!(
        source,
        InteractionSource::Keyboard | InteractionSource::Provider | InteractionSource::Rpc
    );
    // **A modal layer holds the keyboard, so no dock does.** `Domain::Container` means "a dock is
    // being driven", and it is what permits a component's own cursor verbs
    // (`workspaces.delete_selected`, the `j`/`k` nav). While a menu, the palette or the exposé is
    // up, the keys belong to *it* — so a key it had no use for must not fall through and drive the
    // dock underneath it: with a row's menu open, `j`/`k` must not move the pane cursor behind it.
    //
    // The layer used to swallow every key it did not want, by hand, which is the same rule written
    // in the wrong place: it also ate `q` and `Esc`, which are catalogued actions the host resolves.
    // Here the layer claims only the keyboard, and `ActionPolicy` decides the
    // rest — `Global` actions still run, `ContainerFocused` ones do not.
    //
    // The same holds for any surface that holds the keyboard, modal or not, and for a widget
    // outside every dock: the dock is driven only while its own area is where the keys go.
    if keyboard_driven
        && matches!(
            crate::app::input::focused_surface(state),
            crate::app::input::FocusedSurface::Dock { .. }
        )
    {
        return Domain::Container;
    }
    session_domain(state.layout())
}

/// **Is the base context — panes, sidebar, floats — dormant?** The coarse half of
/// `docs/surface-compositor.md` §2.
///
/// It used to be a single call to `chrome::content_covered` — a fact about **what is painted
/// over** — used to decide **which context is live**. The two are orthogonal, and the exposé is
/// exactly where they disagree: the map declares `lock: false` because you can still see
/// the panes through it, and that geometric truth was also, accidentally, saying "the base context
/// is still live". So `prefix+j` moved the focused pane behind the map, `prefix+p` opened the
/// palette over it, and `prefix+/` picked sidebar rows the user could not see. §6's invariant names the shape of that bug: if you are special-casing a surface,
/// the surface is mis-modelled.
///
/// So the coarse question is asked coarsely. `modal` on a layer already answers it — it means
/// "this layer takes the keyboard" (`layers.rs`) — and coverage goes back to meaning only what it
/// says: with nothing holding the keyboard, something opaque over the tiled area still means the
/// panes are not what the user is looking at.
///
/// **The one exception is the active surface acting on itself.** The map's `x`, `r` and `d` are its
/// own declarations, dispatched by its own tree, and they arrive as keys exactly like `prefix+j`
/// does — only [`InteractionSource::Surface`] separates them. They fall through and are judged like
/// anyone else's, by the action's declared policy against the session's domain. That is not an
/// allow-list: a plugin's layer is treated identically for whatever actions it declares, which is
/// the point.
pub(super) fn base_context_is_dormant(
    active_context: Option<SurfaceKey>,
    content_covered: bool,
    source: InteractionSource,
) -> bool {
    match active_context {
        Some(active) => source != InteractionSource::Surface(active),
        None => content_covered,
    }
}

/// The session's own half of the domain — what [`FocusDomain`] already says.
pub(crate) fn session_domain(layout: Layout<'_>) -> Domain {
    match is_floating_domain(layout) {
        true => Domain::Floating,
        false => Domain::Tiled,
    }
}

/// Returns `true` when the active workspace is in `FocusDomain::Floating`.
pub(crate) fn is_floating_domain(layout: Layout<'_>) -> bool {
    layout
        .active_workspace()
        .map(|ws| ws.focus_domain == FocusDomain::Floating)
        .expect("active workspace must exist when checking focus domain")
}

/// Returns the `FocusDomain` of the active workspace.
///
/// Convenience wrapper for code that needs to branch on the domain directly
/// rather than just checking `is_floating_domain()`.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by handlers and sidebar routing in Phase E"
    )
)]
pub(crate) fn active_focus_domain(layout: Layout<'_>) -> FocusDomain {
    layout
        .active_workspace()
        .map(|ws| ws.focus_domain)
        .expect("active workspace must exist when checking focus domain")
}
