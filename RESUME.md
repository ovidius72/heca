# RESUME — heca handoff

> Resume doc for a cleared session. **PLAN.md** is the prioritized task list (single
> source of truth); **this** doc = what just happened + exactly where to pick up + the
> reference map. Detail rule: memory `handoffs-must-be-detailed`. Last updated 2026-06-17.

---

## 0. First steps on resume (do these before coding)

1. `git fetch origin` — **main moves fast** (other devs land terminal work). Branch new
   work off `origin/main`; never assume your last branch is current.
2. Read **`AGENTS.md`** (the two cardinal rules: UI = real `heca-grid-ui` widgets; behavior
   = registries, never hardcode) and the **memories** (loaded as `MEMORY.md` index).
3. **To find code, query graphify, not grep** (memory `prefer-graphify-for-codebase-search`):
   `graphify query "<question>"` against `graphify-out/graph.json`. (The graph can be stale —
   `/graphify --update` if needed. It won't have un-merged local work.)
4. **Don't code while the user is discussing/exploring** (memory `dont-code-during-discussion`).
   A question/observation is NOT a go-ahead. Wait for an explicit "go / do it / fix".
5. Before building ANY UI: **search `docs/widgets.md` + the showcase first**
   (memory `update-showcase-when-changing-grid-ui`). The widget/demo usually already exists.

---

## 1. Status — F4.5 sidebar DnD is DONE + MERGED

The whole F4.5 sidebar drag-and-drop arc shipped this session (and earlier):

| PR | What | Merged |
|----|------|--------|
| #116/#117 | pane DnD 1a + 1b (drag source/target, ghost, drop indicator) | yes |
| #119 | **step 2** — column DnD: move/swap, source-aware targeting, RPC, swap visual, docs+showcase | yes |
| #120 | **step 3** — grab cursor + swap-aware ghost + drag-hover fix | yes |
| #121 | (other dev) terminal pane shell — **split `render.rs` -> `app/terminal_render.rs`** | yes |

**What works now:** drag a sidebar **column** by its `MarkerGroup` left grip -> move (Before/After
thirds) or **swap** (Shift); within- and cross-workspace; drop on a workspace `DockFrame` -> move to
its end. Source-aware targeting (a column drag targets columns/workspaces, never the nested panes --
`drag::resolve_at_filtered`). Pane DnD likewise. Cursor: `Grab` over a draggable, `Grabbing` dragging.
Ghost chip gets a double-frame on swap. No misleading pane-hover during a drag.

**No work is in flight.** This handoff branch (`chore/resume-handoff`) only touches docs.

---

## 2. Decisions locked this session (don't re-litigate)

- **Scope C** for column DnD: columns draggable + drop targets; workspaces are **drop-target only**
  (drag-to-reorder workspaces was explicitly **deferred** -- see section 4).
- **Interaction** mirrors panes: drag = move+focus; **Shift = swap**; thirds = Before/Onto/After with
  **Onto->After** (insert below). Keyboard targeting is **verb-first** (modes `PaneSwap`/`PaneTake`+-focus
  already exist) -- thirds are mouse-only; keyboard/RPC need no thirds.
- **No OS "swap" cursor exists** -> move/swap distinction lives on the **on-target indicator**
  (insertion line vs `swap_indicator` double-frame) **and** the **drag ghost** (double-frame on swap),
  NOT the cursor. Cursor only says grabbable/grabbed.
- **`ColumnId` is NOT a safe DnD id** (assigned inconsistently; can equal a `PaneId`) -> a build-time
  **side-map** (`ChromeDragItem` + `DragItemRegistry` on `RetainedChrome`) decides kind. `AppDragPayload`
  is an enum (`Pane | Column`).
- **Source-aware drop resolution**: drops resolve by an **explicit `DragSourceKind`** passed by the
  caller -- NOT from the live drag payload (the phase is wiped to `Idle` before the release handler runs).
- **Onto/middle-third semantics** for columns = "insert below" (left as-is; user OK'd current feel --
  revisit only if it annoys).
- Workflow rules captured as memories: graphify-first search; don't-code-during-discussion;
  update-showcase-when-changing-grid-ui; never-push/merge-without-explicit-OK.

---

## 3. Files this session touched (all merged via #119/#120)

- **`heca/src/chrome/mod.rs`** -- `ChromeDragItem`/`DragItemRegistry`; `column_view` builds each column as
  a `MarkerGroup` `.draggable(id).drop_target(id)` (id from the registry); workspace `DockFrame`
  `.drop_target`; `DragSourceKind` + `target_accepted_by` + `resolve_sidebar_drop` + `sidebar_drop_target`
  (source-aware); `sidebar_drag_source`; `paint_drag_overlay` (indicator + ghost, swap-aware);
  `chrome_dispatch_move` (hover dispatch).
- **`heca/src/app_state.rs`** -- `AppDragPayload` enum; `current_cursor` field.
- **`heca/src/mouse.rs`** -- drag-start builds Pane/Column payload; `update_cursor` (Grab/Grabbing policy).
- **`heca/src/mouse/{drag,release,surface_left,interactive}.rs`** -- payload enum match arms; column drop
  in `release.rs::handle_sidebar_column_drag_release` (+ `column_move_dst_idx` pure helper + tests);
  `accept_drop` source-aware.
- **`heca/src/{input,handlers,rpc}.rs` + `heca/src/app/{registry,mutations,interaction}.rs`** --
  `WmAction::MoveColumn`/`SwapColumns` full wiring + RPC `move-column`/`swap-columns`.
- **`heca-core/src/layout/scrolling.rs`** -- `reorder_column` / `swap_columns` (+ `animate_columns_from`).
- **`heca-grid-ui/src/drag/resolve.rs`** -- `resolve_at_filtered`; **`component.rs`** -- `swap_indicator`,
  `drag_ghost(rect,text,swap)`; **`drag/mod.rs`** export.
- **`heca/src/app/events.rs`** -- `PointerMoved`->chrome tree (gated `!is_dragging()`); `update_cursor` calls.
- **`docs/widgets.md`** + **`heca-renderer/examples/showcase.rs`** -- DnD section, `swap_indicator`,
  `resolve_at_filtered`, MarkerGroup DnD demo + IndicatorSwatch (move vs swap).

> WARNING: `render.rs` was **re-split by #121** into `heca/src/app/terminal_render.rs` (PaneRenderState,
> `paint_terminal_pane_shell`, `render_terminal_mount`, `selection_overlay_for_pane`, the geometry
> helpers). Re-read `render.rs` + `terminal_render.rs` before touching frame rendering.

