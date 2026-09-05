# Surface compositor & the layered KeyHint model

This document defines how on-screen surfaces are layered, how that layering decides what is
interactive, and **how input reaches a surface**. It is the contract for adding any **layer,
surface, overlay, modal or button** — read it before touching one.

It absorbs what was `docs/input-architecture.md`, so there is one description of this rather than
two. `docs/hint-architecture.md` covers the picker and stays as it is: where it says a pick
introduces "no second dispatch path", that is about the pick and is correct — it is *pointer* input
that has sixteen paths, not the picker.

---

## 0. Surfaces and input — read this first

### 0.1 The short version

**Everything on screen should be a node in one tree.** Chrome, panes, overlays, toasts, the exposé, a
plugin's panel. Nesting is real, ordering is tree position, and one walk delivers input to all of it.

**As of `P097(F003)/T494` (2026-08-31) this is TRUE for every surface.** The chrome, the panes, the
toast stack, the exposé, the command palette, every modal and every context menu are children of
`AppState::window_root`, and the one walk lays them out, paints them, delivers their pointer events
and collects their hint letters.

It was half true until then: chrome and panes lived in the tree, overlays lived in a separate
registry, and input only walked the tree — so anything registered received **no pointer events at
all**. It was painted and dead to the mouse, with no error. § 0.2 is what that cost.

### 0.2 Why it matters — two silent failures, two days apart

Both came from one new surface, in two different subsystems, and neither produced an error.

**A toast's close button did nothing.** The notification stack was registered as a layer. Input walks
the tree; the layer is not in it. So a click on the × found nothing and fell through to the pane
behind. Hover did the same, which is why hovering a toast highlighted the pane underneath it.

**`prefix+/` lettered only the toast.** The hint walk treated every visible layer's box as something
that hides what is beneath it. A toast stack fills the viewport — not because it covers the screen,
but because that is how it *positions* its cards in a corner — so it hid every letter in the app. The
layer had declared `lock: false`. The action router honoured that declaration; the hint
walk did not.

The shape is the same both times: **rebuild, layout, paint, hints and input are five separate walks
over the surfaces, each with its own idea of what a surface is, and nothing checks they agree.** A
new surface must be got right in all five, and gets no error when it is not.

### 0.3 The three questions

**1. "How do I put something on screen?" — Place it in the tree.** It is a child, like any widget.

You will find code that does this instead:

```rust
let id = state.layers.reserve_id();
state.layers.insert(id, None, LayerKind::Persistent, false, false, root);
state.my_layer_id = Some(id);
```

That is the registry path. It hands ownership away, requires `state` — which a plugin does not have
and should not — and asks you three questions before you can put a box on screen. **It is being
removed. Do not add to it.**

**2. "How do I make it receive clicks?" — You don't.** A node in the tree receives pointer events
from the same capture → target → bubble walk every widget uses (AGENTS.md § 0c). If your component
is in the tree and not receiving events, that is a bug in the walk, not something to work around.

**Do not write a dispatch function for your surface.** There are 16 already — 8 chrome, 4 pane
header, 3 pane viewport, 1 modal — and they exist because each new surface added its own. A 17th is
the defect, not the fix.

**3. "How do I stop clicks reaching what's behind?" — `blocking`, on `Overlay`.** Blocking paints a
scrim and swallows every pointer event. Non-blocking lets a press *beside* the panel fall through
while a press *on* it is handled normally — exactly what an ambient surface like a toast needs. A
property you set, not a mechanism you build.

### 0.4 The `Overlay` widget already does all of it

There are two things called "overlay" here: the **`Overlay` widget** in `heca-grid-ui`, and the
**layer registry** in the app. The widget already implements everything the registry provides:

| what a surface needs | `Overlay` already has |
|---|---|
| fill the viewport, place a panel in it | `Pct(1.0)` fill with real layout, so every descendant gets true bounds |
| swallow input (modal) | `blocking(true)` — scrim plus swallow |
| let input fall through | `blocking(false)` — a press beside it reaches the page behind |
| show and hide | `open` / `hide` / `toggle`, or bind `open_signal` |
| arrive and leave | `Presence` and the `Animation` trait |
| hold the keyboard while open | `open` bound to `Base::focused`, so keys arrive down the focus chain |
| occlusion geometry | `overlay_occludes` — whole viewport when blocking, the panel alone when not |
| **nest inside another overlay** | already works — a `Select` dropdown in a modal body composites above it |

