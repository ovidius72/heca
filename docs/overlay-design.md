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

---

# T009 rework — reusable scrollable surface + overlays above the scroll (2026-07-15)

Status: **proposal, awaiting approval.** This is the plan for `plugin-task-ui-7` (F003/P011/T009).
Phase 1 above shipped (`with_overlay` + `overlay_active` + `Select`/`Tooltip`/`Dialog`). Three bugs
found while demoing a Select-in-Dialog in the showcase (see the T009 planner entry — BUG A/B/C) share
one theme: **overlays and scroll don't compose.** This section designs the fix.

> **Verification rule for this whole rework:** every step is verified **interactively in the GPU
> showcase** (the user drives it). Rendering/layout/overlay/scroll behaviour is NOT provable by
> headless unit tests — a speculative `Dialog::on_layout` viewport-recenter passed its unit tests and
> still regressed the showcase (off-screen + backdrop-but-no-panel), and was reverted. Unit-test the
> pure math; confirm the visual before moving on.

## The two root problems

1. **No horizontal scroll, and the page scroll is ad-hoc.** The showcase scrolls with a manual
   `offset_tree(dy)` (showcase.rs ~L1740) that shifts every node's bounds — **vertical only**. Content
   wider than the window runs off-screen with no scrollbar (BUG C). `ScrollRegion` (the real embeddable
   viewport) is used for *one demo section*, not the page.
2. **Overlays live *inside* the scrolled content.** The Dialog is mounted as an in-flow `.child(...)`,
   so the page scroll shifts it, and taffy centers it in its slot → off-screen (BUG B). A `Select`
   opened inside the Dialog paints/routes only in local tree order, so the Dialog's later-painted
   buttons land on top and still hover (BUG A).

## Part 1 — a reusable scrollable surface (the user's ask)

**Keep `ScrollRegion` as THE reusable scroll primitive — do not add a parallel widget.** It already
bakes `-offset` into child bounds (bounds === drawn === clickable), clips via `PushClip`, and resets in
`on_layout`. Extend it, don't fork it.

**1a. Two-axis scrolling.** Add horizontal alongside the existing vertical:
- `axes`: `ScrollAxes { Vertical (default), Horizontal, Both }` via builders `.horizontal()` / `.both()`
  (default stays `Vertical` — back-compat, zero change for current callers).
- Offset becomes per-axis (`scroll_x` / `scroll_y` signals, or one `Point` offset); children laid out at
  natural size on **both** axes so content overflows horizontally too; bake `-offset` on both axes.
- A **horizontal scrollbar** (bottom edge) mirroring the vertical thumb (theme-accent grip, wide grab
  lane, `control_radius`). Auto-shown only when that axis overflows.
- Wheel: plain wheel → vertical; **Shift+wheel → horizontal**; a 2-D trackpad delta drives both. Each
  axis clamped to `[0, max]`.

**1b. "Scrollable *surface*" styling (optional, composable).** So a scroll region can *be* a framed
surface (the user's "surface scrollable"), give `ScrollRegion` optional `StyleExt`
(`.background/.border/.radius/.glow`) — same theme-driven tokens every surface reads. A plain
`ScrollRegion::new().both()` stays frameless; `.background(..).border(..)` makes it a scrollable panel.
No new widget, no hardcoded styling.

**1c. Showcase adopts it.** Replace the manual `offset_tree` page-scroll with a single root
`ScrollRegion::new().both()` wrapping the content. Fixes BUG C and dogfoods the primitive. The app can
later reuse the same widget for sidebar/panels.

## Part 2 — host overlays *above* the scroll (fixes BUG A + BUG B)

Overlays must NOT sit inside the scrolled subtree. Host them in a **top-level overlay layer anchored to
the viewport**, above the `ScrollRegion` — the same shape the app already has (`LayerRegistry` / the
`Modal` band). Then:
- a Dialog centers on the real viewport (it is not inside the scroll offset) → **BUG B fixed**;
- a nested `Select`'s open panel composites in the overlay layer **above** the Dialog's buttons, and
  input routes to the top overlay first → **BUG A fixed**.

**Base `Overlay` widget (composition, not inheritance).** Extract the panel/scrim/positioning into a
base `Overlay`; `Dialog` (modal + actions), dropdown/popover, and tooltip become specializations that
*compose* it. **Blocking is a layer property** (modal blocks + scrim; popover is light-dismiss), not a
per-widget reimplementation. Positioning (center-on-viewport / anchor-to-rect, clamped + flipped) lives
once in the base — which is exactly what the failed `Dialog::on_layout` tried to hand-roll in the wrong
coordinate space.

**Occlusion routing.** An overlay container delivers pointer/key to an `overlay_active` descendant
first (mirroring the host's `focus.rs` overlay scan) and stops propagation to siblings occluded by the
descendant's `panel_rect`.

## Phasing (each verified interactively before the next)
1. ✅ `ScrollRegion` two-axis + horizontal scrollbar (+ optional `StyleExt`). Showcase demo section
   proves it. (T010, commit `c84528a`, user-verified.)
2. ✅ *(built 2026-07-16, pending interactive verification)* Showcase whole-page uses
   `ScrollRegion::new().both()` (drop `offset_tree`). Confirms BUG C fixed. Also made the widget's
   wheel routing **children-first** (innermost hovered region wins) so a whole-page region composes
   with embedded ones.
3. ✅ *(built 2026-07-16, pending interactive verification)* Overlay layer hosting in the showcase: a
   second retained tree (`overlays`: Dialog/palette/menu/toasts) laid out at viewport size,
   dispatched before and painted after the page. Confirms BUG B. (BUG A — a Select nested *inside*
   the Dialog — still needs step 4's layer routing.)
4. Base `Overlay` widget refactor: Dialog/dropdown/tooltip compose it; blocking as a layer property.
5. Docs (`docs/widgets.md` ScrollRegion + the new Overlay) + showcase + rustdoc, per the both-audiences
   rule. Read all styling from theme. (ScrollRegion/Dialog entries updated with steps 2–3.)

## Open questions — resolved
- **Q1 — scrollbar visibility:** each bar reserves a clipped gutter (content never sits under a
  thumb); tracks stop short of each other's gutter (clean corner). Decided in T010.
- **Q2 — offset type:** two `Signal<f32>` (`scroll_offset()` / `scroll_offset_x()`) — back-compat.
  Decided in T010.
- **Q3 — overlay host in the showcase:** minimal showcase-side slot (a second retained tree above the
  page scroll), per the lean; the base `Overlay` + host generalization stays Phase 4.
- **Q4 — scope:** Part 1 landed first (T010), Part 2 steps 2+3 together (they are coupled: page
  scroll without overlay hosting re-triggers BUG B).
