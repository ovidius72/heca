//! **Glue between the app and the layer registry** — the three host passes that keep the
//! dynamically-registered layers (an overlay dialog, a plugin panel, the exposé) laid out,
//! painted and current.
//!
//! Split out of `chrome/mod.rs`. Nothing here is new; the passes are unchanged.

use super::*;

/// Re-register a **host-owned** named layer, so what it shows is current.
///
/// The one place a layer name maps to the code that rebuilds it. A layer whose content is derived
/// from app state cannot be kept fresh by signals alone — those replace a prop, never a child — so
/// it is rebuilt, and `add_named` replacing under the same name is what makes that safe.
///
/// An unknown name is a no-op: a plugin's layer is rebuilt by the plugin, not from here.
pub(crate) fn rebuild_named_layer(state: &mut crate::app_state::AppState, name: &str) {
    if Some(name) == layers::layer_name(layers::HOST_OWNER, "expose").as_deref() {
        expose::register(state);
    } else {
        // A layer another crate added is rebuilt from the closure it gave, which the app keeps.
        crate::entry::rebuild_added_layer(state, name);
    }
}

/// **Register (or re-register) the layer called `name`** — the one place a named layer is built,
/// put in the stack, and given back what it was doing.
///
/// `build` gets the sink the layer's own widgets report to: stamped with this layer's id, which is
/// how the router tells "the surface acted on itself" from "the user typed at the app behind it".
/// The id is the same on every rebuild — the sink names it — so a rebuild replaces the layer in place
/// and a widget in the new tree still names a layer that exists.
///
/// **A rebuild keeps its place in the stack and whether it is up**: the layer is re-shown if it was
/// showing, and nothing replays. The exposé and a layer another crate added take this same path.
pub(crate) fn register_named_layer(
    state: &mut crate::app_state::AppState,
    name: &str,
    build: impl FnOnce(&ChromeIntentEmitter) -> Box<dyn heca_grid_ui::Component>,
) -> LayerId {
    let id = state.layers.slot_for_name(name);
    let emit = layer_emitter(&state.event_proxy, surface_key_of(Some(name), id));
    let was_visible = state.layers.is_visible_named(&state.window_root, name);
    let root = build(&emit);
    let id = state.layers.add_named(
        id,
        name.to_string(),
        None,
        LayerKind::OnDemand,
        root,
        &mut state.window_root,
    );
    if was_visible {
        state.layers.show(&mut state.window_root, id);
    }
    state.needs_redraw = true;
    id
}

// **There is no layout pass and no paint pass here any more.** A registered surface is a child of
// the window root, so the one walk lays it out, paints it, delivers its pointer events and collects
// its hint letters — which is the whole point of `P097(F003)` and why the toast's × works
// (`docs/surface-compositor.md` § 0.8). What used to be `layout_layers` and `paint_layers` was the
// registry keeping a second, parallel set of passes over trees the input walk could not reach.

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::scene::{DrawCommand, HostDraw};
    use heca_grid_ui::widgets::{Flex, Overlay};

    /// Every `HostDraw::Backdrop` radius the scene recorded, in order.
    fn backdrops(scene: &Scene) -> Vec<f32> {
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Host(h) => match h.draw {
                    HostDraw::Backdrop { radius, .. } => Some(radius),
                    HostDraw::Surface { .. } => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Place a surface in a window root and paint the root, exactly as a frame does.
    fn painted(frosted: bool) -> Vec<f32> {
        let theme = GuiTheme::default();
        let mut root = new_window_root();
        let surface = Overlay::new()
            .frosted(frosted)
            .panel(Flex::row())
            .default_open(true);
        place_surface(&mut root, "surface:1", Box::new(surface));
        let scene = paint_chrome_root(&mut root, 1000.0, 800.0, &theme);
        backdrops(&scene)
    }

    /// **The frost travels with the surface, and one walk paints it.**
    ///
    /// Nothing here reaches for a registry: the surface is a child of the window root, the root's
    /// own paint walks it, and the blur it asked for comes out in the scene. That is the whole of
    /// `P097(F003)` in one assertion — there is no second pass over a parallel list of trees.
    #[test]
    fn a_frosted_surface_placed_in_the_tree_asks_for_its_blur() {
        assert_eq!(
            painted(true),
            vec![GuiTheme::default().colors.overlay_frost_radius]
        );
    }

    /// **A surface that did not ask summons no blur pass.**
    #[test]
    fn a_plain_surface_records_no_backdrop() {
        assert!(painted(false).is_empty());
    }
}
