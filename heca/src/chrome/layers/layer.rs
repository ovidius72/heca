//! **What a layer is** — its name, its id, what it holds, and what it declares about itself.
//!
//! The stack that owns them is [`LayerRegistry`](super::LayerRegistry); this file is the data it
//! keeps, and the questions a single layer can answer on its own.

use heca_grid_ui::animation::Presence;
use heca_grid_ui::Component;
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
    /// What the layer holds — a tree the host built, or a **description** it was handed.
    pub(crate) content: LayerContent,
    /// The tree realized from `content` when it is a [`LayerContent::View`]; `None` for a
    /// `Native` layer, whose tree *is* its content.
    ///
    /// Host bookkeeping **about** the content, deliberately not inside it: `LayerContent` is the
    /// vocabulary a plugin author reads, and a realization cache is not part of that vocabulary.
    /// It lives here for the same reason `visible` does. Realizing needs the theme, an intent
    /// emitter, the hint sink and the form bindings — none of which a registry holds — so the host
    /// realizes first and registers both.
    pub(crate) realized: Option<Box<dyn Component>>,
}

/// What a layer's content **is** (F003/P082/T339).
///
/// Two arms because there are two authors. The host builds its own overlays as native trees; a
/// plugin, a config file or an RPC line can only send a [`ViewNode`] — a description — and it is
/// realized through the **one** bridge (`heca_view_realize::realize`, re-exported as
/// [`chrome::realize`](super::realize)). There is deliberately no second mapper: a layer that drew
/// a description its own way would be a parallel implementation of every widget.
///
/// The arm holds the **description only** — the shape ratified in the surface-compositor model
/// (`docs/surface-compositor.md` §9). The node is the source of truth a theme reload or a plugin update re-realizes
/// from; the tree realized from it is host bookkeeping and lives on
/// [`DynamicLayer::realized`](DynamicLayer#structfield.realized), outside the vocabulary a plugin
/// author reads.
pub(crate) enum LayerContent {
    /// Built in Rust — chrome, a pane header, an overlay the host assembled.
    Native(Box<dyn Component>),
    /// Data- or plugin-described; the host `realize()`s it.
    View(ViewNode),
}

impl LayerContent {
    /// The description this layer was described by, if it was.
    pub(crate) fn node(&self) -> Option<&ViewNode> {
        match self {
            Self::Native(_) => None,
            Self::View(node) => Some(node),
        }
    }
}

impl DynamicLayer {
    /// **How opaque this layer's surface is drawing itself this frame.**
    ///
    /// The layer does not fade its own content — the surface does, through its
    /// [`Animation`](heca_grid_ui::Animation), and it paints that itself, backdrop included. This
    /// is left for the tests that assert an exit is playing, and for a host that needs to read a
    /// surface's progress without knowing what animation it declared.
    ///
    /// `1.0` for a surface that declared no animation, which is most of them.
    pub(crate) fn opacity(&self) -> f32 {
        self.root()
            .presence()
            .map_or(1.0, |p| p.frame().opacity)
    }

    /// Is this layer **still in charge** — capturing input, covering the panes, answering as the
    /// front-most modal?
    ///
    /// A dissolving layer is not. It is on screen and it is being painted, but the decision to
    /// dismiss it has already been made, so from that moment it is a picture rather than a modal.
    /// Getting this wrong is not subtle: the exposé closes by dismissing itself and *then* focusing
    /// the pane you chose, and while the dissolve still counted as coverage the focus was refused
    /// by `Domain::Overlay` for the whole length of the animation — every activation blocked, with
    /// `blocked intent from Keyboard` in the log.
    pub(crate) fn is_active(&self) -> bool {
        self.visible && !self.is_leaving()
    }

    /// **Is this surface still leaving?** Any part of its exit still playing — the dissolve, the
    /// shrink, or both.
    ///
    /// A surface's exit is one gesture made of several effects, and it is not gone until every one
    /// of them has finished. Everything that asks "is it still there" reads *this*, so a surface can
    /// sequence its exit — hold the picture, shrink, then dissolve — without any of them retiring it
    /// early. Tying the lifetime to the **fade alone** is what made `Fade::delay` unusable: delaying
    /// the dissolve left the cards on screen after the map itself had gone, so the feature was built
    /// and reverted (F003/P082/T327, 2026-08-12).
    pub(crate) fn is_leaving(&self) -> bool {
        self.root().presence().is_some_and(Presence::is_leaving)
    }

    /// The live tree — what the host lays out, paints and hint-walks.
    ///
    /// For a `Native` layer that is the content itself; for a `View` layer it is
    /// [`realized`](Self::realized), which the host produced from the description before
    /// registering. The arm says where the tree came from, never how the stack treats it.
    pub(crate) fn root(&self) -> &dyn Component {
        match &self.content {
            LayerContent::Native(root) => root.as_ref(),
            LayerContent::View(_) => self
                .realized
                .as_deref()
                .expect("a View layer is registered with its realized tree (add_view/insert_view)"),
        }
    }

    pub(crate) fn root_mut(&mut self) -> &mut Box<dyn Component> {
        match &mut self.content {
            LayerContent::Native(root) => root,
            LayerContent::View(_) => self
                .realized
                .as_mut()
                .expect("a View layer is registered with its realized tree (add_view/insert_view)"),
        }
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
