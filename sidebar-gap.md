# Sidebar Shell vs WorkspacesContainer Gap Analysis

**Date:** 2026-06-05  
**Updated after architecture discussion:** pluggable chrome host, built-in containers, future WASM plugins

## Scope

Comparison between:
- the current workspace-oriented sidebar implementation in:
  - `heca/src/sidebar.rs`
  - `heca/src/sidebar/model.rs`
  - `heca/src/sidebar/hit_test.rs`
  - `heca/src/mouse/sidebar.rs`
  - `heca/src/mouse/sidebar_drop.rs`
- the current `heca-grid-ui` widget set, especially:
  - `Pane`
  - `Item`
  - foundational widgets and focus/event support

## Purpose

This note exists to answer a narrower question than the new full chrome/plugin architecture plan:

- how far is `heca-grid-ui` from being usable for the current sidebar/workspace tree UI?
- what gaps belong to the **Sidebar shell** itself?
- what gaps actually belong to the future built-in **WorkspacesContainer**?
- how much effort would it take to use it now?

This document must now be read with one important architectural correction:

> the current sidebar should no longer be treated as “the sidebar” in the long-term design.
> It should be treated as the future built-in **`WorkspacesContainer`** that will live inside a broader pluggable chrome host.
>
> The `Sidebar` in `heca-grid-ui` should be treated as a **visual/layout shell**, not as the owner of workspace-tree behavior.

That broader system is documented in:
- `pluggable-chrome-plugin-plan.md`

---

## Executive Summary

`heca-grid-ui` is **not yet a drop-in replacement** for the current sidebar/workspace tree.

What it has today is mainly:
- strong **visual primitives**
- a good **container shell** (`Pane`)
- a promising **row primitive** (`Item`)
- generic focus / input / layout foundations

What it does **not** yet have is a full `WorkspacesContainer`-level interaction system with:
- tree/group projection from `Session`
- expand/collapse behavior
- collapsed-strip behavior
- row/button hit testing parity
- drag/drop and swap behavior
- workspace/column/pane-specific semantics
- richer row metadata and inline controls

And, importantly, most of those are **not Sidebar-shell gaps** — they are `WorkspacesContainer` gaps.

So:
- **visual / prototype work now:** feasible
- **full WorkspacesContainer replacement now:** not feasible without significant implementation work

The key updated conclusion is:

> `heca-grid-ui` is already useful as a **presentation layer** for a future `WorkspacesContainer`, but it is not yet sufficient as the **complete interaction system** for that container.

---

## Important Architectural Correction

A lot of confusion disappears if we split the problem into two layers:

### Layer A — Sidebar shell
This is a visual/layout widget that:
- provides region framing
- can be toggled on/off
- can be collapsed/expanded
- can expose its display mode to children
- hosts arbitrary mounted containers vertically

### Layer B — Mounted container
This is where domain behavior lives.

Examples:
- `WorkspacesContainer`
- future plugin containers

These containers own their own semantics, such as:
- tree navigation
- expand/collapse of their internal groups
- drag/drop semantics
- row-specific actions
- search/filter if they want it

This means the old habit of saying “the sidebar needs DnD / expand / visited rows / pane semantics” is inaccurate. Those are usually mounted-container concerns, not Sidebar-shell concerns.

Earlier versions of this analysis treated the current sidebar as if it were the final target abstraction.

That is no longer correct.

The new architecture direction is:
- **Sidebar** = pluggable host/region shell
- **WorkspacesContainer** = one built-in container inside that shell
- other containers may later exist in the same region:
  - agents
  - docker
  - git
  - tasks
  - plugin-defined containers

This changes how we interpret the gap:
- most of the current `heca-grid-ui` gaps are not “missing sidebar shell” gaps
- they are “missing **WorkspacesContainer / sidebar-item / container-frame / tree-list behavior**” gaps

So the correct question becomes:

> Is `heca-grid-ui` ready to render and support a first built-in `WorkspacesContainer` inside a future pluggable chrome host?

Answer:
- **visually: partly yes**
- **behaviorally: not yet**

---

## What the Current Workspaces-Oriented Sidebar Already Does

The current sidebar code is not only paint logic; it is a real interaction subsystem.

### 1. Model / Projection

Current sidebar already owns or derives:
- workspace / column / pane / floating-pane tree
- rebuild from live `Session`
- flat navigation list derived from collapse state
- active / visited state tracking
- special handling for floating panes

