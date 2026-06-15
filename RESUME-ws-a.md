# RESUME — WS-A chrome integration (sidebar → grid-ui)

> Self-contained handoff for **any** agent. Last updated 2026-06-15.
> **Branch:** `grid-ui-chrome-integration` · **HEAD:** `ba8049b` · build green (0 errors), `cargo test -p heca` = 136 passed, warning-clean (only a transitive `block v0.1.6` future-incompat note).
> **Working tree:** `AGENTS.md` is **modified but uncommitted** (this session's widget rules); `grid-ui-review-1/2.md` are pre-existing untracked review docs (ignore).

---

## 0. READ FIRST (rules that constrain everything below)

These were established (some painfully) this session. Violating them = redo.

1. **New UI = a proper, GENERIC, theme-driven `heca-grid-ui` widget.** Never build visual/interactive elements as ad-hoc inline `Flex`/`Surface` composition in the app (`heca/src/chrome.rs`) with hardcoded sizes/colors/alphas. Full rule: **AGENTS.md → "Creating new widgets / components (MANDATORY)"**. Memory: `heca-widgets-in-grid-ui`.
   - Must embed `Base` + impl `Component` (+ builder traits) → inherits visible/disabled/focused/bounds/tab_index/tick.
   - Must read ALL styling from `Theme` via `cx.theme()` — colors, font, border width, radius, glow, transparency. Hardcode NOTHING (respects `config.toml`, theme reload, `[appearance]` transparency).
   - Must be **domain-neutral**: NEVER name/couple a widget to workspace/column/pane (nor docker/agent/git). The chrome regions host *generic containers* (Workspaces today; Docker, AI agents, git, notes, plugins later — see `pluggable-chrome-plugin-plan.md`). e.g. not `ColumnGroup` → a generic `MarkerGroup`/`RailGroup`.
2. **Foundation before style.** Build shared state + data model + feature plan before styling content. (Memory: `foundation-state-before-style`.)
3. **`KeyHint` is universal** — a leader/vimium overlay for ANY clickable widget, never column/ws/pane-specific. (Memory: `grid-ui-keyhint-universal`.)
4. **The showcase is the reference.** `heca-renderer/examples/showcase.rs` (`cargo run -p heca-renderer --example showcase`) exercises every widget + chrome recipes (EXPLORER DockFrame→ItemGroup, PANES cards, ChromeRegion sidebar, RailCell/KeyHint rail). Look at it before using/building widgets.
5. Don't run `cargo fmt`. Fix all warnings. Don't reference the dead `heca-ui` crate (it's `heca-grid-ui`).
6. `transparency`/`vibrancy` are window-creation-time → need a full app **restart** to see (not theme reload). Config: `~/.config/heca/config.toml [appearance]`.

---

## 1. What this workstream is

Replace the hand-drawn left sidebar with **grid-ui** chrome, on the way to the pluggable-chrome architecture. Per `pluggable-chrome-plugin-plan.md` §2.1: the sidebar is a **content-agnostic SHELL**; the workspace tree is a **WorkspacesContainer** mounted inside it (one of many future containers). Foundation-first order: transparency/vibrancy (done) → retained tree + shared state (F4, in progress) → containers/features.

Key design docs: **`F4-chrome-state-design.md`** (the retained-tree + shared-state design), `pluggable-chrome-plugin-plan.md` (the big picture), `grid-ui-integration-and-appearance-plan.md` (F1–F5 plan), `heca-sidebar-design-spec` memory.

---

## 2. DONE this session (committed)

- `4137786` **F5** — sidebar SHELL (full-height frosted bracket `Pane`) + WorkspacesContainer mounted inside: `chrome.rs::build_sidebar_shell` + `build_workspaces_container`. Each workspace = `.frameless()` `DockFrame` (header count `Badge`); columns = compact **left marker bars** + pane cards (no "Col N" header rows); panes = state-tinted `Row` cards. `render.rs`: grid chrome painted LAST (over panes); pane content clipped to `pane_area` so it doesn't bleed under chrome; main canvas left transparent (frosted via vibrancy). Added `.frameless()` to `DockFrame` in heca-grid-ui. Removed orphaned hand-drawn expanded sidebar + dead model items (warning-clean).
- `1c398a1` **F4.1** — retained chrome tree: built once into `AppState.chrome_tree` (`chrome::RetainedChrome { root: Flex, sig }`), rebuilt only when `chrome::chrome_signature` changes; re-laid-out + painted each frame. Stops per-frame signal churn; gives a live tree for event dispatch. `chrome.rs` split: `build_chrome_root`/`chrome_root` (build) vs `paint_chrome_root` (paint) + `chrome_gui_theme`/`chrome_colors`/`chrome_status` + `chrome_signature`.
- `7bad009` **F4.2** — sidebar pane-click selection: click dispatches `PointerPressed` into the retained tree (real bounds) → the `Row`'s `on_activate` records its `pane_id` in `AppState.chrome_sinks.click` → `chrome::chrome_dispatch_click` returns it → `surface_left.rs::click_action` focuses it. Replaced the stale fixed-row hit-test for expanded-sidebar pane selection. **Verified working by user.**
- `45143b5` — **temporarily DISABLED sidebar DnD** (drag source/hover/drop still use stale `sidebar_hit_test` geometry → mis-targets). In `mouse.rs` press handler, a pane press is now a plain click (no drag start). Reversible: restore the drag-start block + the 3 dropped `heca_grid_ui::drag` imports (DragItemKind/DragItemId/DEFAULT_DRAG_THRESHOLD_SQ).
- `66e248b` **F4.3** — workspace collapse-on-click: `DockFrame.on_toggle` → `chrome_sinks.ws_toggle` → `chrome_dispatch_click` returns `ChromeClick::WorkspaceToggle(ws_idx)` → `click_action` flips `SidebarTree::toggle_workspace_collapsed`. Sinks bundled into `chrome::ChromeSinks` (grows for F4.4/F4.5).
- `ba8049b` — active-workspace styling: light accent wash (`theme.accent.with_alpha(28)`) over the active workspace dock. **NOTE: this is exactly the kind of hardcoded inline styling Rule #1 forbids — fold it into the proper widget during F4.4.**

**Uncommitted:** `AGENTS.md` — added the "Creating new widgets" mandatory rule (generic + theme-driven + Base) and showcase references. Commit suggestion: `docs(AGENTS): require new UI be generic, theme-driven heca-grid-ui widgets; point to showcase`.

---

## 3. NEXT — in order (all need FRESH context; each is real work)

### F4.4 — generic marker/rail group widget + targeting  ← **start here**
- **Build a GENERIC widget in `heca-grid-ui`** (e.g. `MarkerGroup`/`RailGroup`): a group of child rows + a left **marker/rail bar** + a `KeyHint` target slot + a drag-handle seam. Theme-driven (bar active/inactive color, width, radius from `Theme` tokens — add tokens if missing). Embed `Base`, impl `Component` + builder traits. Export in `widgets/mod.rs`, add to AGENTS.md catalog + `docs/widgets.md` + unit tests. Model it on existing widgets + the showcase.
- **Migrate** `chrome.rs`'s inline `column_view` (and the hardcoded `pane_card` alphas/padding, and the `ba8049b` active-ws wash) onto theme-driven widgets. After this, `chrome.rs` only *composes* widgets + projects `SidebarTree` state.
- Wire **move/swap/take targeting**: pick mode lights up `KeyHint` letters on panes/columns; app feeds candidates from `collect_all_pane_candidates`.

### Pane numbering feature (agreed, spec'd — memory `heca-pane-numbering-spec`)
- **`prefix+<ws 1-9>+<pane 1-9>`** → focus that pane (deterministic, cross-ws, mode chord; bare `prefix+<digit>` is free — ws-switch is `prefix+w`+digit). Show **per-workspace pane numbers** on cards (visual order). `prefix+q` peek stays as the universal "reach any pane" path. Touches: card display (`chrome.rs`), new `WmAction` (input.rs/actions.rs), default keybindings, handler (ws+index→pane_id). Follow AGENTS.md "Adding New Actions" 11-step checklist.

### F4.5 — re-enable DnD on tree geometry
- Migrate drag source/hover/drop off `sidebar_hit_test` onto the retained-tree geometry; the **generic marker bar (F4.4) becomes the drop target / drag handle**. Restore the drag-start block disabled in `45143b5`.

### Also pending (lower priority)
- Collapsed sidebar rail still uses legacy hand-drawn render + `sidebar_hit_test` (only EXPANDED is grid-ui). Sidebar workspace/column/`+w/+c/+p` button clicks are NOT wired (button_hitboxes now unused). NSWindow vibrancy warning (benign; memory `heca-nswindow-vibrancy-warning`). F3 in-app blur (wired but inert).

---

## 4. Key files

- `heca/src/chrome.rs` — chrome scene build/paint, retained tree, `ChromeSinks`, `chrome_dispatch_click`, sidebar shell + `build_workspaces_container` + `column_view` + `pane_card` (INLINE — to be migrated to a widget).
- `heca/src/app/render.rs` — `render_frame`: chrome rendered last; pane clipping; retained-tree rebuild-on-signature.
- `heca/src/app_state.rs` — `chrome_tree: Option<RetainedChrome>`, `chrome_sinks: ChromeSinks`.
- `heca/src/app/startup.rs` — inits both (`chrome_tree: None`, `chrome_sinks: ChromeSinks::new()`).
- `heca/src/mouse.rs` (press handler, DnD disabled) + `heca/src/mouse/surface_left.rs` (`click_action` uses `chrome_dispatch_click`).
- `heca/src/sidebar/{model,render,hit_test}.rs` — SidebarTree (canonical sidebar state) + legacy collapsed render + the stale `sidebar_hit_test` (still used by collapsed rail + drag).
- `heca-grid-ui/src/widgets/` — generic widgets; `dock_frame.rs` has the `.frameless()` added this session.
- `heca-renderer/examples/showcase.rs` — the reference showcase.

## 5. Memory files written this session (auto-loaded; for context)
`heca-widgets-in-grid-ui` (generic theme-driven widget rule), `foundation-state-before-style`, `grid-ui-keyhint-universal`, `heca-pane-numbering-spec`, `heca-nswindow-vibrancy-warning`. Plus prior: `chrome-integration-realignment`, `heca-transparency-architecture`, `heca-sidebar-design-spec`.

## 6. Workflow
Small slices, commit per verified slice, user verifies in-app before commit (rebuild + restart). Don't `cargo fmt`. `cargo build -p heca` + `cargo test -p heca` after each slice; fix all warnings. The `/grill-me` skill before a new phase.
