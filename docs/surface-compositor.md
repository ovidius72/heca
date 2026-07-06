# Surface compositor & the layered KeyHint model

> **Status:** design spec / target architecture. The universal KeyHint picker
> (`prefix+/`) is the **first consumer**; painting z-order and input routing can adopt the
> same tree later. Until this lands, hint visibility is handled by three interim geometric
> filters (viewport / sidebar-occlusion / pane z-order) in `handle_hint_pick` — this
> architecture **replaces** them with one uniform rule.

This document defines how on-screen surfaces are layered and how that layering decides
what is interactive (today: which KeyHint keycaps are shown). It is the contract for
adding new **layers, surfaces, and buttons** — read it before touching any of them.

---

## 1. Why

Chrome is not flat. At any moment the screen is a stack of things: a blurred background,
the tiled panes, the sidebar over them, floating panes over those, and — on demand —
overlays like a confirm dialog, a context menu, or (future) an exposé, each possibly with
its own child overlays. "Which button is reachable right now" depends on this layering.

We do **not** solve this per-feature. A new surface (a modal, an exposé, a new widget with
buttons) must slot into the model and get correct behaviour **for free** — no bespoke
visibility patch. This is a hard project rule (see AGENTS.md and
`always-generic-never-patch-narrow-problem`).

---

## 2. Core concepts

### Surface
A **Surface** is a mountable retained widget tree plus a little metadata. It is the unit of
layering. A surface owns:

- **root** — its retained component tree (the thing `collect_hint_targets` walks). Its
  hint targets are ordinary widgets that opted in with `.hint_target(id)`.
