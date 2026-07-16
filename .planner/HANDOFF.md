# Handoff

**Created at:** 2026-07-17
**Reason:** Session end (context ~70%). Clean boundary: everything built this session is user-verified, committed, and pushed.

## Current focus
F003 (🧩 Pluggable Chrome) / P011 (plugin-ui). Branch `feat/action-task-c`, **pushed**, PR **#244** open to main (covers action-task-C, T005, T010, T009, T011, T013).

## What was completed this session (ALL user-verified in the GPU showcase)
- **T009 (plugin-task-ui-7) DONE** — overlay/scroll rework complete: showcase page = root `ScrollRegion::new().both()` (BUG C fixed, commit 431e0d1); overlays hosted in a second tree above the scroll (BUG B fixed); base `Overlay` widget (blocking = a layer property) with Dialog composing it; BUG A (nested Select-in-Dialog) fixed low-level — depth-ordered scene overlay segments + nested-overlay-first routing in Dialog (commit d448702). ScrollRegion also gained: children-first wheel (innermost wins), on_layout re-clamp (zoom-out stale-offset bug), press-and-hold track paging.
- **T011 DONE (commits 7ff3b59 + 319a349)** — glow model: theme owns the COLOR (`theme.glow`), the `glow_size` setting owns the AMOUNT (scaled_glow chokepoint; Thin strength 0.5→0.75), new `interaction.control_rest_glow` token + `PaintCx::rest_glow(radius)` = rest halo on every surface (all bordered Button variants incl. Secondary, Input, Toggle, Checkbox box, Select trigger, Pane, DockFrame, filled Row cards, styled ScrollRegion, Tag, Alert, Toast radius 14 in theme color). Ghost glows with its hover fade. Deliberately flat: Ghost/Link rest, unfilled rows, Choice, Tabs strip, RailCell rest, frameless ScrollRegion, shells, disabled.
- **T013 DONE (in 7ff3b59)** — focus: `Base::shows_focus_ring()` (focused && focus_visible) gates all 13 ring sites → ring only on KEYBOARD nav; `[appearance] show_focus_border` override (chrome_gui_theme, live-reload); Checkbox rings its box only.
- `Component::overlay_occludes` + `overlay_occluded_at` (geometric occlusion ≠ overlay_active input grab) — right-click gate; ContextMenu deliberately does not occlude (re-anchor).
- Docs: widgets.md has consolidated **"The glow model — who owns what"** and **"The focus model — ring visibility"** sections + §Overlay entry + updated §Dialog/§ScrollRegion; overlay-design.md phasing closed.

## Open tasks in P011 (pick next)
1. **T012 — ScrollRegion keyboard-scroll actions** (planned): NEW app actions prefix+Home/End/PgUp/PgDown; OPEN decisions with user: horizontal trigger (prefix+Shift+Arrow is TAKEN by view pan) and target routing (focused/hovered region). Full Adding-New-Actions checklist.
2. **T014 — Overlay follow-ups** (planned): anchor-to-rect mode (build WITH first consumer), convert Select/Tooltip/ContextMenu/CommandPalette panels to compose Overlay, `.panel_size` + scrollable modal body, SDF glyph glow renderer capability (so Icons can halo — user-requested).
3. **T006 (builder SDK), T007 (Table on demand), T008 (author docs)** — earlier planned tasks.

## How to resume
`/planner load`, read this handoff, pick a task (likely T012 or T014), `planner task start`. Verify every visual change interactively: `cargo run -p heca-renderer --example showcase` — the USER drives it; green unit tests are NOT sufficient for rendering changes.

## Process rules re-affirmed this session (do not violate)
- A task awaiting the user's test stays **in-progress**, and the **commit waits** for that verification. Sequence: build → user verifies → task status → commit (memory: no-pr-while-awaiting-test-feedback).
- Ids as Fxxx/Pxxx/Txxx. Planner numbering is authoritative (a past handoff called the glow task "T012"; it is T011).
- Widget docs: BOTH rustdoc + docs/widgets.md, both audiences, code examples, every builder.
- Known perf note (user-accepted, low prio): window resize relayout slightly laggy (two-pass natural-width layout on dirty frames).

## Known open questions
- T012's horizontal-scroll trigger + action→region routing need the user's decision before building.
- Toast z vs palette (toast layer paints above overlays tree since T009 step 2/3 restructure — cosmetic, unreported).