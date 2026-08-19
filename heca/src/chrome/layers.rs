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

use heca_grid_ui::effects::{Fade, Zoom};
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
pub(crate) struct LayerId(u64);

impl LayerId {
    /// A specific id, for tests that need two that differ. Runtime ids only ever come from
    /// [`LayerRegistry::reserve_id`] — the counter is the registry's, not a caller's.
    #[cfg(test)]
    pub(crate) fn for_test(n: u64) -> Self {
        Self(n)
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

/// What a layer wants **behind** it — the one thing a widget tree cannot draw for itself.
///
/// A layer paints into a `Scene`, whose vocabulary is rects, text and clips. "Everything already
/// on screen, blurred" is not a shape: it is a GPU pass over the frame so far, which only the
/// renderer can run. So a layer *declares* the backdrop it wants and the host performs it, exactly
/// as [`covers_content`](DynamicLayer::covers_content) declares what it obscures and the router
/// acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum LayerBackdrop {
    /// Whatever is behind shows through unchanged (the historical behaviour).
    #[default]
    Plain,
    /// The frame so far, blurred, stamped under the layer — depth for a full-screen surface, and
    /// the reason a map reads as *above* the session rather than as a replacement for it.
    ///
    /// Strength is the theme's `overlay_frost_radius`, so a theme that wants a flat backdrop sets
    /// it to `0` and every frosted layer answers together.
    Frosted,
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
    /// What this layer wants drawn behind it. See [`LayerBackdrop`].
    pub(crate) backdrop: LayerBackdrop,
    /// Set while a **removal** is waiting on the dissolve: the layer is gone as far as its owner is
    /// concerned and only the picture is still playing out. [`LayerRegistry::tick`] drops it.
    pub(crate) doomed: bool,
    /// The dissolve this layer plays on its way out — [`Fade`], the same effect any widget embeds.
    ///
    /// Declared, not decided by the hider: whether a surface should dissolve or cut is a property
    /// of the surface — a full-screen map that vanishes mid-keystroke reads as a glitch, a context
    /// menu that lingers reads as lag. A zero duration (the default) means "cut", so a layer that
    /// never asked for one needs no special case anywhere.
    pub(crate) fade: Fade,
    /// The **zoom** this layer arrives and leaves with — the counterpart of [`fade`](Self::fade),
    /// and declared the same way: a surface says how it should come and go, and the host performs
    /// it. A `from` of 1.0 (the default) means "no zoom", so a layer that never asked for one costs
    /// nothing and needs no special case anywhere.
    ///
    /// niri's overview opens by zooming **out** from life size, which is what an exposé wants: the
    /// same thing, further away. A fade says "a different picture" instead.
    pub(crate) zoom: Zoom,
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
    /// How opaque to paint this layer — `1.0` normally, falling to `0` across a fade-out.
    pub(crate) fn opacity(&self) -> f32 {
        self.fade.amount()
    }

    /// What scale to paint this layer at — `1.0` normally, walking to or from its `from` while a
    /// zoom runs. Its companion is [`opacity`](Self::opacity): both are things done *to* a surface,
    /// and the widgets inside know about neither.
    pub(crate) fn scale(&self) -> f32 {
        self.zoom.amount()
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
        self.visible && !self.fade.is_running()
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

/// The registry of dynamically added layers, held on `AppState`. The built-in surfaces are
/// **not** stored here (they keep their own trees + lifecycle); this holds only layers added
/// at runtime via [`add`](LayerRegistry::add).
#[derive(Default)]
pub(crate) struct LayerRegistry {
    layers: Vec<DynamicLayer>,
    next: u64,
    /// **The active context surface** — the coarse half of layering, kept apart from the fine
    /// geometric half on purpose.
    ///
    /// It says *which context is live*, and it is moved **only** by opening and closing a context
    /// surface: an exposé, a modal. Zoom, scroll and picking a different dock never touch it, which
    /// is what keeps the sidebar reachable while a pane is zoomed. `None` is the base context —
    /// panes, sidebar and floats together, none of them exclusive.
    ///
    /// An overlay mounts as a child of this, which is how "a modal opened from the exposé is a
    /// child of the exposé" happens without any caller saying so.
    current: Option<LayerId>,
    /// The contexts to fall back through as each one closes, most recent last.
    context_stack: Vec<Option<LayerId>>,
}

impl LayerRegistry {
    /// Register a new layer and return its id. `Persistent` layers are visible immediately;
    /// `OnDemand` layers start hidden (show them with [`show`](LayerRegistry::show)).
    pub(crate) fn add(
        &mut self,
        parent: Option<LayerId>,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        root: Box<dyn Component>,
    ) -> LayerId {
        let id = self.reserve_id();
        self.push_layer(id, parent, kind, modal, covers_content, LayerContent::Native(root), None);
        id
    }

    /// The one place a `DynamicLayer` is constructed. Every registration path — anonymous, named,
    /// described — lands here under an id its caller already holds, so an id is never invented in
    /// two places and a layer's fields can never be initialised two ways.
    #[allow(clippy::too_many_arguments)] // every field is one of the layer's own declarations
    fn push_layer(
        &mut self,
        id: LayerId,
        parent: Option<LayerId>,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        content: LayerContent,
        realized: Option<Box<dyn Component>>,
    ) {
        self.layers.push(DynamicLayer {
            id,
            parent,
            backdrop: LayerBackdrop::default(),
            doomed: false,
            fade: Fade::new(0.0),
            zoom: Zoom::new(0.0, 1.0),
            kind,
            modal,
            covers_content,
            visible: matches!(kind, LayerKind::Persistent),
            name: None,
            content,
            realized,
        });
    }

    /// Register a layer whose content is a **description**. `realized` must be the tree produced
    /// from `node` by the one bridge — the caller realizes, because realizing needs the theme, an
    /// intent emitter, the hint sink and the form bindings, none of which a registry holds.
    ///
    /// `id` comes from [`reserve_id`](Self::reserve_id): the realized tree's intent sink has to
    /// name the layer it lives in (`chrome::layer_emitter`), so the id exists before the tree does.
    // Every argument is one of the layer's own declarations, and grouping them into a spec struct
    // is work T417 would throw away — it deletes `LayerBand` and rebuilds this as a surface tree.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_view(
        &mut self,
        id: LayerId,
        parent: Option<LayerId>,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        node: ViewNode,
        realized: Box<dyn Component>,
    ) -> LayerId {
        self.push_layer(
            id,
            parent,
            kind,
            modal,
            covers_content,
            LayerContent::View(node),
            Some(realized),
        );
        id
    }

    /// Register a layer under an addressable [`name`](DynamicLayer#structfield.name), so
    /// `show_layer` / `hide_layer` can reach it without knowing its `LayerId` — which is a runtime
    /// counter no keybinding, config line or RPC call could ever know.
    ///
    /// Build `name` with [`layer_name`] so the owner half is stamped rather than typed. Re-registering
    /// an existing name **replaces** that layer, which is what a remount should do.
    ///
    /// `id` comes from [`id_of_name`](Self::id_of_name) (a rebuild — the same layer, so the same
    /// id) falling back to [`reserve_id`](Self::reserve_id) (the first registration). The caller
    /// holds it first because the layer's tree carries an intent sink naming the layer it lives in
    /// (`chrome::layer_emitter`), and a rebuild that changed the id would leave every widget in the
    /// new tree naming a layer that no longer exists.
    // Same as `add_view`: the arguments are the layer's declarations, and T417 replaces this
    // signature wholesale.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_named(
        &mut self,
        id: LayerId,
        name: String,
        parent: Option<LayerId>,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        root: Box<dyn Component>,
    ) -> LayerId {
        // **A re-registration keeps the layer's PLACE in the stack.** It is the same layer with
        // fresh content, not a new one arriving — and position is what "on top" means here:
        // `top_modal_root_mut` takes the last active modal in this vector, so a rebuild that
        // removed and re-appended silently promoted the layer above everything opened since.
        //
        // That is how deleting a pane from the exposé did nothing: the confirm dialog opened above
        // the map, the map was rebuilt (the session had changed), and the rebuild put it back on
        // top of the dialog — so the click on "Close" was delivered to the map, the dialog never
        // submitted, and the click landed on a card and moved the focus instead (Antonio, driving,
        // 2026-08-11).
        let previous = self
            .layers
            .iter()
            .position(|l| l.name.as_deref() == Some(name.as_str()));
        // **It also keeps what it was doing**: whether it is up, and any animation in flight. The
        // replacement is a fresh `DynamicLayer`, which starts hidden — so without this a rebuild
        // looked like a first appearance and played the arrival again. `prefix+j` behind the exposé
        // changed the focus, the map rebuilt, and it zoomed open on every keystroke (Antonio,
        // driving, 2026-08-11).
        let carried = previous.map(|at| {
            let l = &self.layers[at];
            (l.visible, l.fade, l.zoom, l.doomed)
        });
        if let Some(at) = previous {
            self.layers.remove(at);
        }
        self.push_layer(id, parent, kind, modal, covers_content, LayerContent::Native(root), None);
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.name = Some(name);
            if let Some((visible, fade, zoom, doomed)) = carried {
                l.visible = visible;
                l.fade = fade;
                l.zoom = zoom;
                l.doomed = doomed;
            }
        }
        // `add` pushed it last; put it back where the old one stood.
        if let Some(at) = previous
            && let Some(now) = self.layers.iter().position(|l| l.id == id)
        {
            let layer = self.layers.remove(now);
            self.layers.insert(at.min(self.layers.len()), layer);
        }
        id
    }

