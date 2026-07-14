# Heca — Pluggable Chrome + WASM Plugin Architecture Plan

**Date:** 2026-06-05  
**Status:** Future architecture plan  
**Starts only after:** the current refactoring is complete (it is — all 10 phases done).
**Important note:** this document defines the next architecture program. The structural cleanup is finished; the codebase now has clean seams for this work to begin.

> **Scope — this is NOT the full plan.** This file is the architecture *north-star / rationale* for **one
> workstream**: the pluggable-chrome + WASM-plugin arc (Phases 0–11). It does **not** track the broader
> active work (appearance/blur/zoom/font, pane numbering, F4.4/F4.5 sidebar, grid-ui maturity backlog,
> render split, …). The **single operational source of truth is [`PLAN.md`](./PLAN.md)** — it owns the
> prioritized task list and current status, and references this file for the long-arc design. Keep this
> doc for *rationale*; track *tasks* in PLAN.md. Checklist (§7) status updated 2026-06-17.

---

## 1. Why This Document Exists

New requirements introduced on **2026-06-05** changed the long-term direction of the UI architecture.

The earlier sidebar discussion treated the sidebar too narrowly as a single workspace/column/pane tree. The new requirements make it clear that heca needs something broader:

- a **pluggable chrome system**, not just a sidebar widget
- multiple pluggable regions:
  - **left sidebar**
  - **right sidebar**
  - **top bar**
  - **bottom bar**
- region contributions that can come from:
  - first-party built-in providers
  - later **WASM plugins**
- plugins that:
  - observe app events
  - read app state through a controlled API
  - **dispatch actions** instead of mutating state directly
  - contribute UI containers to one or more regions
  - register new actions into the action system so the user can later bind them from config

This is a much larger architecture change than the original sidebar refactor.

So this document captures, in one place:

- what was decided
- what must change
- what new subsystems are required
- how to phase the work safely
- what dependencies exist between phases

---

## 2. Core Architectural Decisions

## 2.1 Sidebar is only a shell, not the workspace tree

The left sidebar must no longer be treated as synonymous with the workspace tree.

Instead:

- the **Sidebar widget/shell** is only a visual/layout region shell
- it may be bordered, toggleable, and capable of informing children about its current display mode
  > **Display modes (decided 2026-07-11):** a region is **Expanded ⇄ Hidden**. The **collapsed icon
  > rail is dropped** for now; the shell still informs children of the mode, but only `Expanded` and
  > `Hidden` are used, and Expanded is resizable (width passed to the mounted Provider). The generic
  > "Provider renders an icon rail when collapsed" model — where a Provider describes its content once
  > and the host renders it per mode (write once) — is a **future** item. Full design + rationale:
  > **[`docs/sidebar-provider-modes.md`](./docs/sidebar-provider-modes.md)**.
- it hosts one or more mounted containers
- the workspace tree becomes a **WorkspacesContainer** mounted inside that shell
- other containers may also be present in the same sidebar, in a specific order, top-to-bottom

Examples of future containers:

- Workspaces
- AI Agents
- Docker Containers
- Git Status / Git Worktrees
- Tasks
- Project Notes
- Plugin-defined containers

This means the current `sidebar.rs` logic should ultimately be reinterpreted as:

- the first built-in **WorkspacesContainer** implementation
- not the definition of the entire sidebar system

It also means the following behaviors are **not Sidebar-shell concerns**:

- workspace/column/pane up/down navigation semantics
- workspace/column expand-collapse semantics
- pane drag/drop and swap semantics
- workspace-tree-specific highlight/visited logic

Those belong to the mounted `WorkspacesContainer`, not to the Sidebar shell.

---

## 2.2 Search belongs to containers, not to the sidebar shell

The Sidebar shell itself should not imply that a search input exists. A container may include one as part of its own content and behavior model.

The previous idea of a global sidebar search input was revised.

New rule:

- a **SidebarContainer** may expose its own search/filter input if it wants one
- the sidebar host itself should not own a mandatory search field

Why:

- not every container needs search
- different containers may need different filtering semantics
- search may be scoped differently depending on domain

Examples:

- WorkspacesContainer may filter workspaces / columns / panes
- AgentsContainer may filter by name, role, status
- DockerContainer may filter by container name, image, state
- a simple status container may not need search at all

---

## 2.3 Plugins must not mutate app state directly

A plugin must **not** receive raw mutable access to heca internals.

Instead, plugins should:

- observe events
- read state through a controlled host API / facade
- return UI contributions
- dispatch actions/intents that the app handles

This keeps state ownership explicit.

Correct model:

- **app owns canonical state**
- plugins own only **derived state** or local plugin state
- plugins ask the app to do things through **actions**

Examples:

- good: `app.actions.dispatch("pane.focus", { paneId: 42 })`
- bad: plugin directly mutates `session.workspaces[0]...`

---

## 2.4 ActionRegistry must become dynamically extensible

The current action system is not enough if plugins must add actions that can later be bound in config.

New rule:

- plugins must be able to **register actions dynamically**
- those actions must be visible to:
  - config keybindings
  - command palette
  - UI-triggered buttons/items
  - later RPC/automation if desired

This implies that the current static/closed action model will need to evolve.

The future action system must support:

- stable string action ids
- metadata for actions
- dynamic registration/unregistration
- dispatch with arguments
- separation between built-in actions and plugin-provided actions

Examples of future action ids:

- `workspace.focus_next`
- `pane.close`
- `plugin.docker.restart_selected`
- `plugin.agents.open_chat`

---

## 2.5 Plugins should be code-based, not just static JSON files

It was considered whether plugins could simply return static declarative content. That may help for trivial integrations, but it is too weak for the intended GUI interaction model.

The preferred direction is:

- **code plugins**, not raw text/template-only plugins
- specifically: **WASM plugins** as the long-term plugin format

Why WASM:

- safer than native dylib plugins
- avoids Rust ABI instability across dynamic library boundaries
- still allows code-based interaction and stateful behavior
- supports a host-controlled API boundary
- can be driven by events
- can perform async flows like modal interactions and action handling

WASM plugins should receive a host SDK/facade, not direct Rust object references.

---

## 2.6 Plugins should contribute UI declaratively, but from code

Important distinction:

- plugins should not manually hand-write giant JSON blobs as their primary authoring experience
- plugins also should not directly instantiate internal Rust widget structs across the boundary

Preferred model:

- plugins are written in code
- they use a host SDK / builder API
- internally this produces a host-understood declarative model
- the host maps that model to `heca-grid-ui` widgets and manages rendering/event routing

So plugin authors get a code-first API, while the host still controls:

- rendering
- focus
- overlays
- clipping
- region constraints
- widget validation

### 2.6.1 How a plugin renders our widgets — host-adapter + declarative ViewModel (design; Phase 9)

This is the *design* for the plugin authoring path; it is **not built yet**. The
built-in Rust path (plugin-03) builds real widgets directly; the pieces below are
the WASM plugin path (Phase 9) and are recorded so the seam we ship now stays
compatible with them.

**A) Host-adapter pattern — how a plugin becomes a `Provider`.** A WASM plugin
does **not** cross the boundary as a Rust `Box<dyn Provider>` (unstable ABI,
safety). Instead the host wraps each plugin in a first-party **adapter** —
`WasmProviderAdapter: Provider` — that marshals `build_contribution`, events
(`app.on`), and action dispatch to/from the WASM module. So the `Provider` trait
is the **single seam**: built-ins implement it directly; plugins are reached
through a host-owned adapter that speaks the same trait. Everything downstream
(`ChromeHost`, regions, contributions) is identical for both.

**B) Authoring UI: code → builder SDK → declarative ViewModel → host maps to
`heca-grid-ui`.** The plugin never instantiates our widget structs across the
boundary and never hand-writes JSON as its primary experience. It calls a
**builder SDK** that produces a **serializable ViewModel** — a tree of *typed
nodes with props* drawn from the host's **closed widget vocabulary**. The host
receives that tree, maps each node to the real `heca-grid-ui` widget, applies
props, mounts the subtree into the region, and owns render/focus/clip. Sketch of
a plugin building a panel with a button:

```
// plugin code (compiles to WASM), using the host SDK builder:
Panel::new("docker.containers")
    .title("Containers")
    .child(Row::new()
        .child(Label::new(state.name))
        .child(Button::new("Restart")
            .variant(Variant::Danger)          // semantic variant, not a raw color
            .size(WidgetSize::Small)
            .on_press(intent("plugin.docker.restart", { "id": state.id }))))
// → serializes to a ViewModel: { kind:"Panel", props:{title}, children:[ { kind:"Row", … } ] }
// → host maps each node to the grid-ui widget, themes it, mounts it.
```

