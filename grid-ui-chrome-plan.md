# heca-grid-ui — Chrome Adaptation Plan (Region shells · Docks · Drag-and-Drop · flexible items)

**Date:** 2026-06-09
**Status:** Design plan — the **`heca-grid-ui` (library) side** of the pluggable-chrome program.
**Pairs with:** [`pluggable-chrome-plugin-plan.md`](./pluggable-chrome-plugin-plan.md) (the **app** side: ChromeHost, AppState, providers, dynamic actions, WASM). This doc realizes that plan's **Phase 7 — "heca-grid-ui Chrome Widget Expansion"**, plus the **flexible item layout** and the **Drag-and-Drop** hooks it implies.
**Tracker:** progress lives in [`grid-ui-plan.md`](./grid-ui-plan.md); this doc supersedes its old `C6 Sidebar = tree-nav` framing.

> **Decisions locked (2026-06-09):** build the non-DnD vocabulary **now**; region shell is **generic across all 4 regions**; `DockFrame` is a **new widget reusing Pane's brackets**; grid-ui **stays domain-neutral** for status styling; **adopt the incoming DnD system** (don't build a homegrown one); `Grid` exposes **tracks + named areas**; icons via an **embedded, host-registered icon font**; **request renderer `PushClip`/`PopClip`** for scroll; collapsed region = **icon rail, keyboard-expandable**; shared state = a **namespaced signal store** read via signals / written via actions.

---

## 0. Principles that drive everything

**(P1) `heca-grid-ui` is presentation vocabulary only.** It provides shells, frames, rows, layout, icons, and the *hooks* for drag-and-drop. It owns **no** canonical state, **no** domain logic, and never depends on `heca`/WM state. Layering stays `heca → heca-renderer → heca-grid-ui → heca-core`.

**(P2) Input parity — mouse = keyboard = RPC (hard, app-wide rule).** Every chrome interaction — collapse/expand a region, focus or peek a collapsed dock, move/reorder a dock, select/activate a row — must be a **named action** reachable equally by **mouse, keyboard, and RPC**. **No mouse-only behavior, ever.** A collapsed icon-rail must be keyboard-expandable via an action, exactly like today's keyboard-driven sidebar. (Mirrors chrome plan §2.9 / §11.) For grid-ui this means: widgets expose *intents* (callbacks / opaque action ids), never bury behavior in pointer handlers that the keyboard can't reach.

**(P3) State is read via signals, written via actions.** Widgets and Docks **read** fine-grained `Signal`s and **never mutate canonical state directly** — they dispatch actions (chrome plan §2.3). This keeps writes uniform and automatically reachable from keyboard/RPC (P2).

```
heca (app)                         heca-grid-ui (library)            heca-renderer
──────────                         ──────────────────────            ─────────────
ChromeHost (regions, placement)    ChromeRegion / Sidebar shell      Scene → GPU
AppState (namespaced signal store) DockFrame (title/collapse/handle) PushClip/PopClip
Docks: WorkspacesDock, GitDock…    Grid (tracks + named areas)       (needed for scroll)
  └ own logic + internal items     Item / ItemGroup (rows)
Actions (the only writes)          Icon, StatusDot, Badge, Tag       incoming DnD lands
DnD meaning = dispatch(action)     DnD hooks → incoming DnD system    in this crate soon
```

---

## 1. Terminology & ownership

| Term | Lives in | What it is |
|------|----------|-----------|
| **Region** | app (ChromeHost) + grid-ui shell | A chrome area: left/right **sidebar** (vertical), **top/bottom bar** (horizontal). App owns which regions exist + visibility + placement; grid-ui renders the **region shell**. |
| **Dock** | **app** | The hosted, **movable** unit a region mounts (`WorkspacesDock`, `GitDock`, the docker one, …). Owns its domain logic + internal items. Declares metadata (`id`, `title`, `supported_regions`, `default_region`, `movable`, `collapsible`). The region can host/move/drag a Dock but knows **nothing of its contents**. |
| **`DockFrame`** | grid-ui (**new**) | Visual wrapper around a mounted Dock: title bar, collapse toggle, drag handle, a header-controls slot (a Dock may put its own search/filter there), body. Reuses `Pane`'s rounded-bracket painting. The chrome plan's `SidebarContainerFrame`. |
| **`ChromeRegion` / `Sidebar`** | grid-ui (**new**) | The **shell**: oriented (vertical/horizontal), toggle/collapsible, mode-aware, stacks `DockFrame`s, scrolls, and is a **drop target** for Dock-level DnD. No tree/workspace/expand/drag *semantics* of its own. |
| **AppState** | **app** | A **namespaced signal store** (see §4): canonical state + per-namespace derived/UI state. grid-ui widgets **read** signals from it; they never own it. |
| **Item / ItemGroup** | grid-ui | Reusable row + collapsible group a Dock *may* use internally. Domain-neutral (menus, dropdowns, Docks all reuse them). `Item` already exists = the chrome plan's "SidebarItem". |

