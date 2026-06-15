# DnD Framework Refactor — make `heca-grid-ui::drag` a great, generic, reusable system

> Goal (per user): refactor the existing drag-and-drop framework to be **as generic as
> possible** and make it a **great, reusable framework we can use anywhere in the app** —
> *before* re-implementing actual DnD. Domain-neutral, theme-driven, widget-integrated,
> zero app concepts in the crate. Then DnD (F4.5) is wired on top.

Branch: `grid-ui-chrome-integration`. Constrained by RESUME-ws-a.md §0 rules
(generic + theme-driven widgets; foundation before style; no `cargo fmt`; fix all warnings).

---

## 1. What exists today (and why it isn't "great" yet)

`heca-grid-ui/src/drag/` is a surface-agnostic **state coordinator** — good bones:

- `DragContext` — owns `HashMap<DragSurfaceId, SurfaceDragState>`, tracks the active surface.
- `SurfaceDragState` / `SurfaceDragPhase` — per-surface `Idle → Starting → Dragging` machine,
  threshold capture, `hover_item` / `source_item` / `ghost_label`.
- `DragSurfaceId` / `DragItemId` / `DragItemKind` / `DragItem` — surface enum + opaque index.
- `math.rs` — `DEFAULT_DRAG_THRESHOLD_SQ` + NIRI `rubberband` (pure, tested).

Three things keep it from being the reusable framework we want:

1. **Domain-polluted.** `DragItemKind = Pane | Workspace | Column | FloatingPane`, and
   `SurfaceDragPhase` bakes in `pane_id: Option<u64>` + `original_ws: usize`. The framework
   "knows" about workspaces/panes — so it can't host Docker/agents/git/notes drags later
   (violates the domain-neutral rule, memory `heca-widgets-in-grid-ui`).
2. **No geometry layer.** Drop targets are resolved by `heca/src/sidebar/hit_test.rs::sidebar_hit_test`
   — hand-computed fixed-row math that went stale under the retained grid-ui tree. This is
   *exactly why DnD is currently disabled* (commit `45143b5`). A real framework resolves
   drop targets from the **retained widget tree's bounds**, reusing the F4.2 dispatch path —
   not parallel geometry.
3. **Not widget-integrated; rendering ad-hoc.** No widget opt-in (`draggable` / `drop target`),
   the ghost is drawn inline in `render.rs` (`ghost_h = 22.0` hardcoded), theme tokens are
   named `sidebar_drag_*` (also domain-coupled).

---

## 2. Target architecture

Three clean layers, each reusable independently:

```
┌─ drag/ (heca-grid-ui) ────────────────────────────────────────────────┐
│  GESTURE STATE (domain-neutral, generic over payload P)                │
│   DragContext<P>  · SurfaceDragState<P>  · DragPhase<P>                 │
│   DragSurfaceId   · math (threshold, rubberband)                       │
├─ widget integration (heca-grid-ui: builders.rs + component.rs) ────────┤
│   Draggable builder  (.draggable(payload) / .drag_handle())            │
│   DropTarget builder (.drop_target(sink) / .drop_accepts(pred))        │
│   drag::resolve(tree, point) -> hit  (walks retained Base.bounds)      │
│   PaintCx::drag_ghost(...) / ::drop_indicator(...) (theme-driven)      │
├─ app wiring (heca) ───────────────────────────────────────────────────┤
│   AppDragPayload enum (Pane/Workspace/Column/… — the ONLY place these  │
│     names live)                                                        │
│   DragContext<AppDragPayload> in AppState; dispatch from mouse.rs      │
└────────────────────────────────────────────────────────────────────────┘
```

### 2a. Generic payload — `DragContext<P>` (RECOMMENDED) vs `Rc<dyn Any>`

**Recommendation: make the framework generic over a payload type `P`.**
- `DragContext<P>`, `SurfaceDragState<P>`, `DragPhase<P>` carry `P` instead of
  `kind/pane_id/original_ws`. `P: Clone` (drag state is cloned each frame).
- The app defines the single payload enum and instantiates `DragContext<AppDragPayload>`:
  ```rust
  // heca/src/  (the ONLY place workspace/pane/column appear in drag code)
  #[derive(Clone)]
  pub enum AppDragPayload {
      Pane { pane_id: PaneId, origin_ws: usize },
      Column { col_idx: usize, origin_ws: usize },
      Workspace { ws_idx: usize },
      // Docker / Agent / Git / Notes … added here later, framework untouched
  }
  ```
- Zero-cost, type-safe, no downcasts, idiomatic. Matches the crate's "closed set, compiler-
  enforced" style.
