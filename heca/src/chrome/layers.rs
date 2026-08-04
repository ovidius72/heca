//! Dynamic layer registry — the persistent, registerable form of the surface stack
//! described in `docs/surface-compositor.md` §9.
//!
//! The built-in surfaces (panes, sidebar/chrome, current overlays) are still derived from
//! their existing `AppState` trees; this registry holds the **dynamically added** layers
//! (an on-demand exposé, a plugin panel, a rich modal) and lets them join the same stack.
//! `chrome::active_hint_targets` composes both — built-ins + registered layers — into one
//! band-ordered stack and applies the single visibility rule.
//!
//! The registry + its ordering, and both content arms: a native `Box<dyn Component>` tree the
//! host built, or a [`ViewNode`] description realized through the one bridge (F003/P082/T339).
//!
//! Seam API: the registration/show/hide surface below is consumed by the next migration
//! steps (`ShowLayer`/`HideLayer` actions, the confirm dialog as a layer, plugins), so it
//! carries `#![allow(dead_code)]` like the other chrome seam modules until then.
#![allow(dead_code)]

use heca_grid_ui::Component;
use heca_view::ViewNode;

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

/// A dynamically registered layer. Its content is either a native retained tree or a
/// [`ViewNode`] description — see [`LayerContent`].
pub(crate) struct DynamicLayer {
    pub(crate) id: LayerId,
    pub(crate) band: LayerBand,
    pub(crate) kind: LayerKind,
    /// Captures the context while active — suppresses everything beneath it.
    pub(crate) modal: bool,
    /// **Does it cover the tiled area?** (F003/P086/T371.)
    ///
    /// The one thing an overlay declares about the app underneath it, and the only input
    /// `Domain::Overlay` needs: while something covers the panes, acting on them is refused. A
    /// modal covers by definition; a **non-modal** overlay — a plugin panel over the scrolling
    /// area — declares it, and gets the same protection the blunt "a modal blocks everything" rule
    /// could never give it. A dropdown or a tooltip covers a corner, not the panes, and says
    /// `false`.
    ///
    /// It is deliberately **not** a policy: a plugin declares what its own overlay obscures, never
    /// what may run while it is up. Otherwise every plugin would end up naming `split_horizontal`.
    ///
    /// **Every `modal` layer sets this**, because a modal captures input and demands a decision —
    /// that is what "a modal is an overlay with coverage" means, and it is what preserves the
    /// blanket block the router used to apply while one was open. The flag earns its keep on the
    /// **non-modal** overlays that had no protection at all.
    pub(crate) covers_content: bool,
    /// Whether it participates this frame. `Persistent` layers start visible; `OnDemand`
    /// layers start hidden and are shown via [`LayerRegistry::show`].
    pub(crate) visible: bool,
    /// What the layer holds — a tree the host built, or a **description** it was handed.
    pub(crate) content: LayerContent,
}

/// What a layer's content **is** (F003/P082/T339).
///
/// Two arms because there are two authors. The host builds its own overlays as native trees; a
/// plugin, a config file or an RPC line can only send a [`ViewNode`] — a description — and it is
/// realized through the **one** bridge (`heca_view_realize::realize`, re-exported as
/// [`chrome::realize`](super::realize)). There is deliberately no second mapper: a layer that drew
/// a description its own way would be a parallel implementation of every widget.
///
/// The `View` arm keeps **both** the node and the tree realized from it. The node is the source of
/// truth — it is what a theme change or a plugin update re-realizes from — and the realized tree is
/// what the host lays out, paints and collects hint targets from. Keeping only the tree would throw
/// away the description; keeping only the node would mean re-realizing every frame.
pub(crate) enum LayerContent {
    /// A retained tree the host built itself.
    Native(Box<dyn Component>),
    /// A description, plus the tree realized from it.
    View {
        node: ViewNode,
        realized: Box<dyn Component>,
    },
}

impl LayerContent {
    /// The live tree, whichever arm this is — what the host lays out, paints and hint-walks.
    pub(crate) fn root(&self) -> &dyn Component {
        match self {
            Self::Native(root) => root.as_ref(),
            Self::View { realized, .. } => realized.as_ref(),
        }
    }

    pub(crate) fn root_mut(&mut self) -> &mut Box<dyn Component> {
        match self {
            Self::Native(root) => root,
            Self::View { realized, .. } => realized,
        }
    }

    /// The description this was realized from, if it came from one.
    pub(crate) fn node(&self) -> Option<&ViewNode> {
        match self {
            Self::Native(_) => None,
            Self::View { node, .. } => Some(node),
        }
    }
}