**So the registry adds exactly one thing: reaching a surface that is not in the chrome tree.** That
is the whole difference, and `P097(F003)` removes it.

**The toast should have been an `Overlay`** — non-blocking, placed in the tree. Every behaviour it
needed was already written and tested.

### 0.5 The seam: the drawing pass records, the host performs

Some content cannot be drawn with rectangles and text.

- A **terminal** is rasterised by the GPU into a texture, because its cell glyphs are the hottest
  path in the app. Drawing them as ordinary scene commands is **rejected** and must not be attempted.
- A **frosted backdrop** is the frame so far, blurred. That is not a shape; it is a GPU pass over
  what has already been drawn.

**Neither is a special case in the drawing pass.** A node *records a request* — "my picture belongs
in this box", "blur what is behind me here" — and the host performs the GPU work afterwards. Decided
for the terminal, then found to be exactly what the frost needs; two unrelated features needing one
seam is why it is the right shape.

**The alternative was rejected:** giving the drawing pass a GPU encoder and target. It would hand a
function that describes *what to draw* a second job — owning GPU pass lifetime — force every caller
to supply one including the showcase, which has no GPU pass of its own, and drag `wgpu` types across
a crate boundary a plugin composing a tree must never see.

### 0.6 What changed, and what the registry is now

| | before `P097/T494` | now |
|---|---|---|
| where a surface lives | chrome tree **or** the layer registry | the tree — `AppState::window_root` |
| who owns a surface's tree | the registry | its parent, as a keyed child |
| how input reaches it | 16 per-surface functions; registry surfaces got none | one walk from the root |
| layout and paint | a second set of passes (`layout_layers` / `paint_layers`) | the root's own walk |
| what the registry holds | a parallel tree, ~900 lines | name and nesting — **only** |

**Coverage is the surface's own, declared on the widget** (`P097/T499`). A surface says whether it
stands in front of the page with one builder — `.lock(true)` — and every reader takes it
off the node in the one tree. It was a field on the registry entry, so a plugin could declare it
only by passing it to a host call it had to reach; now the line a plugin author writes is the line
the host writes. **A surface that registers nothing still answers for itself** — the toast stack
does.

**Taking the keyboard is not declared at all — it is read.** A surface that wants keys *holds
focus*, and `Overlay`, `ContextMenu` and `CommandPalette` all bind their open signal to
`Base::focused`; that is the whole of how an open layer takes the keyboard (§ 0c). So an open
overlay is the active context **by being open**, an ambient one like the toast stack is not **by
holding no focus**, and neither author writes anything. `Base::captures_keyboard` is an optional
override for a surface whose keyboard story the framework cannot see; making it a plain flag with a
`true` default would make every ambient surface the active context, and a toast would suppress every
letter behind it.

The registry is left holding **liveness** alone — whether a seated surface is still up — because
hiding a surface leaves its node in the tree, and that is the one thing the tree does not record.

**How to put a surface on screen:** `chrome::place_surface(&mut state.window_root, key, boxed)`. It
positions the surface out of the flow at the full viewport, so it takes no space from the chrome
beside it, and re-placing under the same key replaces it. `chrome::remove_surface` is the
counterpart. Registry-owned surfaces are keyed `chrome::surface_slot(id)` → `"surface:<n>"` and found
with `chrome::surface_node(_mut)`; a surface that has left the registry declares its own name (the
toast stack is `heca.notifications`).

**The chrome is child 0**, seated by `chrome::seat_chrome`, which finds its slot **by key**
(`chrome::CHROME_KEY`) and never by position — a surface may be placed before the first chrome is
ever built, and a positional "child 0" would seat the chrome straight over it. The window root is
**not an `Option`** and outlives every chrome rebuild: the chrome subtree is discarded whenever its
signature changes (window size, scale, sidebar widths, theme) and `reload_config` drops it outright,
so a surface parented to the chrome would lose its open state, its half-played arrival and its focus.

### 0.7 ⚠️ THE TREE TICKS; THE REGISTRY ONLY RECONCILES

**Read this before touching the frame loop.** Surfaces are children of the window root, so
`window_root.tick(dt)` advances them — **once**. Nothing else may advance them.