- *Alternative considered:* type-erased `Rc<dyn Any>` payload — keeps `DragContext`
  non-generic but needs `downcast_ref` at every read and loses compile-time guarantees.
  Rejected unless we later need *heterogeneous* payloads in one context (we don't).

`DragItemKind` is **deleted** from the crate. `DragItemId` (opaque index) stays — it's
already generic and useful for "which row within a surface."

### 2b. Drop targets from the retained tree (delete `sidebar_hit_test` for DnD)

The retained chrome tree already carries real `Base.bounds` (laid out every frame) and we
already dispatch pointer events into it (`chrome_dispatch_click`, F4.2). Reuse that:

- **Widget opt-in** via two new builder traits (mirroring how `.on_activate` works today):
  - `Draggable`: `.draggable(payload: P)` marks a widget as a drag source; `.drag_handle()`
    optionally restricts the grab region to a sub-widget (the MarkerGroup bar in F4.4).
  - `DropTarget`: `.drop_target(id)` marks a widget as a drop zone; optional
    `.drop_accepts(pred)` filters which payloads it takes.
- **Resolution** = `drag::resolve_at(root: &dyn Component, point) -> Option<DropHit>`: a
  bounds walk (top z-order first, like `route_event`) returning the deepest drop target +
  insertion side (before/after/onto) under the cursor. One generic function; no per-surface
  geometry. `sidebar_hit_test` is no longer used by DnD (kept only for the legacy collapsed
  rail until that's migrated too).
- The gesture is driven by feeding `PointerPressed/Moved/Released` through a small
  `drag::dispatch(root, ctx, event, sinks)` that runs the state machine: press on a
  `Draggable` → `Starting`; move past threshold → `Dragging` + ghost; move → update hover via
  `resolve_at`; release over a `DropTarget` → fire the drop sink with `(payload, target)`.

This is the same read-via-sink / write-via-action shape as F4.2/F4.3, so it composes with the
existing `ChromeSinks`.

### 2c. Theme-driven ghost + drop indicator

- Rename tokens `sidebar_drag_*` → generic `drag_ghost_bg`, `drag_ghost_fg`,
  `drag_source_bg`, `drag_source_border`; **add** `drop_target_bg`, `drop_target_border`
  (drop-zone highlight) and an insertion-line color. Keep `#[serde(alias = "sidebar_drag_*")]`
  so existing `config.toml`/themes still parse (no breaking change).
- Add `PaintCx::drag_ghost(rect, label)` and `PaintCx::drop_indicator(rect, side)` so the
  ghost + insertion line are drawn from theme tokens, not inline hardcoded numbers in
  `render.rs`. (The ghost stays an overlay-layer draw for correct z-order.)

---

## 3. Staging (each slice: `cargo build -p heca` + `cargo test -p heca`, warning-clean, user verifies)

**Phase 1 — Generalize the state layer (no behavior change; DnD stays disabled).**
- Make `DragContext`/`SurfaceDragState`/`DragPhase` generic over `P`; delete `DragItemKind`;
  rename `SurfaceDragPhase`→`DragPhase`.
- Add `AppDragPayload` in `heca`; change `AppState.mouse.drag_ctx` to
  `DragContext<AppDragPayload>`; update `mouse/drag.rs`, `mouse.rs`, `render.rs` to construct/
  read payloads through it. Pure refactor — the disabled drag path keeps compiling.
- Rename theme tokens (+ serde aliases). Update `theme.rs`/`defaults.rs` and the two readers
  in `render.rs`.
- Unit tests for the generic context (threshold transition, payload round-trip, cancel_all).

**Phase 2 — Widget integration + resolution + rendering (still not enabled on the sidebar).**
- Add `Draggable`/`DropTarget` builder traits + the `Base`/event hooks they need.
- Add `drag::resolve_at` (bounds walk) and `drag::dispatch` (state machine over the tree).
- Add `PaintCx::drag_ghost` / `::drop_indicator`; port `render.rs`'s inline ghost onto it.
- Unit tests for `resolve_at` (z-order, accepts predicate, insertion side) on a synthetic tree.
- Showcase: add a draggable→drop-target recipe so the framework is exercised in
  `examples/showcase.rs` (the reference) and documented in `docs/widgets.md` + AGENTS.md catalog.

**Phase 3 — Re-enable DnD on the new framework (= F4.5, after F4.4's MarkerGroup exists).**
- F4.4 builds the generic `MarkerGroup`/rail widget already drag-aware (bar = `.drag_handle()`,
  group = `.drop_target()`).
- Wire sidebar pane/column drags through `drag::dispatch` + `ChromeSinks` drop sink → existing
  move/swap `WmAction`s. Restore the drag-start block disabled in `45143b5`.
- Delete the now-dead `sidebar_hit_test` DnD path (keep it only if the legacy collapsed rail
  still needs it; migrate or remove).

### Sequencing vs F4.4
Do **Phase 1 + Phase 2 first** (the framework), then **F4.4** (MarkerGroup, born drag-aware),
then **Phase 3** (wire it). This satisfies "build the framework before implementing the DnD"
and means the new widget is designed against the finished API instead of being retrofitted.

---

## 4. Files touched

- `heca-grid-ui/src/drag/{mod,context,state,item,math}.rs` — generic-over-`P`, delete `DragItemKind`.
- `heca-grid-ui/src/builders.rs` + `component.rs` — `Draggable`/`DropTarget` traits, event hooks.
- `heca-grid-ui/src/drag/` (new) — `resolve_at`, `dispatch`, `DropHit`.
- `heca-grid-ui/src/component.rs` (`PaintCx`) — `drag_ghost` / `drop_indicator`.
- `heca-config/src/{theme,defaults}.rs` — rename tokens + serde aliases + add drop-target colors.
- `heca/src/app_state.rs` — `AppDragPayload`, `DragContext<AppDragPayload>`.
- `heca/src/mouse.rs`, `heca/src/mouse/drag.rs`, `heca/src/app/render.rs` — payload + ghost port.
- `heca/src/chrome.rs` — drop sink in `ChromeSinks` (Phase 3).
- `heca-renderer/examples/showcase.rs`, `docs/widgets.md`, `AGENTS.md` — recipe + catalog.

## 5. Out of scope (now)
- Cross-window / OS-level drag. Multi-payload-type single context (`dyn Any`). Animating the
  drop (rubberband settle) — `rubberband` exists; wiring its visual is a later polish.