Files involved:
- `heca/src/sidebar.rs`
- `heca/src/sidebar/model.rs`
- `heca/src/sidebar/hit_test.rs`
- `heca/src/mouse/sidebar.rs`
- `heca/src/mouse/sidebar_drop.rs`

### 2. Navigation Behavior

Current code supports:
- cursor up/down
- collapsed-mode navigation rules
- expand / collapse workspace and column
- cursor clamping
- scroll offset handling

### 3. Mouse Behavior

Current code supports:
- row hit testing
- button hit testing
- non-selectable floating rows
- click routing
- drag-hover highlighting
- drag source highlighting
- sidebar drop targets
- sidebar drag/drop move and swap flows

### 4. Rendering Behavior

Current code supports:
- expanded mode
- collapsed strip mode
- indentation by hierarchy
- inline action buttons
- active / visited coloring
- candidate letters for pane-select / pane-swap
- drag/drop visuals

This is effectively the current built-in `WorkspacesContainer`, even though the code does not yet call it that.

---

## What `heca-grid-ui` Has Today

Observed implemented pieces relevant to a future `WorkspacesContainer`:

### 1. `Pane`

`heca-grid-ui/src/widgets/pane.rs`

Provides:
- a styled shell/container
- bracket-framed panel visuals
- child composition
- row/column layout direction

Useful for:
- a container frame shell
- sidebar/rightbar panel shells
- future top/bottom bar group shells if adapted

### 2. `Item`

`heca-grid-ui/src/widgets/item.rs`

Provides:
- row layout with:
  - optional leading slot
  - label
  - optional trailing slot
- active state
- hover state
- muted label mode
- optional slot borders
- pointer and keyboard activation
- focusability through `FocusManager`

Useful for:
- first-generation row presentation
- menu-like navigation entries
- active current-row visuals

### 3. Generic Widget Foundations

Available foundations include:
- `Flex`
- `Label`
- `StatusDot`
- `Badge`
- `Button`
- `Input`
- `FocusManager`
- `Event`
- `Scroll` event support in the component model

Useful for:
- row adornments
- section labels
- optional per-container search input
- basic focus and keyboard navigation
- future composable container implementation

### 4. Showcase Evidence

`origin/heca-grid-ui:heca-renderer/examples/showcase.rs` already demonstrates a sidebar-like panel made of:
- `Pane`
- `Item`
- `StatusDot`
- `Label`

This proves the **visual direction** is viable.

---

## Sidebar Shell Gaps

These are the gaps that actually belong to the `Sidebar` shell concept in `heca-grid-ui`.

## Gap S1 — No Explicit Sidebar Shell Widget Yet

`Pane` is close visually, but it is still just a generic container.

What a true `Sidebar` shell should likely add:
- explicit sidebar identity/role
- collapsed vs expanded shell mode
- shell-level width/compactness semantics
- a way to let mounted children know whether the shell is collapsed/compact
- a host-friendly place for mounted containers to live top-to-bottom

This is a real shell-level gap.

**Severity:** Medium

---

## Gap S2 — No Sidebar Shell Context / Display-Mode Propagation

A mounted container needs to know whether the shell is:
- expanded
- collapsed
- narrow/compact
- maybe hidden

Today there is no obvious Sidebar-shell-specific context API in `heca-grid-ui` for children to react to shell mode and render less/more information.

This matters because mounted containers may want to change their row presentation based on shell mode.

**Severity:** Medium

---

## Gap S3 — No Generic Mounted-Container Framing Primitive

The future architecture will likely want a reusable visual wrapper for mounted containers, something like:
- `SidebarContainerFrame`

This is not strictly required if containers render themselves fully, but it may be useful for consistent:
- titles
- borders
- collapse affordances
- section spacing

This is a shell/composition gap, not a WorkspacesContainer behavior gap.

**Severity:** Medium-Low

---

## WorkspacesContainer Gaps

These are the gaps that belong to the future built-in `WorkspacesContainer`, not to the Sidebar shell.

## Gap W1 — No Real Container-Frame / WorkspacesContainer Widget Family Yet

## Gap W1 — No Real Container-Frame / WorkspacesContainer Widget Family Yet

There is no implemented family of widgets for:
- container frame
- collapsible container/group headers
- rich workspace tree rows
- hierarchical list/tree behavior

What exists is:
- `Pane`
- `Item`
- supporting widgets