**C) Props vs styling.** Props are **serializable data** — strings, numbers,
bools, and *semantic enums* the host knows (`WidgetSize::Small`,
`Variant::Danger|Accent`, `TooltipSide`, …). **Styling is not a free prop:**
colors/fonts/alphas come from the `Theme`/config, never from raw values passed by
the plugin. The plugin picks *intent* (danger/accent/size); the host resolves the
actual pixels from the theme. This keeps every plugin visually consistent and
theme-driven.

**D) Interaction is intent-based, not callbacks across the boundary.** A
`Button::on_press` does not run plugin code during paint. It carries an **intent**
= a string action id + serializable args. The host routes it: either dispatch a
registered action, or deliver it to the plugin as an event (`app.on`). This keeps
input routing, ordering, and re-entrancy on the host side.

**E) Overlays are requested, not mounted.** A **modal/dropdown/popover is NOT a
region contribution** the plugin mounts — it is host-owned (§2.7.1). The plugin
**requests** it and awaits a typed result:

```
// Simple case via the message convenience; body could instead be a full tree (§2.6.2/§2.7.1).
let result = ctx.overlay.open_modal(
    ModalSpec::message("Restart nginx?", "The container will stop briefly.")
        .danger(true)
        .actions([ModalAction::danger("restart", "Restart"), ModalAction::new("cancel", "Cancel")]),
).await;                                // WASM: marshals as request-id + a resolve event
match result {
    ModalResult::Action { id, .. } if id == "restart" => ctx.actions.dispatch("plugin.docker.restart", args),
    _ => {}
}
```

The host owns z-order, focus trap, ESC, click-outside, positioning, and returns
the typed `ModalResult`/`DropdownResult`. So: **panels/toolbars/status segments →
mounted into a region as a `Contribution`; modals/dropdowns → requested from the
`OverlayHost`.**

**Why this shape:** no Rust object and no GPU/focus/overlay control crosses the
boundary — only serializable data + string ids; the host stays the single owner
of rendering, theming, input routing, and overlay z-order (§2.6/§2.7 guardrails).
Shipped today: the `heca-grid-ui` widgets themselves + the read/observe `App`
facade. Planned: the builder SDK + ViewModel + WASM bridge (Phase 9) and the
`OverlayHost` async API (Phase 8).

### 2.6.2 The declarative widget tree (`ViewNode`) — a SwiftUI/Flutter-style model

The "ViewModel" above is concretely a **recursive widget tree**: a *container*
node holds a **vector of child widgets**, each of which may itself be a container.
This is the same shape as Flutter's `Widget` tree or SwiftUI's `View` tree.

