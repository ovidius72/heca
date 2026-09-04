//! **What a layer is** — its name, its id, what it holds, and what it declares about itself.
//!
//! The stack that owns them is [`LayerRegistry`](super::LayerRegistry); this file is the data it
//! keeps, and the questions a single layer can answer on its own.

use heca_view::ViewNode;

/// Build a layer's addressable name: `<owner>.<short>` — `layer_name("docker", "expose")` is
/// `"docker.expose"`.
///
/// **The owner half is the caller's `kind()`, never the author's string**, which is what makes the
/// namespace unforgeable: a plugin has nowhere to write a prefix, so it cannot claim `heca.*` or
/// another component's names. The same construction `action_id_for` uses for `<component>.<name>`.
///
/// A `short` that already contains a dot is **rejected** rather than joined, so a second segment
/// cannot be smuggled in (`expose.thing` would otherwise become `docker.expose.thing` and read as
/// if `docker.expose` owned it).
pub(crate) fn layer_name(owner: &str, short: &str) -> Option<String> {
    if short.is_empty() || short.contains('.') || owner.is_empty() {
        return None;
    }
    Some(format!("{owner}.{short}"))
}

/// The namespace the host's own layers live under — the reserved counterpart of a provider's
/// `kind()`. A plugin cannot register here, because it never supplies the owner half.
pub(crate) const HOST_OWNER: &str = "heca";

/// Opaque, stable id for a registered layer. Returned by [`LayerRegistry::add`]; a plugin
/// keeps its id to show/hide/update its layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct LayerId(pub(super) u64);

impl LayerId {
    /// The raw counter value — for deriving the surface's key in the window root
    /// (`chrome::surface_slot`), which is the one place outside this module that needs it.
    pub(crate) fn raw(self) -> u64 {
        self.0
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

/// A registered surface's **bookkeeping** — its identity, its nesting, and what it declares about
/// itself.
///
/// The surface's live tree is **not here**: it is a child of the window root
/// (`docs/surface-compositor.md` § 0.8), which is what lets one walk lay it out, paint it, deliver
/// its pointer events and collect its hint letters. This is what is left once a thing on screen is
/// a node like any other.
pub(crate) struct DynamicLayer {
    pub(crate) id: LayerId,
    /// **Who this surface hangs from — and therefore where it sits in z.**
    ///
    /// `None` is a child of the root. A child is drawn *above* its parent and later siblings above
    /// earlier ones, so z is the chain of sibling indices from the root down
    /// ([`z_path`](LayerRegistry::z_path)) and ordering is lexicographic on it — which is a
    /// pre-order walk, which is paint order. **Nobody writes a z.**
    ///
    /// This replaced a five-variant `LayerBand` enum every caller picked from, which was a stored z
    /// one step removed and is what §6 of the surface-compositor model forbids. Two bugs found in
    /// one day were both "z is stored" bugs and neither is expressible here: a rebuild replaces a
    /// node in place, and there is exactly one order for paint and input to read.
    ///
    /// An overlay is a child of **whatever opened it**, so a modal raised from the exposé sits
    /// above the exposé and goes when it goes — bookkeeping every caller could get wrong in a flat
    /// list, and free in a tree.
    pub(crate) parent: Option<LayerId>,
    /// Set while a **removal** is waiting on the dissolve: the layer is gone as far as its owner is
    /// concerned and only the picture is still playing out. [`LayerRegistry::tick`] drops it.
    pub(crate) doomed: bool,
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
    /// The stable name an action addresses this layer by — `"heca.expose"`,
    /// `"docker.expose"` — or `None` for a layer nobody names (an overlay addressed only by the
    /// `LayerId` its opener kept: a dropdown, a modal, the command palette).
    ///
    /// **A name is `<owner>.<short>`, and the owner half is never written by the author.** It is
    /// stamped by [`layer_name`] from the registering provider's `kind()`, exactly as
    /// `action_id_for` builds `<component>.<name>`, `ActionMeta.owner` is stamped in
    /// `register_provider_actions`, and a context path is derived from `kind()`. A plugin therefore
    /// cannot claim `heca.*` or another plugin's namespace: the API gives it nowhere to put a dot.
    ///
    /// **Type-level, not per placement.** A component seated twice declares one layer name; which
    /// seating an action means is a separate optional argument, the way `[[keys.component]]` takes
    /// an optional `id` and `focus_dock` an optional `dock`.
    pub(crate) name: Option<String>,
    /// **The description this surface was described by**, when it was described rather than built.
    ///
    /// The source of truth a theme reload or a plugin update re-realizes from. The *live tree* is
    /// not here: it is a child of the window root, found by
    /// [`chrome::surface_node`](crate::chrome::surface_node), because a thing on screen is a node in
    /// the one tree and nowhere else (`docs/surface-compositor.md` § 0.8). What remains in this
    /// struct is what the registry is *for* — the name, the nesting, and what the surface declares
    /// about itself.
    pub(crate) node: Option<ViewNode>,
}


impl DynamicLayer {
    /// Is this layer **still in charge** — capturing input, covering the panes, answering as the
    /// front-most modal?
    ///
    /// A dissolving layer is not. It is on screen and it is being painted, but the decision to
    /// dismiss it has already been made, so from that moment it is a picture rather than a modal.
    /// Getting this wrong is not subtle: the exposé closes by dismissing itself and *then* focusing
    /// the pane you chose, and while the dissolve still counted as coverage the focus was refused
    /// by `Domain::Overlay` for the whole length of the animation — every activation blocked, with
    /// `blocked intent from Keyboard` in the log.
    /// `leaving` is the surface's own answer, read from its node — see
    /// [`LayerRegistry::is_leaving`](super::LayerRegistry::is_leaving).
    pub(crate) fn is_active(&self, leaving: bool) -> bool {
        self.visible && !leaving
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A name is `<owner>.<short>`, and the owner half is never written by the author** — so a
    /// plugin cannot claim `heca.*` or another plugin's namespace: the API gives it nowhere to put
    /// a dot.
    #[test]
    fn a_layer_name_is_stamped_from_its_owner_and_cannot_be_forged() {
        assert_eq!(layer_name("docker", "expose").as_deref(), Some("docker.expose"));
        assert_eq!(layer_name(HOST_OWNER, "expose").as_deref(), Some("heca.expose"));
        assert_eq!(layer_name("docker", "expose.thing"), None, "no smuggled second segment");
        assert_eq!(layer_name("docker", "heca.expose"), None, "cannot claim another namespace");
        assert_eq!(layer_name("docker", ""), None);
    }
}
