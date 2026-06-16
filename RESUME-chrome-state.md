# RESUME — chrome SharedChromeState migration

> Self-contained handoff. Last updated 2026-06-16. **Single source of truth for tasks/priorities is `PLAN.md`** — read it. This doc = precise current state + exact next steps + gotchas for the in-flight SharedChromeState work.
>
> **Branch:** `grid-ui-chrome-integration` (== `origin/main`; PR #108 merged). **Build:** `cargo build --workspace` clean; `cargo test -p heca` = 168 passed; warning-clean except pre-existing terminal-WIP notes (`SelectionPhase`/`SelectionRenderMode`/`render_mode`/… from `app/selection_model.rs`, **not ours**) + transitive `block v0.1.6`.

---

## 0. READ-FIRST rules (violating = redo; I kept breaking these)

1. **Chrome is container-namespaced EVERYWHERE** (memory `chrome-is-container-namespaced-everywhere`). Sidebar = content-agnostic **shell** hosting **movable containers**. Per-container UI state (collapse, selection, search, **targeting**, content-scroll, DnD) lives in the **container's** state, NEVER flat on global/shell `SharedChromeState`. Applies to state + widgets + actions. Scroll has two levels: **container content-scroll** (too many ws/cols/panes → container) vs **shell dock-list scroll** (too many docks → shell).
2. **New UI = generic, theme-driven `heca-grid-ui` widget** (Base + Component, all styling from Theme; domain-neutral). Never inline Flex/Surface in the app. Memory `heca-widgets-in-grid-ui`.
3. **Never merge/push/force-push without an explicit, action-specific OK** (memory `never-merge-or-push-without-explicit-ok`). Open PRs when asked; the user merges.
4. **Verify end-to-end before calling anything "reusable/done"** — trace the full consumer path; green ≠ consumable (memory `verify-end-to-end-before-claiming-reusable`).
5. No `cargo fmt`. Fix warnings. Read `AGENTS.md` top "⛔ STOP" section. Showcase is the reference: `cargo run -p heca-renderer --example showcase`.

---

## 1. What's DONE (merged to main)

**Generic DnD framework** (`heca-grid-ui/src/drag/`): domain-neutral `DragContext<P>`, universal `DragExt` (`.draggable`/`.drop_target`), `resolve_at`/`source_at` (tree-geometry), `PaintCx::drag_ghost`/`drop_indicator`, generic theme tokens. Docs in `docs/widgets.md`.

**Reusable frosted-backdrop path** (renderer): `blur::Blur` (separable Gaussian over any texture) + `backdrop::Backdrop` (draw a texture region into a rect). Path: `Compositor.scene_view()` → `Blur::process(view, radius_px)` → `Backdrop::draw(target, blurred, viewport_px, dst_rect, src_uv, opacity)` → draw translucent content over it. **Units gotcha:** blur/backdrop are PHYSICAL px; `appearance.blur_radius()` is logical → multiply by `scale_factor`. Terminal agents reuse this. **App-side wiring NOT done** (no `Blur` field on AppState, no call in `render_frame`).

**SharedChromeState** (`heca/src/chrome/state.rs`) — signal-backed, split shell vs container:
- `SharedChromeState` = container-agnostic shell: `left`/`right` RegionState (mode+size) + `pub workspaces: WorkspacesContainerState`. Region accessors: `left_visible()`/`left_size()`/`set_left_mode()`/… (RegionMode visible=Expanded, hidden=Hidden).
- `WorkspacesContainerState` (`chrome_state.workspaces`) = per-container: `collapsed_ws` (canonical), `selection` (active/hovered pane), `pick_candidates` (Vec, empty=inactive), content `scroll`. Write via `set_*`/`toggle_*`; read via accessors (`.with()` for collections). Fields `pub(crate)`; not `Copy` (Clone = shallow alias); `!Send+!Sync`.
- Constructed in `AppState` at startup (reactive runtime works there). `#![expect(dead_code)]` in state.rs covers not-yet-consumed API (selection/targeting/scroll) — **self-cleaning: remove when all consumed**.
- Reviewed by rust-skills + Apollo (Copy removed, pub(crate)+accessors, `.with()`, `!Send+!Sync`, `DEFAULT_SIDEBAR_WIDTH`, `#[expect]`, 8 tests).

**Consumer migrations done (per-slice commits):**
- **Region vis/width** → `chrome_state` region accessors; **deleted `SidebarState`** struct. ~29 read sites + handler writes (`handle_sidebar_left/right/focus` via `region_mode()` helper in `handlers.rs`).
- **Per-workspace collapse** → `chrome_state.workspaces.collapsed_ws` canonical; `SidebarTree.WsEntry.collapsed` is a **one-way mirror** refreshed by `SidebarTree::apply_ws_collapsed(set, changed_ws)` (preserves cursor-to-parent). Writes via `handlers::apply_ws_collapse(state, ws_idx, Option<bool>)` (toggle/set) — called by the 3 ws handlers + the sidebar click dispatch (`mouse/surface_left.rs` `WorkspaceToggle` arm). Keyboard `toggle_expand`/`expand`/`collapse` take `&WorkspacesContainerState` and route the ws case through it (columns stay in the tree). `sync_from_session` no longer self-preserves ws collapse; `app/focus.rs::sync_focus` re-applies it from `chrome_state.workspaces` after each sync (so it persists across rebuilds). **User-visible — verify in-app: click a workspace header to collapse, and sidebar-nav keys.**

---

## 2. NEXT — in order (consumer migration continues)

### A. Migrate selection / targeting / scroll consumers onto `chrome_state.workspaces`
These are now **homed** in `WorkspacesContainerState` but **not yet read** (hence the `#[expect(dead_code)]`). They MIRROR canonical state, so each needs a small design call (does the canonical move, or does chrome cache it?):
- **selection.active_pane** mirrors `state.focused_pane` (canonical WM focus). Decide: chrome reads `chrome_state.workspaces.active_pane()` set on focus change, OR keep deriving from `focused_pane`. **hovered_pane** is net-new (set on sidebar pointer hover; today hover is ad-hoc).
- **pick_candidates** mirrors `input_mode` candidates (the mode enum is canonical). Wire the chrome KeyHint overlay to read `chrome_state.workspaces.pick_candidates` (F4.4 targeting) — but that's also F4.4 widget work.
- **scroll**: net-new — the sidebar/WorkspacesContainer content scroll isn't stored anywhere yet. Wire when the container gets a scroll region (needs renderer clip, which **is implemented**: `PushClip`/`PopClip` in `heca-renderer/src/scene.rs`).

### B. Read-via-signals widget binding (the deep part, deferred so far)
So far chrome reads `chrome_state` at build time + rebuild-on-signature (F4.1). The locked decision is **fine-grained signals**: bind the retained chrome widgets to the `Signal`s (`.get()` in paint/layout → subscribe; writes mark dirty → coalesced redraw), reconciled with the `needs_paint`/`collect_damage` damage path. Do this when wiring the above, or as its own slice. When all consumed, **remove `#![expect(dead_code)]`** from `state.rs` (it will error if left).

### C. Remove the `ChromeSinks` stopgap
`chrome_state` is now the state owner, but the **dispatch mechanism** (`ChromeSinks` Rc<Cell> → `chrome_dispatch_click` → `ChromeClick`) is still used for click/ws-toggle routing in `chrome/mod.rs` + `mouse/surface_left.rs`. Evaluate replacing with widget closures writing `chrome_state` directly (or keep — it's an event-routing concern, orthogonal to state). Not urgent.

### Then (PLAN.md order): F4.4 generic MarkerGroup/rail widget + targeting → F4.5 ≡ DnD Phase 3 (re-enable sidebar DnD on the framework; restore drag-start disabled in `45143b5`) → pane numbering → blur app-wiring → grid-ui backlog.

---

## 3. Key files
- `heca/src/chrome/state.rs` — `SharedChromeState` + `WorkspacesContainerState` (THE store). `chrome/mod.rs` re-exports both; also has `ChromeSinks`/`ChromeClick`/`chrome_dispatch_click`/`build_chrome_root`/`chrome_signature` + `DEFAULT_SIDEBAR_WIDTH`.
- `heca/src/app_state.rs` — `chrome_state: SharedChromeState` field. `heca/src/app/startup.rs` — constructs it.
- `heca/src/sidebar/model.rs` — `SidebarTree`, `apply_ws_collapsed`, `toggle_expand/expand/collapse(&WorkspacesContainerState)`, `sync_from_session` (no ws-collapse preserve).
- `heca/src/handlers.rs` — `apply_ws_collapse` helper + `region_mode` helper + sidebar handlers.
- `heca/src/app/focus.rs::sync_focus` — re-applies collapse after sync.
- `heca/src/mouse/surface_left.rs` — sidebar click dispatch (`WorkspaceToggle` arm).
- `heca-renderer/src/{blur,backdrop,composite}.rs` — frosted-backdrop primitives.

## 4. Gotchas
- `region_mode(visible)`: visible→Expanded, else Hidden. The width<`SIDEBAR_EXPANDED_THRESHOLD` "rail" check stays width-based (`left_size()`), `RegionMode::CollapsedRail` unused for now.
- `chrome_state` is `!Send+!Sync` + signals need the UI-thread reactive runtime (fine in AppState).
- `apply_ws_collapse` clones the small collapsed set (cold path) to avoid borrow conflicts — fine.
- Don't reintroduce workspace/pane concepts into `SharedChromeState` (shell) — they go in `WorkspacesContainerState`.
- Other devs' terminal work touches `app/selection_model.rs`, `terminal_host.rs`, `render.rs` — coordinate; the region slice already touched `terminal_host.rs` (2 read swaps).

## 5. Memories written this effort (auto-loaded)
`chrome-is-container-namespaced-everywhere` (cardinal), `never-merge-or-push-without-explicit-ok`, `verify-end-to-end-before-claiming-reusable`, `appearance-blur-zoom-font`, `consolidate-planning-docs`. Plus prior chrome/widget/foundation memories.