**Half of it already exists.** `heca-grid-ui` is already a retained, recursive
tree: every widget has `Base.children: Vec<Box<dyn Component>>`, the `Parent`
trait exposes `.child(...)`, and `Flex`/`Row`/`Card`/… hold arbitrary nestable
children. That is our *RenderObject/Element* layer. What the plugin path adds is
the **declarative layer on top** — a serializable `ViewNode` (Flutter's `Widget` /
SwiftUI's `View`) that the host **realizes** into that existing retained tree.

**The declarative node** (design; Phase 9):

```
struct ViewNode {
    kind:     WidgetKind,               // closed enum of host-known widgets:
                                        //   containers: Column|Row|Grid|Card|Scroll|Panel
                                        //   leaves:     Label|Button|Badge|Icon|Input|Toggle|StatusDot|…
    props:    PropMap,                  // serializable scalars + semantic enums (variant, size, align…)
    events:   Map<EventName, Intent>,   // on_press / on_change → intent(action_id, args)
    children: Vec<ViewNode>,            // recursive; empty for leaves
}
```

Containers (`Column`/`Row`/`Grid`/`Card`/`Scroll`/`Panel`, and the modal `body` in
§2.7.1) carry `children`; leaves don't. A typed, SwiftUI-like **builder SDK** sits
on top for ergonomics and emits this uniform node (just as Flutter's typed
`Widget` classes lower to `Element`/`RenderObject`):

```
Column::new().gap(8).padding(12)
    .child(Label::new(title).variant(Variant::Heading))
    .child(Row::new()
        .child(Badge::new(status).variant(Variant::Accent))
        .child(Button::new("Restart").variant(Variant::Danger)
            .on_press(intent("plugin.docker.restart", { "id": id }))))
```

**The host mapper** — `realize(&ViewNode) -> Box<dyn Component>` — is a recursive
walk: create the `heca-grid-ui` widget for `kind`, resolve `props` against the
`Theme`, wire `events` to intent routing, then recurse on `children` and attach
each via `.child(...)`. Because the retained tree already exists, the mapper only
**translates**; it never reimplements layout, paint, focus, or clipping.

**Vocabulary is closed to plugins, extensible by the host.** Adding a new
`WidgetKind` (e.g. `Table`) is host-side work — the widget in `heca-grid-ui`, its
showcase demo + `docs/widgets.md`, and a mapper arm — **never** something a plugin
invents. Until a first-class `Table` exists, a table is *composed* from the
existing building blocks (`Grid`/`Flex` + `Row` + `Label`/`Badge` + `Scroll`).

---

## 2.7 Overlays must be host-owned

Because heca is a GUI app, modal/dialog/dropdown/popover behavior must be managed by the host.

Plugins may request overlays, but the host must own:

- z-order
- focus trap
- keyboard routing
- ESC behavior
- click-outside dismissal
- positioning / anchoring

This is critical.

So plugin model should support things like:

- `await app.overlay.openModal(...)`
- `await app.overlay.openDropdown(...)`

This solves the “how do I know which button was pressed?” problem much better than pure JSON triggers.

### 2.7.1 Formal contract (`plugin-task-03`, proposed 2026-07-02 — pending review)

**Grounding — what already exists.** The grid-ui overlay widgets (`Modal`,
`Select`, `CommandPalette`, `Tooltip`, `ToastStack`) each report
`overlay_active()` + `focusable()` while open, draw on the scene's overlay layer
via `cx.with_overlay(...)`, and the `FocusManager` routes pointer/key events to
the active overlay first. Open/close is a **host-owned `Signal<bool>`** per
widget (see `heca-grid-ui/src/widgets/modal.rs`). Dismissal paths are already
correct: buttons, `Esc` (= cancel), scrim click; `dismissible(false)` forces a
button decision. **Two gaps** this contract closes:

1. **No central stack.** Each widget owns its own bool signal; nothing arbitrates
   z-order between several open overlays or owns a single focus trap.
2. **No result value.** Interaction is callback-only (`Modal::confirm`/`cancel`
   closures) — a provider cannot `await` "which button was pressed".

**Decision — a host-owned `OverlayHost`.** A new app-side overlay stack
(`heca/src/chrome/overlay.rs`, built in `plugin-02`/Phase 8) owns an explicit
z-ordered `Vec` of active overlays. It renders each entry through the *existing*
grid-ui widget bound to host-owned signals — providers/plugins **never** build
overlay widgets across the boundary (§2.6); they submit a **spec** and receive a
**typed result**. Input routes to the top of the stack first (reusing the
`overlay_active()`/`FocusManager` path). A **modal** entry is focus-trapping +
scrim + blocks everything below; a **dropdown/popover** entry is light-dismiss
(click-outside or `Esc` pops it) with no scrim.

> **Built on the surface compositor (`docs/surface-compositor.md`).** `OverlayHost` is **not**
> a separate stack: it is the overlay-level API on top of the app's `LayerStack`/
> `LayerRegistry` (the single layering mechanism that owns band z-order, occlusion, hint
> visibility, and later paint + input). `open_modal` `realize`s the `ViewNode` body + actions
> into a native tree, **pushes it as a `Modal`-band layer**, and resolves `ModalResult` when a
> button's intent fires. So the overlay z-order/focus-trap here and the compositor's layering
> are the same stack, described from the overlay API's angle.

**Result-returning API shape.**

```rust
/// Host-owned overlay stack. Providers/plugins submit a spec and await a typed
/// result; the host owns z-order, focus trap, Esc, click-outside, positioning.
pub trait OverlayHost {
    /// Push a modal; resolves when the user confirms, cancels, or dismisses.
    fn open_modal(&self, spec: ModalSpec) -> OverlayFuture<ModalResult>;
    /// Push a dropdown/popover anchored to a rect; resolves on pick or dismiss.
    fn open_dropdown(&self, spec: DropdownSpec) -> OverlayFuture<DropdownResult>;
}

pub struct ModalSpec {
    pub title: String,
    /// The dialog body — a full declarative widget tree (§2.6.2), so a modal can
    /// hold a table/form/list, not just text. `ModalSpec::message(&str)` is a
    /// convenience that wraps a single `Label` in a `body`.
    pub body: ViewNode,
    /// Bottom action buttons. Their id comes back in `ModalResult::Action`.
    pub actions: Vec<ModalAction>,
    pub danger: bool,
    /// `false` = forced decision (Esc/scrim swallowed) — mirrors `Modal::dismissible`.
    pub dismissible: bool,
}
pub struct ModalAction { pub id: String, pub label: String, pub danger: bool }
/// The chosen action id, plus any data the body collected (e.g. a selected row,
/// form field values) marshalled back from the realized widget tree.
pub enum ModalResult { Action { id: String, data: PropMap }, Dismissed }

pub struct DropdownSpec {
    pub anchor: heca_core::layout::Rectangle, // viewport-space anchor (§5.7 geometry)
    pub side: OverlaySide,                     // preferred side; host flips on overflow
    pub items: Vec<DropdownItem>,
}
pub struct DropdownItem { pub id: String, pub label: String, pub enabled: bool }
pub enum DropdownResult { Picked(String), Dismissed }

pub enum OverlaySide { Above, Below, Start, End }
```

**`OverlayFuture<T>` — single-threaded reality.** heca's UI is single-threaded
(`floem_reactive`, `Rc`), so this is **not** a `Send`/`Sync` executor future. It
is a host one-shot handle whose result is delivered on the UI thread. First-party
providers may equivalently pass an `FnOnce(T)` completion; both map to the same
host stack entry. For the WASM bridge (Phase 9) the call marshals as a
`request-id` + a later `resolve` event carrying the result variant — the same
event→read boundary as `App::on` / `App::state`.

**Positioning.** Anchors are in viewport space using `heca-core::layout`
`Rectangle`/`Point`/`Size` (§5.7). The host clamps to the viewport and flips
`side` on overflow — the same behavior `Modal` (centering) and `Select`
(anchoring) already implement, now owned once by the host.

### 2.7.2 Intent / dispatch / overlay-control — decided 2026-07-06

Ratified while building the surface compositor + `ViewNode`; drives `realize`
(plugin-task-ui-3) and `OverlayHost`. **Everything is an action; there is one dispatch.**

- **`view::Intent { action, args }` is the universal invocation currency** — used identically
  by click, the KeyHint picker (`prefix+/`), **RPC**, and plugins. A `ViewNode` node carries
  it via `events` (`on_press`/`on_change`); it holds no closures, so it stays serializable.
- **Convergence carrier:** `InteractionIntent::View(view::Intent)`. `realize` wires each
  actionable node to BOTH `on_click → emit(View(vi))` and a KeyHint target
  `hints.register(View(vi))`, so click + picker fire the same thing; RPC feeds the same
  `dispatch_view_intent` directly. `realize` stays context-agnostic.
- **One dispatch point** `dispatch_view_intent(state, registry, vi)`: resolve `vi.action` to a
  built-in `WmAction` (`action_from_name` + args) or a plugin action, execute via
  `ActionRegistry`. No parallel dispatch path.
- **Overlay control is actions too**, carrying the overlay **id** so any surface can target a
  specific overlay: `WmAction::SubmitOverlay { overlay: OverlayId, action: String }`,
  `WmAction::CloseOverlay { overlay: OverlayId }` (`OverlayId` = the layer's `LayerId`). The
  **`OverlayHost` owns the id** and injects it into each action button when it builds them
  from `ModalSpec.actions` (the author only supplies `ModalAction{id,label,danger}`). The
  `SubmitOverlay` handler resolves that overlay's `OverlayFuture<ModalResult>` (collecting the
  realized body's data into `ModalResult::Action{id,data}`) and pops it.
- **How RPC closes/confirms a modal:** it received the `OverlayId` from `open_modal`, so it
  dispatches `submit_overlay{ overlay: <id>, action }` / `close_overlay{ overlay }` — the same
  action a button press or KeyHint pick fires. Modals are RPC-driven identically to the UI
  (action reachability), targeting a specific overlay by id (not "the top").

So `OverlayHost` is the overlay-level API built **on** the `LayerRegistry` (see §2.7.1 note +
`docs/surface-compositor.md` §9): `open_modal` realizes the `ViewNode` body + injected action
buttons and pushes a `Modal`-band layer; its buttons dispatch overlay-control actions.

---

## 2.8 The design is not sidebar-only; it is chrome-wide

The same pluggable system should power:

- left sidebar
- right sidebar
- top bar
- bottom bar

This means the real target is not a “sidebar plugin API”.

The real target is a:

- **pluggable chrome host system**

with region-specific contribution APIs.

**Every region is a container over an ordered `Vec` of items, not a fixed hand-built
layout.** The left sidebar, right sidebar, top bar, and bottom bar each hold an
ordered list of contributions (buttons / segments / containers) that first-party code
*and plugins* append to and reorder (via `app.regions.<region>.add_container(...)` —
`plugin-task-16`). So any region restyle (`sidebar-fu-9` bottom bar, `sidebar-fu-15`
top bar, the sidebars) must be **built as a vector-of-items container from the start**,
so a plugin adding a button is just a push into that region's list — never a rewrite.

Examples:

- left sidebar may host WorkspacesContainer
- right sidebar may host AgentsContainer
- top bar may host mode/status/tool segments
- bottom bar may host diagnostics, notifications, git info, plugin status

---

## 2.9 Container movement across compatible regions is a host concern

The architecture should support mounted containers being:

- reordered within a region
- moved between compatible regions
- persisted in their chosen placement

Important distinction:

- **host-level container drag/drop** is a ChromeHost concern
- **container-internal drag/drop** remains the mounted container’s concern

Examples:

- moving `WorkspacesContainer` from left sidebar to right sidebar is **host-level placement behavior**
- dragging panes inside `WorkspacesContainer` is **container-internal behavior**

To support this cleanly, mounted containers should expose metadata such as:

- `id`
- `title`
- `supported_regions`
- `default_region`
- `default_order`
- `movable`
- `collapsible`

This movement must not be mouse-only.

Important app-wide action rule:

- when container movement is supported, it must also be representable as an **action**
- for example, moving a container from left sidebar to right sidebar should be doable by:
  - mouse drag/drop
  - keybinding via an action
  - RPC via the same action model

This rule aligns with the broader heca principle that app capabilities should not be trapped behind only one input surface.

---

## 3. Target Architecture

## 3.0 Runtime architecture map (⚠️ KEEP THIS CURRENT)

> **This map is a living document.** Every phase that adds, moves, or renames a
> runtime subsystem **must** update this tree in the same change, and flip its
> `SHIPPED` / `PLANNED(phase)` marker. A stale map is worse than no map — if you
> touch the ownership graph and don't update this, the change is incomplete.
> Verified against code 2026-07-02 (post plugin-02).

Legend: `[✓]` shipped · `[~]` partially shipped · `[ ]` planned (owning phase noted).

```
HecaApp                                   # winit runtime — the outer shell
├── registry: ActionRegistry          [✓] # NOT inside AppState: stateless rules,
├── keymap: KeymapRegistry             [✓] #   handlers run it on `state`
│   └── mode_keymaps / mode_triggers   [✓]
├── app_config, event_proxy            [✓]
└── state: Box<AppState>               [✓] # THE canonical app state
     │
     ├── ── Axis 1: canonical layout/session (the source of truth) ──
     ├── session: Session              [✓] # workspaces → columns → panes
     │    ├── workspaces: Vec<Workspace>    #   position = index (ordinal, not px)
     │    ├── active_workspace_idx          #   which workspace is visible/focused
     │    └── Workspace{ scrolling(columns), floating_panes, focus_domain }
     │         └── Column{ panes, active_pane_idx, width, … }
     │              └── Pane{ id, title, custom_name, runtime, … }
     ├── sidebar_tree: SidebarTree     [✓] # nav model (a projection of session)
     ├── focused_pane: Option<PaneId>  [✓] # cache of the resolved focused pane
     ├── input_mode: InputMode         [✓]
     │
     ├── ── Axis 2: chrome (the pluggable surround that views/drives Axis 1) ──
     ├── chrome_state: SharedChromeState[✓] # signal-backed DERIVED mirror:
     │    │                                 #   active_pane, per-ws collapse, pick,
     │    │                                 #   scroll, PaneRuntime (proc/status/cwd/git)
     │    └── events: ChromeEventBus    [✓] # string-named events + "*" catch-all
     ├── chrome_host: ChromeHost        [~] # SHIPPED runtime (plugin-02), still EMPTY
     │    └── regions: [RegionHost; 4]  [~] #   one generic RegionHost per RegionId
     │         │                            #   (LeftSidebar/RightSidebar/TopBar/BottomBar)
     │         └── MountedContribution  [ ] #   mounted providers — plugin-03
     │              └── (built via Provider::build_contribution → grid-ui subtree)
     └── renderers / backends / theme   [✓]

App facade (app.on / app.state)         [✓] # built FROM chrome_state (AppState::host());
                                            #   the read/observe half of the plugin API

Providers (mounted into chrome_host.regions):
  WorkspacesContainerProvider           [ ] # plugin-03 — first built-in; projects `session`
  Agents / Docker / Git / Notes …       [ ] # later built-ins, then WASM plugins

Planned subsystems (not yet in the tree):
  ActionRegistry (dynamic, string-id)   [ ] # plugin-04 (Phase 6) — beside the enum registry
  OverlayHost (modal/dropdown + async)  [ ] # Phase 8 (§2.7.1)
  WASM plugin runtime + host SDK        [ ] # Phase 9
  AgentDriverRegistry (per-pane agents) [ ] # agent-integration plan (parked)
  Container placement persistence        [ ] # plugin-07 (in-memory today)
```

**Two orthogonal axes — the load-bearing idea.** *Axis 1* (`session`) owns the
canonical layout: which workspaces/columns/panes exist, their ordinal position,
what's visible (`active_workspace_idx`), and focus (hierarchical:
`active_workspace_idx` → `workspace.focus_domain` → `column.active_pane_idx`,
cached in `focused_pane`). It is mutated **only** through actions
(`WmAction → ActionRegistry.execute → handler`). *Axis 2* (chrome:
`chrome_state` + `chrome_host` + event bus) is a **derived mirror + pluggable
surround** that *reads/observes* Axis 1 and *dispatches actions* to change it —
it never owns layout truth. A pane therefore has two faces: its layout position
lives in `session`; its runtime (process/status/cwd/git) is mirrored into
`chrome_state` (`PaneRuntime`) and emitted on the bus so chrome/plugins react
without touching the session. The `WorkspacesContainer` is one Axis-2 *projection*
of Axis 1 — **the sidebar is a shell that hosts it, not the workspace tree itself.**

## 3.1 ChromeHost

A new host/controller layer should own all pluggable chrome regions.

Responsibilities:

- maintain registries for all regions
- track region ordering and visibility
- track container placement within and across regions
- collect container contributions
- mount/unmount built-in or plugin-provided containers
- own host-level container move/reorder behavior
- own valid drop-target logic for container placement
- handle invalidation / refresh scheduling
- bridge plugins with app state and action system

Subregions conceptually:

- `LeftSidebarHost`
- `RightSidebarHost`
- `TopBarHost`
- `BottomBarHost`

These may be implementations of one generic region host abstraction.

### 3.1.1 Formal contract (`plugin-task-01`, proposed 2026-07-02 — pending review)

**Decision — canonical region identity: `RegionId`.** Today the only region
identifier in the app is the **event-payload** enum
`chrome::events::ChromeRegion { Left, Right }` (used by `RegionModeChanged` /
`RegionSizeChanged`), and `SharedChromeState` exposes only `left_*` / `right_*`
region reads/writes. The contract widens this to the four canonical regions:

```rust
pub enum RegionId { LeftSidebar, RightSidebar, TopBar, BottomBar }
```

- **In `plugin-02`**, rename the event enum `ChromeRegion` → `RegionId`, add
  `TopBar`/`BottomBar`, and generalize the `SharedChromeState` region API from
  `left_*`/`right_*` pairs to a per-`RegionId` map. The two event variants carry
  `RegionId` unchanged in shape.
- The **grid-ui `ChromeRegion` widget keeps its name** — it is the *oriented
  shell*, not a region identity. One widget instance is mounted per `RegionId`:
  `ChromeRegion::vertical()` for the two sidebars, `ChromeRegion::horizontal()`
  for the two bars. `RegionId` says *which* region; the widget says *how it
  renders*.

**Decision — contribution taxonomy + per-region allow-list.** A contribution is
one of five semantic units (never raw pixels):

```rust
pub enum Contribution {
    Container(ContainerContribution),   // mounted, movable domain container
    ToolbarGroup(ToolbarGroup),         // clustered action buttons
    StatusSegment(StatusSegment),       // text/badge segment
    Panel(PanelContribution),           // fixed, non-movable panel
    OverlayRequest(OverlaySpec),        // ModalSpec | DropdownSpec → OverlayHost (§2.7.1)
}

/// The two overlay specs from §2.7.1, unified for the `OverlayRequest` variant.
pub enum OverlaySpec { Modal(ModalSpec), Dropdown(DropdownSpec) }
```

Allowed per region:

| RegionId                      | Allowed contributions                 |
| ----------------------------- | ------------------------------------- |
| `LeftSidebar` / `RightSidebar`| `Container` (primary), `Panel`        |
| `TopBar` / `BottomBar`        | `StatusSegment`, `ToolbarGroup`       |
| any                           | `OverlayRequest` (region-agnostic → OverlayHost) |

The **`Container`** carries all host-level placement metadata plus a build hook:

```rust
pub struct ContainerContribution {
    pub id: ContainerId,              // stable string id (== provider id)
    pub title: String,
    pub supported_regions: RegionSet, // which RegionIds it may live in
    pub default_region: RegionId,
    pub default_order: i32,           // stacking order within a region (lower = earlier)
    pub movable: bool,
    pub collapsible: bool,
    /// Builds the container body as a host-understood grid-ui subtree. Called by
    /// the region host on (re)mount / invalidation. Returns a *model*; the host
    /// owns render/focus/clip/overlays (§2.6).
    pub build: Box<dyn Fn(&ChromeCtx<'_>, &mut BuildCx<'_>) -> Box<dyn heca_grid_ui::Component>>,
}
```

**The two halves of the seam (settled, plugin-03 `t005`).** Building a body is not a
pure read: it *allocates host ids* — a drag id per draggable/droppable row, a hint
target id per pickable row, a signal per value that updates without a structural
rebuild. So the seam takes two contexts, and the split is the point:

- **`ChromeCtx<'a>` — read-only.** The plugin-facing facade: state selectors
  (`app.state.*`), event subscriptions, plus the frame's render inputs — `tree()`,
  `programs()`, `theme()`, `emit_intent()`. These are `Option`: a context built to
  *observe* (`ChromeCtx::new`, e.g. at `on_activate`) has no frame in flight, so there
  is no theme or tree to read; a context built for a render pass
  (`ChromeCtx::for_build`) has them all.
- **`BuildCx<'a>` — the mutable half.** The host's per-build registries (`signals`,
  `drag`, `hints`), borrowed `&mut` for the duration of one build.

