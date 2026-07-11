# Sidebar Providers & Region Display Modes

**Status:** Agreed design — 2026-07-11 (grill-me session).
**Scope:** How the sidebar shell hosts Providers, what display modes a region has, and how a
Provider renders itself for a mode. This is the authoritative record; where it conflicts with the
older "collapsed icon rail" text in [`grid-ui-chrome-plan.md`](../grid-ui-chrome-plan.md) §2.5 or the
"DockView selector" note in [`surface-compositor.md`](./surface-compositor.md) §5, **this document
wins** and those sections point here.

> **Headline decision:** the **collapsed icon rail is dropped for now.** A region is either
> **Expanded** or **Hidden**. The generic "a Provider renders an icon rail when collapsed" design is
> **kept as a future item** (§4), to be built only when a Provider genuinely needs an always-visible
> status rail. This removes the hand-drawn rail and its separate hit-test, which is what caused
> `chrome-bug-collapsed-sidebar-picks`.

---

## 1. The three layers (plain vocabulary)

Keep these separate; most confusion comes from blurring them.

1. **Sidebar shell (the region host).** A generic, domain-neutral `ChromeRegion` widget that occupies
   the left (or right) column: framing, background, border, width, visibility, and display mode. It
   knows **nothing** about workspaces, Docker, or agents. There is one shell per region.
2. **Provider (the hosted widget).** The thing mounted inside the shell — **WorkspacesContainer**
   today; **Docker**, **AI Agents**, git, notes, plugin-defined ones later. A region can host one or
   more Providers and shows one at a time (the selector). A Provider owns its own content and
   behavior. This is what the user means by "widgets like the workspace collection, Docker, AI
   Agents — they are hosted in the sidebar."
3. **A Provider's content.** Groups and Items (nested to any depth). For WorkspacesContainer:
   workspaces (groups) → columns → panes (items). Columns are a structural level; they are not shown
   as their own rows today.

> **The shell hosts `Component` trees.** A plugin Provider contributes its content as a serializable
> `ViewNode` tree, which the host turns into components with `realize(&ViewNode) -> Box<dyn
> Component>`. A built-in Rust Provider may build components directly. Both end up as the same
> `Component` tree the shell mounts — `ViewNode` is the plugin authoring format, not a second
> renderer. See [`plugin-authoring.md`](./plugin-authoring.md).

---

## 2. Region display modes

A region has a **display mode**, held as canonical state in the shared chrome store and readable by
anyone (including plugins):

- `heca/src/chrome/state.rs` — `RegionState { mode: Signal<RegionMode>, size: Signal<f32> }`, with
  `left_mode()` / `right_mode()` / `set_left_mode()` / `set_left_size()`.
- `heca-grid-ui/src/widgets/chrome_region.rs` — `enum RegionMode { Expanded, CollapsedRail, Hidden }`.

### 2.1 The modes we actually use — Expanded ⇄ Hidden

- **Expanded** — the full sidebar. **Resizable** (drag the edge), width clamped to a minimum from
  config. The width is passed to the hosted Provider so it can lay its content out to the space.
- **Hidden** — the region is removed from layout entirely; all its space is reclaimed by the panes.
  (Fix the current quirk where `Hidden` still paints a ~40px strip — `Hidden` must be truly gone.)
- **`CollapsedRail`** — **not used for now.** The enum variant stays (a future rail will use it), but
  no region enters it and nothing renders it.

The toggle button (and its action / keybinding / RPC) flips **Expanded ⇄ Hidden**.

### 2.2 The mode is decided by state, never by width

This is the rule that prevents the class of bug we hit. **The renderer branches on `RegionMode`, not
on a width threshold.** Every input that changes the mode — the toggle button, a keybinding, RPC, and
the drag-resize — **writes the mode state**; the renderer only **reads** it. So:

- Dragging the edge below the minimum width **dispatches the action that sets `Hidden`** (the drag
  writes the state); it does not make the renderer inspect the width.
- The `SIDEBAR_EXPANDED_THRESHOLD` (`80.0`) constant and every `width >= threshold` branch in
  `build_chrome_root` / `surface_left.rs` are **removed**; those branches read `left_mode()` /
  `right_mode()` instead.

