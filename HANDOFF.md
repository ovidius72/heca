# HANDOFF — sidebar rail drop, shell unification, menu-nav (2026-07-11)

> Supersedes and replaces the deleted `HANDOFF-context-menu.md` + `HANDOFF-pane-rename-naming.md`
> (their open items are in BACKLOG: `context-menu`, `menu-nav` (now DONE), `pane-naming`).
> **Read first:** `AGENTS.md` (⛔ STOP rules), `docs/sidebar-provider-modes.md` (the sidebar decision
> record), `docs/widgets.md`, `docs/surface-compositor.md`.

---

## 0. Cardinal rules (obeyed this session — keep obeying)
- **Never hardcode** style/colors/sizes/keys — all from `Theme`, a widget **variant**, or the
  action/keymap registries. No magic numbers.
- **Reuse/extend existing widgets**; never hand-draw in a consumer's `paint`.
- Every setting → `config.default.toml`; every keybound action → `keybindings.default.toml`.
- Every new `WmAction` classified in `action_policy()` (`heca/src/app/interaction.rs`).
- Docs in **both** rustdoc AND `docs/widgets.md` for any widget/UI-model change.
- **Don't run `cargo fmt`**; verify with `cargo clippy --workspace --all-targets --all-features`.
- **Communication: plain simple English, no jargon/buzzwords.**
- Left and right sidebars are **one component, two instances** — never two implementations.

---

## 1. Branch / PR state
All work this session is **merged to `main`** (HEAD `01f9a9c`). Three PRs, each off `main`:
- **PR #230** (merged) — `reload-bugs`.
- **PR #231** (merged) — drop collapsed sidebar rail + unify sidebar shell.
- **PR #232** (merged) — `menu-nav`.
Remote: `git@github.com:ovidius72/heca.git` (gh authed as `ovidius72`).

---