What is missing:
- `SidebarContainerFrame`
- `SidebarItemGroup` / group header behavior
- richer `SidebarItem` / `WorkspacesItem` row semantics
- tree/list-specific behaviors

**Severity:** High

---

## Gap W2 — No Workspaces Projection/View-Model Layer in `heca-grid-ui`

The current workspace-oriented sidebar reflects:
- workspaces
- columns / column-like groups
- panes
- floating panes

`heca-grid-ui` has no built-in projection/view-model layer for this.

This is acceptable architecturally — projection should largely remain app-side — but it means `heca-grid-ui` cannot replace the current logic by itself.

Needed if used now:
- keep projection logic in `heca`
- feed `heca-grid-ui` with a view model
- eventually build a `WorkspacesContainer` that consumes that model

**Severity:** High

---

## Gap W3 — No Collapsed WorkspacesContainer Mode

Current code has a distinct collapsed strip mode with different:
- rendering
- hit testing
- navigation semantics
- visible item mapping

`heca-grid-ui` currently has no equivalent built-in collapsed behavior.

This is one of the biggest parity gaps.

**Severity:** High

---

## Gap W4 — No Drag/Drop System for WorkspacesContainer Semantics

Current code supports:
- drag-hover target highlighting
- drag source highlighting
- move vs swap flows
- detached-pane reinsertion through sidebar targets

`heca-grid-ui` has no built-in container/tree drag/drop abstraction for this.

This is one of the hardest missing pieces.

**Severity:** Very High

---

## Gap W5 — `Item` Is Not Enough for Current Row Interactivity

`Item` is a good primitive, but current rows do more than select.

Current rows include inline per-row actions such as:
- create workspace
- add column / add pane
- delete workspace
- delete column
- close pane

Important observation:
- `Item` handles its own activation
- it is not yet a full row-with-independent-inline-controls abstraction
- embedded trailing widgets are more visual/compositional than a proven replacement for the current per-row action button model

This means the current row behavior likely needs:
- `Item` extension
- or a dedicated `SidebarItem` / `WorkspacesItem` widget
- or app-side custom event routing

**Severity:** High

---

## Gap W6 — No Native “Visited” State Primitive

Current workspace/sidebar rows distinguish:
- active
- visited
- none

`Item` currently provides:
- active
- hover
- muted

Visited-state styling would still need custom logic.

This is not a blocker, but it is a missing piece.

**Severity:** Medium

---

## Gap W7 — No Rich Secondary Metadata Line for Pane Rows

New requirements now suggest that pane rows should eventually support richer secondary metadata, such as:
- cwd short path
- git branch
- program label
- runtime status / activity

The current generic `Item` is effectively a one-line row primitive.

A future `SidebarItem` / `WorkspacesItem` likely needs:
- title line
- subtitle/meta line
- status indicators
- action badges/chips

This is now an important gap because it affects the future shape of rows, not just styling.

**Severity:** Medium-High

---

## Gap W8 — No Floating-Pane Row Semantics

Current code has special rules for floating panes:
- they are shown
- but treated differently for navigation / hit testing / dragging

`heca-grid-ui` has no notion of this. That logic would still need to live in `heca` or in a dedicated `WorkspacesContainer` behavior layer.

**Severity:** Medium

---

## Gap W9 — No Generic Scrollable Tree/List Container Yet

The component model has a `Scroll` event, and `Select` internally uses scrolling behavior, but there is not yet a reusable tree/list scroll container.

Current code already has:
- scroll offset
- visible-line calculations
- expanded/collapsed-specific row mapping

That behavior would need to be reimplemented or generalized.

**Severity:** Medium-High

---

## Gap W10 — No Action/Intent Model at the Widget Layer

Current sidebar/workspace tree is tightly integrated with WM actions and mouse flows.

That is architecturally okay — `heca-grid-ui` should remain presentation-focused — but it means replacing current behavior is not just a rendering swap.

Needed integration work still includes:
- click-to-intent mapping
- keyboard-to-intent mapping
- row action buttons
- drag/drop action dispatch

This becomes even more important under the new long-term architecture, where:
- built-in containers
- and later plugins

must dispatch actions rather than mutating state directly.

**Severity:** Medium

---

## What Is Already Close Enough

These parts look good enough to reuse soon:

### Sidebar Shell Visual Base
- `Pane` is a good base for the eventual `Sidebar` shell

### First-Generation Row Presentation
- `Item` is a good base for early row visuals

