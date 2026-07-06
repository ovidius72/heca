//! Dynamic layer registry — the persistent, registerable form of the surface stack
//! described in `docs/surface-compositor.md` §9.
//!
//! The built-in surfaces (panes, sidebar/chrome, current overlays) are still derived from
//! their existing `AppState` trees; this registry holds the **dynamically added** layers
//! (an on-demand exposé, a plugin panel, a rich modal) and lets them join the same stack.
//! `chrome::active_hint_targets` composes both — built-ins + registered layers — into one
//! band-ordered stack and applies the single visibility rule.
//!
//! Step 1 of the migration: the registry + its ordering. Native (`Box<dyn Component>`)
//! content only; the `ViewNode` content path (plugins) and paint/input wiring land later.
//!
//! Seam API: the registration/show/hide surface below is consumed by the next migration
//! steps (`ShowLayer`/`HideLayer` actions, the confirm dialog as a layer, plugins), so it
//! carries `#![allow(dead_code)]` like the other chrome seam modules until then.
#![allow(dead_code)]

use heca_grid_ui::Component;

/// Opaque, stable id for a registered layer. Returned by [`LayerRegistry::add`]; a plugin
/// keeps its id to show/hide/update its layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LayerId(u64);

/// Semantic z **band** — replaces raw z numbers (no magic values). Order within a band is
/// insertion order. Higher bands paint/hint **in front of** lower ones.
///
/// Ordinal order (front → back) is `Modal > Overlay > Floating > Content > Background`,
/// given by the enum discriminant via [`LayerBand::rank`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerBand {
    /// Frosted background behind everything. Passive.
    Background,
    /// The base UI: panes.
    Content,
    /// Floating panes, above the tiled content.
    Floating,
    /// Chrome and on-demand panels drawn above the content (sidebar, an exposé).
    Overlay,
    /// Blocking dialogs/menus that capture the context.
    Modal,
}

impl LayerBand {
    /// Front-to-back rank: higher paints/hints in front. Used to order the stack.
    pub(crate) fn rank(self) -> u8 {
        match self {
            LayerBand::Background => 0,
            LayerBand::Content => 1,
            LayerBand::Floating => 2,
            LayerBand::Overlay => 3,
            LayerBand::Modal => 4,
        }
    }
}

/// Whether a layer is always present or shown on demand — the "panes-like vs exposé-like"
/// distinction. Only [`OnDemand`](LayerKind::OnDemand) layers are meaningfully driven by
/// `ShowLayer`/`HideLayer`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LayerKind {
    /// Always in the stack, eligible while its context is active (panes, sidebar).
    Persistent,
    /// Hidden until shown; becomes the active context while up (exposé, palette).
    OnDemand,
}

/// A dynamically registered layer. Its `root` is a native retained tree for now; the
/// `ViewNode` content path (for plugins) is added in a later migration step.
pub(crate) struct DynamicLayer {
    pub(crate) id: LayerId,
    pub(crate) band: LayerBand,
    pub(crate) kind: LayerKind,
    /// Captures the context while active — suppresses everything beneath it.
    pub(crate) modal: bool,
    /// Whether it participates this frame. `Persistent` layers start visible; `OnDemand`
    /// layers start hidden and are shown via [`LayerRegistry::show`].
    pub(crate) visible: bool,
    /// The layer's retained content tree (laid out + painted by the host; walked by
    /// `collect_hint_targets` for its hint targets).
    pub(crate) root: Box<dyn Component>,
}

/// The registry of dynamically added layers, held on `AppState`. The built-in surfaces are
/// **not** stored here (they keep their own trees + lifecycle); this holds only layers added
/// at runtime via [`add`](LayerRegistry::add).
#[derive(Default)]
pub(crate) struct LayerRegistry {
    layers: Vec<DynamicLayer>,
    next: u64,
}

impl LayerRegistry {
    /// Register a new layer and return its id. `Persistent` layers are visible immediately;
    /// `OnDemand` layers start hidden (show them with [`show`](LayerRegistry::show)).
    pub(crate) fn add(
        &mut self,
        band: LayerBand,
        kind: LayerKind,
        modal: bool,
        root: Box<dyn Component>,
    ) -> LayerId {
        let id = LayerId(self.next);
        self.next += 1;
        self.layers.push(DynamicLayer {
            id,
            band,
            kind,
            modal,
            visible: matches!(kind, LayerKind::Persistent),
            root,
        });
        id
    }

    /// Remove a layer entirely.
    pub(crate) fn remove(&mut self, id: LayerId) {
        self.layers.retain(|l| l.id != id);
    }

    /// Show a layer (bring it into the stack this frame). `ShowLayer` dispatches here.
    pub(crate) fn show(&mut self, id: LayerId) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.visible = true;
        }
    }

    /// Hide a layer. `HideLayer` dispatches here.
    pub(crate) fn hide(&mut self, id: LayerId) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.visible = false;
        }
    }

    /// The currently-visible layers, in **front → back** order (highest band first, then
    /// most-recently-added within a band). Consumed by `active_hint_targets`.
    pub(crate) fn visible_front_to_back(&self) -> Vec<&DynamicLayer> {
        let mut out: Vec<&DynamicLayer> = self.layers.iter().filter(|l| l.visible).collect();
        // Stable sort by band rank DESC (front first) — keeps insertion order within a band.
        out.sort_by_key(|l| std::cmp::Reverse(l.band.rank()));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::widgets::Flex;

    fn empty_root() -> Box<dyn Component> {
        Box::new(Flex::row())
    }

    #[test]
    fn on_demand_starts_hidden_persistent_starts_visible() {
        let mut reg = LayerRegistry::default();
        let a = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, empty_root());
        let b = reg.add(LayerBand::Content, LayerKind::Persistent, false, empty_root());
        let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![b], "on-demand hidden until shown; persistent visible");
        reg.show(a);
        // Now both visible; Overlay band ranks in front of Content.
        let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![a, b], "shown overlay sorts in front of content");
        reg.hide(a);
        assert_eq!(reg.visible_front_to_back().len(), 1);
    }

    #[test]
    fn band_orders_front_to_back() {
        let mut reg = LayerRegistry::default();
        let content = reg.add(LayerBand::Content, LayerKind::Persistent, false, empty_root());
        let modal = reg.add(LayerBand::Modal, LayerKind::Persistent, true, empty_root());
        let overlay = reg.add(LayerBand::Overlay, LayerKind::Persistent, false, empty_root());
        let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(order, vec![modal, overlay, content], "Modal > Overlay > Content");
    }
}