> App-side dock names are the app's call (e.g. the docker one as `DockerDock` reads awkwardly — name it by function/title). grid-ui is indifferent.

---

## 2. What grid-ui must provide

Presentation-only. "Status" is relative to PR #30/#32.

### 2.1 `Grid` layout widget — the flexible item content
The enabler for rich items ("a CSS grid where we can put whatever we want"). taffy (already a dep) supports CSS Grid, so:
- New `Grid` widget over taffy `display: grid`, exposing **both** explicit tracks and named areas:
  - column/row **tracks** (`px` / `fr` / `auto` / `min-content`), `gap`;
  - place a child by **named area** *or* by explicit **cell + span**.
- A rich pane row becomes a `Grid` of arbitrary cells:
  ```
  areas:  "icon  title    status"
          "icon  subtext  exit"
  ```
  each cell holds any `Component` (Label, Badge, StatusDot, Icon, …).
- Extends `Style`/`to_taffy()` with grid fields (or a parallel `GridStyle`).
- **Status:** new. Independent — first to build.

### 2.2 Icon support — embedded, host-registered icon font
- Vendor a permissively-licensed glyph icon font; the **host registers** it as a second family (same mechanism as Geist Mono; family stays theme-configurable). Needs a ~1-line OK from the renderer dev to register a 2nd font.
- `Icon` widget renders a single codepoint via the existing text path — no renderer texture work.
- **Status:** new (known gap). Confirm 2nd-font registration with renderer.

### 2.3 `ItemGroup` (collapsible group)
- Header row (label + chevron + optional count/controls) over a collapsible child set. Collapse state is a `Signal` (owned by AppState in real use). Expand/collapse is an **action** (P2).
- **Status:** new. Builds on `Item`.

### 2.4 `DockFrame` (new; reuses Pane brackets)
- Titled, **collapsible** frame: title bar (title + collapse toggle + **drag handle** + header-controls slot) over a body that hosts the Dock's content; keeps the rounded corner brackets.
- Keep `Pane` as the plain framed container; `DockFrame` is the chrome wrapper.
- **Status:** new (Pane exists, has no header/collapse/handle).

### 2.5 `ChromeRegion` / `Sidebar` shell (generic, all 4 regions)
- **Oriented** shell: vertical (sidebars) or horizontal (top/bottom bars). One widget covers all four regions.
- Toggle/collapse, **mode-aware**: informs children of `Expanded` / `CollapsedRail` / hidden via a signal.
- **Collapsed = icon rail** (thin rail of dock icons; click *or keyboard action* to expand/peek — P2). DockFrames render icon-only in rail mode.
- Stacks `DockFrame`s, scrolls (§2.8), exposes **Dock-level drop targets**.
- **No** workspace/tree/expand/drag *semantics* — those belong to the mounted Dock. Replaces the old "Sidebar = tree-nav".
- **Status:** new.

### 2.6 Drag-and-Drop — adopt the incoming system (see §3)
- **Do not build a homegrown DragManager.** A DnD system is landing in this crate; DockFrame's drag handle, region drop-targets, and item-reorder will be **thin hooks onto it**.
- grid-ui's job here: expose draggable/drop-target *affordances* on `DockFrame`/`ChromeRegion`/`Item`, and ensure every drop is expressed as an **action** (P2), not a direct mutation.
- **Status:** **deferred until the incoming DnD code lands**, then hook in.

### 2.7 Status-driven item composition — neutral primitives + a recipe
- The rich pane row (program name · git status+icon · exit code · running/idle/stopped style) is **composed**, not a monolith: `Grid` + `Label` + `StatusDot` + `Badge`/`Tag` + `Icon`.
- grid-ui **stays domain-neutral**: ships the primitives + style/color tokens; the **Dock maps** program/git/activity state onto them (running→accent, idle→muted, stopped/exit≠0→danger). No `ActivityStatus` enum in the library.
- Add a thin `Tag`/`Chip` (e.g. git branch) and optionally a `MetricRow` convenience.
- **Status:** primitives mostly exist; needs `Grid` (2.1) + `Icon` (2.2) + `Tag`.