### Row Adornments
- `StatusDot`, `Label`, `Badge` are enough for leading/trailing content in early versions

### Focus Foundation
- `FocusManager` is a solid base for keyboard focus handling

### Composition / Layout
- `Flex` composition is adequate for container layouts and row composition

### Search Primitive
- `Input` is likely good enough for per-container search once container behavior exists

So the biggest gap is still **not look-and-feel**.
The biggest gap is **behavioral completeness and richer mounted-container semantics**.

---

## If We Tried to Use It Now

## Scenario A — Sidebar Shell + Visual WorkspacesContainer Prototype Only

Use `heca-grid-ui` now for:
- Sidebar shell visuals
- visual container/frame
- rows
- active row visuals
- simple click activation

Likely keep in `heca`:
- projection/view-model
- action dispatch
- advanced mouse behavior
- collapse logic
- current special cases

Result:
- good visual milestone
- not a full replacement

**Feasibility now:** Yes

---

## Scenario B — Partial Usable WorkspacesContainer

Add enough behavior for:
- expanded mode
- projection/view-model from `Session`
- keyboard navigation
- expand/collapse
- some row interactions
- maybe simple scrolling
- maybe richer title/subtitle rows

Still missing or incomplete:
- full inline button parity
- collapsed strip parity
- drag/drop parity

**Feasibility now:** Yes, but not small

---

## Scenario C — Full WorkspacesContainer Replacement

Required parity would include:
- expanded mode
- collapsed mode
- action buttons
- visited/active states
- candidate letters
- floating-pane special rules
- drag/drop and swap
- mouse and keyboard parity
- richer row metadata if adopted

**Feasibility now:** Not without a substantial build-out phase

---

## Effort Estimate

## 1. Basic Visual Expanded WorkspacesContainer

Includes:
- `Pane` container shell
- `Item` rows
- active row styling
- simple row click activation

**Effort:** Medium  
**Rough estimate:** 2–5 days

---

## 2. “Good Enough for Daily Use” Expanded WorkspacesContainer

Includes:
- expanded mode only
- session projection/view-model
- tree collapse/expand
- keyboard navigation
- simple scroll handling
- action dispatch integration
- maybe limited inline row actions
- maybe richer subtitle/meta line support

**Effort:** Medium-High  
**Rough estimate:** 1–2 weeks

---

## 3. Full Replacement with Behavioral Parity

Includes:
- expanded + collapsed modes
- inline row action buttons
- active + visited + candidate letters
- floating-row special handling
- drag/drop / swap / hover/source parity
- close behavioral parity with current implementation
- richer row metadata if that becomes part of the built-in UX

**Effort:** High  
**Rough estimate:** 2–4 weeks

This estimate depends heavily on whether we:
- extend `Item`
- add dedicated `SidebarContainerFrame` / `SidebarItem` / `SidebarItemGroup` widgets
- keep complex interaction logic in `heca`
- phase metadata support after the first UI migration

---

## Main Architectural Conclusion

`heca-grid-ui` is currently strongest as:
- a **Sidebar-shell and presentation/composition layer**
- not yet as a complete `WorkspacesContainer` interaction system

This suggests a staged migration path is better than a hard cutover.

---

## Recommended Migration Strategy

### Stage 1
Use `heca-grid-ui` for:
- Sidebar shell visuals
- container shell/frame visuals
- row appearance
- typography / adornments / theme consistency

Keep in `heca` / built-in `WorkspacesContainer` logic:
- projection/view-model
- hit testing and navigation semantics
- drag/drop semantics
- action dispatch
- special workspace/pane rules

### Stage 2
Add missing reusable container/row capabilities to `heca-grid-ui`, for example:
- `SidebarContainerFrame`
- richer `SidebarItem`
- `SidebarItemGroup`
- visited-state styling
- scrollable tree/list container
- inline row control support
- optional subtitle/meta line support

### Stage 3
Only replace the current workspace-tree implementation once:
- collapsed mode exists
- row button behavior is solved
- drag/drop and special semantics are handled
- row metadata design is stable enough

---

## Final Assessment

If the question is:

### “Can `heca-grid-ui` be used now to start the new workspace container visually?”
**Yes.**

### “Can it replace the current workspace/tree implementation right now with comparable behavior?”
**No, not without significant implementation effort.**

### “What is the main missing area?”
**Behavioral completeness and richer mounted-container semantics, not Sidebar-shell visuals.**