---

## 4. Next tasks (pick from PLAN.md; nearest-in first)

### F4.5 leftovers (small, optional polish)
1. **Grip widening** -- the column drag handle is the `MarkerGroup` left gutter, **`GRIP_W = 12.0`** in
   `heca-grid-ui/src/widgets/marker_group.rs`. User finds 12px fiddly. *How:* widen the invisible grab
   zone (bump `GRIP_W`, or add a separate hit-pad) while keeping the visible bar thin; verify `in_grip`
   + `style.padding.left` (children inset) stay consistent + the test `reserves_left_grip_gutter`.
2. **Workspace drag-to-reorder** (deferred feature) -- make the workspace `DockFrame` a **drag source**
   (`chrome/mod.rs::build_workspaces_container`): register `ChromeDragItem::Workspace{ws}` as draggable,
   add a `Workspace{ws,swap}` variant to `AppDragPayload`, a `MoveWorkspace`/reorder `WmAction` + handler
   (+ `Session`/workspace reorder primitive -- none exists; workspaces are the vertical `Vec<Workspace>`)
   + RPC. Mirror the column slices. Workspaces are a vertical discrete list.
3. **Onto semantics** -- if the middle-third "insert below" feels wrong, change in
   `release.rs::handle_sidebar_column_drag_release` (the `before = side == DropSide::Before` line) -- e.g.
   middle = swap, or nearest-edge.

### Then per PLAN.md priority sequence
- **P3 — Pane numbering** (memory `heca-pane-numbering-spec`): `prefix+<ws>+<pane>` deterministic
  addressing + per-ws sidebar numbers; `prefix+q` peek stays.
- **P4 — Appearance & sizing** (memory `appearance-blur-zoom-font`): app-wide zoom like the showcase;
  font-size in/dec for BOTH app chrome and terminal; finish in-app blur.
- **P0 — SharedChromeState** (the pluggable-chrome foundation; PLAN.md P0 has the locked design).
- **`render.rs` split** -- now **partly done** by #121 (terminal extracted). Re-assess the Foundation-gap
  item against the new `terminal_render.rs` before doing more.

---

## 5. Where everything lives (reference map)

| Need | Look in |
|------|---------|
| Cardinal rules, action/keymap system, conventions, "adding new actions" checklist | **`AGENTS.md`** |
| Prioritized task list + locked designs (P0-P4, backlog, foundation gaps) | **`PLAN.md`** |
| Every `heca-grid-ui` widget + `PaintCx` API + DnD framework + patterns | **`docs/widgets.md`** |
| Live widget/chrome demos (run it) | `cargo run -p heca-renderer --example showcase` (`heca-renderer/examples/showcase.rs`) |
| Run the real app to eyeball DnD/cursor/visuals | `cargo run -p heca` |
| Long-term chrome/plugin architecture | `pluggable-chrome-plugin-plan.md` |
| DnD framework design rationale | `dnd-framework-refactor-plan.md` |
| NIRI layout model | `.agents/skills/niri/SKILL.md`, `docs/niri-wiki/` |
| Codebase search (cheap) | `graphify query "..."` (`graphify-out/`) |
| Persistent rules/preferences (per-session) | `MEMORY.md` index + `memory/*.md` |

**Build/verify:** `cargo build -p heca` -- `cargo test --workspace` -- `cargo clippy --workspace
--all-targets --all-features` (must stay clean; fix even pre-existing warnings). **Don't run `cargo fmt`**
(memory `grid-ui-no-cargo-fmt`). Pre-commit: load `~/.agents/skills/rust/SKILL.md` review -> clippy -> commit.

---

## 6. Gotchas (learned the hard way)

- Drop resolution: pass `DragSourceKind` explicitly; the live payload is `Idle` by release time.
- `chrome_dispatch_click` nulls `chrome_tree`; `chrome_dispatch_move` must NOT. Hover dispatch is gated
  off during a drag (else pane rows highlight as droppable).
- Branch per task + PR per task; **never push/merge without explicit, action-specific OK**
  (memory `never-merge-or-push-without-explicit-ok`). Opening a PR != permission to merge.
- Sidebar collapsed rail is still legacy hand-drawn (`render.rs` + `sidebar_hit_test`) -- only the
  EXPANDED sidebar is the grid-ui retained tree.