### 2.8 Scroll / list primitive — **renderer dependency requested**
- Region shells and dock lists overflow → need embeddable scrolling.
- **Hard dependency:** real clipping needs `PushClip`/`PopClip`, currently a **no-op** in `heca-renderer/src/scene.rs` (owned by another dev). **Action: request the renderer dev implement it.** Until then, only whole-page scroll works (shells render unclipped).
- **Status:** gated on renderer clip.

---

## 3. Drag-and-Drop — how grid-ui hooks the incoming system

grid-ui does **not** own the DnD mechanism (the incoming system does). grid-ui owns the *affordances* and keeps DnD honest about input parity.

### Two altitudes (same mechanism, payload-typed)
- **Dock-level** — move/reorder a `DockFrame` within a region or **between regions** (left ↔ right sidebar, into a bar). Drop targets = region shells / inter-dock gaps. Payload = a dock id.
- **Item-level** — reorder rows *inside* a Dock (e.g. panes within `WorkspacesDock`). Drop targets = the Dock's rows/gaps. Payload = an item id. Stays inside the Dock.

Distinguished by **payload kind**; a drop target declares which kinds it accepts. Payloads are **opaque ids** (same philosophy as the deferred `ActionSink` opaque action ids).

### grid-ui's responsibilities
- Mark `DockFrame` (via its drag handle) and `Item` as drag **sources** carrying an opaque payload.
- Mark `ChromeRegion` / inter-dock gaps / `Item` rows as drop **targets** with an `accepts(kind)` predicate + a drop position (before/after/into).
- Render the incoming system's ghost/insertion visuals in the **overlay layer** (already exists).

### App's responsibilities (P2 + P3)
- Decide **what's draggable** and **what a drop does**: a drop emits an **intent**, the app **dispatches an action** (`chrome.dock.move_to_region`, `chrome.dock.reorder_before`, a Dock-internal `workspace.pane.move`, …).
- The **same move must be reachable by keyboard + RPC** via that action — DnD is only the mouse surface.
- Persist placement in AppState.

---

## 4. Shared state strategy (the flexible, plugin-ready pattern)

> Answers: "keep the scrollable area, the WorkspacesDock (panes), and future plugins in sync — most flexible solution" and "state must not be hardwired to panes/ws/columns; define a reusable pattern for future plugins/core features."

**Model: one app-owned, namespaced, signal-backed store. Read via signals; write via actions. grid-ui widgets are pure consumers.**

### Shape
- **AppState** (app-side) = a registry of **namespaces**, each owned by a feature/dock/plugin:
  - `chrome` — region visibility, dock placement/order per region, region collapse mode, scroll offsets, overlay stack, active drag session.
  - `workspaces` — canonical ws / columns / panes (from the WM).
  - `<dock-id>` / `<plugin-id>` — that unit's own UI + derived state.
- **Two tiers** (chrome plan §2.3 / §3.3):
  - **canonical** — owned by the app/WM; never mutated directly by docks/plugins.
  - **derived / UI** — per-namespace; a dock owns its slice (selection, hovered, expanded ids, search query, scroll offset).
- **Everything is addressable as `(namespace, key) → Signal<T>`.** UI concerns like selection / hover / collapse / search are **not special-cased** — they're ordinary namespaced keys, so a plugin gets the same capabilities as a core dock with no core change.

### Access contract
- **Read** = subscribe to fine-grained `Signal`s / selectors. floem reactivity gives "attach and react" for free — only dependents repaint; the scroll area, WorkspacesDock, and plugin docks all observe one coherent store and update reactively (no manual refresh wiring).
- **Write** = **dispatch an action** (`namespace.verb`, args) — even for a unit's own slice. Uniform, and keyboard/RPC-reachable (P2/P3).

### How grid-ui consumes it (two options; start simple, scale later)
- **(a) Explicit signal injection** *(start here)* — the app passes the specific `Signal`s a Dock/widget needs at construction. Simplest, explicit, zero new library machinery.
- **(b) Scoped context handle** *(scale path)* — the app hands a Dock a `ChromeCtx` carrying read-selectors + `dispatch` for its namespace. More ergonomic with many docks/plugins. The handle is **app-side**; grid-ui *may* later add a tiny "pass a context down the subtree" helper, but it is **not** required and must not become a global store inside the library.

### Extensibility
- A new dock/plugin **registers a namespace** + declares its state keys + its actions — no core change. This is what lets future plugins (WASM, later) read coherent state through a controlled surface and write only via dispatched actions (chrome plan §3.5).