- **occluder** — the opaque region it paints over the surfaces beneath it, taken from its
  **real laid-out geometry** (a pane's frame, the sidebar's rect, a dialog's scrim). Never
  a hardcoded rectangle. A surface with no opaque body (e.g. a keycap-only overlay) has no
  occluder.
- **modal** — if true, it is a *blocking* context: when active it captures the interaction
  and suppresses everything beneath it.
- **visible** — whether it participates this frame.

A button never knows or declares its layer. **It inherits its layer from the surface it is
mounted in.** Put a button in the pane header → it's in the pane-content context. Put it in
a dialog → it's in that dialog's context. That's the whole rule for buttons.

### The surface tree (the compositor tree)
Surfaces form a **tree**, not a flat list. A child surface is drawn *above* its parent, and
overlays are **children of whatever opened them**:

```
root
├─ [0]   background            (blurred bg + gradient; passive, no targets)
├─ [1]   panes container       (base context)
│  ├─ [1,0] pane A
│  └─ [1,1] pane B
├─ [2]   sidebar               (base; drawn above the panes; hosts an internal DockView selector)
├─ [3]   floating panes        (base; drawn above the tiled panes)
└─ [4]   exposé                (a context surface, opened on demand)
   └─ [4,0] exposé's modal     (child of the exposé → sits above it)
```

**z is the position in this tree — never a stored number.** A surface's z-path is the chain
of sibling indices from the root to it (`[4,0]` above). Ordering is **lexicographic on the
path**, which equals a **pre-order traversal**, which equals **paint order**:

```
[0] < [1] < [1,0] < [1,1] < [2] < [3] < [4] < [4,0]
```

Consequences that fall out for free:
- a **child is above its parent** (`[4] < [4,0]`), so a modal is above its opener;
- **later siblings are above earlier ones** (`[1,0] < [1,1]`, `[1] < [2] < [3]`), so the
  sidebar is above the panes and floats are above the tiled panes;
- **inserting "in the middle"** is `Vec::insert` at the right sibling index — no float
  fragility (`3.15` between `3.1`/`3.2`), no renumbering by hand.

You **never** write a z value. You mount a surface as a child of a parent at a position and
the path — hence the order — is derived from the structure.

### Two orthogonal mechanisms
Layering does **two different jobs**; keep them separate.

1. **Coarse — context activation (`current_index`).**
   `current_index` (on `AppState`) points at the **active context surface**. It decides
   *which context is live*. It is moved **only** by opening/closing **context** surfaces
   (an exposé, a modal). Zoom, scroll, and DockView selection do **not** move it.
   - Opening a **modal** sets `current_index` to it → it's exclusive → everything beneath
     goes dormant. Closing restores the previous context.
   - The **base context** (panes + sidebar + floats) is **not** exclusive: several surfaces
     are live together there.

2. **Fine — geometric occlusion by z-order.**
   *Within* the set that the coarse rule has made eligible, a target is hidden iff a surface
   **higher in the total z-order** covers it geometrically. "Covers" = the target button's
   **centre** falls inside that higher surface's **occluder** (its real frame). This is the
   single rule behind zoom, floats, and the sidebar covering scrolled-under panes.

> "Same level" ≠ "same z". Surfaces in the same *context* still have a **total draw order**
> (their tree position). There are never ties — two surfaces can't share a tree position —
> so occlusion is always decided by draw order + geometry, **not** by the coarse level.

---

## 3. The resolution rule (one algorithm)

Producing the set of active hint targets is a single pass:

1. Find `current_index` — the active context node.
2. **Eligible set** = the active context: `current_index` plus its **visible, non-blocking
   descendants**. Everything below the active context is suppressed (a modal is exclusive).
   At the base context this is panes + sidebar + floats together.
3. Walk the eligible surfaces **top → bottom** (highest z first), accumulating each one's
   occluder. A target is kept iff:
   - it lies within the **viewport**, **and**
   - its centre is **not** inside any already-seen (higher-z) surface's occluder.
4. The kept targets, in order, get the picker letters.

This subsumes every ad-hoc case:
- **off-screen** pane (scrolled out) → not in viewport → dropped;
- **hidden behind the sidebar** → the sidebar surface is higher-z; its occluder covers the
  pane button beneath → dropped;
- **zoomed pane over another** → the zoomed pane is a later sibling (higher z); its frame
  occluder covers the other pane's buttons → dropped;
- **float over tiled panes** → float is higher-z; same as zoom;
- **modal open** → `current_index` is the modal; it's exclusive → only its buttons are
  eligible.

### Worked example — zoom / float
Two tiled panes overlap because pane B is zoomed/enlarged and drawn on top of pane A. Both
are the base context (both *eligible*), but B is a later sibling (higher z). Walking
top→bottom: B's targets are kept (nothing above covers them), then B's frame is pushed as an
occluder; A's targets whose centre lands in B's frame are dropped; A's targets outside B
stay. A floating pane behaves identically (it is simply higher-z than the tiled panes).

---

## 4. Current surfaces (mapping)

| Surface | Context | Occluder | Modal | Notes |
|---|---|---|---|---|
| Background | root, lowest | none | no | Blur + gradient. Passive, no hint targets. |
| Panes container | base | — | no | Tiled panes are siblings; **zoom** = one pane drawn later, covering others (geometry, not a level). Each pane's header buttons are its hint targets. |
| Sidebar | base, above panes | its laid-out rect | no | Occludes pane content beneath it. Hosts an internal **DockView selector** (see below). Its cards/toggles are hint targets. |
| Floating panes | base, above tiled | each float's frame | no | Cover tiled panes beneath them. |
| Context menu | child of its opener | its rect | yes | Exclusive while open. |
| Confirm dialog / Modal | child of its opener | scrim | yes | Exclusive while open. |
| Exposé (future) | child of its opener | its region | maybe | A context surface; opening it sets `current_index`. Its own modal is a **child** of it. |

---

## 5. How to implement future things

### …a new button
1. Build it as a normal widget in **some surface's** retained tree.
2. Give it its action, then register the intent and attach it:
   ```rust
   let hint_id = hints.register(InteractionIntent::ActivateAction(action.clone()));
   let button = IconButton::new(icon).hint_target(hint_id).on_click(/* same intent */);
   ```
   (Tooltip + shortcut come automatically — see AGENTS.md "Chrome buttons → action,
   tooltip, KeyHint".)
3. **Do not** set any layer/z on the button. It inherits its layer from the surface it
   lives in. If it needs `FocusPaneThenAction` (active-targeted, like zoom/float), register
   that intent instead — same as pane-header buttons.

That's it. Because the button lives in a surface, the resolver assigns its layer and its
visibility for free.

### …a new surface
1. Build its retained tree (its `root`).
2. Register it in the compositor tree as a **child of the correct parent**:
   - part of the base UI → child of the base context;
   - an overlay opened *by* something → **child of that something** (so it sits above it).
3. Provide its **occluder from its real layout** (its frame/bounds), not a constant. If it
   paints nothing opaque, no occluder.
4. Set `modal` if it must be exclusive, and drive `visible`.
5. Do **not** assign a z. Its position in the tree is its z.

### …a new context / layer (e.g. an exposé)
An exposé is just a surface mounted as a child of its opener. If it is a **context switch**
(it takes over interaction), opening it sets `current_index` to it and closing restores the
previous value. If it should block what's beneath, mark it `modal`. A modal opened *from*
the exposé is a **child of the exposé** — it automatically sits above it and, being modal,
becomes the new `current_index`. No numbers, no recomputation: dropping a new context
anywhere in the tree just works because z and occlusion are relational.

### …a DockView in the sidebar
DockViews are **mutually exclusive** (one-of-N visible), so they are **not** compositor
surfaces and **not** a stack. The sidebar keeps a single `active_dock_view` id; each
DockView's `visible = (id == active_dock_view)`; switching flips that id. If a DockView
opens an overlay/modal, that overlay **is** a compositor surface — a **child of the
sidebar** — handled by the global rules above, not by the DockView selector.

---

## 6. Invariants (do / don't)

- **No hardcoded z / levels anywhere.** z is tree position; occluders come from real
  layout geometry. If you're typing a magic number for a layer or a region, stop.
- **Buttons never declare a layer.** They inherit it from their surface.
- **`current_index` is moved only by context surfaces** (exposé / modal). **Never** by
  zoom, scroll, or DockView selection — those keep the base context live (so the sidebar
  stays hintable while a pane is zoomed).
- **Occlusion is decided by draw order (fine z) + geometry, not by the coarse level.**
- **Overlays/modals are children of their opener** and are exclusive when `modal`.
- **Sidebar DockViews are a selector, not a stack.**
- **One rule for visibility.** If you find yourself special-casing a surface's hints, the
  surface is mis-modelled — fix the model, don't add a filter.

---

## 7. Proposed data structures (target API)

Sketch — the shape the implementation should land on (names may refine):

```rust
/// A stable id for a surface in the compositor tree.
struct SurfaceId(u64);

/// A mountable layer: its retained tree, the region it occludes (from layout), and
/// whether it is a blocking (modal) context.
struct Surface {
    id: SurfaceId,
    parent: Option<SurfaceId>,     // None = root child; overlays point at their opener
    // how to enumerate this surface's hint targets + read its occluder each frame
    // (a handle to its retained root; occluder computed from that root's bounds/frame)
    modal: bool,
    visible: bool,
}

/// The compositor tree, owned by AppState. z-order = pre-order over children Vecs.
struct SurfaceTree {
    surfaces: /* arena keyed by SurfaceId, children ordered per parent */,
    current_index: SurfaceId,      // the active context node
}

impl SurfaceTree {
    /// The single visibility rule of §3: eligible-by-context, then geometric occlusion.
    fn active_hint_targets(&self, viewport: Rectangle)
        -> Vec<(HintTargetId, Rectangle)>;
}
```

`handle_hint_pick` becomes: `SurfaceTree::active_hint_targets(...)` → assign letters. The
hint `HintTargetRegistry` (the shared monotonic allocator) is unchanged — it still maps
opaque ids → intents; the surface tree only decides **which** ids are eligible this frame.

---

## 8. Consumers

- **Now:** the universal KeyHint picker (`prefix+/`).
- **Later (same tree, no new model):** paint z-order (draw surfaces in pre-order), input
  routing (a modal context captures events), background blur/vibrancy ordering.

The point of the tree is to be the **single source of truth for layering**, so every one of
these reads the same structure instead of re-deriving order per feature.

---

## 9. Dynamic layers — registry, actions, plugins (DRAFT)

> **Status: draft for review.** §§1–8 are implemented (hints consume a stack assembled in
> `active_hint_targets`). This section promotes that hand-assembled stack to a **persistent,
> registerable layer stack** so layers can be added dynamically — from Rust **and** from
> plugins — and so overlays/modals are hintable like everything else.

### The gap this closes
Two things drive it:
- **Overlays currently show no KeyHint.** The `Modal` (e.g. delete-pane confirm) **draws its
  buttons manually** — there is no child subtree — so `collect_hint_targets` finds nothing to
  hint. It shows its own hand-drawn letter shortcuts, but not the universal keycaps. The goal
  is *KeyHint on every action*, so its buttons must be **real components** carrying
  `.hint_target(...)`.
- **Plugins must be able to add layers** (an exposé, a panel, a rich modal) without touching
  paint/input/hint machinery.

Both are the same fix: a **layer registry** whose content is either a native tree or a
**`ViewNode`** tree realized to components, where **actionable nodes get a hint target
automatically**.

### The registry (target model, on `AppState`)
Every surface — panes, sidebar, floats, overlays, future exposé/plugin layers — is a
registered **Layer**.

```rust
struct LayerId(u64);                 // stable; returned by add_layer (a plugin keeps its id)

enum LayerBand {                     // SEMANTIC z (no magic numbers); order within a band = insertion order
    Background, Content, Floating, Overlay, Modal,   // extensible
}

enum LayerKind {
    Persistent,   // always in the stack, eligible when its context is active (panes, sidebar)
    OnDemand,     // hidden until shown (exposé, palette); `ShowLayer` brings it up + makes it the context
}

enum LayerContent {
    Native(Box<dyn Component>),      // built in Rust (chrome, pane header)
    View(ViewNode),                  // data/plugin-described; host `realize()`s it to components
}

struct Layer {
    id: LayerId,
    band: LayerBand,
    kind: LayerKind,
    modal: bool,                     // captures the context (suppresses beneath) while active
    visible: bool,
    parent: Option<LayerId>,         // an overlay opened FROM a layer is its child (sits above it)
    content: LayerContent,
    // occluder is read from the realized tree's laid-out bounds — never hardcoded
}

struct LayerStack {                  // on AppState — the single source of truth for layering
    layers: /* arena of Layer, ordered by (band, insertion) with parent nesting */,
    active: LayerId,                 // the active context (== `current_index` in §2)
}
```

`LayerBand` replaces raw z numbers with **semantic bands** — a caller/plugin picks a band, and
ordering within a band is insertion order (and parent nesting). No `3.15`, no renumbering.

### API (code + plugins)
```rust
let id = layers.add_layer(LayerSpec { band: Overlay, kind: OnDemand, modal: true, .. });
layers.layer_mut(id).set_content(ViewNode::stack([w1, w2, /* … */]));   // or Native(tree)

dispatch(WmAction::ShowLayer(id));   // OnDemand → visible + active (context switch)
dispatch(WmAction::HideLayer(id));   // dismiss, restore the previous active context
```
- **Persistent vs OnDemand** is the "panes-like vs exposé-like" distinction you asked for:
  persistent layers are always present; on-demand layers are shown via `ShowLayer` and become
  the active context while up.
- `ShowLayer(id)` sets `active = id` (+ `visible`), so `current_index` = that layer → the base
  context goes dormant when the layer is a context switch (modal or full-screen on-demand).

### Actions + keybindings
```rust
WmAction::ShowLayer(LayerId)
WmAction::HideLayer(LayerId)
WmAction::ToggleLayer(LayerId)
```
A keybinding maps to `ShowLayer(id)` like any action (through the registries), so a plugin's
exposé opens via a normal, **rebindable** binding — and is reachable from mouse/RPC too.

### Plugins
A plugin registers a layer with a **`ViewNode`** tree (the serializable UI model from the
plugin-ui plan). The host `realize(&ViewNode) -> Box<dyn Component>` builds the components, and
**every actionable node (button) is given a `hint_target`** wired to its declared action
(`WmAction` or a plugin action id). So a plugin exposé/panel/modal is **painted, input-routed,
and hintable for free** — the plugin never touches that machinery.

### Consumer changes
- **Hints:** `active_hint_targets` iterates `LayerStack` instead of the hand-coded branches —
  same rule (§3).
- **The delete-pane modal** becomes an on-demand modal layer with real / `ViewNode` buttons →
  its KeyHints appear like everywhere else (gap closed).
- **Paint / input (later):** draw layers in stack order; route input to the active/modal layer.

### Migration (incremental, no big bang)
1. Introduce `LayerStack` and **register the existing surfaces** (panes, sidebar, chrome,
   current overlays); `active_hint_targets` reads the stack. Behaviour identical.
2. Convert the confirm dialog to a layer with **hintable buttons** → overlay-KeyHint gap fixed.
3. Add `WmAction::ShowLayer` + on-demand layers → first native exposé.
4. Add the `ViewNode` content path → **plugins add layers**.
