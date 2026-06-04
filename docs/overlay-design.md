# Design proposal — Overlay / popover layer

Status: **proposal, awaiting approval.** Scope: the infrastructure that lets content render
**above** the normal component tree and **capture input** — needed by `Select` (first
consumer), then `Tooltip`, `Modal`/`Dialog`, context menus.

## Requirements

1. **Z-order** — overlay content (a dropdown list) must paint on top of *everything*, including
   widgets that come later in the tree / sit visually beneath it.
2. **Event capture** — while open, the overlay must receive pointer + key events *first*, so an
   option click outside the trigger's layout box reaches the right widget instead of whatever is
   underneath it.
3. **Dismissal** — close on outside click and on `Esc`.
4. **Self-contained** — fit the existing widget model (embed `Base`, no per-widget context handles)
   and stay GPU-free.

## Current architecture (what we have to work with)

- **Paint**: depth-first; later siblings paint over earlier ones (implicit tree z-order). No
  `z_index`. Clipping (`PushClip`/`PopClip`) is **stubbed**, so painting outside bounds already shows
  (nothing clips) — but z-order is still wrong for a popover.
- **Pointer events**: `ui.event(PointerPressed{pos})` routes to children reverse-order, first
  `Handled::Yes` wins; `FocusManager::focus_at(pos)` sets keyboard focus to the hit widget.
- **Keyboard**: `FocusManager::deliver_key(root, key)` → the focused component (by depth-first
  visit index).
- Widgets already self-manage internal layout + hit-testing without child components (see `Tabs`).

## Options considered

**A. Portal / root-owned overlay manager** (React-style). Widgets push an overlay subtree + anchor
to a root `Overlays` stack; the manager paints it last and routes events to the topmost entry.
Fully general (modals, nested popovers, focus trap) — but needs a handle threaded into widgets and
callbacks to push state back, which cuts against the current self-contained widget design. More
plumbing than the first consumer needs.

**B. Widget-owned popover + minimal global hooks** (incremental). The widget owns its popover
content and hit-tests its own expanded region; two small additions make z-order and capture correct:
- a **deferred overlay paint pass** so popover draws land on top;
- an **`overlay_active()` poll** so the event router gives the open widget first dibs.
Smallest correct step; grows into A later if modals/nested popovers demand it.

## Recommendation — Option B

### 1. Deferred overlay paint pass
- `Scene` gains a second command list `overlay` (drawn after `commands`; `enqueue_scene` processes
  base then overlay → always on top).
- `PaintCx` gains `with_overlay(&mut self, f: impl FnOnce(&mut PaintCx))` (or `push/pop_overlay`):
  draws inside it target the overlay list. A widget paints its trigger normally and its open popover
  via `with_overlay`.

### 2. Overlay-aware event routing
- New trait method `Component::overlay_active(&self) -> bool` (default `false`). A widget returns
  `true` while its popover is open.
- `FocusManager` (or a thin routing helper) gains a pointer entry point: if any focusable reports
  `overlay_active()`, deliver pointer events to **that** component first; otherwise fall back to the
  normal `ui.event` routing. Keyboard already targets the focused widget (the open `Select` *is*
  focused), so `deliver_key` is unchanged.
- The open widget decides dismissal itself: outside-press → close (consume), `Esc` → close.

This reuses the visit-index identity + cheap tree-poll pattern `FocusManager` already uses, and adds
**no** per-widget context handles.

### 3. `Select` (first consumer) — self-contained, like `Tabs`
- Holds `options: Vec<String>`, `selected: Signal<usize>`, `open: bool`.
- Trigger paints the current value + a caret; the dropdown is painted in the overlay pass as option
  rows whose rects are computed from `bounds` + row height (no child components).
- `event`: click trigger toggles open; click option selects + closes + emits; click outside / `Esc`
  close; `↑/↓` move highlight, `Enter` commits.
- Emits `Action::value("select-change", SignalData::Usize(index))` via `on_change` — same change-widget
  contract as `Toggle`/`Tabs`. Honors `disabled`, focusable, focus-visible ring.

```rust
Select::new(["LOW", "MEDIUM", "HIGH"])
    .selected(1)
    .on_change(|a| if let SignalData::Usize(i) = a.data { set_level(i); });
```

### Host integration (showcase + future heca)
One change: route pointer events through the overlay-aware path, and append the overlay scene when
rendering. ~5 lines in the event loop / render.

## Phasing
1. Paint pass + `overlay_active` routing + `Select` (single-select). ← this task
2. `Tooltip` (hover-anchored, non-focstealing) reusing the paint pass.
3. `Modal`/`Dialog` — promote to Option A's manager (scrim + focus trap) when needed.

## Open questions (need a decision before building)
1. **Outside-click**: close *and consume* the click (recommended), or also pass it through to the
   widget beneath?
2. **First cut**: single-select `Select` only (recommended), or multi-select too?
3. **Modal scrim / focus-trap**: defer to phase 3 (recommended), or need it now?
4. **Positioning**: dropdown always below the trigger first (recommended), flip-up-near-edge later?
