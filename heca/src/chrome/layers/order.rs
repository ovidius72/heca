//! **Where a surface sits, and who is in charge** — z as a path through the tree, the active
//! context, and the iteration every paint and input pass reads.
//!
//! Kept apart from the rest of [`LayerRegistry`](super::LayerRegistry) because it is a different
//! job: that file registers surfaces and runs their lifetime; this one answers *order*. Nobody
//! writes a z — it is the chain of sibling indices from the root down, so paint order and input
//! order cannot disagree.

use super::{DynamicLayer, LayerId, LayerRegistry};

impl LayerRegistry {
    /// **This surface's z, as a path.** The chain of sibling indices from the root down to it —
    /// `[4, 0]` is the first child of the fifth root surface.
    ///
    /// Ordering is lexicographic on this, which is a pre-order walk, which is paint order. Three
    /// things fall out of that and none of them is written anywhere: a child is above its parent,
    /// a later sibling is above an earlier one, and inserting in the middle is an insert at a
    /// sibling index — no renumbering, no fractional z.
    pub(super) fn z_path(&self, id: LayerId) -> Vec<usize> {
        let mut path = Vec::new();
        let mut at = Some(id);
        // Up the parent chain, recording where each one stands among its own siblings. A cycle
        // cannot be built through the API (a parent is always an id that already exists), but the
        // walk is bounded by the layer count regardless so a malformed registry cannot hang a frame.
        for _ in 0..=self.layers.len() {
            let Some(this) = at else { break };
            let Some(layer) = self.layers.iter().find(|l| l.id == this) else { break };
            let among = self
                .layers
                .iter()
                .filter(|s| s.parent == layer.parent)
                .position(|s| s.id == this)
                .unwrap_or(0);
            path.push(among);
            at = layer.parent;
        }
        path.reverse();
        path
    }

    /// The active context surface, or `None` for the base context. See
    /// [`current`](DynamicLayer#structfield.parent) — an overlay hangs from this.
    pub(crate) fn current(&self) -> Option<LayerId> {
        self.current
    }

    /// Make `id` the active context, remembering what to go back to.
    ///
    /// Only a **context** surface calls this. An ordinary overlay — a dropdown, a tooltip — is a
    /// child of the current context and does not become one.
    pub(super) fn enter_context(&mut self, id: LayerId) {
        if self.current == Some(id) {
            return;
        }
        self.context_stack.push(self.current);
        self.current = Some(id);
    }

    /// Leave `id` as the active context, restoring the one beneath it.
    ///
    /// A no-op when `id` is not the current context: closing a surface that something else has
    /// already opened over must not steal the context from it.
    pub(super) fn leave_context(&mut self, id: LayerId) {
        if self.current != Some(id) {
            // It is somewhere further down the stack — drop it from the history so restoring never
            // lands on a surface that has gone.
            self.context_stack.retain(|c| *c != Some(id));
            return;
        }
        self.current = self.context_stack.pop().flatten();
    }

    /// The currently-visible layers, in **front → back** order (highest band first, then
    /// most-recently-added within a band). Consumed by `active_hint_targets`.
    pub(crate) fn visible_front_to_back(&self) -> Vec<&DynamicLayer> {
        let mut out: Vec<&DynamicLayer> = self.layers.iter().filter(|l| l.visible).collect();
        // Lexicographic on the z-path, reversed: the deepest, latest surface is the front-most.
        out.sort_by_key(|l| std::cmp::Reverse(self.z_path(l.id)));
        out
    }

    /// The currently-visible layers in **back → front** paint order (Background first, Modal
    /// last on top). Consumed by `paint_layers`.
    pub(crate) fn visible_back_to_front(&self) -> Vec<&DynamicLayer> {
        let mut out = self.visible_front_to_back();
        out.reverse();
        out
    }

    /// The id of the front-most visible **modal** layer (the one that captures input), if any.
    /// Front-most = most-recently inserted (a later modal opens on top of an earlier one).
    /// The front-most active modal layer, **in the order the user is looking at** — band first,
    /// insertion order within the band. The same order [`visible_front_to_back`] paints in.
    ///
    /// ⚠️ This used to be "the last one added", ignoring the band entirely — so there were **two
    /// answers to which layer is in front**: painting used the band, input used insertion order,
    /// and nothing made them agree. A `Modal`-band dialog was drawn over the exposé while an
    /// `Overlay`-band map added after it quietly took the pointer, which is how a click on a
    /// confirm dialog's "Close" reached the map behind it and deleted nothing (Antonio, driving,
    /// 2026-08-11).
    ///
    /// A component author never calls this and never declares anything for it: a layer says which
    /// **band** it belongs to, and that is the whole of what it has to know.
    pub(super) fn top_modal_index(&self, window: &heca_grid_ui::widgets::Flex) -> Option<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_active(self.is_leaving(window, l.id)) && l.modal)
            // The front-most is the greatest z-path. Paint reads the same order, so the two can no
            // longer disagree about which surface is in front.
            .max_by_key(|(_, l)| self.z_path(l.id))
            .map(|(i, _)| i)
    }

    pub(crate) fn top_modal_id(&self, window: &heca_grid_ui::widgets::Flex) -> Option<LayerId> {
        self.top_modal_index(window).map(|i| self.layers[i].id)
    }
}