The registry's job is only to notice the frame an exit *finished*, and that means looking either side
of a tick it does not own:

```rust
let leaving_before = state.layers.leaving_before_tick(&state.window_root);   // BEFORE
let mut chrome_animating = state.window_root.tick(dt);                       // the one advance
chrome_animating |= state.layers.retire_finished_exits(&mut state.window_root, &leaving_before);
```

When the registry ticked them as well, every surface advanced twice a frame and the registry read
`was_leaving` *after* the tree's tick had already consumed the transition. On the frame an exit
finished it saw "was not leaving", never retired the surface and never requested the frame that
paints its absence: **the exposé stuck at a tenth opacity until some other input forced a repaint,
stayed modal, and swallowed `ctrl+h/j/k/l` for ever after** (Antonio, driving, 2026-08-31).

Tests must run the real order — snapshot, tick the **tree**, reconcile. A test that calls a registry
tick alone cannot see this, because it never runs the walk doing the second advance. See the `frame`
helper in `heca/src/chrome/layers/tests.rs`.

### 0.8 The target model — four passes, one tree

There is **one retained tree** per window. Everything visible is a node in it: the chrome is a
subtree, an overlay (a toast stack, a menu, a dialog, a plugin panel) is a positioned node, a modal
is a node that **swallows** what it does not itself handle, a pane's header is a child of that pane,
and the terminal viewport is a **leaf whose paint is special** — it still draws through the GPU
terminal path, but for layout, hit-testing and event delivery it is an ordinary leaf.

Given that tree the host does four things per frame, each **one walk**:

| pass | what it does |
|---|---|
| layout | size and place every node |
| paint | draw every node (the terminal leaf swaps in its GPU path here) |
| input | deliver each device event: capture down to the target, bubble back up |
| hints | collect the `prefix+/` letter targets |

`heca-grid-ui` already provides the input walk — `dispatch(node, ev)` in
`heca-grid-ui/src/component.rs` is DOM-shaped and complete. **The host's job is to own one tree and
call it once per event**, not to call it once per root and arbitrate between roots itself.

### 0.9 Two designs that get proposed repeatedly, and why both are wrong

Stated so they are not proposed a third time.

**Not: a second dispatcher for "overlay" events.** The tempting local fix, when an overlay receives
no input, is to add a dispatch function for that kind of overlay beside the one for modals. It does
not scale — the next ambient surface needs a third, then a fourth. Each is a hand-written enumeration
of which roots exist and in what order, and **each new surface must be added to every one of them**:
the hint walk, the visibility check, the letter assignment. A miss is not a compile error; it is a
surface that is painted and dead to the mouse, with nothing reporting the gap.

**Not: a host-maintained list of surfaces.** The cleaner-looking version collapses those functions
into one struct — `{ root, rect, z, modal }` — and one dispatch looping a `Vec<Surface>`. The
duplication goes, but **a list the host maintains** remains: to make a widget receive input, a
developer must know it has to become a `Surface` and enrol it. That enrolment is a registry, and a
registry is the signature of a missing API — the thing that should be automatic becomes a rule every
caller must remember. The framework already walks children automatically. Put the widget **in the
tree**, where the existing walk finds it.

**What one tree buys.** Adding a surface becomes **adding a child**. No new dispatch function, no new
match arm, no new hint-walk case. Pointer input, keyboard, hint letters, layout and paint all arrive
from the traversal every other node already gets. The showcase and the app converge, because neither
has any bespoke wiring left in which to differ — a widget behaves the same mounted in a demo, in the
chrome, or in a plugin's overlay.

### 0.10 Plugin overlays — plumbing and capability are different questions

**The plumbing half follows directly.** Once input is a tree walk, a *described* overlay contributed
by a plugin is just another node: pointer input, keyboard, hint letters, layout and paint, with
nothing the host has to be taught.

