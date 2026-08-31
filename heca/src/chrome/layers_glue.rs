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
    }
}

/// Lay out every visible dynamically-registered layer (an overlay dialog, a plugin panel)
/// at full viewport size. Mutable pass, run before the scene-texture borrow so
/// [`paint_layers`] can take a shared `&LayerRegistry`. Each layer's root is a self-centering /
/// self-positioning tree (e.g. a [`Dialog`](heca_grid_ui::Dialog) fills the viewport and
/// centers its panel). No-op when the registry is empty. This is the generic replacement for
/// the per-overlay `layout_*` passes (`docs/surface-compositor.md` §9).
pub(crate) fn layout_layers(state: &mut crate::app_state::AppState, w: f32, h: f32) {
    let font = chrome_gui_theme(state).font_size;
    for root in state.layers.visible_roots_mut() {
        LayoutEngine::new()
            .base_font(font)
            .compute(root.as_mut(), Size::new(w as f64, h as f64));
    }
}

/// Paint every visible dynamically-registered layer, **back → front** by band (so a Modal
/// paints over an Overlay paints over Content), on top of the chrome scene. Each layer's root
/// paints itself (overlay widgets draw their own scrim on `cx.with_overlay`). Run
/// [`layout_layers`] first. Generic replacement for the per-overlay `paint_*` passes.
pub(crate) fn paint_layers(
    layers: &LayerRegistry,
    scene: &mut Scene,
    w: f32,
    h: f32,
    theme: &GuiTheme,
) {
    let visible = layers.visible_back_to_front();
    if visible.is_empty() {
        return;
    }
    let mut cx = PaintCx::new(scene, theme).with_viewport(Size::new(w as f64, h as f64));
    for layer in visible {
        // **A surface's arrival and exit are its own** — an `Overlay` paints itself under its
        // `Animation`'s frame, about its own centre, so this pass simply draws each layer. The
        // host used to apply the opacity and the scale here, which is why the capability was
        // reachable only through the registry and never from a widget (F003/P082/T459).
        heca_grid_ui::paint_child(layer.root(), &mut cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::scene::{DrawCommand, HostDraw};
    use heca_grid_ui::widgets::{Flex, Overlay};

    fn root(frosted: bool) -> Box<dyn Component> {
        Box::new(Overlay::new().frosted(frosted).panel(Flex::row()))
    }

    /// Every `HostDraw::Backdrop` radius the scene recorded, in order.
    fn backdrops(scene: &Scene) -> Vec<f32> {
        scene
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Host(h) => match h.draw {
                    HostDraw::Backdrop { radius } => Some(radius),
                    HostDraw::Surface { .. } => None,
                },
                _ => None,
            })
            .collect()
    }

    /// **The frost travels with the surface, not with the registry.**
    ///
    /// The layer stack says nothing about a backdrop any more — a frosted surface asks for its own
    /// blur while it paints, and the host performs the request. That is what lets a layer root move
    /// into the one tree (`P097(F003)/T494`) without the frost breaking: there is no host pass keyed
    /// on the registry left to lose, and no `LayerBackdrop` for a plugin to be unable to reach.
    #[test]
    fn the_frost_comes_from_the_surface_that_asked_for_it() {
        let theme = GuiTheme::default();
        let mut reg = LayerRegistry::default();
        let id = reg.add(None, LayerKind::OnDemand, true, true, root(true));
        reg.show(id);

        let mut scene = Scene::default();
        paint_layers(&reg, &mut scene, 1000.0, 800.0, &theme);

        assert_eq!(backdrops(&scene), vec![theme.colors.overlay_frost_radius]);
    }

    /// **A plain layer summons no blur pass it did not ask for.**
    #[test]
    fn a_plain_layer_records_no_backdrop() {
        let theme = GuiTheme::default();
        let mut reg = LayerRegistry::default();
        let id = reg.add(None, LayerKind::OnDemand, true, true, root(false));
        reg.show(id);

        let mut scene = Scene::default();
        paint_layers(&reg, &mut scene, 1000.0, 800.0, &theme);

        assert!(backdrops(&scene).is_empty());
    }
}
