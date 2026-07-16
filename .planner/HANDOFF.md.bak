Reason: context limited (~65% used); pausing at a clean boundary after committing T010

# Handoff — 2026-07-16

## Branch `feat/action-task-c` — NOT pushed. Recent commits (newest first):
- `6162a51` chore(planner): T010 done; add T011 + T012
- `c84528a` feat(grid-ui): ScrollRegion two-axis + scrollable surface (F003/P011/T010)
- `70b7292` docs(planner): T009 overlay/scroll rework design + T010 split
- `c152ef7` / `9c92696` — F003/P011/T005 Select modal-form marshalling (done)

## Done this session (F003 = 🧩 Pluggable Chrome, P011 = plugin-ui)
- **T005 (plugin-task-ui-4) DONE**: named Select in a modal body marshals its chosen value into ModalResult::Action.data.
- **T010 DONE + user-verified**: ScrollRegion → two-axis + scrollable surface (StyleExt), horizontal scrollbar, gutter + clean corner, click-in-track paging, Event::Scroll made two-axis (host maps Shift→horizontal). REMOVED the widget's hardcoded keys + tab-stop focusability (tmux model).

## NEXT (user-chosen priority order)
1. **T012 — GLOW REGRESSION** (do next). Broad: user lists NO rest-glow on separator, grid, Inputs, Buttons, Pane frame-variants, Pane info-bar, ScrollRegion (both), EXPLORER container, PANES container, drop indicator — glow now only on FOCUSED elements. Almost certainly a GLOBAL rest-glow loss, not per-widget. Leads in the T012 description: scaled_glow (component.rs ~1093) scales by theme.colors.glow_size; showcase glow select works for OLD widgets; refactored widgets likely stopped passing a rest-state glow. git-diff refactored widget paint() glow usage vs pre-refactor. VERIFY in GPU showcase (glow select scales all widgets at rest).
2. **T011 — app scroll actions** (prefix-gated, configurable, RPC) — gated on ScrollRegion being mounted in the app (sidebar/panels), so later.
3. **T009 (plugin-task-ui-7) BUG A/B/C** — the overlay/scroll rework: overlays hosted ABOVE the scroll (fixes nested-Select z/occlusion + Dialog-off-screen), base Overlay widget, and the showcase whole-page ScrollRegion.both() adoption (BUG C, coupled with overlays-above-scroll). Design in docs/overlay-design.md §"T009 rework".

## HARD RULES (learned/confirmed this session)
- NO plain-key defaults (tmux): scroll bindings must be prefix-gated + configurable. Widgets bind no keys.
- Do NOT ship unverified rendering/layout widget changes — green unit tests ≠ verified; the user drives the GPU showcase. (A speculative Dialog::on_layout regressed and was reverted.)
- Refer to features/phases/tasks by Fxxx/Pxxx/Txxx ids.
- Update docs/widgets.md (both audiences + code examples) whenever a widget changes.