impl DynamicLayer {
    /// The layer's live tree — see [`LayerContent::root`].
    pub(crate) fn root(&self) -> &dyn Component {
        self.content.root()
    }

    pub(crate) fn root_mut(&mut self) -> &mut Box<dyn Component> {
        self.content.root_mut()
    }
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
        covers_content: bool,
        root: Box<dyn Component>,
    ) -> LayerId {
        let id = LayerId(self.next);
        self.next += 1;
        self.layers.push(DynamicLayer {
            id,
            band,
            kind,
            modal,
            covers_content,
            visible: matches!(kind, LayerKind::Persistent),
            content: LayerContent::Native(root),
        });
        id
    }

    /// Register a layer whose content is a **description**. `realized` must be the tree produced
    /// from `node` by the one bridge — the caller realizes, because realizing needs the theme, an
    /// intent emitter, the hint sink and the form bindings, none of which a registry holds.
    pub(crate) fn add_view(
        &mut self,
        band: LayerBand,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        node: ViewNode,
        realized: Box<dyn Component>,
    ) -> LayerId {
        let id = LayerId(self.next);
        self.next += 1;
        self.layers.push(DynamicLayer {
            id,
            band,
            kind,
            modal,
            covers_content,
            visible: matches!(kind, LayerKind::Persistent),
            content: LayerContent::View { node, realized },
        });
        id
    }

    /// Reserve the next id **without** adding a layer, for the case where the layer's own
    /// content must reference its id *before* the tree exists — e.g. an overlay whose action
    /// buttons carry `SubmitOverlay { overlay: <this id> }`. Pair with [`insert`](Self::insert).
    pub(crate) fn reserve_id(&mut self) -> LayerId {
        let id = LayerId(self.next);
        self.next += 1;
        id
    }

    /// Add a layer under an id previously handed out by [`reserve_id`](Self::reserve_id).
    /// `Persistent` layers are visible immediately; `OnDemand` layers start visible here
    /// too (an overlay is shown the moment it's inserted), unlike [`add`](Self::add).
    pub(crate) fn insert(
        &mut self,
        id: LayerId,
        band: LayerBand,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        root: Box<dyn Component>,
    ) {
        self.layers.push(DynamicLayer {
            id,
            band,
            kind,
            modal,
            covers_content,
            visible: true,
            content: LayerContent::Native(root),
        });
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

    /// Is the tiled area covered by any visible layer? The input to `Domain::Overlay`
    /// (F003/P086/T371).
    ///
    /// **A `modal` layer counts whether or not it declared coverage** — it captures the keyboard and
    /// demands a choice, so acting on the panes behind it is refused by construction. Derived here
    /// rather than trusted at each `insert`, because a call site that passes `false` for a modal
    /// re-opens exactly one hole: the prefix sequence deliberately falls through the overlay key
    /// path (`app/events.rs`, so `prefix+/` can pick a menu entry), reaches the router, and runs.
    /// `prefix+x` with a context menu open raising the close-pane confirm was that hole (user,
    /// 2026-07-30). The declared flag is what a **non-modal** overlay — a plugin panel over the
    /// scrolling area — uses to get the same protection.
    pub(crate) fn content_covered(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.visible && (l.covers_content || l.modal))
    }

    /// The currently-visible layers, in **front → back** order (highest band first, then
    /// most-recently-added within a band). Consumed by `active_hint_targets`.
    pub(crate) fn visible_front_to_back(&self) -> Vec<&DynamicLayer> {
        let mut out: Vec<&DynamicLayer> = self.layers.iter().filter(|l| l.visible).collect();
        // Stable sort by band rank DESC (front first) — keeps insertion order within a band.
        out.sort_by_key(|l| std::cmp::Reverse(l.band.rank()));
        out
    }

    /// The currently-visible layers in **back → front** paint order (Background first, Modal
    /// last on top). Consumed by `paint_layers`.
    pub(crate) fn visible_back_to_front(&self) -> Vec<&DynamicLayer> {
        let mut out = self.visible_front_to_back();
        out.reverse();
        out
    }

    /// Mutable roots of the visible layers (order-independent) — for the per-frame layout pass.
    pub(crate) fn visible_roots_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Component>> {
        self.layers
            .iter_mut()
            .filter(|l| l.visible)
            .map(|l| l.content.root_mut())
    }

    /// The id of the front-most visible **modal** layer (the one that captures input), if any.
    /// Front-most = most-recently inserted (a later modal opens on top of an earlier one).
    pub(crate) fn top_modal_id(&self) -> Option<LayerId> {
        self.layers
            .iter()
            .rev()
            .find(|l| l.visible && l.modal)
            .map(|l| l.id)
    }

    /// The root of the front-most visible modal layer, mutably — the input target while a modal
    /// is up. Pairs with [`top_modal_id`](Self::top_modal_id).
    pub(crate) fn top_modal_root_mut(&mut self) -> Option<&mut (dyn Component + 'static)> {
        self.layers
            .iter_mut()
            .rev()
            .find(|l| l.visible && l.modal)
            .map(|l| l.content.root_mut().as_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::widgets::Flex;

    fn empty_root() -> Box<dyn Component> {
        Box::new(Flex::row())
    }

    /// **A modal covers whatever it declared** — the property no call site can get wrong
    /// (F003/P086/T371). A layer that captures the keyboard and demands a choice must refuse acts on
    /// the panes behind it; the alternative is trusting a `bool` at every `insert`, and the one that
    /// passed `false` let `prefix+x` raise the close-pane confirm with a context menu open (user,
    /// 2026-07-30).
    #[test]
    fn a_modal_covers_the_content_even_if_it_says_otherwise() {
        let mut reg = LayerRegistry::default();
        // Deliberately declaring `false`, as the dropdown path once did.
        reg.insert(LayerId(7), LayerBand::Overlay, LayerKind::OnDemand, true, false, empty_root());
        assert!(
            reg.content_covered(),
            "capturing input IS coverage, whatever the flag says",
        );
    }

    /// The one input `Domain::Overlay` reads: a *visible* covering layer, and only that
    /// (F003/P086/T371). A layer that covers but is hidden is not covering anything.
    #[test]
    fn coverage_is_reported_only_while_the_layer_is_visible() {
        let mut reg = LayerRegistry::default();
        let corner = reg.add(LayerBand::Overlay, LayerKind::Persistent, false, false, empty_root());
        assert!(!reg.content_covered(), "a non-covering layer covers nothing");

        let over = reg.add(LayerBand::Modal, LayerKind::OnDemand, true, true, empty_root());
        assert!(!reg.content_covered(), "on-demand starts hidden");
        reg.show(over);
        assert!(reg.content_covered(), "shown, and it covers the panes");
        reg.hide(over);
        assert!(!reg.content_covered());
        reg.remove(corner);
    }

    #[test]
    fn on_demand_starts_hidden_persistent_starts_visible() {
        let mut reg = LayerRegistry::default();
        let a = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, false, empty_root());
        let b = reg.add(LayerBand::Content, LayerKind::Persistent, false, false, empty_root());
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
        let content = reg.add(LayerBand::Content, LayerKind::Persistent, false, false, empty_root());
        let modal = reg.add(LayerBand::Modal, LayerKind::Persistent, true, false, empty_root());
        let overlay = reg.add(LayerBand::Overlay, LayerKind::Persistent, false, false, empty_root());
        let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(order, vec![modal, overlay, content], "Modal > Overlay > Content");
    }

    /// **A described layer is a real layer.** It sorts, shows, hides and covers exactly like a
    /// native one — the arm decides where the tree came from, never how the stack treats it.
    #[test]
    fn a_view_layer_behaves_like_any_other_and_keeps_its_description() {
        use heca_view::{ViewNode, WidgetKind};
        let mut reg = LayerRegistry::default();
        let native = reg.add(LayerBand::Content, LayerKind::Persistent, false, false, empty_root());
        let node = ViewNode::new(WidgetKind::Label);
        let described = reg.add_view(
            LayerBand::Overlay,
            LayerKind::Persistent,
            false,
            true,
            node,
            empty_root(),
        );

        let order: Vec<LayerId> =
            reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(order, vec![described, native], "band decides order, not the content arm");
        assert!(reg.content_covered(), "a described layer declares coverage like any other");

        // The description is KEPT, not thrown away once realized: it is what a theme reload or a
        // plugin update re-realizes from.
        let layer = reg
            .visible_front_to_back()
            .into_iter()
            .find(|l| l.id == described)
            .expect("the described layer is in the stack");
        assert!(layer.content.node().is_some(), "the ViewNode survives realization");
        assert_eq!(
            layer.content.node().map(|n| n.kind),
            Some(WidgetKind::Label),
        );

        // And a native layer has no description to offer — the arms are not interchangeable.
        let native_layer = reg
            .visible_front_to_back()
            .into_iter()
            .find(|l| l.id == native)
            .expect("the native layer is in the stack");
        assert!(native_layer.content.node().is_none());
    }
}
