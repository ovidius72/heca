//! Which surface holds the keyboard, and the key a surface claims.
//!
//! A key acts on the surface in front of you: a layer, a focused dock, or the panes. The rule that
//! resolves one is a pure function of plain data, so it is tested without a window.

use crate::app_state::AppState;
use crate::keymap::{KeyCombo, KeymapRegistry};
use std::collections::HashMap;

/// The keymap consulted while a chrome container holds keyboard focus.
///
/// It is a mode **keymap**, not an [`InputMode`](crate::app_state::InputMode): chrome focus already answers
/// "where do the keys go", and a mode beside it would be a second fact that can disagree with the first. Like
/// the `sidebar` map it has no trigger — you enter it by focusing a dock, not by pressing something.
///
/// What lives here is what the **widgets** answer — paging and edges for whatever scroll area the
/// focused container nests — because a scroll region behaves identically wherever it is mounted and
/// no component should have to declare that. What a *component* declares is a separate, per-kind
/// layer (`[keys.<kind>]`) resolved through this same seam.
pub(crate) const FOCUS_LAYER: &str = "focus";

/// The keymap consulted while a **layer** holds the keyboard — the exposé, a modal, a plugin's
/// surface.
///
/// The layer twin of [`FOCUS_LAYER`], and it exists for the same reason: what every layer answers
/// alike does not belong in each layer's own declaration. Today that is `Escape` — the front-most
/// layer closes itself — asserted as a floor in `registry::floors` so a layer that
/// declares nothing is still closable from the keyboard.
pub(crate) const LAYER_FLOOR: &str = "layer";

/// The surface name of the **scrolling area** — the panes, and what is in front when no layer and
/// no dock hold the keyboard.
///
/// It is a surface like any other, so it can be spoken about in `[[keys.surface]]` the way a dock
/// and an overlay already are. It ships with **no bindings of its own**, and that absence is
/// deliberate: a key nothing here claims reaches the program running in the pane, which is why
/// `Escape` gets to mean what it means inside vim.
pub(crate) const PANES_SURFACE: &str = "heca.panes";

/// Which surface holds the keyboard — the one question the key-resolution rule is asked.
///
/// Reduced from [`AppState`] by [`focused_surface`] so the rule itself is a pure function of plain
/// data and can be tested without a window. Ordered from the front backwards: a layer covers a
/// dock, a dock covers the panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FocusedSurface {
    /// A layer is in front. `name` is its addressable, owner-prefixed name — the one `show_layer`
    /// takes — and is `None` for a layer nothing named.
    Layer { name: Option<String> },
    /// A chrome container holds the keyboard: its mount id, and the `kind()` of the provider
    /// mounted there.
    Dock { mount: String, kind: Option<String> },
    /// A widget outside every named area holds it — a control a plugin placed in a sidebar, say.
    /// Its keys go to it and bubble up like any key; no surface's bindings apply.
    Widget,
    /// Nothing is in front of the panes.
    Panes,
}

/// The action a key resolves to in **one named surface's** binding layer — a dock's `kind()`, a
/// placement id, or a layer's own name.
///
/// Split out so a layer and a dock consult the same map by the same rule.
/// Whichever surface holds the keyboard, its `[[keys.surface]]` entry is the more specific thing
/// the key is aimed at, and there is exactly one lookup for both.
fn surface_layer_action(
    component_keymaps: &HashMap<String, KeymapRegistry>,
    surface: &str,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    component_keymaps
        .get(surface)
        .and_then(|map| map.resolve(surface, combo))
        .cloned()
}

/// The action a key resolves to in one **floor** — the keys a whole kind of surface answers alike
/// (`focus` for docks, `layer` for layers), which no individual surface should have to declare.
fn floor_action(
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    floor: &str,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    mode_keymaps
        .get(floor)
        .and_then(|map| map.resolve(floor, combo))
        .cloned()
}

/// **The reserved key**: the way out of a focused dock, if `combo` is it. Answered *before* the
/// tree gets the key, like a browser's reserved shortcuts, so a terminal that takes every key it is
/// sent cannot trap you in the dock. Only the way out is reserved — the rest of the dock's keys
/// (paging, the sidebar's `j`/`k`) are handlers that act on what the tree did not take, see
/// [`surface_action`].
///
/// A layer has none here: it is in the tree, and the tree delivers `Escape` to it as its own
/// dismissal.
pub(crate) fn way_out_action(
    surface: &FocusedSurface,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    match surface {
        FocusedSurface::Dock { .. } => {
            floor_action(mode_keymaps, FOCUS_LAYER, combo).filter(|act| {
                matches!(
                    act,
                    crate::keymap::ActionRef::Builtin(crate::input::WmAction::UnfocusDock)
                )
            })
        }
        FocusedSurface::Layer { .. } | FocusedSurface::Widget | FocusedSurface::Panes => None,
    }
}