Keeping mode and width as one source of truth each — mode = which form, width = how wide when
Expanded — is what keeps mouse, keyboard, and RPC in agreement.

### 2.3 Width is independent, resizable state

`size: Signal<f32>` already exists. In Expanded it is user-resizable and clamped to a configured
minimum; it is passed to the Provider. In Hidden it is irrelevant (the region is gone).

---

## 3. How a Provider renders for a mode — the general contract

This contract is **generic for every Provider and its children** — not specific to
WorkspacesContainer. WorkspacesContainer is only the first one wired up.

> **This is a FUTURE target, exercised only when there is more than one mode to render.** With the
> collapsed rail dropped (§4), a region is Expanded or Hidden, so today a Provider renders **only its
> Expanded form** and none of the write-once machinery below needs building yet. Build §3 together with
> the collapsed rail, when a Provider needs it. It is written here so the contract is designed before
> it is implemented (and so `plugin-task-10`'s Provider shape stays compatible with it).

### 3.1 Write once — the author never writes two trees

A Provider describes its content **once** as a semantic tree of **Groups** and **Items**. Each node
carries its meaning a single time:

```
Item {
  icon:    <glyph>,            // e.g. the program icon
  label:   <text>,            // e.g. the pane name
  status:  Idle | Running | Success | Error,
  intent:  <action>,          // e.g. FocusPane
  actions: [ ... ],           // extra affordances (git, exit, buttons) — expanded only
}
Group {                       // e.g. a workspace
  identity: icon | name | number,
  collapsed: bool,            // show/hide its children
  children: [ Group | Item ]  // nested to any depth
}
```

**Display mode is the host's concern, not the author's.** The host renders this one tree for the
region's current mode. The author never writes an "expanded version" and a "collapsed version" — that
double authoring is exactly what we reject, and it would fall on every plugin too.

- **Expanded** → nested frames + full rows: `Group → DockFrame`, `Item → Row` (with all extras).
- **Collapsed (future, §4)** → a `RailCell` per node, recursively: a Group cell, then (if that Group
  is expanded) its children cells beneath it; an `Item` → a `RailCell`. Expanded-only extras (git,
  exit, action buttons) are simply omitted.

The mode-awareness lives in exactly **one place: `realize()`** (the host mapper renders `Item` /
`ItemGroup` as a row or a rail cell depending on the region mode). It is not duplicated in the
author's code and not duplicated across widgets.

### 3.2 Group collapse is generic

"A Group shows or hides its children" is one host behavior available to every Provider. A workspace
showing/hiding its panes is just this behavior applied to WorkspacesContainer (state lives in
`collapsed_ws: Signal<HashSet<usize>>`). Nothing about it is workspace-specific.

### 3.3 Two collapse flavors (a per-Provider property)

Locked earlier in `grid-ui-chrome-plan.md` §2.5 and unchanged in intent:

- **List Provider** (workspaces, and similar) — **enumerates**: one cell per item when collapsed.
- **Tool Provider** (a Docker panel, say) — **folds**: the whole Provider collapses to a single icon.

The Provider declares which flavor it is; the host applies it. Still one description, still write
once.

### 3.4 Interaction is generic

Status→color, the letter/KeyHint picker over cells, the active/cursor states, and right-click menus
are **host** capabilities applied to any Provider's items. The Provider only supplies, per item, its
`intent`, its `status`, and its context-menu entries.

---

## 4. FUTURE — the collapsed (icon-rail) design, if a Provider needs it

**Not being built now.** Recorded so the thinking is not lost. Build this only when a real Provider
benefits from an always-visible status rail (e.g. Docker showing running containers, AI Agents
showing activity). When built, it must be the **generic** host renderer of §3 — never a
workspace-only, hand-drawn rail.

The agreed visual spec for the collapsed WorkspacesContainer (the reference implementation of the
generic renderer):

- **Structure:** a flat vertical list (inside a `ScrollRegion` for overflow) of `RailCell`s —
  workspace cell, then its pane cells when that workspace is expanded, then the next workspace cell.
  Columns are not shown as cells; an expanded workspace lists its panes flattened across its columns.
- **Cell identity — two config settings** (both `[appearance]`, typed snake_case enums, default
  `"icon"`; both go in `config.default.toml` + README):
  - `workspace_collapsed_style = "icon" | "name" | "number"` — `icon` = `Stack` glyph; `name` =
    initial letter of the workspace name; `number` = 1-based workspace number.
  - `pane_collapsed_style = "icon" | "name" | "number"` — `icon` = the program icon from the catalog,
    **falling back to `FolderSimple`**; `name` = initial letter of the pane name (custom or program);
    `number` = pane number.
- **Colors (theme tokens, never literals):**
  - Workspace cell = `accent`; the active workspace (holds the focused pane) renders brighter
    (`RailCell.active`).
  - Pane cell = status: `Idle → muted`, `Running → accent`, `Success → success`, `Error → danger`.
    The focused pane renders brighter (`RailCell.active`). Workspace vs pane never read the same
    because the glyphs differ (`Stack` vs the pane icon).
- **Cursor:** the sidebar-nav cursor (`InputMode::SidebarNav`) shows as a distinct hollow outline —
  add `nav_selected` to `RailCell`, mirroring `Row` / `MarkerGroup` / `DockFrame`.
- **Clicks:** a workspace cell click **toggles its pane list** (`collapsed_ws`) and does **not**
  switch workspace; a pane cell click **focuses that pane** (`FocusPane`).
- **Right-click:** the **same** context menu as the expanded form (reuse the sidebar pane/workspace
  menu builders), resolved from the cell's own bounds via `chrome_dispatch_press`.
- **KeyHint:** the universal `prefix+/` picker stamps letters over the cells (wrap each `RailCell` in
  `KeyHint`), fed by the existing pick-candidate signals. Column-target picks are unavailable while
  collapsed because columns are not shown — an accepted limitation.
- **Widget work required:** extend `RailCell` to show a short text label (for `name` / `number`),
  add its `nav_selected` state, and add `Glyph::Stack` + `Glyph::FolderSimple` to the icon set.

---

## 5. What changes in the code now (the "drop it" work)

1. **Mode drives everything.** `build_chrome_root` (and `surface_left.rs`) branch on `left_mode()` /
   `right_mode()`, not on width. Delete `SIDEBAR_EXPANDED_THRESHOLD`.
2. **Toggle = Expanded ⇄ Hidden.** The sidebar toggle action sets the mode signal; `Hidden` reclaims
   all space (fix the 40px-strip quirk).
3. **Delete the hand-drawn rail and its separate hit-test:** `render_sidebar_collapsed`
   (`heca/src/sidebar/render.rs`), the collapsed branch of `sidebar_hit_test` and
   `collapsed_pane_label` (`heca/src/sidebar/hit_test.rs`, `render.rs`), and the collapsed-rail draw
   calls in `heca/src/app/render.rs`. This removes `chrome-bug-collapsed-sidebar-picks` by removing
   the broken code.
4. **Keep** the resizable-width work and the `RegionMode`/`size` signals. `RegionMode::CollapsedRail`
   stays in the enum (unused) for the future rail; `RailCell` stays a library widget but is not
   mounted anywhere for now.

---

## 6. Cross-references

- [`grid-ui-chrome-plan.md`](../grid-ui-chrome-plan.md) §2.5 — the library side (shell, `DockFrame`,
  `RailCell`, the two rail flavors). Its "collapsed = icon rail" is **deferred** per this document.
- [`pluggable-chrome-plugin-plan.md`](../pluggable-chrome-plugin-plan.md) — the app side (ChromeHost,
  Providers, `WorkspacesContainerProvider`, `build_contribution`). The render-per-mode contract (§3
  here) is the target Provider shape.
- [`plugin-authoring.md`](./plugin-authoring.md) — `ViewNode`, `realize`, and how a plugin Provider
  describes content once.
- [`surface-compositor.md`](./surface-compositor.md) §5 — sidebar layering / hint visibility.
- BACKLOG: `chrome-bug-collapsed-sidebar-picks` (resolved by removal), `app-task-21` (dropped),
  `sidebar-fu-2` (dropped), `plugin-task-10` (WorkspacesContainerProvider, render-per-mode target).
