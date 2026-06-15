# F4 — Shared Chrome State + Retained Chrome Tree

**Status:** design (pre-implementation) · 2026-06-15
**Why now:** F5 (sidebar visuals) is done but display-only; clicks mis-select because there's no event handling — the foundation must come before more features. See memory `foundation-state-before-style`, `chrome-integration-realignment`, `pluggable-chrome-plugin-plan.md` Phase 2.

## 1. The problem

- **Immediate-mode rebuild.** `chrome::build_chrome_scene` rebuilds a fresh grid-ui `Scene` **every frame**. There is no persistent widget tree, so nothing receives events. Clicks fall back to the **legacy fixed-geometry** `sidebar/hit_test.rs` (`y / ITEM_HEIGHT` over `flat_items`), whose geometry no longer matches the rendered grid layout → wrong / flaky selection.
- **Scattered UI state.** Chrome/UI state lives in several places: `SidebarTree` (cursor, collapsed), `AppState.sidebar` (visibility, width), `mouse.drag_ctx` (drag), `input_mode` (move/swap/take candidate letters). No single shared layer the chrome reads/writes.

## 2. Target architecture (mirror the showcase)

The showcase builds its UI **once**, re-lays-out each frame, and **dispatches events into the retained tree**. Chrome adopts the same model:

1. **Retained chrome tree.** Build the chrome widget tree once, store it in `AppState`, re-layout + paint each frame. Rebuild only on **structural change** (workspaces/columns/panes added/removed); value changes (names, counts, active, collapse) flow through **signals**.
2. **Shared chrome/UI state** — new `heca/src/chrome/state.rs`, the canonical UI state the tree binds to and actions write:
   - region visibility + mode (expanded / collapsed-rail / hidden), widths
   - selection: active/selected pane id, hovered id
   - per-workspace collapse set (columns no longer collapse)
   - drag state (reuse the shipped `drag/` framework)
   - targeting: active pick mode + candidate letters → drive `KeyHint` signals (universal — see `grid-ui-keyhint-universal`)
   - scroll offset
   Canonical *session* state stays in `Session`; this layer holds **derived UI state** + ids.
3. **Event flow.** App forwards mouse (move/press) + relevant keys into the chrome tree's dispatch; widgets (`Row`/`Item`/`DockFrame`/`KeyHint`) hit-test against their **laid-out bounds** → correct selection; on activate they fire **actions** that mutate session/shared state (read-via-signals / write-via-actions).
4. **Delete** the legacy fixed-geometry sidebar hit-test (the click-mismatch source) once dispatch replaces it.

## 3. Phased slices (each independently shippable)

- **F4.1 — Retain + state scaffold.** Add `chrome/state.rs` `SharedChromeState` (signals) and convert chrome from rebuild-every-frame to **build-once + relayout/paint** with the *current* content. No behavior change — de-risks the immediate-mode→retained shift. Rebuild on a structural-signature change (ws/col/pane id set).
- **F4.2 — Click selection.** Dispatch mouse events into the chrome tree; pane-card click focuses/selects that pane. Delete the legacy selection hit-test. (Fixes the reported click bug.)
- **F4.3 — Collapse.** Workspace collapse toggle via `DockFrame` header → action → shared state.
- **F4.4 — Targeting.** Move/swap/take pick mode feeds `KeyHint` signals; extract the column **marker bar into a `KeyHint`-aware widget** (the `column_view` → widget step).
- **F4.5 — Drag.** Pane DnD via the `drag/` framework; column bar as drag handle / drop target.
- **F4.6 — Region toggles** through shared state.

## 4. Open decisions
- **Structural-change detection:** start simple — rebuild the tree when a structural signature (ordered ws/col/pane ids) changes; values reactive via signals.
- **State ownership split:** session stays canonical; `chrome/state.rs` holds derived UI state only.
- **Keybindings:** existing sidebar-mode + global bindings keep working — they dispatch the same actions the widgets do.
