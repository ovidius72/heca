# RESUME — F4.5 sidebar DnD (re-enable on the generic framework)

> Detailed handoff for the **in-flight F4.5** feature. Single source of truth for *tasks* is
> **PLAN.md P2**; this doc = exhaustive current state + exact next steps + gotchas so a cleared
> session continues without re-deriving. **Delete this file when F4.5 (1b+2+3) is fully merged.**
> Detail rule: memory `handoffs-must-be-detailed` (resolved decisions + why + `file:line` + steps).

## Branch / build / PRs
- **Branch:** `grid-ui-f4.5-sidebar-dnd` (off `origin/main`). 1a committed + pushed.
- **Open PRs:** **#116** = F4.5 **1a** (this work) → main. **#115** = docs cleanup (removes the old
  `RESUME-chrome-state.md`, folds grid-ui crate-review debt into PLAN.md item 9) → main. Neither merged yet.
- **Build:** `cargo test -p heca` = 191 pass; clippy clean (bar pre-existing terminal-WIP dead-code
  notes in `app/selection_model.rs`/`backend_factory`/RPC + the `render.rs:58` `too_many_arguments`,
  all main's terminal code, NOT ours; + transitive `block v0.1.6`).
- Local `main` worktree (`/Users/antonio/projects/heca-ui`) was synced through PR #114; re-sync after
  merging #115/#116.

## The big picture (read once)
F4.5 = re-enable sidebar drag-and-drop, which was **disabled in `45143b5`** because its source/hover/
drop all used the legacy fixed-row `sidebar::sidebar_hit_test`, whose geometry no longer matches the
grid-ui retained tree. The DnD **framework was genericized after `45143b5`** (`DragItemKind`/
`SurfaceDragPhase` removed → `DragContext<P>`/`DragPhase` + the app's `AppDragPayload`), so it is a
**reimplementation, not a revert**. Design rationale: `dnd-framework-refactor-plan.md`; tasks: PLAN.md P2.

### Framework API (all in `heca-grid-ui`, ready)
- `DragItemId(usize)` — opaque; `::new(i)` / `.raw()`. `builders::DragExt`: `.draggable(id)` sets
  `Base.drag_source`, `.drop_target(id)` sets `Base.drop_target` (blanket-impl for every `Component`).
- `drag::source_at(root: &dyn Component, Point) -> Option<DragItemId>` — topmost/deepest drag source
  under a point. `drag::resolve_at(...) -> Option<DropHit{ id, bounds, side: DropSide }>` where
  `DropSide` = `Before|Onto|After` (vertical thirds).
- `PaintCx::drag_ghost(rect, text)`, `PaintCx::drop_indicator(bounds, side)` — for 1b visuals.
- App side: `DragContext<AppDragPayload>` at `state.mouse.drag_ctx`; `DragPhase` =
  `Idle | Starting{payload,start_pos,threshold_sq} | Dragging{payload}`; per-surface
  `SurfaceDragState{ phase, hover_item, source_item, ghost_label }`.

## DONE — 1a (functional pane drag), PR #116
Pane cards (only) are draggable + drop targets; press starts a drag via `source_at`, release
moves/swaps via `resolve_at`. **No in-drag visual yet** (1b).

- `heca/src/chrome/mod.rs`:
  - `pane_card` Row: `.draggable(DragItemId::new(pane_id.0 as usize)).drop_target(same)`. **Pane ids
    are globally unique**, so the opaque `DragItemId` IS the pane id — no side-map needed (decode:
    `PaneId(id.raw() as u64)`). (Columns will NOT have this luxury — see step 2.)
  - New helpers near `chrome_dispatch_click`: `pub(crate) fn sidebar_drag_source(state, pos) ->
    Option<PaneId>` (→ `source_at` on `chrome_tree.root`) and `sidebar_drop_target(state, pos) ->
    Option<(PaneId, DropSide)>` (→ `resolve_at`). Both `state.chrome_tree.as_ref()?`.
  - Imports added: `builders::DragExt`, `drag::DragItemId`.
- `heca/src/mouse.rs` `on_mouse_input` Left-Pressed sidebar arm:
  - **GOTCHA (the bug that cost a debug cycle):** `surface_click_action` → `surface_left::click_action`
    → `chrome::chrome_dispatch_click` which sets **`state.chrome_tree = None`** (mod.rs ~`:661`). So the
    drag source MUST be captured **before** `surface_click_action`. Order is now: `let drag_source =
    chrome::sidebar_drag_source(state, pos);` THEN `let sidebar_action = surface_click_action(...)`.
  - If `drag_source` is Some: store `pending_click_action`, set
    `DragPhase::Starting{ payload: AppDragPayload{pane_id, origin_ws, swap=shift}, start_pos, threshold_sq:
    DEFAULT_DRAG_THRESHOLD_SQ }`, `source_item = Some(DragItemId::new(pane_id.0))`, `set_active(LeftSidebar)`,
    return None. `origin_ws` from `crate::find_pane_location`. Imports restored: `DragItemId`,
    `DEFAULT_DRAG_THRESHOLD_SQ`, `AppDragPayload`.
  - Threshold→`Dragging` transition + ghost_label already live in `mouse/drag.rs::handle_sidebar_drag_starting`.
- `heca/src/mouse/surface_left.rs::accept_drop`: resolves target via `chrome::sidebar_drop_target` (not
  `sidebar_hit_test`). Swap branch: `if swap && let Some(target_pid) = target` → `handle_swap_param`.
  Move branch: synthesizes `SidebarItem::Pane{ pane_id: target_pid }` and reuses the existing
  `place_pane_at_sidebar_target` (its `Pane` arm inserts into the target's column at `t_pi+1`).
  **`DropSide` is currently IGNORED** (always inserts after) — refine in 1b/later.
- `heca/src/mouse/hit_test.rs`: removed the now-dead `sidebar_pane_hit_test` (+ its import in `mouse.rs`).
  NOTE: `sidebar::sidebar_hit_test` itself STILL EXISTS and is used by the **collapsed-rail** path +
  `handle_interactive_move_drop` (content-area drag) — do NOT delete it.

## NEXT — in order (each its own PR; PLAN.md P2)

### 1b — in-drag visuals (ghost + drop indicator)
Today the drag shows nothing until drop. Add, via the grid-ui paint path:
- A **drag ghost** following the cursor (`PaintCx::drag_ghost(rect, label)`) and a **drop indicator**
  on the hovered target (`PaintCx::drop_indicator(hit.bounds, hit.side)`).
- Where: a drag-overlay pass after `paint_chrome_root` in `heca/src/app/render.rs` (~the chrome paint
  block), reading `state.mouse.drag_ctx` (is_dragging + payload for the label) and
  `chrome::sidebar_drop_target`/`resolve_at(chrome_tree.root, mouse_pos)` for the indicator bounds/side.
  The dragged source Row can dim via its `source_item`.
- **The existing legacy ghost** (`mouse/drag.rs` sets `ghost_label: DragLabel`; `update_sidebar_drag_hover`
  sets `hover_item` via `sidebar_hit_test`) is drawn by the **hand-drawn** sidebar render — for the
  EXPANDED grid-ui sidebar that path is not active, so `update_sidebar_drag_hover`'s legacy `hover_item`
  is effectively unused there. Replace/retire it; drive the indicator from `resolve_at` instead.
- Once `DropSide` is painted, also honor it in `accept_drop` (Before → insert at `t_pi`, After/Onto → `t_pi+1`).

### 2 — column drag (panes AND columns; decided 2026-06-16)
- **`AppDragPayload` (`heca/src/app_state.rs:118`) must become an enum** (`Pane{pane_id,origin_ws,swap}
  | Column{ws,col_idx,swap}`) — the struct comment already says it. This breaks the struct-destructures
  in `mouse/drag.rs` (handle_sidebar_drag_starting/move) + `mouse.rs` + `release.rs` — update all.
- Mark `MarkerGroup` (column) `.draggable(id)`/`.drop_target(id)` in `chrome/mod.rs::column_view`.
  Columns have NO global id → need a **side-map** `DragItemId → (ws, col_idx)` collected during build
  (same pattern as `ChromeSignals`, store on `RetainedChrome`), since the pane-id-as-id trick won't work.
  Use a disjoint id space (e.g. high-bit tag, or the side-map decides kind).
- Hit-testing is innermost-first, so a press in the `MarkerGroup` **grip gutter** (its only own surface,
  not covered by child rows) → column; a press on a pane card → pane. Falls out for free.
- New WM action for column reorder; apply in `accept_drop` (column branch). Route through the registry.

### 3 — hover dispatch + grab cursor (decided 2026-06-16; PLAN P2)
- **Hover:** the app dispatches **only `PointerPressed`** into the chrome tree today, so `MarkerGroup`/
  `Row` `hovered` never updates in-app (the grip hover/grab cue is inert; works only in the showcase).
  Dispatch `Event::PointerMoved{pos}` into `state.chrome_tree.root` from the cursor-moved path
  (`mouse/drag.rs::on_cursor_moved` or `mouse.rs`) + request repaint. Do NOT null the tree on move
  (unlike `chrome_dispatch_click`).
- **Cursor:** heca sets NO OS cursor today (`grep set_cursor` = none). Add a general cursor-policy helper
  in cursor-moved: `is_dragging()` → `CursorIcon::Grabbing`; pointer over a grabbable grip/source (via
  `MarkerGroup::hovered()` + `source_at`) → `Grab`; else default. heca-grid-ui stays cursor-free (emits a
  Scene); the app sets `window.set_cursor(...)`. Build it general (extensible to text I-beam/resize later).

## Gotchas (don't re-learn the hard way)
- `chrome_dispatch_click` **nulls `state.chrome_tree`** → capture any `source_at`/`resolve_at` result
  BEFORE calling `surface_click_action`. (Drop is fine: the tree rebuilds on the next render frame
  during the drag, so it exists at release.)
- `pos` in `on_mouse_input`/`chrome_dispatch_click` is **logical** window coords, same space as the
  retained tree's laid-out bounds (clicks already rely on this). `source_at`/`resolve_at` use the
  LAST-rendered bounds — valid because the tree was laid out last frame.
- Pane-id-as-`DragItemId` only works for panes (unique ids); columns need a build-time side-map.
- `mouse.rs` / `mouse/*` is WM-dev territory; `mouse-redo` worktree is the planned mouse REWRITE
  (focus/scroll/interactive-move per `redo-mouse.md`) — **separate** from F4.5; user confirmed it's not
  actively owned, so F4.5 here is safe, but don't conflate the two.
- `MarkerGroup` styling is config-driven (glow radius←`theme.glow_size` via `cx.rect`, strength←
  `theme.intensity`); keep new widgets the same — never hardcode (memory `grid-ui-font-configurable`).

## Key files
- `heca/src/chrome/mod.rs` — pane_card (`.draggable`), `sidebar_drag_source`/`sidebar_drop_target`,
  `chrome_dispatch_click` (nulls tree), `column_view` (MarkerGroup — step 2), `ChromeSignals`/`RetainedChrome`.
- `heca/src/mouse.rs` — `on_mouse_input` (drag-start), `process_edge_scroll`.
- `heca/src/mouse/drag.rs` — `on_cursor_moved`, `handle_sidebar_drag_starting/move`, `update_sidebar_drag_hover`.
- `heca/src/mouse/surface_left.rs` — `accept_drop`, `place_pane_at_sidebar_target`, `click_action`.
- `heca/src/mouse/release.rs` — `handle_sidebar_drag_release` → `target::surface_accept_drop` → `accept_drop`.
- `heca/src/app_state.rs:118` — `AppDragPayload` (enum-ify for step 2).
- `heca/src/app/render.rs` — chrome build/paint block (drag-overlay for 1b).
- `heca-grid-ui/src/drag/{resolve,context,state,item}.rs`, `builders.rs` (DragExt), `component.rs`
  (`drag_ghost`/`drop_indicator`, `as_drag_source`/`as_drop_target`).
