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

use heca_grid_ui::effects::Fade;
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
    pub(crate) band: LayerBand,
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
/// (F003/P019 §9). The node is the source of truth a theme reload or a plugin update re-realizes
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
            backdrop: LayerBackdrop::default(),
            doomed: false,
            fade: Fade::new(0.0),
            kind,
            modal,
            covers_content,
            visible: matches!(kind, LayerKind::Persistent),
            name: None,
            content: LayerContent::Native(root),
            realized: None,
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
            backdrop: LayerBackdrop::default(),
            doomed: false,
            fade: Fade::new(0.0),
            kind,
            modal,
            covers_content,
            visible: matches!(kind, LayerKind::Persistent),
            name: None,
            content: LayerContent::View(node),
            realized: Some(realized),
        });
        id
    }

    /// Register a layer under an addressable [`name`](DynamicLayer#structfield.name), so
    /// `show_layer` / `hide_layer` can reach it without knowing its `LayerId` — which is a runtime
    /// counter no keybinding, config line or RPC call could ever know.
    ///
    /// Build `name` with [`layer_name`] so the owner half is stamped rather than typed. Re-registering
    /// an existing name **replaces** that layer, which is what a remount should do; it returns the
    /// new id either way.
    pub(crate) fn add_named(
        &mut self,
        name: String,
        band: LayerBand,
        kind: LayerKind,
        modal: bool,
        covers_content: bool,
        root: Box<dyn Component>,
    ) -> LayerId {
        self.layers.retain(|l| l.name.as_deref() != Some(name.as_str()));
        let id = self.add(band, kind, modal, covers_content, root);
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.name = Some(name);
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
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.fade = Fade::new(seconds);
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
            backdrop: LayerBackdrop::default(),
            doomed: false,
            fade: Fade::new(0.0),
            kind,
            modal,
            covers_content,
            visible: true,
            name: None,
            content: LayerContent::Native(root),
            realized: None,
        });
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
        let Some(l) = self.layers.iter_mut().find(|l| l.id == id) else { return };
        if l.visible {
            l.fade.start();
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
            l.visible = true;
            // Re-shown mid-fade: cancel it and be fully there again, rather than opening
            // half-transparent and finishing a disappearance nobody still wants.
            l.fade.cancel();
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
            }
            // No fade declared (or none left to run) ⇒ it goes now.
            if !l.fade.is_running() {
                l.visible = false;
            }
        }
    }

    /// Is the tiled area covered by any visible layer? The input to `Domain::Overlay`
    /// (F003/P086/T371).
    ///
    /// **A `Modal`-band layer counts whether or not it declared coverage.** It demands a decision,
    /// so acting on the panes behind it is refused by construction rather than trusted at each
    /// `insert` — a call site passing `false` there re-opens exactly one hole: the prefix sequence
    /// deliberately falls through the overlay key path (`app/events.rs`, so `prefix+/` can pick a
    /// menu entry), reaches the router, and runs. `prefix+x` with a context menu open raising the
    /// close-pane confirm was that hole (user, 2026-07-30).
    ///
    /// **Below that band it is `covers_content` alone**, and `modal` has nothing to do with it.
    /// The two answer different questions and conflating them left one surface impossible to
    /// describe: `modal` is "does this take the keyboard", `covers_content` is "may actions still
    /// touch the panes". The exposé is both at once — it takes the keyboard, and it is a *map of
    /// the panes*, so acting on the one you can see in it is the entire point. While `modal`
    /// implied coverage, the map blocked every act on the pane it was built to let you choose, and
    /// no arrangement of intents could get past it (Antonio, 2026-08-05: *"we have actions in
    /// ActionRegistry for all the methods we need — why is this so difficult here?"*). It was not
    /// the actions; it was this.
    pub(crate) fn content_covered(&self) -> bool {
        self.layers
            .iter()
            .any(|l| l.is_active() && (l.covers_content || l.band == LayerBand::Modal))
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
            .map(|l| l.root_mut())
    }

    /// The id of the front-most visible **modal** layer (the one that captures input), if any.
    /// Front-most = most-recently inserted (a later modal opens on top of an earlier one).
    pub(crate) fn top_modal_id(&self) -> Option<LayerId> {
        self.layers
            .iter()
            .rev()
            .find(|l| l.is_active() && l.modal)
            .map(|l| l.id)
    }

    /// The root of the front-most visible modal layer, mutably — the input target while a modal
    /// is up. Pairs with [`top_modal_id`](Self::top_modal_id).
    pub(crate) fn top_modal_root_mut(&mut self) -> Option<&mut (dyn Component + 'static)> {
        self.layers
            .iter_mut()
            .rev()
            .find(|l| l.is_active() && l.modal)
            .map(|l| l.root_mut().as_mut())
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
        let plain = reg.add(LayerBand::Modal, LayerKind::OnDemand, true, true, empty_root());
        let frosted = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, true, empty_root());
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
        let id = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, true, empty_root());
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
        let id = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, true, empty_root());
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
        let id = reg.add(LayerBand::Modal, LayerKind::OnDemand, true, true, empty_root());
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
    fn a_modal_band_layer_covers_the_content_even_if_it_says_otherwise() {
        let mut reg = LayerRegistry::default();
        // Deliberately declaring `false`, as the dropdown path once did.
        reg.insert(LayerId(7), LayerBand::Modal, LayerKind::OnDemand, true, false, empty_root());
        assert!(
            reg.content_covered(),
            "a decision-demanding surface covers, whatever the flag says",
        );
    }

    /// **Below the `Modal` band, coverage is what the layer declared — `modal` says nothing about
    /// it.** The two answer different questions: `modal` is "does this take the keyboard",
    /// `covers_content` is "may actions still touch the panes". Conflating them made one surface
    /// impossible to describe — the exposé takes the keyboard *and* is a map of the panes, so while
    /// `modal` implied coverage it refused every act on the pane it exists to let you choose.
    #[test]
    fn an_overlay_that_takes_the_keyboard_can_still_declare_it_covers_nothing() {
        let mut reg = LayerRegistry::default();
        let map = reg.add(LayerBand::Overlay, LayerKind::OnDemand, true, false, empty_root());
        reg.show(map);
        assert!(!reg.content_covered(), "a map of the panes does not cover them");
        assert_eq!(reg.top_modal_id(), Some(map), "and it still owns the keyboard");
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

    /// A named layer is addressable without knowing its `LayerId` — which is a runtime counter no
    /// keybinding or RPC call could know. Re-registering the name replaces it, as a remount should.
    #[test]
    fn a_named_layer_is_addressable_and_re_registering_replaces_it() {
        let mut reg = LayerRegistry::default();
        let anonymous = reg.add(LayerBand::Overlay, LayerKind::OnDemand, false, false, empty_root());
        let name = layer_name(HOST_OWNER, "expose").expect("valid");

        let first = reg.add_named(
            name.clone(), LayerBand::Overlay, LayerKind::OnDemand, false, true, empty_root(),
        );
        assert_eq!(reg.by_name(&name), Some(first));
        assert!(!reg.is_visible_named(&name), "OnDemand starts hidden");

        reg.show(first);
        assert!(reg.is_visible_named(&name));

        // A remount registers the same name again: one layer, the new one.
        let second = reg.add_named(
            name.clone(), LayerBand::Overlay, LayerKind::OnDemand, false, true, empty_root(),
        );
        assert_ne!(second, first);
        assert_eq!(reg.by_name(&name), Some(second), "the name follows the new registration");
        assert!(!reg.is_visible_named(&name), "and the replacement starts hidden again");

        // An anonymous layer answers to no name, and is untouched by a named registration.
        assert_eq!(reg.by_name("heca.nothing"), None);
        reg.show(anonymous);
        assert!(
            reg.visible_front_to_back().iter().any(|l| l.id == anonymous),
            "the unnamed layer is still in the stack after two named registrations",
        );
    }
}