/// Which surface holds the keyboard, read off the session — the whole of [`AppState`] this rule
/// needs, so [`surface_action`] can stay a pure function of plain data.
///
/// **The nearest named area around whatever holds the keyboard**, wherever it sits: a popup's keys
/// are the popup's even when a dock opened it, and a widget outside every area answers its own.
/// The panes are in front when the keyboard is with a pane's terminal, or with nothing.
pub(crate) fn focused_surface(state: &AppState) -> FocusedSurface {
    if let Some(id) = state.layers.top_modal_id(&state.window_root) {
        return FocusedSurface::Layer {
            name: state.layers.name_of(id),
        };
    }
    let with_panes = heca_grid_ui::keyboard_holder(&state.window_root).is_none()
        || state
            .window_keyboard_terminal()
            .is_some_and(|id| state.client.backends.pane_of(id).is_some());
    if with_panes {
        return FocusedSurface::Panes;
    }
    surface_of_area(
        heca_grid_ui::keyboard_area(&state.window_root),
        |slot| state.layers.name_of_slot(slot),
        |mount| {
            state
                .chrome_host
                .provider(mount)
                .map(|p| p.kind().to_string())
        },
    )
}

/// The surface a named area is, once the keyboard is known to be away from the panes: a surface is
/// a layer, by its name; a region is a dock, by its mount and kind; no area is the widget itself.
pub(crate) fn surface_of_area(
    area: Option<heca_grid_ui::KeyboardArea>,
    layer_named: impl Fn(&str) -> Option<String>,
    kind_of: impl Fn(&str) -> Option<String>,
) -> FocusedSurface {
    match area {
        Some(heca_grid_ui::KeyboardArea::Surface(slot)) => FocusedSurface::Layer {
            name: layer_named(&slot),
        },
        Some(heca_grid_ui::KeyboardArea::Region(mount)) => {
            let kind = kind_of(&mount);
            FocusedSurface::Dock { mount, kind }
        }
        None => FocusedSurface::Widget,
    }
}

/// The action a key resolves to **in the surface that holds the keyboard**.
///
/// One order, whatever kind of surface it is: the surface's **own** `[[keys.surface]]` declaration
/// first — a dock's rows are the more specific thing a key is aimed at than anything host-wide —
/// then the **floor** its kind is guaranteed. `None` means nobody in front claimed the key, and the
/// caller falls back to the global map.
///
/// A dock is consulted at two names, placement before component: an entry that named an `id` was
/// built into a layer under that mount id, already carrying the id-less base merged underneath it,
/// so finding the mount means the user narrowed this seating and missing it means they spoke about
/// the component as a whole. Two lookups, no merging at press time.
///
/// Both key routes come through here, deliberately: the direct one (an unprefixed key while a dock
/// is focused) and the prefix fall-through (the global `normal` map missed). One declaration, both
/// doors — so a container's `r` works whether the user typed `r` or `prefix+r`.
pub(crate) fn surface_action(
    surface: &FocusedSurface,
    mode_keymaps: &HashMap<String, KeymapRegistry>,
    component_keymaps: &HashMap<String, KeymapRegistry>,
    combo: &KeyCombo,
) -> Option<crate::keymap::ActionRef> {
    let (names, floor): (Vec<&str>, Option<&str>) = match surface {
        FocusedSurface::Layer { name } => {
            (name.as_deref().into_iter().collect(), Some(LAYER_FLOOR))
        }
        FocusedSurface::Dock { mount, kind } => (
            [Some(mount.as_str()), kind.as_deref()]
                .into_iter()
                .flatten()
                .collect(),
            Some(FOCUS_LAYER),
        ),
        // The panes declare nothing by default, and have no floor: a key nothing claims belongs to
        // the program running in the pane.
        FocusedSurface::Widget => (Vec::new(), None),
        FocusedSurface::Panes => (vec![PANES_SURFACE], None),
    };
    names
        .into_iter()
        .find_map(|name| surface_layer_action(component_keymaps, name, combo))
        .or_else(|| floor.and_then(|floor| floor_action(mode_keymaps, floor, combo)))
}