They are two explicit parameters rather than one context with interior mutability:
that keeps `ChromeCtx` a pure read/observe facade (what the WASM bridge will marshal),
turns a double borrow into a **compile** error instead of a runtime panic, and matches
how `realize()` already threads the same registries. A WASM provider never sees
`BuildCx` — it returns a `ViewNode`, and the host's adapter realizes it, registering the
ids on its behalf.

**ChromeHost responsibilities** (app-side, `heca/src/chrome/host.rs`, `plugin-02`;
bridges to the shipped `App` facade in `heca/src/host.rs` for state + events):

- own a registry of mounted contributions **per `RegionId`**, with order + visibility;
- track container placement (which `RegionId`, which order) and **persist** it;
- own **host-level** container move/reorder between compatible regions, validated
  against `supported_regions` — distinct from **container-internal** DnD, which
  stays inside the mounted container (§2.9);
- compute host-level drop targets for container DnD;
- schedule **invalidation** — re-call a provider's `build_contribution` when the
  events it subscribed to fire (bridged from the `ChromeEvent` bus via `App::on`);
- bridge event dispatch to providers.

```rust
pub struct ChromeHost { /* per-RegionId registries, placement map, App bridge */ }
impl ChromeHost {
    pub fn register(&mut self, provider: Box<dyn Provider>);
    pub fn contributions(&self, region: RegionId) -> &[MountedContribution];
    pub fn move_container(&mut self, id: ContainerId, to: RegionId) -> Result<(), MoveError>;
    pub fn reorder(&mut self, id: ContainerId, before: Option<ContainerId>);
    pub fn set_region_visible(&mut self, region: RegionId, visible: bool);
}
```

