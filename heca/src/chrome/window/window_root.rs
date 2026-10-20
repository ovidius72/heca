//! **The window root** — the chrome and every surface placed beside it, as children of one tree.

use crate::chrome::LayerId;
use crate::chrome::panes::workspace;
use heca_grid_ui::Component;
use heca_grid_ui::builders::{IdentityExt, LayoutExt};
use heca_grid_ui::style::Length;
use heca_grid_ui::widgets::Flex;

/// **A fresh, empty window root** — what [`AppState::window_root`](crate::app_state::AppState)
/// starts as, before any chrome has been built or any surface placed.
///
/// It fills the window and imposes nothing else: the chrome subtree sizes itself in real pixels and
/// a surface placed beside it takes itself out of the flow, so this level changes no geometry. It
/// exists to **outlive** the chrome, not to lay anything out.
pub(crate) fn new_window_root() -> Flex {
    Flex::column().width(Length::FULL).height(Length::FULL)
}

/// **The chrome subtree's identity in the window root.** Its slot is found by this, never by
/// position — a surface may be placed before the first chrome is ever built (the toast stack is,
/// at startup), and a positional "child 0" would then seat the chrome *over* it.
pub(crate) const CHROME_KEY: &str = "heca.chrome";

/// **Seat a freshly built chrome subtree in the window root**, keeping every surface beside it.
///
/// Doing it this way rather than replacing the retained tree is the whole point of the extra level.
/// The chrome is rebuilt on a resize, a sidebar toggle and a theme reload — and dropped outright by
/// `reload_config` — all of which happen while an overlay is open. None of them may take it with
/// them.
///
/// It goes **right after the workspace**, so it is drawn over the panes, and every surface placed
/// beside it paints and hit-tests above it: child order is z-order in one tree, which is what
/// replaces the layer stack's separate sort (`docs/surface-compositor.md` § 0.6).
pub(crate) fn seat_chrome(root: &mut Flex, chrome: Flex) {
    let chrome = Box::new(chrome.key(CHROME_KEY));
    let children = &mut root.base_mut().children;
    match children
        .iter()
        .position(|c| c.base().key.as_deref() == Some(CHROME_KEY))
    {
        Some(at) => children[at] = chrome,
        None => {
            let after_workspace = children
                .iter()
                .position(|c| c.base().key.as_deref() == Some(workspace::WORKSPACE_KEY))
                .map_or(0, |at| at + 1);
            children.insert(after_workspace, chrome);
        }
    }
}

/// **A registry-owned surface's key in the window root.**
///
/// One scheme, derived from the id the registry already allocates, so the tree and the registry
/// cannot disagree about which node is which layer. A surface that has left the registry entirely
/// (the toast stack) declares its own name instead.
pub(crate) fn surface_slot(id: LayerId) -> String {
    format!("surface:{}", id.raw())
}

/// The live tree of a registry-owned surface, if it is placed.
pub(crate) fn surface_node(root: &Flex, id: LayerId) -> Option<&dyn Component> {
    let key = surface_slot(id);
    root.base()
        .children
        .iter()
        .find(|c| c.base().surface_slot.as_deref() == Some(key.as_str()))
        .map(|c| c.as_ref())
}

/// The same, mutably — for a caller that must drive the surface (open it, tick it, fire a pick in
/// it) rather than only read it.
pub(crate) fn surface_node_mut(root: &mut Flex, id: LayerId) -> Option<&mut Box<dyn Component>> {
    let key = surface_slot(id);
    root.base_mut()
        .children
        .iter_mut()
        .find(|c| c.base().surface_slot.as_deref() == Some(key.as_str()))
}

/// **Take a surface out of the window root.** The counterpart of [`place_surface`]; an unknown key
/// is a no-op, so removing twice is safe.
pub(crate) fn remove_surface(root: &mut Flex, key: &str) {
    root.base_mut()
        .children
        .retain(|c| c.base().surface_slot.as_deref() != Some(key));
}

/// **Place a surface in the window root** — the whole of "how do I put something on screen"
/// (`docs/surface-compositor.md` § 0.3).
///
/// It is a child, like any widget: the one walk lays it out, paints it, delivers its pointer events
/// and collects its hint letters, with nothing registered and no dispatch function added for it.
///
/// Positioned out of the flow at the full viewport, so it takes no space from the chrome beside it
/// and places its own content within itself — which is what `at_rect` means and why layers needed
/// no new layout capability to become children.
///
/// Re-placing under the same `key` **replaces** that surface, so a rebuild is a swap rather than a
/// second copy accumulating behind the first.
pub(crate) fn place_surface(root: &mut Flex, key: &str, surface: Box<dyn Component>) {
    let mut surface = surface;
    // **The slot, not the author's name.** `key` is left alone, so a surface that named itself
    // (`.key("confirm.close")`) can still be pointed at once it is on screen.
    surface.base_mut().surface_slot = Some(key.to_string());
    // Seated above the page, so it answers the pointer the way a positioned wrapper does in a
    // browser: through, except where it covers something. The author of the surface writes nothing
    // for this and cannot get it wrong.
    surface.base_mut().surface = true;
    // **The seat says where, the surface says how big.** Out of the flow at the window's origin,
    // with both sizes left to the surface: every layer-shaped one already declares itself
    // full-viewport (`Overlay`, `ToastStack`, `CommandPalette` all set `Percent(1.0)` in their own
    // constructors), so they are unchanged — while a surface that is *not* a layer keeps the size
    // it gives itself. A `ContextMenu` is the case: the widget **is** its panel, so being handed
    // the viewport would stretch it down the whole window and make every point clickable as the
    // menu.
    surface.base_mut().style.layout.placement = Some(heca_grid_ui::style::Placement {
        left: Length::Percent(0.0),
        top: Length::Percent(0.0),
        width: Length::Auto,
        height: Length::Auto,
    });
    let children = &mut root.base_mut().children;
    match children
        .iter()
        .position(|c| c.base().surface_slot.as_deref() == Some(key))
    {
        Some(at) => children[at] = surface,
        None => children.push(surface),
    }
}