### Selection model
- Lives as a namespaced key (`<dock>.selection`): a single id today, `Vec<id>` when a dock needs multi-select. `Item`/`ItemGroup` stay pure — they read an `is_active` signal and emit activate **intents**; they never own selection.

---

## 5. What stays OUT of grid-ui (app-side)

- `ChromeHost`, region registry, dock placement/order, persistence.
- `AppState` (the namespaced store), canonical state, the **action registry**, keybindings, RPC.
- The **Docks** themselves — `WorkspacesDock` (the current `sidebar.rs` logic, migrated per chrome plan Phase 5), `GitDock`, the docker dock, …
- The **DnD mechanism** (incoming system) and the **meaning** of a drop (action dispatch).
- Overlay *ownership* (host-owned per chrome plan §2.7) — grid-ui provides the overlay *layer*; the host owns z-order/focus-trap/ESC/anchoring.

---

## 6. Phases / tasks (grid-ui side)

Realizes chrome plan Phase 7. Build the vocabulary **now**; app integration is gated behind chrome plan Phase 0 (the refactor); DnD is gated on the incoming system; scroll is gated on the renderer.

| # | Task | Depends on | Gate |
|---|------|-----------|------|
| **G1** | `Grid` layout widget (taffy grid; tracks + named areas) | — | none — start now |
| **G2** | `Icon` widget + embedded icon font (host-registered) | renderer 2nd-font OK (tiny) | mostly none |
| **G3** | `ItemGroup` (collapsible group over `Item`) | Item (done) | none |
| **G4** | `DockFrame` (title + collapse + drag handle + header slot; reuse Pane brackets) | — | none |
| **G5** | `ChromeRegion`/`Sidebar` shell (oriented all-4, collapsible w/ icon-rail, mode-aware, hosts DockFrames, drop targets) | G4 | none for shell; rail uses G2; full use needs G6 + ChromeHost |
| **G6** | DnD **hooks** (drag handle / drop targets / item reorder) onto the **incoming** DnD system | incoming DnD code; G4/G5 | **wait for incoming DnD** |
| **G7** | Scroll/list primitive (embeddable) | **renderer `PushClip`/`PopClip`** | **request + wait on renderer** |
| **G8** | Rich status-item recipe + `Tag`/`Chip`; showcase: mock WorkspacesDock with program/git/status rows, collapse, (DnD reorder once G6) | G1, G2, G3 | none (DnD reorder after G6) |

**Suggested order now:** G1 → G3/G4 (+G5 alongside) → G2 → G8 (visible payoff). Then G6 when the DnD code lands, G7 when clip lands.

Each task: showcase section, `cargo test -p heca-grid-ui` + clippy green, board update, branch+PR per workflow. **Watch for the incoming DnD code in this crate** — coordinate before adding any drag types.

---

## 7. Open questions (resolved + remaining)

**Resolved 2026-06-09:** sequencing (vocabulary now, DnD waits) · region scope (generic all-4) · DockFrame (new, reuse Pane) · status styling (neutral) · DnD (adopt incoming) · Grid API (tracks + areas) · icons (embedded, host-registered) · scroll (request renderer clip) · collapsed mode (icon rail, keyboard-expandable) · state (namespaced signal store; read=signals, write=actions; inject explicitly first, context handle later) · selection (namespaced key, app-owned).

**Remaining:**
- **DnD ↔ action bridge signature** — finalize once the incoming DnD lands; align with the `ActionSink` opaque-id design (grid-ui-plan §12) so DnD + shortcuts share one path.
- **Icon font choice** — which family + licensing; confirm renderer 2nd-font registration.
- **`Grid` surface detail** — how much of taffy grid to expose (min-viable: tracks + areas + span).
- **`ChromeCtx` shape** — defer until enough docks exist to justify (b) over (a).
- App-side (owned by the chrome plan, not here): dock metadata, the `AppState` namespace registry, the action ids.

---

## 8. Relationship to the other plans

- **`pluggable-chrome-plugin-plan.md`** — app side; this doc = its Phase 7 + DnD hooks + flexible items. App integration (ChromeHost, providers, `WorkspacesDock` migration, dynamic actions, WASM, overlays) is gated behind that plan's Phase 0.
- **`grid-ui-plan.md`** — the tracker. Its old **C6 "Sidebar = tree nav"** is the wrong altitude and is **superseded** by §2.5 here (Sidebar = dumb shell). C6/C7 will be rewritten to reference this doc; the tree behavior is explicitly **app-side** (`WorkspacesDock`), not a grid-ui widget.