**Geometry rule (§5.7).** Every rect/point/size in ChromeHost / provider /
overlay APIs uses `heca-core/src/layout/types.rs` `Rectangle` / `Point` / `Size`.
The legacy `heca_core::types::Rect` must not appear in any chrome-facing API
(cleanup is `plugin-task-04`).

**Movement-as-action rule (§2.9).** Every host-level placement mutation
(`move_container`, `reorder`, `set_region_visible`) must also be reachable as a
named `WmAction` (`plugin-task-08`) so mouse, keyboard, and RPC hit the same
path. The methods above are the internal API; the actions are the public surface.

---

## 3.2 Region contributions

A contribution should not mean “raw pixels”.

A contribution should be one of a small set of semantic units, such as:

- container
- toolbar group
- status segment
- panel
- overlay request

For sidebar regions, the most important contribution type is:

- a **mounted container contribution** hosted inside the Sidebar shell

Important separation:

- the Sidebar shell provides visual/layout hosting behavior
- the mounted container provides domain-specific interaction behavior

Examples:

- `WorkspacesContainer` owns workspace-tree semantics
- `AgentsContainer` owns agent-list semantics
- `DockerContainer` owns docker-list semantics

A future `SidebarContainerFrame` widget may be useful as a visual wrapper around mounted containers, but it should not be confused with the provider/container logic itself.

Important convergence note with terminal work:

- the real terminal implementation should eventually mount here as a hosted
  content provider inside a pane shell / ChromeHost-managed container boundary
- the terminal backend/renderer must not become a separate competing pane
  architecture
- the pane shell / ChromeHost layer should own:
  - outer chrome
  - region placement
  - content rect and clipping
  - process/global metadata presentation
- the terminal host should own:
  - PTY/backend/runtime state
  - terminal snapshots
  - terminal content rendering
  - terminal input routing
- this convergence is tracked in `terminal-implementation.md` Phase 8

---

## 3.3 Shared UI / chrome state

> **Foundation landed** (pane-runtime initiative Phases 0–1): `SharedChromeState`
> (`heca/src/chrome/state.rs`) is the shared, signal-backed store — region
> visibility/size, active/hovered pane, per-workspace collapse, pick candidates,
> scroll, and the per-pane runtime mirror. Reads via selectors; writes via the
> store's `set_*` chokepoint (which emits events). Providers read it through the
> host API's `app.state.*` (§3.5), not directly.

A new shared state layer will be required to coordinate:

- scrolling area
- workspace tree / WorkspacesContainer
- future additional containers
- overlay state
- focus/selection
- per-container UI state such as search/filter query or collapse state

This state must live outside the widgets.

Examples of likely shared UI state:

- expanded/collapsed container ids
- selected row ids
- hovered row ids
- drag state
- per-container search queries
- container order
- container placement by region
- region visibility
- scroll offsets
- active overlay stack

> **Concrete bug this state must solve (2026-06-22):** today the expanded
> sidebar / WorkspacesContainer highlights only `active_pane`, while sidebar
> navigation mutates `AppState.sidebar_tree.cursor/current_item()`. Result:
> `prefix+e` → `j/k` moves the nav model internally, but **nothing visibly
> changes** in the expanded sidebar because the selected row/item is not
> projected into shared chrome state. This is the canonical example of why
> `selected row ids` / `focus-selection` must live in shared UI state rather
> than inside ad hoc widget-local or module-local structures.

---

## 3.4 Provider model

Built-in first, plugin-driven later.

Conceptually each provider should:

- identify itself
- declare which region(s) it supports
- provide container placement metadata
- subscribe to host/app events
- read state through the host API
- build **container contribution models** / UI models
- register actions
- respond to action invocations

A provider should not be thought of as “providing sidebar rows”. It provides a mounted container contribution.

The first provider should be:

- `WorkspacesContainerProvider`

The current sidebar code should be gradually migrated into that shape.

### 3.4.1 Formal contract (`plugin-task-02`, proposed 2026-07-02 — pending review)

**The `Provider` trait.** A provider never mutates app state directly (§2.3): it
reads through `ChromeCtx` selectors, reacts to events, and dispatches actions.

```rust
/// A built-in (later WASM-backed) contributor of chrome content.
pub trait Provider {
    /// Stable identity — also the `ContainerId` when it contributes a container.
    fn id(&self) -> &str;
    /// Regions this provider's contribution may be placed in.
    fn supported_regions(&self) -> RegionSet;
    /// Where it mounts by default on first run.
    fn default_region(&self) -> RegionId;
    /// Default stacking order within a region (lower = earlier).
    fn default_order(&self) -> i32 { 0 }
    /// Human title (rail/tab label, move menu).
    fn title(&self) -> &str;
    /// Host-level move/reorder allowed?
    fn movable(&self) -> bool { true }
    /// Collapsible within its region shell?
    fn collapsible(&self) -> bool { true }
    /// Build the contribution model. Called on mount and on each invalidation.
    fn build_contribution(&self, ctx: &ChromeCtx<'_>) -> Contribution;
    /// Subscribe to events / register actions on activation. The returned RAII
    /// handles are held by the host while the provider is mounted, and dropped
    /// (unsubscribing) on unmount.
    fn on_activate(&mut self, _ctx: &ChromeCtx<'_>) -> ProviderHandles {
        ProviderHandles::default()
    }
}
```

**`ChromeCtx` — the provider/plugin-facing facade.** It *extends* the shipped
read/observe `App` (`heca/src/host.rs`, which already gives `on(event)` +
`state()` selectors) with the write/contribute halves that §3.5 rows 3–10 defer.
`plugin-01` only names them; they are implemented in later phases.

```rust
pub struct ChromeCtx {
    app: App,                 // SHIPPED: on(event) + state() read selectors (§3.5 rows 1–2)
    // actions: ActionDispatch,   // dispatch/register string actions   (plugin-04/05)
    // overlay: OverlayHandle,    // open_modal/open_dropdown (§2.7.1)   (plugin-05/Phase 8)
    // regions: RegionHandle,     // add/move containers                 (plugin-05)
}
```

**Lifecycle (state machine).**

1. **register** — `ChromeHost::register(Box<dyn Provider>)` records it and reads
   its placement metadata (`supported_regions` / `default_region` / `default_order`).
2. **on_activate** — provider subscribes to events (`ctx.on(...)`) and registers
   actions; returns `ProviderHandles` the host keeps alive.
3. **build_contribution** — host calls it, receives a `Contribution` *model*, and
   mounts the mapped grid-ui subtree into the region shell.
4. **react** — on a subscribed `ChromeEvent`, the provider marks itself dirty; the
   host **invalidates** and re-calls `build_contribution`.
5. **move / reorder** — host updates placement (validated vs `supported_regions`);
   the contribution is remounted in its new slot.
6. **unmount** — host drops the provider's `ProviderHandles`, unsubscribing events
   and unregistering actions.

**Rules.** `build_contribution` returns a *model*, never widget references held
across rebuilds — the host owns render/focus/clip/overlays (§2.6). The first
concrete provider is **`WorkspacesContainerProvider`** (`plugin-task-10`),
migrating `heca/src/sidebar/` into this shape; its container-internal DnD stays
inside the container (§2.9), and its sidebar-nav selection projects into shared
chrome state (`plugin-task-10a`).

---

## 3.5 Future WASM plugin host API

The host should expose a controlled plugin API that supports:

- `app.on(event, handler)`
- `app.state.*` read accessors/selectors
- `app.actions.register(...)`
- `app.actions.dispatch(...)`
- `app.overlay.openModal(...)`
- `app.overlay.openDropdown(...)`
- `app.regions.leftSidebar.addContainer(...)`
- `app.regions.rightSidebar.addContainer(...)`
- `app.regions.topBar.addContainer(...)`
- `app.regions.bottomBar.addContainer(...)`
- `app.regions.moveContainer(containerId, targetRegion, options?)`

This is the eventual developer-facing contract.

> **Foundation landed (pane-runtime initiative Phase 8, 2026-06-21).** The first two
> rows — `app.on(event, handler)` and `app.state.*` read selectors — are implemented
> first-party in `heca/src/host.rs` (`App::on` / `App::state()`), over the Phase 0
> event bus + reactive store. `App` is a cheap clone of `SharedChromeState`;
> `state.host()` hands one out. Plugins/providers **react via events and read via
> selectors — never the internal `floem_reactive` signals** — which is exactly the
> boundary the WASM bridge (Phase 9) will marshal. `app.actions.*`, `app.overlay.*`,
> and `app.regions.*` remain future phases.

---

## 4. Phase Plan

The phases below are ordered deliberately. The dependency structure matters.

---

