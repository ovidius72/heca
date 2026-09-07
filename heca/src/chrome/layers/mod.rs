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
//!
//! **This stack is four files**: the registry here, [what a layer is](layer), [where it sits and
//! who is in charge](order), and its tests.

mod layer;
mod order;

pub(crate) use layer::{layer_name, DynamicLayer, LayerId, LayerKind, HOST_OWNER};

use heca_grid_ui::Component;
use heca_view::ViewNode;

/// The key string a surface is known by — its name, or `surface:<n>` for one nobody named.
///
/// A free function, taking the name rather than looking it up, for a reason that bites otherwise:
/// a caller builds its tree's emitter **before** registering (the tree carries the sink naming the
/// surface it lives in), so a registry lookup would answer `surface:7` then and `heca.expose`
/// afterwards — one surface with two identities, and the policy comparing them would silently stop
/// matching. Derived from what the caller already holds, it is the same key either side of
/// registration.
///
/// It is also the key such a surface will declare on **itself** once it is a node in the one tree
/// and there is no registry left to ask (`docs/surface-compositor.md` § 0.8).
pub(crate) fn surface_key_of(name: Option<&str>, id: LayerId) -> crate::app::interaction::SurfaceKey {
    crate::app::interaction::SurfaceKey::of(
        &name.map_or_else(|| format!("surface:{}", id.raw()), str::to_owned),
    )
}

/// **Does the surface seated for `id` stand in front of the page?** — its own declaration, read
/// off the node in the one tree (F003/P097/T499).
///
/// A surface that is not seated declares nothing, which is the same answer as declaring `false`.
fn lock(window: &heca_grid_ui::widgets::Flex, id: LayerId) -> bool {
    crate::chrome::surface_node(window, id).is_some_and(|n| n.base().lock)
}