## 2. DONE — `reload-bugs` (PR #230)
- **reload-bug-header-icons — FIXED.** Pane-header action icons stayed faint/stale on existing panes
  after a theme reload. Retained headers (`state.pane_headers`) bake theme colors at build time, and
  `pane_header_key` has no theme identity, and `reload_config` never cleared them. Fix: new
  `chrome::clear_pane_headers` (drops each header's `HintTargetRegistry` range, then clears) called
  from `reload_config` (`heca/src/main.rs`); also de-dups the identical inline cleanup in
  `sync_pane_headers` (`heca/src/chrome/mod.rs`).
- **reload-bug-terminal-transparency — NON-BUG.** The handoff hypothesis ("render key lacks
  `surface_alpha`") was false: `terminal_layer_render_key` already hashes it and `style_changed` forces
  a full repaint. User-verified no longer reproduces (predated the merged terminal-theming-2 fix).

---

## 3. DONE — drop collapsed rail + unify sidebar shell (PR #231)
**Decision record: `docs/sidebar-provider-modes.md` (authoritative). Read it.**

### 3.1 Dropped the collapsed sidebar rail (`app-task-21`)
The collapsed icon rail was hand-drawn (`render_sidebar_collapsed`) with a **separate** magic-number
`sidebar_hit_test` that re-derived the rail geometry — the two drifted, causing
`chrome-bug-collapsed-sidebar-picks` (clicks one row off). Rather than rebuild it, we **dropped** it: a
region is now **Expanded ⇄ Hidden**.
- **Collapse decided by `RegionMode` state, not width.** `build_chrome_root` + `mouse/surface_left.rs`
  branch on visibility (`width > 0`). Deleted `SIDEBAR_EXPANDED_THRESHOLD` (80) +
  `DEFAULT_COLLAPSED_SIDEBAR_WIDTH` (40) (`heca/src/chrome/mod.rs`).
- **Hidden reclaims all space** — `left/right_sidebar_width()` return 0 when hidden
  (`heca/src/app_state.rs`), fixing the 40px-strip quirk.
- **Deleted:** `heca/src/sidebar/render.rs` (whole file), the collapsed branch of `sidebar_hit_test`
  (+ its unused `sidebar_width` param — the off-by-one source), `collapsed_pane_label`,
  `cursor_up/down_collapsed` (`sidebar/model.rs`), the rail draw blocks in `heca/src/app/render.rs`,
  and the dead `sidebar_hovered_btn_idx`.
- Entering `SidebarNav` while Hidden now **expands** (`handle_sidebar_focus`, `heca/src/handlers.rs`).
- `RegionMode::CollapsedRail` and `RailCell` stay as **unused library pieces** for a future rail.

### 3.2 Unified left/right sidebar into ONE shell (`heca/src/chrome/mod.rs`)
Was two builders (`build_sidebar_shell` + `build_right_sidebar_shell`). Now **one**
`build_sidebar_shell(region_w, .., content: Option<Flex>)`: left mounts the `WorkspacesContainer`,
right passes `None`. `build_chrome_root` builds both sides from it. Framing / mode / rail-drop are now
shared by construction.

### 3.3 Design captured for the FUTURE (not built)
`docs/sidebar-provider-modes.md` §3–§4 records the **generic Provider render-per-mode** model
(write-once: a Provider describes a semantic Group/Item tree once; the host renders expanded rows OR a
collapsed icon rail per region mode) and the full collapsed icon-rail visual spec (`RailCell` per
item, `workspace_collapsed_style`/`pane_collapsed_style` config, status→color). **Build only when a
Provider (Docker / AI-Agents) needs an always-visible status rail** — as the generic host path, never
a workspace-only hand-drawn rail.

---

## 4. DONE — `menu-nav` (PR #232)
One configurable key set for **overlay list/menu navigation** (context menu + command palette). Sidebar
nav is OUT of scope (keeps its own `[keys.mode] sidebar`).
- **`heca-grid-ui`:** new semantic `Event::MenuNav(MenuNav{Prev,Next,Activate,Dismiss})`
  (`src/component.rs`, exported in `src/lib.rs`). `ContextMenu` + `CommandPalette` `event()` are now
  **intent-only for nav** — hardcoded arrow/`Ctrl+j`/`k`/Enter/Esc removed. Quick-pick letters (menu)
  and typing (palette filter) stay raw `Event::Key`.
- **App:** `[keys] menu_up`/`menu_down`/`menu_activate`/`menu_dismiss` (defaults ↑/`Ctrl+k`,
  ↓/`Ctrl+j`, Enter, Esc) in `keybindings.default.toml`. `build_menu_keymap` (`heca/src/app/registry.rs`)
  resolves them to `MenuNav` in a dedicated **`AppState.menu_keymap`** (rebuilt on reload in
  `main.rs`), consumed **only** in the overlay key branch (`heca/src/app/events.rs`). Kept out of the
  main keymap so these keys never hijack normal-mode input.
- **Dialog fallback (regression fix, same PR):** the overlay branch first tries `MenuNav`; if the
  overlay returns `Handled::No` (a `Dialog`/`Modal` doesn't understand `MenuNav`), it **falls back to
  the raw key** — so the rename dialog's **Esc = cancel / Enter = submit** still work. menu-nav only
  drives list menus.
- Docs: README (`menu-nav` section), `docs/widgets.md` (ContextMenu/CommandPalette).

---

## 5. Decisions taken this session (don't relitigate)
- **Drop the collapsed rail** now; rebuild later only as the generic Provider render (§3.3).
- **Collapse = `RegionMode` state, never width.** Every input (toggle/key/RPC/drag-below-min) *writes*
  the mode; the renderer only *reads* it.
- **Left/right sidebar are one component, two instances** — differ only by position + mounted content.
- **`ViewNode` is optional for built-in providers** (they may build `Component`s directly); it is
  mandatory only at the **plugin boundary**. `Component` is the common denominator; `realize(ViewNode)`
  is the adapter. So the sidebar isn't required to be a `ViewNode` tree today.
- **menu-nav is one shared binding set**; **Space is NOT an activator** (would break command-palette
  space-typing). Enter activates.
- **Sidebar resize is NOT wired** (config-driven width only; `set_left_size` only called on reload) — a
  separate future feature.

---

## 6. WHAT COMES NEXT (ranked, all in BACKLOG)
1. **`widget-keys-config`** — apply the **menu-nav pattern** (semantic host-driven event + host-owned
   config resolution) to the remaining widgets that hardcode keys: `Dialog` (focus-nav), `Input`
   (editing), `Select`/`Tabs`. menu-nav is the shipped reference. Self-contained, medium size.
2. **`available-actions-1`** — context→available-actions query on top of `action_policy` +
   `resolve_active_context` + `ActionCatalog`; pure + unit-tested; feeds the command palette.
3. **`chrome-bug-titlebar-doubleclick-fullscreen`** — macOS `NSWindow` titlebar double-click zoom;
   needs **on-machine repro**; risky, do carefully (`heca/src/app/startup.rs`).
4. **Right-sidebar interactive surface** — blocked: the right sidebar has no content/Provider yet, and
   `DragSurfaceId` has no `RightSidebar` (a `// TODO`). Comes with **`plugin-03`** (the right gaining a
   Provider). Nothing to click on an empty right sidebar today.
5. **`plugin-03` — `WorkspacesContainerProvider`** — the big architectural next step: package the
   workspace tree as a registered `Provider`, then (later) the generic render-per-mode + a real
   collapsed rail (§3.3). Large; needs its own planning pass.

## 7. How to work on the next task (`widget-keys-config`)
- **Follow menu-nav exactly.** For each widget: remove literal-key matching from `event()`; make it
  respond to a **semantic event/intent** (like `Event::MenuNav`); add config keys in
  `keybindings.default.toml`; resolve config→intent **in the host** (a small map like
  `build_menu_keymap`, on `AppState`, rebuilt on reload) and deliver in the right host key branch.
- **Keep the Dialog fallback lesson:** any host interception of overlay keys must **fall back to the
  raw key when the widget returns `Handled::No`**, or you break other overlays.
- Widget-side tests drive the semantic event (see `heca-grid-ui/tests/phase_a.rs` palette tests);
  app-side unit-test the config→intent map (see `menu_keymap_maps_default_nav_bindings` in
  `heca/src/app/registry.rs`).

## 8. Gotchas / useful info
- **Two `ITEM_HEIGHT`s existed** (sidebar `hit_test.rs` + the deleted `render.rs`); now one in
  `hit_test.rs` (`pub(crate)`). The surviving `sidebar_hit_test` is the **expanded** drag-hover path
  only (used by `mouse/surface_left.rs` + `mouse/drag.rs`).
- **`.claude/settings.local.json`** now allowlists read-only commands (grep/ls/cat/git status/diff/…)
  to avoid permission prompts.
- **Config reload** rebuilds keymap, theme, appearance, `menu_keymap`, and clears `pane_headers` +
  `terminal_layers`; nulls `chrome_tree`.
- Verify UI changes **in-app** (GPU app) — user does the visual pass. This session's in-app-verified:
  reload icon recolor, no rail after collapse, rename-dialog Esc/Enter, menu/palette nav.