## Phase 0 — Finish the Current Refactor Cleanly

**Purpose**

This phase is not the new architecture itself. It exists to complete the structural cleanup already in motion, because the current refactor is preparing the seams we will need.

**What this phase is for**

- finish the structure-first refactor (completed — all 10 phases done, 279 tests passing)
- complete the `sidebar.rs` split and associated cleanup to a stable stopping point
- avoid switching architectural direction while large files are still only half-separated
- get the codebase into a condition where the new chrome/plugin design can be introduced deliberately instead of on top of chaos

**Where it sits in the process**

This phase happens **before any of the new chrome/plugin phases below**.

**Dependencies**

- none; this is the precondition for the new program

**Exit criteria**

- current refactor plan has reached its intended stopping point
- large in-flight files are split enough to expose clean seams
- no half-migrated state between old and new sidebar assumptions

---

## Phase 1 — Architecture/Contract Phase

**Purpose**

Design the target system before implementing it.

**What this phase is for**

This phase exists to prevent the team from forgetting requirements or accidentally coding the wrong abstraction. It captures the formal architecture for the next program.

It must define, in detail:

- chrome regions and what each region is allowed to host
- difference between sidebar shell and sidebar containers
- shared UI/chrome state boundaries
- built-in provider boundaries
- plugin/provider lifecycle
- dynamic action registration contract
- overlay ownership rules
- host-side widget / semantic contribution model
- canonical geometry types for chrome/container APIs

Geometry rule for the future architecture:

- new chrome/container/overlay contracts should use the logical-pixel geometry types from `heca-core/src/layout/types.rs`
- prefer `Rectangle` / `Point` / `Size`
- do not carry the old legacy `heca_core::types::Rect` forward into new ChromeHost/provider/plugin-facing APIs

**Where it sits in the process**

Immediately after the current refactor ends, and before implementation of the chrome host.

**Dependencies**

- Phase 0 complete enough to provide clean structural seams

**Deliverables**

- a final written architecture/spec for:
  - ChromeHost
  - region contribution APIs
  - shared UI/chrome state
  - provider model
  - dynamic action system evolution
  - future WASM host SDK

**Why this phase matters**

Without this phase, we risk implementing:

- a sidebar-specific system instead of a chrome-wide system
- static action assumptions that later block plugin actions
- widget ownership mistakes
- overlay APIs that cannot return meaningful results

---

## Phase 2 — Shared UI/Chrome State Layer

**Purpose**

Introduce the shared app-side UI/chrome state that widgets and providers will consume.

**What this phase is for**

This phase creates the state boundary that separates:

- canonical layout/runtime state
- derived UI/chrome state
- provider-local state

Examples of likely responsibilities:

- per-region visibility
- per-container collapse state
- per-container search/filter state
- selected/hovered row state
- drag state
- overlay state
- scroll positions

**Where it sits in the process**

After the architecture/spec phase, before providerization of the current sidebar.

**Dependencies**

- Phase 1 architecture/contracts defined

**Why this phase matters**

Today too much behavior is embedded inside sidebar/workspace-specific code. This phase introduces the shared coordination layer needed for:

- sidebars to reflect scrolling/focus state correctly
- multiple containers to coexist
- plugins to read coherent UI state safely

---

## Phase 3 — ChromeHost + Region Host Introduction

**Purpose**

Create the host/runtime that manages pluggable chrome regions.

**What this phase is for**

This phase introduces the infrastructure that will own:

- left sidebar contributions
- right sidebar contributions
- top bar contributions
- bottom bar contributions

Responsibilities include:

- container registration
- ordering
- visibility/mounting
- placement persistence
- host-level reorder/move between compatible regions
- drag/drop targets for mounted containers
- invalidation scheduling
- rendering orchestration handoff
- event dispatch bridge to providers

**Where it sits in the process**

After shared UI/chrome state exists, before dynamic plugins.

**Dependencies**

- Phase 1 contracts
- Phase 2 shared state layer

**Why this phase matters**

This is where the app stops being hardcoded chrome and becomes a host for containers.

---

## Phase 4 — Built-in Provider System

**Purpose**

Implement the provider abstraction using only built-in first-party providers first.

**What this phase is for**

Before loading external plugins, we should prove the provider model internally.

The first built-in provider should be:

- `WorkspacesContainerProvider`

Potential later built-in providers:

- right sidebar placeholders
- top bar status/mode provider
- bottom bar diagnostics/status provider

This phase should also validate provider/container metadata such as:

- supported regions
- default region
- movable/collapsible flags

**Where it sits in the process**

After ChromeHost exists, before WASM runtime/plugin scanning.

**Dependencies**

- Phase 2 shared state layer
- Phase 3 ChromeHost/region hosts

**Why this phase matters**

It lets us validate:

- whether the provider interface is correct
- whether region host orchestration is good enough
- whether `WorkspacesContainer` truly fits as “one provider” rather than “the sidebar itself”

---

## Phase 5 — WorkspacesContainer Migration

**Purpose**

Move the current sidebar/workspace tree behavior into the new built-in provider shape.

**What this phase is for**

This phase reinterprets the current sidebar logic as:

- one mounted container contribution
- one built-in provider
- one consumer of shared UI state

This includes:

- workspace/column/pane projection
- floating-pane representation rules
- expand/collapse behavior
- per-container search if enabled
- current row/item rendering mapped to the new widget layer
- drag/drop semantics staying inside `WorkspacesContainer` rather than being attributed to the Sidebar shell

**Where it sits in the process**

After the provider model exists.

**Dependencies**

- Phase 4 built-in provider system

**Why this phase matters**

It is the bridge between the old sidebar world and the new pluggable chrome architecture.

---

## Phase 6 — Dynamic Action Registry Evolution

**Purpose**

Make the action system capable of hosting plugin actions and later config-bindable dynamic actions.

**What this phase is for**

This phase updates the action architecture so that actions can be:

- built-in
- dynamically registered by providers/plugins
- bound by string id from config
- invoked from UI, keybindings, command palette, or RPC

Expected changes:

- stable string-based action ids
- action metadata descriptors
- dynamic registration/unregistration
- dispatch with structured args
- compatibility path for existing built-in `WmAction` behavior
- explicit support for actions that are invokable from UI, keybindings, and RPC

Examples of important host-level actions in this family:

- `chrome.container.move_to_region`
- `chrome.container.move_left_sidebar`
- `chrome.container.move_right_sidebar`
- `chrome.container.reorder_before`
- `chrome.container.reorder_after`

**Where it sits in the process**

After built-in providers start to exist, but before WASM plugins.

**Dependencies**

- Phase 1 contract work
- ideally Phase 4 provider model already proven

**Why this phase matters**

Without this, plugins cannot cleanly integrate with:

- keybindings
- command palette
- future external automation

---

## Phase 7 — `heca-grid-ui` Chrome Widget Expansion

**Purpose**

Add or evolve widgets needed by the new chrome host and provider architecture.

**What this phase is for**

The current grid-ui primitives are not enough yet for the full pluggable chrome system.

Likely needed widgets/components:

- `Sidebar` (shell only)
- `SidebarContainerFrame` (optional visual wrapper for mounted containers)
- `SidebarItem`
- `SidebarItemGroup`
- richer inline action button/support widgets
- future right/top/bottom region primitives
- possible scroll/list container primitives

Important separation rule:

- `Sidebar` is a shell widget, not a workspace-tree widget
- `WorkspacesContainer` is a mounted container/component, not a sidebar item
- `SidebarItem` is a reusable row primitive that a container may choose to use internally

Important rule:

- widgets remain presentation components
- canonical state stays outside them

**Where it sits in the process**

Can begin alongside built-in provider work, but must follow the architecture decisions.

**Dependencies**

- Phase 1 contracts
- ideally Phase 3/4 so widget requirements are grounded in real provider needs

**Why this phase matters**

This is where the UI vocabulary catches up to the architecture.

---

## Phase 7.5 — Shell Compositing Effects (Transparency / Blur)

**Purpose**

Introduce compositor-owned visual effects for pane shells and chrome regions
without coupling them to terminal rendering internals or to any single pane
implementation.

**What this phase is for**

This phase defines and implements the rendering/compositing layer needed for:

- translucent chrome surfaces
- translucent pane shells
- blur behind floating panes
- blur/translucency for the broader app shell where appropriate
- clip-aware composition so effects respect pane/container/overlay bounds

Important scope rule:

- transparency/blur is a **host/compositor concern**
- terminal, WorkspacesContainer, and future providers should not each invent
  their own blur logic
- mounted content should render into host-provided bounds; the shell/compositor
  decides whether to apply opacity, backdrop capture, or blur