    /// Declare what a layer wants drawn behind it (see [`LayerBackdrop`]).
    ///
    /// Set after registering rather than passed to `add`, the way [`name`](Self::add_named) is:
    /// every layer has a backdrop and almost every one wants the default, so it does not belong in
    /// the argument list four call sites would have to carry.
    pub(crate) fn set_backdrop(&mut self, id: LayerId, backdrop: LayerBackdrop) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.backdrop = backdrop;
        }
    }

    /// Is any layer participating this frame?
    ///
    /// Asked by the renderer rather than "did the layer scene draw anything", because an
    /// [`Overlay`](heca_grid_ui::Overlay) paints its whole panel into the scene's **overlay** layer
    /// — its base layer is empty, so a scene-emptiness check silently skipped the flush and the
    /// exposé drew nothing at all.
    pub(crate) fn any_visible(&self) -> bool {
        self.layers.iter().any(|l| l.visible)
    }

    /// Declare that this layer dissolves over `seconds` when hidden instead of cutting.
    pub(crate) fn set_fade_out(&mut self, id: LayerId, seconds: f32) {
        self.set_fade_out_after(id, seconds, 0.0);
    }

    /// The same, but staying fully present for `delay` seconds first — for a layer that leaves by
    /// doing something else on the way out. A [`Zoom`] shrinking away wants this: fading at the
    /// same time dims the movement before it has played, so the surface looks like it dissolved
    /// rather than left.
    pub(crate) fn set_fade_out_after(&mut self, id: LayerId, seconds: f32, delay: f32) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.fade = Fade::new(seconds).delay(delay);
        }
    }

    /// Declare that this layer **zooms** between `from` and life size over `seconds` when it is
    /// shown and hidden, instead of simply appearing.
    ///
    /// The counterpart of [`set_fade_out`](Self::set_fade_out), and usable by anything that
    /// registers a layer — a plugin's panel declares its arrival exactly as the host's exposé does.
    /// `from` below 1.0 grows in from smaller (niri's overview is `0.5`), above 1.0 drops in from
    /// larger. Not set ⇒ no zoom, and nothing anywhere pays for it.
    pub(crate) fn set_zoom(&mut self, id: LayerId, seconds: f32, from: f32) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.zoom = Zoom::new(seconds, from);
            // A layer that is already up when it declares one is *there*, not arriving.
            if l.visible {
                l.zoom.cancel();
            }
        }
    }

    /// How opaque a layer should be painted this frame — `1.0` unless it is on its way out.
    pub(crate) fn opacity(&self, id: LayerId) -> f32 {
        self.layers
            .iter()
            .find(|l| l.id == id)
            .map_or(1.0, DynamicLayer::opacity)
    }

    /// Advance any fade in flight, retiring layers whose fade has run out. Returns `true` while one
    /// is still going, which is what keeps frames coming — a fade nobody ticks is a frozen layer.
    pub(crate) fn tick(&mut self, dt: f32) -> bool {
        let mut fading = false;
        for l in &mut self.layers {
            // The zoom rides alongside: it never decides when a layer goes — the fade does that —
            // it only asks for frames while the picture is still moving. A layer that declared no
            // zoom reports `false` immediately and costs nothing.
            if l.zoom.tick(dt) {
                fading = true;
            }
            let was_running = l.fade.is_running();
            match l.fade.tick(dt) {
                true => fading = true,
                // The frame a running fade reports finished is the frame the layer can go.
                false if was_running => l.visible = false,
                false => {}
            }
        }
        // A layer whose removal was waiting on its dissolve leaves for good now.
        self.layers.retain(|l| !l.doomed || l.fade.is_running());
        fading
    }

    /// How strongly to stamp the frosted backdrop this frame — the **boldest** frosted layer's own
    /// opacity, so the blur under a dissolving map dissolves with it instead of snapping back
    /// sharp in one frame at the end.
    pub(crate) fn frost_opacity(&self) -> f32 {
        self.layers
            .iter()
            .filter(|l| l.visible && l.backdrop == LayerBackdrop::Frosted)
            .map(DynamicLayer::opacity)
            .fold(0.0, f32::max)
    }

    /// Does any layer participating this frame want a frosted backdrop?
    ///
    /// One question for the renderer, because the blur is **one pass over the whole frame**: two
    /// frosted layers up at once share it rather than each paying for their own.
    pub(crate) fn wants_frost(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.visible && l.backdrop == LayerBackdrop::Frosted)
    }

    /// **The id [`add_named`](Self::add_named) will register `name` under** — the existing layer's
    /// own id when this is a rebuild, a fresh one when it is the first registration.
    ///
    /// A caller needs this *before* it builds the tree, because the tree carries an intent sink
    /// naming the layer it lives in (`chrome::layer_emitter`). The rule lives here rather than at
    /// the call site so "which layer is this name" has one answer: matching `add_named`'s own
    /// search, a **doomed** layer still counts — it is the one about to be replaced, and handing
    /// back a new id for it would break the id continuity a plugin holding a `LayerId` relies on.
    pub(crate) fn slot_for_name(&mut self, name: &str) -> LayerId {
        match self.layers.iter().find(|l| l.name.as_deref() == Some(name)) {
            Some(l) => l.id,
            None => self.reserve_id(),
        }
    }

    /// **What this layer is called** — the addressable, owner-prefixed name (`heca.expose`,
    /// `docker.panel`), or `None` for an anonymous one.
    ///
    /// The reverse of [`by_name`](Self::by_name), and the same identity: it is what `show_layer`
    /// and `hide_layer` take, and what a `[[keys.surface]]` entry names, so a layer has one name
    /// wherever it is spoken about.
    pub(crate) fn name_of(&self, id: LayerId) -> Option<String> {
        self.get(id).and_then(|l| l.name.clone())
    }

    /// The layer registered under `name`, if any.
    pub(crate) fn by_name(&self, name: &str) -> Option<LayerId> {
        self.layers
            .iter()
            // A layer already on its way out is not the one a name means any more.
            .find(|l| l.name.as_deref() == Some(name) && !l.doomed)
            .map(|l| l.id)
    }

    /// Is the layer named `name` participating this frame?
    pub(crate) fn is_visible_named(&self, name: &str) -> bool {
        self.layers
            .iter()
            .any(|l| l.name.as_deref() == Some(name) && l.is_active())
    }

    /// The names of the **host's own** visible layers — what
    /// [`rebuild_named_layer`](crate::chrome::rebuild_named_layer) can refresh when the session
    /// changes under an open surface.
    ///
    /// Host-owned only, by the [`HOST_OWNER`] prefix the naming scheme already guarantees: a
    /// plugin's layer is rebuilt by the plugin, and calling into one from a mutation hook would
    /// make every layout change run foreign code.
    pub(crate) fn visible_host_layer_names(&self) -> Vec<String> {
        let prefix = format!("{HOST_OWNER}.");
        self.layers
            .iter()
            .filter(|l| l.is_active())
            .filter_map(|l| l.name.clone())
            .filter(|n| n.starts_with(&prefix))
            .collect()
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
        parent: Option<LayerId>,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        root: Box<dyn Component>,
    ) {
        self.layers.push(DynamicLayer {
            id,
            parent,
            backdrop: LayerBackdrop::default(),
            doomed: false,
            fade: Fade::new(0.0),
            zoom: Zoom::new(0.0, 1.0),
            kind,
            modal,
            covers_content,
            visible: true,
            name: None,
            content: LayerContent::Native(root),
            realized: None,
        });
        if modal {
            self.enter_context(id);
        }
    }

    /// Remove a layer entirely — **after its dissolve, if it declared one.**
    ///
    /// A layer that fades is still on screen for those few frames, so taking it out of the registry
    /// the instant it is resolved makes it vanish between two frames and the fade never plays. This
    /// is the path Escape and a widget's own dismiss take (`overlay::resolve`), which is why the
    /// map cut on Escape while it dissolved on a click: the two dismissals went different ways.
    ///
    /// Its completion has already run by then; what lingers is only the picture. [`tick`](Self::tick)
    /// finishes the job.
    pub(crate) fn remove(&mut self, id: LayerId) {
        // The context goes back now, not when the picture finishes: as far as its owner is
        // concerned this surface is already gone, and what lingers is only the dissolve.
        self.leave_context(id);
        let Some(l) = self.layers.iter_mut().find(|l| l.id == id) else { return };
        if l.visible {
            l.fade.start();
            l.zoom.leave();
        }
        if l.fade.is_running() {
            l.doomed = true;
            return;
        }
        self.layers.retain(|l| l.id != id);
    }

    /// Show a layer (bring it into the stack this frame). `ShowLayer` dispatches here.
    pub(crate) fn show(&mut self, id: LayerId) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            // **Showing something already shown is not an arrival.** A layer is re-registered and
            // re-shown every time the session changes underneath it, so starting the entry zoom
            // here unconditionally replayed the whole animation on every rebuild: `prefix+j` behind
            // the map moved the focus, the map rebuilt, and it zoomed in again as if it had just
            // opened (Antonio, driving, 2026-08-11).
            let arriving = !l.visible;
            l.visible = true;
            // Re-shown mid-fade: cancel it and be fully there again, rather than opening
            // half-transparent and finishing a disappearance nobody still wants.
            l.fade.cancel();
            match arriving {
                true => l.zoom.enter(),
                // Already up: whatever the zoom was doing, it is *here* now.
                false => l.zoom.cancel(),
            }
        }
        // **A modal that arrives becomes the active context**, and only a modal does. That is §2's
        // coarse mechanism: an exclusive surface makes everything beneath it dormant, while the
        // base context keeps several surfaces live together. A non-modal overlay — a dropdown, a
        // toast — is a child of the context, never one itself.
        if self.layers.iter().any(|l| l.id == id && l.modal && l.visible) {
            self.enter_context(id);
        }
    }

    /// Hide a layer. `HideLayer` dispatches here.
    ///
    /// A layer that declared a [`fade_out`](DynamicLayer::fade_out) does not go now — it starts
    /// dissolving and stays visible until [`tick`](Self::tick) runs the fade out. Everything that
    /// reads `visible` therefore keeps treating it as up for those few frames, which is right:
    /// while you can still see a modal it is still covering the panes.
    pub(crate) fn hide(&mut self, id: LayerId) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            if l.visible {
                l.fade.start();
                l.zoom.leave();
            }
            // No fade declared (or none left to run) ⇒ it goes now.
            if !l.fade.is_running() {
                l.visible = false;
            }
        }
        self.leave_context(id);
    }

    /// Is the tiled area covered by any visible layer? The input to `Domain::Overlay`
    /// (F003/P086/T371).
    ///
    /// **It is `covers_content` alone, and `modal` has nothing to do with it.** The two answer
    /// different questions: `modal` is "does this take the keyboard", `covers_content` is "may
    /// actions still touch the panes". Conflating them left one surface impossible to describe —
    /// the exposé is both a keyboard owner and a *map of the panes*, so acting on the one you can
    /// see in it is the entire point. While `modal` implied coverage the map blocked every act on
    /// the pane it was built to let you choose, and no arrangement of intents could get past it
    /// (Antonio, 2026-08-05: *"we have actions in ActionRegistry for all the methods we need — why
    /// is this so difficult here?"*). It was not the actions; it was this.
    ///
    /// A `Modal`-band override used to force coverage on top of the declaration, because a call
    /// site passing `false` re-opened exactly one hole: the prefix sequence deliberately falls
    /// through the overlay key path (`app/events.rs`, so `prefix+/` can pick a menu entry), reaches
    /// the router and runs — `prefix+x` with a context menu open raised the close-pane confirm
    /// (user, 2026-07-30). **The override went with the band** (F003/P082/T417), and it is not
    /// missed: both paths that raise a decision-demanding overlay — `open_modal` and
    /// `insert_menu_layer` — declare coverage themselves, which is where the declaration belongs.
    /// Forcing it here meant the exposé could never say the truth about itself.
    pub(crate) fn content_covered(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.is_active() && l.covers_content)
    }

    /// One layer by id, whatever its visibility — how a [`HintTarget`](super::HintTarget) finds
    /// the tree it was collected from.
    pub(crate) fn get(&self, id: LayerId) -> Option<&DynamicLayer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// The twin of [`get`](Self::get) for a caller that must run something in the layer's tree —
    /// a hint pick acting on the widget through its own handlers, which are `FnMut`.
    pub(crate) fn get_mut(&mut self, id: LayerId) -> Option<&mut DynamicLayer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    /// **This surface's z, as a path.** The chain of sibling indices from the root down to it —
    /// `[4, 0]` is the first child of the fifth root surface.
    ///
    /// Ordering is lexicographic on this, which is a pre-order walk, which is paint order. Three
    /// things fall out of that and none of them is written anywhere: a child is above its parent,
    /// a later sibling is above an earlier one, and inserting in the middle is an insert at a
    /// sibling index — no renumbering, no fractional z.
    fn z_path(&self, id: LayerId) -> Vec<usize> {
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
    fn enter_context(&mut self, id: LayerId) {
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
    fn leave_context(&mut self, id: LayerId) {
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

    /// Mutable roots of the visible layers (order-independent) — for the per-frame layout pass.
    pub(crate) fn visible_roots_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Component>> {
        self.layers
            .iter_mut()
            .filter(|l| l.visible)
            .map(|l| l.root_mut())
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
    fn top_modal_index(&self) -> Option<usize> {
        self.layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.is_active() && l.modal)
            // The front-most is the greatest z-path. Paint reads the same order, so the two can no
            // longer disagree about which surface is in front.
            .max_by_key(|(_, l)| self.z_path(l.id))
            .map(|(i, _)| i)
    }

    pub(crate) fn top_modal_id(&self) -> Option<LayerId> {
        self.top_modal_index().map(|i| self.layers[i].id)
    }

    /// The root of the front-most visible modal layer, mutably — the input target while a modal
    /// is up. Pairs with [`top_modal_id`](Self::top_modal_id).
    pub(crate) fn top_modal_root_mut(&mut self) -> Option<&mut (dyn Component + 'static)> {
        let at = self.top_modal_index()?;
        Some(self.layers[at].root_mut().as_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use heca_grid_ui::widgets::Flex;

    fn empty_root() -> Box<dyn Component> {
        Box::new(Flex::row())
    }

    /// **A frost is only paid for while the layer asking for it is up.** The blur is a full-frame
    /// GPU pass, so a hidden exposé must not keep the renderer running it, and a plain dialog must
    /// never trigger one it did not ask for.
    #[test]
    fn only_a_visible_layer_that_asked_for_it_wants_a_frost() {
        let mut reg = LayerRegistry::default();
        let plain = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        let frosted = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        reg.set_backdrop(frosted, LayerBackdrop::Frosted);

        assert!(!reg.wants_frost(), "both are hidden — nothing to frost behind");
        reg.show(plain);
        assert!(!reg.wants_frost(), "a plain layer does not summon a blur pass");
        reg.show(frosted);
        assert!(reg.wants_frost());
        reg.hide(frosted);
        assert!(!reg.wants_frost(), "hidden again, and the pass stops with it");
    }

    /// **A dissolving layer is removed only after its dissolve.** Escape and a widget's own
    /// dismiss both go through `overlay::resolve`, which *removes*; hiding was the only path that
    /// faded. So the map dissolved on a click and cut on Escape — two dismissals, two behaviours.
    #[test]
    fn removing_a_fading_layer_waits_for_the_fade() {
        let mut reg = LayerRegistry::default();
        let id = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        reg.set_fade_out(id, 0.1);
        reg.show(id);

        reg.remove(id);
        assert!(reg.any_visible(), "it is on its way out, not gone");
        assert!(reg.tick(0.05), "still dissolving");
        assert!(reg.opacity(id) < 1.0, "and visibly on its way: {}", reg.opacity(id));
        assert!(!reg.tick(0.05), "and now it is done");
        assert!(!reg.any_visible(), "the layer is gone for good");
    }

    /// **A dissolving layer stops being in charge the moment it is dismissed**, even though it is
    /// still on screen.
    ///
    /// The exposé closes by dismissing itself and *then* focusing the pane you chose. While the
    /// dissolve still counted as coverage, that focus was refused by `Domain::Overlay` for the
    /// whole animation — Enter and Space did nothing and the log filled with `blocked intent from
    /// Keyboard`. Adding a fade must not make a surface hold onto input it has already given up.
    #[test]
    fn a_dissolving_layer_no_longer_covers_the_content() {
        let mut reg = LayerRegistry::default();
        let id = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        reg.set_fade_out(id, 0.1);
        reg.show(id);
        assert!(reg.content_covered(), "up and in charge");

        reg.hide(id);
        assert!(!reg.content_covered(), "dismissed — a picture now, not a modal");
        assert!(reg.any_visible(), "and still painted while it dissolves");
        assert!(reg.top_modal_id().is_none(), "so it takes no more input either");
    }

    /// A layer with no dissolve declared still goes at once — a menu that lingers reads as lag.
    #[test]
    fn removing_a_plain_layer_is_immediate() {
        let mut reg = LayerRegistry::default();
        let id = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        reg.show(id);
        reg.remove(id);
        assert!(!reg.any_visible());
    }

    /// **A `Modal`-band layer covers whatever it declared** — the property no call site can get
    /// wrong (F003/P086/T371). It demands a decision, so acts on the panes behind it are refused by
    /// construction; the alternative is trusting a `bool` at every `insert`, and the one that
    /// passed `false` let `prefix+x` raise the close-pane confirm with a context menu open (user,
    /// 2026-07-30).
    #[test]
    fn coverage_is_what_the_surface_declared_not_what_its_place_implies() {
        let mut reg = LayerRegistry::default();
        // Takes the keyboard, declares it covers nothing — and is believed.
        reg.insert(LayerId(7), None, LayerKind::OnDemand, true, false, empty_root());
        assert!(
            !reg.content_covered(),
            "a modal that says it covers nothing covers nothing — the band used to overrule this",
        );

        // The decision-demanding surfaces declare it themselves, which is where it belongs:
        // `open_modal` and `insert_menu_layer` both pass `true`.
        reg.insert(LayerId(8), None, LayerKind::OnDemand, true, true, empty_root());
        assert!(reg.content_covered(), "and a surface that says it covers, does");
    }

    /// **Below the `Modal` band, coverage is what the layer declared — `modal` says nothing about
    /// it.** The two answer different questions: `modal` is "does this take the keyboard",
    /// `covers_content` is "may actions still touch the panes". Conflating them made one surface
    /// impossible to describe — the exposé takes the keyboard *and* is a map of the panes, so while
    /// `modal` implied coverage it refused every act on the pane it exists to let you choose.
    #[test]
    fn an_overlay_that_takes_the_keyboard_can_still_declare_it_covers_nothing() {
        let mut reg = LayerRegistry::default();
        let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
        reg.show(map);
        assert!(!reg.content_covered(), "a map of the panes does not cover them");
        assert_eq!(reg.top_modal_id(), Some(map), "and it still owns the keyboard");
    }

    /// The one input `Domain::Overlay` reads: a *visible* covering layer, and only that
    /// (F003/P086/T371). A layer that covers but is hidden is not covering anything.
    #[test]
    fn coverage_is_reported_only_while_the_layer_is_visible() {
        let mut reg = LayerRegistry::default();
        let corner = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        assert!(!reg.content_covered(), "a non-covering layer covers nothing");

        let over = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
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
        let a = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
        let b = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![b], "on-demand hidden until shown; persistent visible");
        reg.show(a);
        // Both visible now. `b` was registered second, so it is the later sibling and in front.
        let vis: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(vis, vec![b, a], "a later sibling is in front");
        reg.hide(a);
        assert_eq!(reg.visible_front_to_back().len(), 1);
    }

    /// **z is the position in the tree, so order is a pre-order walk of it.** Two rules, and both
    /// fall out of comparing the z-paths rather than being written anywhere: a later sibling is in
    /// front of an earlier one, and a child is in front of its parent.
    ///
    /// This replaced `band_orders_front_to_back`, which asserted a five-variant enum's ranking —
    /// a stored z one step removed, and the thing §6 of the surface-compositor model forbids.
    #[test]
    fn z_is_a_path_so_a_child_is_in_front_of_its_parent_and_a_later_sibling_of_an_earlier() {
        let mut reg = LayerRegistry::default();
        let first = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        let second = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        // Opened BY `first`, so it hangs from it — and sits above it without outranking `second`'s
        // own children, because a path is compared left to right.
        let childs_child = reg.add(Some(first), LayerKind::Persistent, false, false, empty_root());

        let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(
            order,
            vec![second, childs_child, first],
            "[1] > [0,0] > [0] — lexicographic on the path",
        );
    }

    /// **A surface goes above whatever opened it, wherever that is.** The deciding case for
    /// plugins: a panel opens a modal, and the modal must sit above *that* panel — not above
    /// whatever happens to have been registered last.
    #[test]
    fn an_overlay_sits_above_its_opener_not_above_the_newest_layer() {
        let mut reg = LayerRegistry::default();
        let panel = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        let unrelated = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        let modal = reg.add(Some(panel), LayerKind::Persistent, true, false, empty_root());

        let order: Vec<LayerId> = reg.visible_front_to_back().iter().map(|l| l.id).collect();
        assert_eq!(order, vec![unrelated, modal, panel]);
        assert_eq!(
            reg.top_modal_id(),
            Some(modal),
            "and input agrees with the picture, because both read the one order",
        );
    }

    /// **A described layer is a real layer.** It sorts, shows, hides and covers exactly like a
    /// native one — the arm decides where the tree came from, never how the stack treats it.
    #[test]
    fn a_view_layer_behaves_like_any_other_and_keeps_its_description() {
        use heca_view::{ViewNode, WidgetKind};
        let mut reg = LayerRegistry::default();
        let native = reg.add(None, LayerKind::Persistent, false, false, empty_root());
        let node = ViewNode::new(WidgetKind::Label);
        let slot = reg.reserve_id();
        let described = reg.add_view(
            slot,
            None,
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
        assert!(layer.realized.is_some(), "and its realized tree is registered beside it");
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

    /// **The owner half is unforgeable.** An author supplies only the short name, so a plugin has
    /// nowhere to write a prefix — and a short name carrying its own dot is rejected rather than
    /// joined, or `expose.thing` would read as if `docker.expose` owned it.
    #[test]
    fn a_layer_name_is_stamped_from_its_owner_and_cannot_be_forged() {
        assert_eq!(layer_name("docker", "expose").as_deref(), Some("docker.expose"));
        assert_eq!(layer_name(HOST_OWNER, "expose").as_deref(), Some("heca.expose"));
        assert_eq!(layer_name("docker", "expose.thing"), None, "no smuggled second segment");
        assert_eq!(layer_name("docker", "heca.expose"), None, "cannot claim another namespace");
        assert_eq!(layer_name("docker", ""), None);
    }

    /// **A rebuild does not replay the arrival.** A layer is re-registered and re-shown whenever
    /// the session changes under it, and an animation that restarts every time reads as the surface
    /// flickering open again — which is what `prefix+j` behind the exposé did.
    #[test]
    fn re_showing_a_visible_layer_does_not_restart_its_zoom() {
        let mut reg = LayerRegistry::default();
        let name = layer_name(HOST_OWNER, "expose").expect("valid");
        let slot = reg.slot_for_name(&name);
        let id = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
        );
        reg.set_zoom(id, 0.2, 1.3);
        reg.show(id);
        assert!(reg.layers[0].zoom.is_running(), "opening animates");

        // Let it finish, the way a few frames would.
        while reg.layers[0].zoom.tick(0.05) {}
        assert_eq!(reg.layers[0].scale(), 1.0, "settled at life size");

        // The session changes: the layer is rebuilt and re-shown.
        let slot = reg.slot_for_name(&name);
        let rebuilt = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
        );
        reg.set_zoom(rebuilt, 0.2, 1.3);
        reg.show(rebuilt);

        let l = reg.layers.iter().find(|l| l.id == rebuilt).expect("the rebuilt layer");
        assert!(!l.zoom.is_running(), "a rebuild must not replay the entry zoom");
        assert_eq!(l.scale(), 1.0, "it is simply there");
    }

    /// **Input goes to the layer the user sees in front, and there is only one order to read.**
    ///
    /// Painting used to sort by band while input took "the last one added", so the two could name
    /// different layers — a dialog was drawn over the exposé while the map quietly took the
    /// pointer. Both now read the z-path, so disagreeing is not expressible.
    #[test]
    fn input_goes_to_the_front_most_surface_not_the_last_one_added() {
        let mut reg = LayerRegistry::default();
        let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
        reg.show(map);
        // Raised FROM the map, so it is the map's child and above it.
        let dialog = reg.add(Some(map), LayerKind::OnDemand, true, true, empty_root());
        reg.show(dialog);

        assert_eq!(
            reg.top_modal_id(),
            Some(dialog),
            "a surface opened from the map sits above the map",
        );

        // Among siblings, the later one is in front.
        let second_dialog = reg.add(Some(map), LayerKind::OnDemand, true, true, empty_root());
        reg.show(second_dialog);
        assert_eq!(reg.top_modal_id(), Some(second_dialog), "same parent, later wins");

        // And a layer on its way out never holds the input.
        reg.remove(second_dialog);
        assert_eq!(reg.top_modal_id(), Some(dialog), "a dissolving layer is not the target");
    }

    /// **The active context follows the exclusive surfaces**, so an overlay knows what to hang
    /// from without any caller telling it. Closing one restores the context beneath it.
    #[test]
    fn the_active_context_follows_the_modal_surfaces() {
        let mut reg = LayerRegistry::default();
        assert_eq!(reg.current(), None, "the base context: panes, sidebar and floats together");

        let map = reg.add(None, LayerKind::OnDemand, true, false, empty_root());
        reg.show(map);
        assert_eq!(reg.current(), Some(map));

        let dialog = reg.add(reg.current(), LayerKind::OnDemand, true, true, empty_root());
        reg.show(dialog);
        assert_eq!(reg.current(), Some(dialog), "a modal raised from the map takes the context");
        assert_eq!(reg.get(dialog).and_then(|l| l.parent), Some(map), "and hangs from it");

        reg.hide(dialog);
        assert_eq!(reg.current(), Some(map), "closing it hands the context back");
        reg.hide(map);
        assert_eq!(reg.current(), None, "and back to the base context");
    }

    /// **A rebuild keeps the layer's place, so it cannot climb over what opened above it.**
    ///
    /// `top_modal_root_mut` takes the *last* active modal in the stack, so position is what "on
    /// top" means. Re-registering by removing and re-appending therefore promoted a layer above
    /// everything opened since — which is how deleting a pane from the exposé did nothing at all:
    /// the confirm dialog opened above the map, the session change rebuilt the map, the rebuild put
    /// it back on top of the dialog, and the click on "Close" went to the map (Antonio, driving,
    /// 2026-08-11).
    #[test]
    fn re_registering_a_layer_does_not_promote_it_above_a_newer_one() {
        let mut reg = LayerRegistry::default();
        let name = layer_name(HOST_OWNER, "expose").expect("valid");
        let slot = reg.slot_for_name(&name);
        let map = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
        );
        reg.show(map);
        // A confirm dialog opens ON TOP of it.
        let dialog = reg.add(None, LayerKind::OnDemand, true, true, empty_root());
        reg.show(dialog);
        assert_eq!(reg.top_modal_id(), Some(dialog), "the dialog is the input target");

        // The session changes under both, so the map is rebuilt.
        let slot = reg.slot_for_name(&name);
        let rebuilt = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, true, false, empty_root(),
        );
        reg.show(rebuilt);

        assert_eq!(
            reg.top_modal_id(),
            Some(dialog),
            "the rebuilt map must NOT take the keyboard and the pointer from the dialog above it",
        );
        assert_eq!(reg.by_name(&name), Some(rebuilt), "and the name still resolves to the new tree");
    }

    /// A named layer is addressable without knowing its `LayerId` — which is a runtime counter no
    /// keybinding or RPC call could know. Re-registering the name replaces it, as a remount should.
    #[test]
    fn a_named_layer_is_addressable_and_re_registering_replaces_it() {
        let mut reg = LayerRegistry::default();
        let anonymous = reg.add(None, LayerKind::OnDemand, false, false, empty_root());
        let name = layer_name(HOST_OWNER, "expose").expect("valid");

        let slot = reg.slot_for_name(&name);
        let first = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, false, true, empty_root(),
        );
        assert_eq!(reg.by_name(&name), Some(first));
        assert!(!reg.is_visible_named(&name), "OnDemand starts hidden");

        reg.show(first);
        assert!(reg.is_visible_named(&name));

        // A remount registers the same name again: one layer, the new one — and it **stays up**.
        //
        // ⚠️ Changed 2026-08-11. It used to start hidden, which made every caller remember the
        // `let was_visible = …; if was_visible { show(id) }` dance around its own rebuild — and got
        // it subtly wrong: re-showing restarted the entry animation, so the exposé zoomed open
        // again on every keystroke that changed the session behind it. A re-registration is the
        // same layer with fresh content, so it keeps its place, its visibility and any animation in
        // flight.
        let slot = reg.slot_for_name(&name);
        let second = reg.add_named(
            slot, name.clone(), None, LayerKind::OnDemand, false, true, empty_root(),
        );
        // **And it keeps its ID.** Changed 2026-08-12 (F003/P082/T416): a rebuild used to mint a
        // new one, so anything holding a `LayerId` across a session change — a plugin's handle, and
        // every intent sink inside the layer's own tree, which names the layer it lives in — was
        // left pointing at a layer that no longer existed. "The same layer with fresh content" has
        // to mean the same id, or the sentence is only about the stack position.
        assert_eq!(second, first, "a rebuild is the same layer, so it is the same id");
        assert_eq!(reg.by_name(&name), Some(second), "the name follows the new registration");
        assert!(
            reg.is_visible_named(&name),
            "a rebuild of a layer that is up leaves it up — the caller does not re-show it",
        );

        // An anonymous layer answers to no name, and is untouched by a named registration.
        assert_eq!(reg.by_name("heca.nothing"), None);
        reg.show(anonymous);
        assert!(
            reg.visible_front_to_back().iter().any(|l| l.id == anonymous),
            "the unnamed layer is still in the stack after two named registrations",
        );
    }
}