**The capability half is separate and larger.** A plugin can only build an overlay from widgets that
have a declarative form. Several do not — including `CardGrid`, which the exposé's cursor is built
on — and some (`ChromeRegion`, `Pane`, `FocusScope`) are host-only by design and never will. Until an
overlay of the app's own is built purely through the described path, **"a plugin can add an overlay"
describes the intended architecture, not a demonstrated fact.** Declarative coverage, plus one real
overlay rebuilt through it, are the proof.

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
2. Give it its action and say what a pick does — **one line on the widget itself**:
   ```rust
   let button = IconButton::new(icon)
       .on_click(/* the intent */)
       .on_hint(/* what prefix+/ does to it — usually the same intent */);
   ```
   (Tooltip + shortcut come automatically — see AGENTS.md "Chrome buttons → action,
   tooltip, KeyHint".)
3. **Do not** set any layer/z on the button. It inherits its layer from the surface it
   lives in. If it needs `FocusPaneThenAction` (active-targeted, like zoom/float), that is the
   intent you attach — same as pane-header buttons.

   ⚠️ **`hints.register(...)` and `.hint_target(id)` no longer exist.** `HintTargetRegistry`,
   `HintTargets`, `HintTargetId`, `Base::hint_target` and `named_press` were deleted under Rule Zero
   (F004/P084/T399): a plugin could construct none of them. A pick is now one builder on the widget,
   collected out of the laid-out tree. Nothing is registered, so nothing has to be un-registered when
   a tree rebuilds. Do not reintroduce any of those names.

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

> **Note (2026-07-11):** "DockView" here is the **selector** concept (which mounted Provider is
> shown), not a widget — the mounted unit is a **Provider** (see
> [`plugin-authoring.md`](./plugin-authoring.md)). Sidebar **display modes** are a separate axis:
> a region is **Expanded ⇄ Hidden** (the collapsed icon rail is dropped for now), so there is no
> collapsed-rail hint-visibility case to model. Full sidebar Provider / display-mode design:
> **[`sidebar-provider-modes.md`](./sidebar-provider-modes.md)**.

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

## 9. Dynamic layers — registry, actions, plugins (SHIPPED, then SUPERSEDED)

> ⚠️ **This section describes the layer registry as it was when it owned surface trees. It no
> longer does** — `P097(F003)/T494` moved every tree into the window root (§ 0.6) and the registry
> now keeps only name, nesting, modality and `lock`. Read § 0 first; what follows is
> history plus the parts of the registry that survive. It was built and it
> works, but it turned out to be a **parallel tree implementation** — it stores parent links and
> re-derives nesting every frame, duplicating what child position gives for free, and every walk over
> it is a separate implementation free to disagree with the others. That is what produced both
> failures in § 0.2.
>
> **Read § 0 first.** A surface belongs in the tree; `Overlay` already provides everything the
> registry does except top-level reach (§ 0.4). `P097(F003)` reduces the registry to a name → node
> lookup.
>
> Kept because the app still works this way today and you will meet it in the code. **Do not build
> anything new on it** — see § 0.7 if you have no choice.

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

### Relationship to `OverlayHost` (`pluggable-chrome-plugin-plan.md` §2.6 / §2.7.1)

The plugin plan already ratifies two contracts this section must **build on, not duplicate**:

- **`ViewNode`** (§2.6.2) — the serializable declarative widget tree. Implemented in
  `heca/src/chrome/view.rs` (plugin-task-ui-1). It is the `LayerContent::View(...)` above and
  the body of a modal. `realize(&ViewNode) -> Box<dyn Component>` (plugin-task-ui-3) turns it
  into a retained tree; **actionable nodes (an `on_press` intent) get a KeyHint target for
  free**, which is exactly how an overlay's buttons become hintable.
- **`OverlayHost`** (§2.7.1) — the host-owned overlay API: `open_modal(ModalSpec { title,
  body: ViewNode, actions, … }) -> OverlayFuture<ModalResult>` (and `open_dropdown`).

These are **two levels of one stack**, not two stacks:

- the **`LayerStack`/`LayerRegistry`** here is the low-level layering mechanism (push/pop a
  layer; band z-order; occlusion; hint visibility; later paint + input routing);
- **`OverlayHost` is built on it**: `open_modal` `realize`s the `ViewNode` body + action
  buttons into a native tree, **pushes it as a `Modal`-band overlay layer**, and resolves the
  returned `ModalResult` when a button's intent fires. Its overlays *are* layers here.

So step 2 (the confirm dialog) is done the conformant way: as an `OverlayHost::open_modal`
(a `ViewNode` body + actions) rather than a bespoke modal — closing the overlay-KeyHint gap
and standing up the first slice of `OverlayHost` on the `LayerRegistry`.