Important future terminal convergence rule:

- floating terminal panes should gain transparency/blur by being mounted inside
  pane shells that support those effects
- the terminal host itself should not become responsible for backdrop blur

Likely implementation areas:

- renderer support for offscreen surfaces or captured backdrop textures
- host-managed blur passes in `heca-renderer`
- effect policies on pane shells / region shells
- clipping/scissor integration with ChromeHost and overlay hosts
- theme/config tokens for shell opacity, blur radius, and effect enablement

**Where it sits in the process**

After ChromeHost and pane/container hosting boundaries are established enough
that shell-level effects can be applied once in the right place.

**Dependencies**

- Phase 3 ChromeHost + region hosts
- Phase 4 built-in provider/container model
- Phase 7 `heca-grid-ui` chrome widget expansion
- ideally terminal/pane hosting convergence is already structurally in place

**Why this phase matters**

If implemented too early, blur/transparency would likely be tied to the
current pane host and need to be reworked during the pane-shell migration. At
this stage, the effect system can be attached to the long-term shell boundary
and reused by:

- floating panes
- region shells
- overlays/modals/dropdowns
- the broader application shell

---

## Phase 8 — Overlay / Modal / Dropdown Host APIs

**Purpose**

Create host-owned overlay APIs that providers and future plugins can use safely.

**What this phase is for**

This phase defines and implements:

- modal host APIs
- dropdown/popover host APIs
- focus trap behavior
- result-returning overlay calls
- provider/plugin-friendly async overlay flow

Example target usage:

- provider/plugin opens modal
- waits for result
- dispatches action based on selected button

**Where it sits in the process**

After action/provider basics are solid; before WASM plugins become interactive enough to depend on overlays.

**Dependencies**

- Phase 3 ChromeHost
- Phase 6 dynamic actions
- Phase 7 widget support

**Why this phase matters**

This solves the problem that JSON-only or fire-and-forget triggers could not solve well: real interactive result-driven GUI flows.

---

## Phase 8.1 — Placeholder variables (Formats/Tokens)

> TO BE ANALYZED LATER.

> **Deferred customization shape (recorded by pane-runtime Phase 8).** The pane-info
> bar customization was split: the **segment-list selection** already shipped as plain
> config — `[appearance] pane_title_segments` / `pane_title_actions`, ordered lists of
> known kinds (`location`/`app_name`/`git_branch`/`git_status`; `split`/`move_left`/
> `move_right`/`close`). What remains deferred to **this** phase is the **`${token}`
> templating** (the tmux-style placeholders below). Agreed model when built = the
> **hybrid**: a user-authored list of **widget-typed segments**, each carrying a
> `${token}` template string — so a segment is both a typed widget *and* a format
> string, not one or the other. The Phase 7 fixed defaults become the default segment
> list once templating ships.

> ALLOWS EXTEND KEYBINDING WITH TOKENS LIKE $paneIndex and so on for propper RPC usage

**Purpose**
Add a way to create placeholder variables for plugins. like tmux does

syntax: `#{var}` or `${var}` or `%{var}` or `#{@$var}`

`${paneIndex}` should be parsed as the current pane index number
`${prevPanesIndex}` should be parsed as the previous pane index
`${paneTitle}`  should be parsed as the current pane title
`${prevPaneTitle}`  should be parsed as the previous pane title
`${panesCount}` should be parsed as the total number of panes
`${paneProgram}` the current pane program
`${paneCwd}` the current pane working directory
`${columnIndex}` the current column index number
`${columnTitle}` the current column title
`${columnsCount}` the total number of columns
`${workspaceTitle}` the current workspace title
`${workspaceIndex}` the current workspace index number
`${workspacesCount}` the total number of workspaces
`${leftSidebarStatus}` whether the left sidebar is open or closed
`${rightSidebarStatus}` whether the right sidebar is open or closed
`${pid}` the current process id

more... (sessions, selections, etc)

---

## Phase 8.2 — Simple Plugins from config.toml

> TO BE ANALYZED LATER

**Purpose**
Add a way to create simpole plugins from config.toml
e.g.

```toml
[[plugins.name]]
name="myplugin"
placement="bottomBar"
weight=100
text="current pane: ${paneIndes}"

```

---

## Phase 9 — WASM Plugin Runtime

**Purpose**

Add code-based external plugins using WASM as the plugin format.

**What this phase is for**

This phase adds:

- plugin discovery/scanning
- WASM module loading
- plugin lifecycle (`activate`, teardown, reload if desired)
- event bus bridge
- host SDK/facade exposure
- region contribution APIs
- action registration APIs
- overlay APIs for plugins

**Where it sits in the process**

After built-in providers, dynamic actions, and overlay system are already working.

**Dependencies**

- Phase 3 ChromeHost
- Phase 4 built-in providers
- Phase 6 dynamic actions
- Phase 8 overlays

**Why this phase matters**

It introduces external extensibility without locking the app to native ABI instability or direct in-process unsafe plugin boundaries.

---

## Phase 10 — Multi-Region First-Party Proof Plugins/Providers

**Purpose**

Prove the architecture with more than the WorkspacesContainer.

**What this phase is for**

This phase validates that the system is not secretly hardcoded around workspaces.

Examples:

- add a second real built-in provider in right sidebar or bottom bar
- add a simple first-party plugin/provider using the public-style API
- prove ordering, coexistence, collapse state, region contribution, action registration

**Where it sits in the process**

After WASM runtime exists or alongside late built-in provider work.

**Dependencies**

- Phase 4 built-in provider system
- Phase 9 WASM runtime for external/provider path validation

**Why this phase matters**

It verifies that the architecture is genuinely general-purpose.

---

## Phase 11 — Config / Keybinding / Command Palette Integration

**Purpose**

Finalize the user-facing integration of dynamic actions.

**What this phase is for**

This phase ensures plugin-provided actions participate fully in:

- config keybindings
- command palette
- RPC
- user-visible action listings and diagnostics

It should also enforce the broader app rule that important actions are not trapped behind only one surface. In particular, host/container placement actions such as moving a container from the left sidebar to the right sidebar must be reachable through:

- mouse interaction
- keybinding/action dispatch
- RPC

It also likely needs:

- action discovery UX
- action id validation
- action conflict diagnostics

**Where it sits in the process**

After dynamic actions and plugin runtime exist.

**Dependencies**

- Phase 6 dynamic action registry
- Phase 9 plugin runtime

**Why this phase matters**

This is what makes the plugin/action system actually usable by end users, not just technically present.

---

## 5. Things That Must Explicitly Change in the Existing Codebase

This architecture implies future changes to at least these areas:

### 5.1 Sidebar assumptions

- current sidebar code must stop being treated as “the sidebar”
- it becomes `WorkspacesContainer` logic

### 5.2 Action system

- current static action routing must evolve toward dynamic registration

### 5.3 Shared UI state

- more state must move out of ad hoc widget/module-local assumptions and into a shared UI/chrome state layer

### 5.4 App/plugin event system

- the app needs a formal event publication/subscription model
- **DONE** (pane-runtime Phase 0 + 8): a typed `ChromeEvent` bus (`heca/src/chrome/events.rs`)
  with string-named events + `"*"` catch-all and RAII subscriptions, emitted from the
  store's mutation chokepoint; exposed first-party as `app.on(event, handler)` in
  `heca/src/host.rs`. WASM bridging is Phase 9.

### 5.5 Overlay ownership

- modals/dropdowns/popovers must be managed by the host, not ad hoc per feature

### 5.6 `heca-grid-ui`

- must expand with richer chrome/container/item primitives
- but should still remain presentation-focused

### 5.7 Geometry unification — **DONE (verified 2026-07-02, `plugin-task-04`)**

- chrome-facing geometry is unified on `heca-core/src/layout/types.rs`
- ~~migrate remaining legacy `heca_core::types::Rect` usage out of chrome-facing code~~ — **already gone**: `heca-core` has no `types` module and no `Rect` geometry type (only `Rectangle` in `layout::types`); nothing in `heca/src` imports a bare `Rect`. `heca/src/chrome.rs` is now the split `heca/src/chrome/`, which uses `Rectangle`/`Point`/`Size`.
- new ChromeHost / provider / overlay APIs use `Rectangle` / `Point` / `Size` as the canonical geometry contract (§3.1.1)

---

## 6. Risks and Guardrails

## Risks

- trying to jump into plugin runtime before the host contracts are designed
- keeping action system too static for plugin actions
- overloading generic widgets instead of introducing the right chrome-specific ones
- letting plugins mutate app state directly
- making the system sidebar-specific instead of chrome-wide
- starting WASM runtime before built-in providers prove the model
- implementing transparency/blur before pane shells and ChromeHost own the
  right compositing boundary