/// **Does it take the keyboard?** — read from the tree, not declared (F003/P097/T499).
///
/// A surface that wants keys *holds focus*, and every layer widget binds its open signal to it, so
/// an open overlay answers `true` by being open and an ambient one answers `false` by holding no
/// focus. An author writes nothing; `Base::captures_keyboard` overrides it for a surface whose
/// keyboard story the framework cannot see.
fn captures_keyboard(window: &heca_grid_ui::widgets::Flex, id: LayerId) -> bool {
    crate::chrome::surface_node(window, id).is_some_and(|n| {
        n.base()
            .captures_keyboard
            .unwrap_or_else(|| heca_grid_ui::holds_keyboard(n))
    })
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
        root: Box<dyn Component>,
        window: &mut heca_grid_ui::widgets::Flex,
    ) -> LayerId {
        let id = self.reserve_id();
        self.push_layer(id, parent, kind, None);
        crate::chrome::place_surface(window, &crate::chrome::surface_slot(id), root);
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
        node: Option<ViewNode>,
    ) {
        self.layers.push(DynamicLayer {
            id,
            parent,
            doomed: false,
            kind,
            visible: matches!(kind, LayerKind::Persistent),
            name: None,
            node,
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
    /// `name` is what an action, a key binding or RPC addresses it by — build it with
    /// [`layer_name`] so the owner half is stamped rather than typed. `None` for a surface nobody
    /// names, exactly as for a native one.
    ///
    /// **A described layer could not be named at all** until F003/P097/T502: this took no `name`
    /// and [`push_layer`](Self::push_layer) wrote `None`, while [`add_named`](Self::add_named) took
    /// a native tree. So a plugin got a surface it could put up and then had no way to point at —
    /// `show_layer` / `hide_layer` address by name, and its layer had none. The two halves were
    /// reachable one at a time and never together.
    ///
    /// The named case goes through `add_named` rather than repeating it, so a described layer
    /// keeps its place in the stack on a rebuild and carries its visibility and its animation in
    /// flight — the two bugs that logic exists for are not ones to learn twice.
    pub(crate) fn add_view(
        &mut self,
        id: LayerId,
        name: Option<String>,
        parent: Option<LayerId>,
        kind: LayerKind,
        node: ViewNode,
        realized: Box<dyn Component>,
        window: &mut heca_grid_ui::widgets::Flex,
    ) -> LayerId {
        match name {
            Some(name) => {
                self.add_named(id, name, parent, kind, realized, window);
            }
            None => {
                self.push_layer(id, parent, kind, None);
                crate::chrome::place_surface(window, &crate::chrome::surface_slot(id), realized);
            }
        }
        // **The description is kept whichever way it was registered** — it is what a theme reload
        // or a plugin update re-realizes from.
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.node = Some(node);
        }
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
        root: Box<dyn Component>,
        window: &mut heca_grid_ui::widgets::Flex,
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
            (
                l.visible,
                l.doomed,
                crate::chrome::surface_node_mut(window, id)
                    .and_then(|n| n.presence_mut().map(std::mem::take)),
            )
        });
        if let Some(at) = previous {
            self.layers.remove(at);
        }
        self.push_layer(id, parent, kind, None);
        crate::chrome::place_surface(window, &crate::chrome::surface_slot(id), root);
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.name = Some(name);
            if let Some((visible, doomed, presence)) = carried {
                l.visible = visible;
                l.doomed = doomed;
                // **The gesture carries on across a rebuild.** The replacement is a fresh tree,
                // which starts closed and at rest — so without this a rebuild looked like a first
                // appearance and played the arrival again: `prefix+j` behind the exposé changed
                // the focus, the map rebuilt, and it zoomed open on every keystroke (Antonio,
                // driving, 2026-08-11). Handing over the whole `Presence` carries the animation
                // *in flight* as well as the fact that it is open, so a rebuild mid-arrival
                // continues rather than restarting or snapping.
                if let (Some(carried), Some(fresh)) = (
                    presence,
                    crate::chrome::surface_node_mut(window, id).and_then(|n| n.presence_mut()),
                ) {
                    *fresh = carried;
                }
                if visible {
                    // The same call the stack makes to raise it, and it replays nothing: the
                    // carried presence already says the surface is up, or that it is leaving.
                    if let Some(node) = crate::chrome::surface_node_mut(window, id) {
                        node.open();
                    }
                }
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

    /// Is any layer participating this frame?
    ///
    /// Asked by the renderer rather than "did the layer scene draw anything", because an
    /// [`Overlay`](heca_grid_ui::Overlay) paints its whole panel into the scene's **overlay** layer
    /// — its base layer is empty, so a scene-emptiness check silently skipped the flush and the
    /// exposé drew nothing at all.
    pub(crate) fn any_visible(&self) -> bool {
        self.layers.iter().any(|l| l.visible)
    }

    // **A layer does not declare its animation — its surface does.** `set_fade_out_after` and
    // `set_zoom` used to live here, and a caller needed three registry calls and a `LayerId` to get
    // what the exposé had, while knowing the sequencing rule itself. A plugin could reach none of
    // it. It is now one builder on the widget — `Overlay::new().panel(body).animation(..)` — and
    // this registry only *drives* it: `show`/`hide` state whether the surface should be open, and
    // the surface says when its exit has played out (⭐⭐ RULE ZERO, F003/P082/T459).

    /// How opaque a layer should be painted this frame — `1.0` unless it is on its way out.
    pub(crate) fn opacity(&self, window: &heca_grid_ui::widgets::Flex, id: LayerId) -> f32 {
        crate::chrome::surface_node(window, id)
            .and_then(|n| n.presence())
            .map_or(1.0, |p| p.frame().opacity)
    }

    /// **Is this surface still leaving?** Any part of its exit still playing.
    ///
    /// Read from the surface's own node, because the gesture is the surface's: a dismissed overlay
    /// stays on screen, inert, until every effect of its exit has finished. Tying the lifetime to
    /// the fade alone is what made `Fade::delay` unusable (F003/P082/T327).
    pub(crate) fn is_leaving(&self, window: &heca_grid_ui::widgets::Flex, id: LayerId) -> bool {
        crate::chrome::surface_node(window, id)
            .and_then(|n| n.presence())
            .is_some_and(heca_grid_ui::animation::Presence::is_leaving)
    }

    /// **Which surfaces are mid-exit** — captured *before* the tree is ticked.
    ///
    /// Half of [`retire_finished_exits`](Self::retire_finished_exits), and separate from it for the
    /// reason the pair exists at all: the surfaces are children of the window root now, so **the
    /// tree ticks them**, once, in the same walk as everything else. The registry no longer
    /// advances anything — it only has to notice the frame an exit *finished*, and that means
    /// looking either side of a tick it does not own.
    pub(crate) fn leaving_before_tick(&self, window: &heca_grid_ui::widgets::Flex) -> Vec<LayerId> {
        self.layers
            .iter()
            .filter(|l| l.visible && self.is_leaving(window, l.id))
            .map(|l| l.id)
            .collect()
    }

    /// **Retire the surfaces whose exit finished during this frame's tick**, and report whether the
    /// picture changed.
    ///
    /// Pass the ids [`leaving_before_tick`](Self::leaving_before_tick) returned *before*
    /// `window_root.tick`. A surface that was leaving then and is not leaving now has just finished
    /// its exit: it stops being visible, and one more frame is requested so its absence is painted.
    ///
    /// ⚠️ **This used to tick the trees itself, and that was the bug** (Antonio, driving,
    /// 2026-08-31). Once the surfaces became children of the window root they were advanced twice a
    /// frame — once by the tree's walk, once here — and the registry read `was_leaving` *after* the
    /// tree's tick had already consumed the transition. So on the frame an exit finished it saw
    /// "was not leaving", never retired the surface, and never asked for the frame that paints it
    /// gone: the exposé stuck at a tenth opacity until some other input forced a repaint, stayed
    /// modal, and swallowed `ctrl+h/j/k/l` for ever after. **The tree ticks. This only reconciles.**
    pub(crate) fn retire_finished_exits(
        &mut self,
        window: &mut heca_grid_ui::widgets::Flex,
        was_leaving: &[LayerId],
    ) -> bool {
        let mut changed = false;
        for id in was_leaving {
            if self.is_leaving(window, *id) {
                continue;
            }
            // **The frame the whole exit finishes is the frame the surface goes** — and the surface
            // answers for *every* part of its gesture, so nothing retires it while a shrink is
            // still playing under a dissolve that has already ended.
            if let Some(l) = self.layers.iter_mut().find(|l| l.id == *id) {
                l.visible = false;
                // **And ask for one more frame, to paint its absence.** Without it nothing requests
                // another, and the last frame drawn is the one before — still faintly visible. The
                // map stayed on the glass at about a tenth opacity until some other input forced a
                // repaint (Antonio, driving, 2026-08-19).
                changed = true;
            }
        }
        // A layer whose removal was waiting on its exit leaves for good now — and that, too, is a
        // change to the picture, so the frame that paints it is requested here.
        let gone: Vec<LayerId> = self
            .layers
            .iter()
            .filter(|l| l.doomed && !self.is_leaving(window, l.id))
            .map(|l| l.id)
            .collect();
        if !gone.is_empty() {
            for id in &gone {
                crate::chrome::remove_surface(window, &crate::chrome::surface_slot(*id));
            }
            self.layers.retain(|l| !gone.contains(&l.id));
            changed = true;
        }
        changed
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

    /// **The identity the interaction policy knows this layer by.**
    ///
    /// Its addressable name when it has one (`heca.expose`), and `surface:<n>` when it does not —
    /// a dropdown or an ad-hoc modal nobody named. One derivation, so the key a layer answers to
    /// here is the same key it will declare on itself once it is a node in the one tree and the
    /// registry no longer exists to be asked (`docs/surface-compositor.md` § 0.8).
    pub(crate) fn surface_key(&self, id: LayerId) -> crate::app::interaction::SurfaceKey {
        surface_key_of(self.name_of(id).as_deref(), id)
    }

    /// **What the surface seated under `slot` declares about itself**, or `None` when nothing in
    /// this registry owns that slot (F003/P097/T499).
    ///
    /// The one direction of truth, kept here because the registry is what holds the association.
    /// A caller walking the window root has a node and its **own declared key**, and needs the two
    /// things the tree cannot answer — does this surface cover the panes, and does it take the
    /// keyboard. It asks by that key rather than parsing an id out of it: [`LayerId::raw`] says
    /// deriving the slot is "the one place outside this module that needs it", and a reverse parse
    /// would be a second encoding of an identity this module already owns.
    ///
    /// **`None` is an answer, not a gap.** A surface may be in the tree and not in this registry —
    /// the toast stack is placed as `heca.notifications` and registers nothing — and such a surface
    /// has simply declared nothing: it covers no content and takes no keyboard. That is the same
    /// answer an explicit `lock: false` gives, so there is no case to special-case and no
    /// surface name written down anywhere.
    ///
    /// Built so that removing it later is a **deletion**: once a surface declares these on its own
    /// node (`docs/surface-compositor.md` § 0.8) the caller reads them from the node it already has
    /// and this method goes, with nothing else to unpick.
    pub(crate) fn declaration_at(&self, slot: &str) -> Option<&DynamicLayer> {
        self.layers
            .iter()
            .find(|l| crate::chrome::surface_slot(l.id) == slot)
    }

    /// The same, **but only while that surface is still in charge** (F003/P097/T499).
    ///
    /// ⚠️ **Presence in the tree is not liveness, and this is the whole of the difference.** Hiding
    /// a surface calls `node.hide()` and leaves it **seated** in the window root —
    /// [`remove_surface`](crate::chrome::remove_surface) runs only when it is destroyed — so the
    /// tree is full of dismissed surfaces, and the exposé and the command palette both declare
    /// `lock` and `modal`. A reader that takes their declarations at face value applies a
    /// hidden exposé's occluders and treats a hidden modal as the active context.
    ///
    /// It is [`DynamicLayer::is_active`] with [`is_leaving`](Self::is_leaving), which is the same
    /// pair [`content_covered`](Self::content_covered) asks — deliberately, so "is this surface in
    /// charge" has **one** answer. Writing the predicate out at the call site instead is what let a
    /// hidden surface blank every letter in the app (Antonio, driving, 2026-09-04).
    pub(crate) fn live_declaration_at(
        &self,
        window: &heca_grid_ui::widgets::Flex,
        slot: &str,
    ) -> Option<&DynamicLayer> {
        self.declaration_at(slot)
            .filter(|l| l.is_active(self.is_leaving(window, l.id)))
    }

    /// **Is the surface seated in `slot` still in charge?** — the liveness half alone, for a caller
    /// that reads the surface's *declarations* off its own node and only needs to know whether they
    /// still count (F003/P097/T499).
    pub(crate) fn slot_is_live(&self, window: &heca_grid_ui::widgets::Flex, slot: &str) -> bool {
        self.live_declaration_at(window, slot).is_some()
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
    pub(crate) fn is_visible_named(&self, window: &heca_grid_ui::widgets::Flex, name: &str) -> bool {
        self.layers
            .iter()
            .any(|l| l.name.as_deref() == Some(name) && l.is_active(self.is_leaving(window, l.id)))
    }

    /// The names of the **host's own** visible layers — what
    /// [`rebuild_named_layer`](crate::chrome::rebuild_named_layer) can refresh when the session
    /// changes under an open surface.
    ///
    /// Host-owned only, by the [`HOST_OWNER`] prefix the naming scheme already guarantees: a
    /// plugin's layer is rebuilt by the plugin, and calling into one from a mutation hook would
    /// make every layout change run foreign code.
    pub(crate) fn visible_host_layer_names(&self, window: &heca_grid_ui::widgets::Flex) -> Vec<String> {
        let prefix = format!("{HOST_OWNER}.");
        self.layers
            .iter()
            .filter(|l| l.is_active(self.is_leaving(window, l.id)))
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
    #[allow(clippy::too_many_arguments)] // the layer's own declarations, plus where to place it
    pub(crate) fn insert(
        &mut self,
        id: LayerId,
        parent: Option<LayerId>,
        kind: LayerKind,
        root: Box<dyn Component>,
        window: &mut heca_grid_ui::widgets::Flex,
    ) {
        self.layers.push(DynamicLayer {
            id,
            parent,
            doomed: false,
            kind,
            visible: true,
            name: None,
            node: None,
        });
        crate::chrome::place_surface(window, &crate::chrome::surface_slot(id), root);
        // **The surface says whether it takes the keyboard**, so this reads the tree it was just
        // placed in rather than being told a second time.
        if captures_keyboard(window, id) {
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
    pub(crate) fn remove(&mut self, window: &mut heca_grid_ui::widgets::Flex, id: LayerId) {
        // The context goes back now, not when the picture finishes: as far as its owner is
        // concerned this surface is already gone, and what lingers is only the dissolve.
        self.leave_context(id);
        if self.layers.iter().all(|l| l.id != id) {
            return;
        }
        if let Some(node) = crate::chrome::surface_node_mut(window, id) {
            node.hide();
        }
        if self.is_leaving(window, id) {
            if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
                l.doomed = true;
            }
            return;
        }
        self.layers.retain(|l| l.id != id);
        crate::chrome::remove_surface(window, &crate::chrome::surface_slot(id));
    }

    /// Show a layer (bring it into the stack this frame). `ShowLayer` dispatches here.
    pub(crate) fn show(&mut self, window: &mut heca_grid_ui::widgets::Flex, id: LayerId) {
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id) {
            l.visible = true;
            // **The surface decides what showing means to it.** Whether this is a real arrival —
            // rather than a rebuild of something already up, or a re-show of something already on
            // its way out — is a rule about surfaces, not about layers, so it lives in the widget
            // (`Presence::show`) and every host gets it for free.
        }
        if let Some(node) = crate::chrome::surface_node_mut(window, id) {
            node.open();
        }
        // **A modal that arrives becomes the active context**, and only a modal does. That is §2's
        // coarse mechanism: an exclusive surface makes everything beneath it dormant, while the
        // base context keeps several surfaces live together. A non-modal overlay — a dropdown, a
        // toast — is a child of the context, never one itself.
        if captures_keyboard(window, id)
            && self.layers.iter().any(|l| l.id == id && l.visible)
        {
            self.enter_context(id);
        }
    }

    /// Hide a layer. `HideLayer` dispatches here.
    ///
    /// A layer that declared a [`fade_out`](DynamicLayer::fade_out) does not go now — it starts
    /// dissolving and stays visible until [`tick`](Self::tick) runs the fade out. Everything that
    /// reads `visible` therefore keeps treating it as up for those few frames, which is right:
    /// while you can still see a modal it is still covering the panes.
    pub(crate) fn hide(&mut self, window: &mut heca_grid_ui::widgets::Flex, id: LayerId) {
        if let Some(node) = crate::chrome::surface_node_mut(window, id) {
            node.hide();
        }
        // Nothing declared to play on the way out ⇒ it goes now.
        let leaving = self.is_leaving(window, id);
        if let Some(l) = self.layers.iter_mut().find(|l| l.id == id)
            && !leaving
        {
            l.visible = false;
        }
        self.leave_context(id);
    }

    /// Is the tiled area covered by any visible layer? The input to `Domain::Overlay`
    /// (F003/P086/T371).
    ///
    /// **It is `lock` alone, and `modal` has nothing to do with it.** The two answer
    /// different questions: `modal` is "does this take the keyboard", `lock` is "may
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
    pub(crate) fn content_covered(&self, window: &heca_grid_ui::widgets::Flex) -> bool {
        self.layers
            .iter()
            .any(|l| l.is_active(self.is_leaving(window, l.id)) && lock(window, l.id))
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

}

#[cfg(test)]
mod tests;
