# Handoff

**Created at:** 2026-07-24
**Updated at:** 2026-07-24
**Reason:** Session end at ~74% context. All user-reported issues resolved and confirmed; work committed locally and synced with main.

## Current focus
F003 (🧩 Pluggable Chrome) / P011 (plugin-ui). Branch `feat/action-task-c`. PR #244 was MERGED to main this session (origin/main = f431cf4); main was merged back into the branch (`8cd0072`). ~24 NEW local commits on top — **NOT pushed**. A fresh PR will be needed for this session's work.

## What was being done
Continued the overlay follow-ups (T014) and split the remainder into T015/T016. Delivered T015 (overlay sizing + scrollable modal body) and fixed a long chain of visual bugs the user found by driving the GPU showcase. All fixes were pushed INTO the widgets, never onto callers (a recurring user demand: "internal calculation, not something developers set on every modal").

## How to resume
`/planner load`, read this handoff. The visual work is user-verified ("Other issues are solved", "gap is ok"). Likely next: push these commits + open a PR; or start F003/P011/T016 (SDF glyph glow) or F004/P001/T008 (unify scrollbars). Every rendering change is user-verified in `cargo run -p heca-renderer --example showcase` — green tests are NOT sufficient (memory: no-unverified-rendering-widget-changes).

## Files touched (all committed locally; workspace 868 tests green, clippy clean)
- `heca-grid-ui/src/widgets/overlay.rs` — placement authorities (`place_anchored`/`place_anchored_on`/`place_at_point`, `AnchorSide`), `paint_panel_chrome`+`PanelChrome`, `OverlayPosition`, `panel_size` + unconditional viewport cap, `VIEWPORT_MARGIN` padding, Scroll/PointerReleased routing into the panel.
- `heca-grid-ui/src/widgets/select.rs` — panel placement + chrome via the shared authorities; pure-`panel_rect` regression guard.
- `heca-grid-ui/src/widgets/context_menu.rs`, `command_palette.rs` — panel chrome via `paint_panel_chrome`.
- `heca-grid-ui/src/widgets/dialog.rs` — `panel_size`; `fit_body` (fill width, take leftover height, default `gap_spacing = Md`); Scroll/PointerReleased routing.
- `heca-grid-ui/src/widgets/scroll_region.rs` — min 0 / shrink 1 (so it can bound-and-scroll); glow-bleed clip on the non-scrolling axis; `pad_spacing_y = Sm`, `gap_spacing = Md`.
- `heca-grid-ui/src/style.rs` — `min_width/min_height/max_width/max_height`, `flex_shrink: Option<f32>`, `gap_spacing: Option<Spacing>` + taffy mapping.
- `heca-grid-ui/src/layout.rs` — resolve `gap_spacing` from font (beside pad tokens).
- `heca-grid-ui/src/builders.rs` — `LayoutExt::gap_spacing(Spacing)`.
- `heca-grid-ui/src/scene.rs` — restore an open clip on a parent overlay's continuation segment after a nested overlay splits the stream.
- `heca-theme/src/theme.rs` + `lib.rs`, `heca-config/src/appearance.rs` + `theme.rs`, `heca/src/chrome/mod.rs` — `FrameStyle` + `overlay_frame` token + `[appearance] overlay_border_style` (bracketed/bordered/none), live-reloaded.
- `heca-renderer/examples/showcase.rs` — OVERLAY FRAME select; the Delete-pane dialog demos sized panel + scrollable body + nested Select.
- `docs/widgets.md`, `config.default.toml`, `README.md` — docs for all of the above; expanded Flex entry (spacing tokens + form-field grouping recipe).

## Blockers
None. F003/P011/T012 stays blocked (needs F004/P001/T006, a chrome ScrollRegion) — unchanged.

## Next steps
1. Push this session's commits + open a fresh PR (PR #244 already merged).
2. F003/P011/T014 — settle the Tooltip decision (leaning: leave its chrome/placement alone; a tooltip is not a panel) and CLOSE the item.
3. F003/P011/T016 — SDF glyph/text glow (Icon + RailCell rest glow); renderer work, fully GPU-gated.
4. F004/P001/T008 — unify scrollbar behaviour (4 widgets, 3 thumb widths; audit in the task).
5. F003/P011/T017 — Field widget (DEFERRED/optional; Flex + gap_spacing already lays forms out).

## Recent decisions
- **Overlay panel presentation is centralized:** all four overlay panels (Overlay/Dialog, Select, ContextMenu, CommandPalette) share `paint_panel_chrome`; `FrameStyle::` is matched in exactly ONE place (overlay.rs). Verified by grep.
- **Panel frame is a user setting, never hardcoded:** `overlay_frame` (`bracketed`/`bordered`/`none`) owns the WHOLE edge — bracketed=edge+reticle, bordered=edge, none=NO edge. `PanelChrome.border` is the widget's preferred edge COLOUR, ignored under none. Lives in heca-theme (grid-ui reads it at paint, can't depend on heca-config) — same path as `show_focus_border`.
- **`panel_rect()` must be a PURE function** of bounds+side+content (no viewport clamp): it's computed at layout AND paint, and any time-varying input detaches rows from the panel. Kept on-screen by capping row COUNT, not moving the panel. Regression tests lock it.
- **A scroll viewport bounds-and-scrolls only if its parent bounds it** and it can shrink below its content — hence `panel_size` + ScrollRegion `min 0 / shrink 1`. Both are widget-internal.
- **Nested overlay clip fix (scene.rs):** a nested overlay splits the parent's segment; the clip open before it must be re-established on the continuation, or a scrolled dialog body's rows escape the modal after a Select opens. The nested segment itself stays UNCLIPPED (a dropdown may extend past its region).
- **Spacing is font-relative tokens, not px:** `Style::gap_spacing` + `LayoutExt::gap_spacing(Spacing)` added (gap had no token, only raw px — the last hardcoded-spacing hole).
- **Form fields use nested Flex, not a Field widget:** `Flex::column().gap_spacing(Xs)` groups label+control tight; the container's Md gap falls between fields. Field widget deferred to T017 (only justified for a declarative ViewNode kind / less boilerplate).
- **Dialog vs modal:** a dialog uses fixed `Px` panel sizes; `Pct` (viewport-proportional) is for genuinely modal overlays.
- **Fixed a PRE-EXISTING red test:** `GlowLevel::Thin.strength_scale()` was 0.75 in code (T011) but the assert still said 0.5 — heca-theme was failing on HEAD.

## Reminder
Ids as Fxxx/Pxxx/Txxx (web UI shows plain numbers: Feature 3 / Phase 11). Build→user-verifies→commit for any rendering/overlay/scroll change. When deferring a piece mid-task, put it in the planner in the same breath, not just the chat reply (a repeated correction this session). Both grid-ui docs (rustdoc + docs/widgets.md) for any widget change. Don't push unverified visual work; these commits ARE user-verified but still local — push + new PR is the next action.