## Guardrails

- finish current refactor first
- define contracts before implementation
- build built-in providers before external WASM plugins
- keep plugins dispatch-only for mutations
- keep overlays host-owned
- keep `heca-grid-ui` presentation-focused
- keep blur/transparency host-owned at the shell/compositor layer, not
  provider-owned

---

## 7. Implementation Checklist

## Precondition

- [x] Complete the current cleanup roadmap (all 10 phases done)

## Architecture

- [x] Write and ratify the formal chrome host + provider + plugin contracts — *`plugin-01`: ChromeHost §3.1.1, Provider §3.4.1, overlay §2.7.1 (2026-07-02)*
- [x] Define chrome regions and allowed contribution types — *`RegionId` (4 regions) + 5-unit `Contribution` taxonomy + per-region allow-list, §3.1.1*
- [x] Define shared UI/chrome state model — *design locked (`F4-chrome-state-design.md`); `SharedChromeState` foundation landed (PR #107)*
- [x] Define provider lifecycle model — *`Provider` trait + 6-step lifecycle + `ChromeCtx`, §3.4.1*
- [x] Define overlay ownership and result-returning API shape — *`OverlayHost` + `open_modal`/`open_dropdown` + `OverlayFuture`, §2.7.1*

## Core runtime

- [ ] Introduce shared UI/chrome state layer — *(partial: `SharedChromeState` foundation landed, PR #107; consumer migration is PLAN.md P0, not done)*
- [x] Introduce ChromeHost and region hosts — *`plugin-02`: `ChromeHost` (4-region array + placement index + moves) + generic `RegionHost` in `heca/src/chrome/host.rs`, wired empty into `AppState`. Runtime only; render is plugin-03.*
- [~] Introduce built-in provider system — *`Provider` trait + `ChromeCtx` + `Contribution` model landed (`plugin-02`); first real provider + app-side registration are `plugin-03`.*
- [ ] Migrate current sidebar/workspace logic into `WorkspacesContainerProvider`

## Actions

- [ ] Evolve ActionRegistry to support dynamic/string-based actions
- [ ] Support action metadata, registration, unregistration, dispatch, and args
- [ ] Ensure future config keybindings can target dynamic actions
- [ ] Ensure important host/container actions are reachable from mouse, keybinding/action dispatch, and RPC
- [ ] Add host-level container placement actions for moving compatible containers between left and right sidebars

## UI/widgets

- [x] Add `SidebarContainerFrame`-style presentation primitives to `heca-grid-ui` — *`DockFrame` (bracket-framed, collapsible, rail-aware container shell)*
- [x] Add richer sidebar item/group widgets as needed — *`Item`, `ItemGroup`, `MarkerGroup`, `RailCell`, `Row`, `KeyHint`*
- [x] Add or generalize region/top/bottom/right-side widgets — *`ChromeRegion` (one oriented shell for all 4 regions)*
- [ ] Add list/scroll primitives if needed — *G7: unblocked (renderer clip landed), not yet built*
- [x] Declarative `ViewNode` widget-tree model (§2.6.2) — **DONE 2026-07-06**, `heca/src/chrome/view.rs` (`plugin-task-ui-1`). Serializable `{kind, props, events, children}`, events `press`/`change`, `PropValue` (scalars + `ViewSize`/`ViewVariant`/`ViewAlign` + color/glyph names), `Intent(action+args)`; **scope widened per user**: `WidgetKind` covers the WHOLE grid-ui vocabulary and the model is **app-wide**; behaviour is **Intent-only** (serializable for native + WASM). JSON round-trip test. *Typed builder SDK (`plugin-task-ui-2`) still open.*
- [ ] Host mapper `realize(ViewNode) -> Box<dyn Component>` (recursive, theme-resolved) — *`plugin-ui`* (**design decided 2026-07-06, see §2.7.1 note; next to implement**)
- [ ] Extend `Modal` to host a `body` child subtree (rich modal content) — *`plugin-ui` / Overlays*
- [ ] `Table` widget (on demand) + showcase/docs — *`plugin-ui`*

## Compositing effects

- [ ] Define shell-level transparency/blur effect contracts — *(partial: window transparency + vibrancy shipped; blur primitive done; the shell-level contract/policy is still informal)*
- [x] Add renderer support for backdrop capture / offscreen compositing where needed — *`Blur` + `Backdrop` (PR #105/#107)*
- [ ] Add blur/translucency support for floating pane shells — *`terminal_blur` open (PLAN.md P4)*
- [ ] Add blur/translucency support for the broader app shell/chrome where appropriate — *app blur-wiring open (PLAN.md P4)*
- [x] Ensure effects remain clip-aware and host-owned rather than terminal/provider-owned — *`PushClip`/`PopClip` (scissor) in the renderer; effects owned by the `Compositor`*

## Overlays

- [x] Add host-owned modal API — *`Modal` widget (host-routed input, overlay layer)*
- [x] Add host-owned dropdown/popover API — *`Select` (overlay-layer dropdown); `Tooltip`, `CommandPalette`, `ToastStack` also host-owned*
- [ ] Support async result-returning overlay flows
- [ ] Overlay `body` accepts a `ViewNode` tree (rich modals: table/form/list) — *`plugin-ui` (§2.7.1)*

## Plugins

- [ ] Design WASM host API/facade
- [ ] Add plugin discovery/loading lifecycle
- [ ] Add event bus bridge to plugins
- [ ] Add region contribution API for plugins
- [ ] Host-adapter `WasmProviderAdapter: Provider` bridging WASM plugins (§2.6.1)
- [ ] Add plugin action registration API
- [ ] Author `docs/plugin-authoring.md` (ViewNode examples) + README pointer — *mark "upcoming" until `plugin-08`*

## Validation

- [ ] Prove architecture with WorkspacesContainer first
- [ ] Prove multi-container coexistence in left/right/top/bottom regions
- [ ] Prove at least one non-workspaces provider/plugin path
- [ ] Prove config-bindable plugin actions
- [ ] Prove a compatible container can be moved left ↔ right by mouse, by action/keybinding, and by RPC

---

## 8. Final Summary

This plan defines the next architecture program for heca **after** the current cleanup/refactor roadmap is complete enough.

The main shift is:

- from a hardcoded sidebar/tree model
- to a **pluggable chrome host** with region contributions, shared UI state, dynamic actions, and future WASM plugins

The first migration target should be:

- turning the current workspace tree/sidebar logic into a built-in `WorkspacesContainerProvider`
- converging the real terminal host with the future pane shell / ChromeHost boundary so terminals become mounted content inside the same pluggable chrome architecture

The long-term goal should be:

- left/right/top/bottom pluggable regions
- host-owned overlays
- dynamic action registration
- code-based WASM plugins using a controlled host SDK
- pane shells that can surface shared process/global metadata such as idle/running/error state, git branch/status, and AI-agent activity without coupling that UI to terminal rendering internals

---

## 9. Next initiative — AI Agent Integration (`agent-integration/agent-integration-plan.md`)

**After this pluggable-chrome/plugin arc is complete**, the next program is **AI Agent
Integration**, planned in [`agent-integration/agent-integration-plan.md`](agent-integration/agent-integration-plan.md)
(orchestration board: [`agent-integration/agent-integration-tasks.md`](agent-integration/agent-integration-tasks.md)).

It gives every heca pane that runs an AI agent (Claude Code, Codex, pi, …) a structured
`AgentStatus` (Working / WaitingForInput / WaitingForPermission / Finished / Error / Compacting /
SubagentRunning) sourced from each agent's own lifecycle hooks, carried over a per-driver transport
(in-band OSC 9 — including Claude Code's `terminalSequence` and Codex's native emission — or an
AF_UNIX side-channel socket for pi) into `PaneRuntime.agent`, mirrored reactively into the chrome
store and emitted as the typed `pane.agent.changed` event on the bus this plan's Phase 2 / pane-runtime
Phase 0 introduce — so plugins (`app.on('pane.agent.changed', data)`) and the pane-info widgets react,
plus transition sounds (rodio). It is generic and pluggable: an `AgentDriver` trait + registry is the
strategy-pattern seam — built-in Rust drivers (Claude Code / Codex / pi) ship as first-party providers,
and the same contract becomes the WASM plugin host contract for third-party agents (Aider, Cursor,
…) once this plan's Phase 9 runtime exists. Research is complete and the design is locked; the
initiative is parked until this pluggable-chrome/plugin arc lands.
