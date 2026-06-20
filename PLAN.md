# heca — PLAN (single source of truth)

> **The** prioritized plan for active work. Rewritten 2026-06-15 to end plan-doc sprawl.
> Done/not-done below is **verified against the live codebase** (not the stale `.md` boards).
> Supersedes the grid-ui/chrome/WS-A task boards (archived → `.planning/archive/`).
>
> **Rule:** outstanding *tasks* live here; when one ships, **delete it** (git keeps history).
> Deep *design rationale* lives in the design docs linked at the bottom.

---

## Status snapshot (updated 2026-06-18)

> **This is the single planning file.** It carries both the prioritized plan **and** the "where we are
> now" resume point. (The architecture *rationale* docs at the bottom stay as references, not task lists.)

- **`origin/main` @ #125.** Landed since the last snapshot:
  - **#123** — F4.4 KeyHint move/swap/take **pick overlay restored** in the Dock (+ `KeyHint::CenterRight`).
  - **#124** — planning docs **consolidated into this file** (stale handoff/review docs removed).
  - **#125** — **terminal-blur** (other dev): outer border, frosted tint, floating knobs, stencil clip — blur rework still WIP.
  - *(earlier: WS-A appearance/transparency #102; in-app blur primitive #105/#107; SharedChromeState foundation #107; F4.5 sidebar DnD #116–#120; terminal pane-shell #121/#122 → `app/terminal_render.rs`.)*
- **In flight (local-only):** `feature/f4.5-grip-widen` — grip gutter 12→20px + swap-arrows (⇄) cue.
  Committed, **not pushed/PR'd**.
- Build + tests green; warning-clean **bar** the transitive `block v0.1.6` note **and** pre-existing
  dead-code warnings in `app/selection_model.rs` / `terminal_render.rs`.

---

## Priority sequence (this file is authoritative; updated 2026-06-18)

**Done foundations** (don't re-plan): ~~consolidate docs~~ ✅ (#124) · in-app blur **primitive** ✅
(`Blur`+`Backdrop`, #105/#107) · SharedChromeState **foundation** ✅ (#107) · ~~F4.5 sidebar DnD~~ ✅
(#116–#120) · ~~F4.4 pane pick overlay~~ ✅ (#123).

**Near-term (active):**
1. **Pane Runtime State + reactive chrome store + plugin event bus** — the active initiative (design locked
   2026-06-18). Full plan: **`pane-runtime-state-plan.md`**; orchestration board: **`pane-runtime-tasks.md`**
   (separate from `shared-tasks.md`). **Status (2026-06-20): Phases 0–6 merged; Phase 7 (pane-info display)
   in progress on `feature/phase-7` (PR #147) — sidebar card done + an in-pane segmented info bar (config
   `[appearance] pane_title_segments`/`pane_title_actions`, Geist Mono UI font, configurable `sidebar_width`);
   remaining = the bar's action buttons (interactive) + decoupling fonts from color themes into `[settings]`
   (⚠ landmine: `heca-config Theme.font_size` default 32.0 is dead; UI renders at `grid_tron` 15 — normalize
   before mapping). **Full resume detail in `phase7-pane-info-bar-RESUME.md`.** Phase 8 (plugin
   `app.on`/`app.state`) is last.**
   Its **Phase 0 IS** the SharedChromeState consumer migration + the new
   typed event bus (the old P0); later phases add per-pane process/status/cwd/git tracking, the
   process→icon catalog, real command-spawn (float + close-policy), and the default pane-info widgets.
   **Phase 2 locked decisions (2026-06-18):** pane lifetime = the terminal/shell (PTY child) — auto-close
   only on shell death, foreground-program exit → `Idle` (no close); **event-first detection, NO polling
   timer** (output/EOF wakes + 250 ms debounce; periodic poll deferred); **status = `Running`/`Idle`/
   `Success`/`Error`** (no `Exit` — a dead pane closes); **`[programs.<raw>]`** config catalog
   (`name`/`icon`/`description`/`color`) with seeded shell defaults. See plan §0.2/§0.6/§0.7.
2. **niri parity audits** (niri section below) — catalog every animation vs niri; test for real whether
   adding/removing a column resizes the others; re-audit the still-unverified compat items.
3. **render.rs split** — partly done by #121/#122; reassess the remainder.
4. **Small leftovers, opportunistic** — F4.4 column-level pick keycaps (needs column-pick candidates) +
   the F4.4 widget-migration; F4.5 Onto-third drop semantics.

**Deferred — future implementation** (parked on purpose 2026-06-18; revisit after the foundation work):
- **Appearance & sizing** — app-wide zoom + separate app/terminal font-size controls + finish the in-app
  blur app-wiring. *(terminal-blur partly landed via #125 — revisit blur scope against that first.)*
- **Pane numbering** — `prefix + workspace# + pane#` jump-to-pane + numbers on the sidebar cards.
- **Finish sidebar drag-and-drop** — workspace drag-to-reorder (panes + columns already drag).

5. grid-ui maturity backlog (scroll, Pane shell, app-integration, bloom) — see bottom.

> The `P0–P4` labels on the detailed sections below are **stable anchors, not priority rank** — follow
> this list for order. The detailed write-ups for the three **deferred** items still live below
> (marked DEFERRED) so they're ready when revisited.

---

## Now / Next (detailed)

### In-app blur (F3) — reusable primitive DONE; app-wiring REMAINS
- ✅ `heca-renderer::blur::Blur` — separable-Gaussian over any source texture (PR #105).
- ✅ `heca-renderer::backdrop::Backdrop` — draw a texture region into a rect (UV + opacity); the
  stage that makes blur consumable. End-to-end path: Compositor scene tex → `Blur::process` →
  `Backdrop::draw(rect)` → translucent content over it (PR #107). Terminal agents reuse this.
- **REMAINING (app's own use):** wire it in `render_frame` — after rendering content into the
  compositor scene texture, blur it and frost the chrome (sidebar/status) over the blurred
  backdrop, driven by `appearance.blur_radius()` **converted logical→physical** (`* scale_factor`;
  compositor texture is physical-sized). Clip-aware.

### P0 — SharedChromeState (global AppState store) — DESIGN LOCKED, build in progress
Consolidate the scattered chrome UI state (`SidebarTree` cursor/collapsed, `AppState.sidebar`
vis/width, `input_mode` candidates, the `ChromeSinks` `Rc<Cell>` stopgap; drag stays in
`mouse.drag_ctx`) into one store. Session stays canonical; this holds **derived UI state + ids**.

**Decisions (locked 2026-06-15):**
- **Concrete struct, namespace-shaped** — a real `SharedChromeState` for today, designed so the
  future namespaced signal store wraps it as the `chrome` namespace (not a hack, not the full
  plugin registry yet).
- **Scope:** consolidate **all** chrome UI state in this effort (regions vis/mode/width, chrome
  selection active/hovered pane, per-workspace collapse, targeting candidates, scroll).
- **Read model = fine-grained signals** (`Signal<T>` fields; chrome widgets bind via `.get()` in
  paint/layout; writes mark dirty → coalesced redraw). Truest to the locked read-via-signals /
  write-via-actions contract.
- **Module:** `heca/src/chrome/state.rs` (chrome.rs already restructured → `chrome/mod.rs`).
- **Naming:** chrome's selection is **active/hovered PANE** — name it `ChromeSelection` inside
  `SharedChromeState`; do NOT collide with the terminal `SelectionState` (text selection, merged
  from main).

**Integration notes discovered (must handle):**
- heca creates **no signals at startup today**; no `Scope`/runtime setup exists. Verify
  `floem_reactive` signals can be created at `AppState` construction (global runtime on the UI
  thread) — app-lifetime signals never dispose (fine).
- Reconcile fine-grained signals with the **F4.1 rebuild-on-signature + damage** model: widgets
  re-subscribe on rebuild; the only-affected-repaint payoff needs signal-dirty to feed the
  `needs_paint`/`collect_damage` path. This is the deep part of the work.

**Design refs:** the locked SharedChromeState design lives in this P0 section (above); see also
`grid-ui-chrome-plan.md` §4, `pluggable-chrome-plugin-plan.md` Phase 2/§3.3.

### P1 — F4.4 — generic marker/rail widget + targeting (built drag-aware)
- ✅ **`heca-grid-ui::MarkerGroup` built** — generic, theme-driven, `Base`+`Component`+builders;
  vertical row group + left **marker bar**. Bar reads 3 ways: dim=inactive, accent+glow=active
  (`active` signal), brighter+thicker=grip **hover**. Reserves a ~12px left **grip gutter** (the
  group's own grab/target surface; bar is its 3px visual). Exposes `state()` (active) + `hovered()`
  (grip hover) signal handles. Cataloged (`docs/widgets.md`), in the showcase (PANES col), 6 tests.
  Stays generic — drag/peek are **composed on** via universal `DragExt`/`KeyHint`, never built in.
- **REMAINING F4.4:** migrate `chrome/mod.rs`'s inline `column_view` `Surface` bar / `pane_card`
  alphas+padding / the active-ws wash onto `MarkerGroup` + theme-driven `Row` (constant card bg +
  signal-driven active overlay) — after this `chrome.rs` only *composes* + projects. **Then** the
  active/hover signal-binding + signature-strip from the consumer migration becomes unblocked.
- **Wire move/swap/take targeting — ✅ DONE for panes (PR #123).** The pick `KeyHint` overlay (lost when
  the grid-ui Dock replaced the hand-drawn sidebar) is restored: each pane card is wrapped in the universal
  `KeyHint`, and `sync_chrome_signals` projects `state.input_mode.candidates()` onto a per-pane hint signal
  each frame (pure `pick_keycap` helper). Right-aligned via the new `KeyHint::CenterRight`. The keyboard
  logic (candidates + key consumption in `app/input.rs`) was always intact — only the visual was lost.
  **Remaining:** column-level pick keycaps — there are no column-pick candidates today, so this needs new
  candidate computation (genuinely new keyboard logic). KeyHint stays universal (memory `grid-ui-keyhint-universal`).

### P2 — F4.5 ≡ DnD Phase 3 — sidebar DnD — ✅ DONE (#116/#117/#119/#120)
> Panes + columns drag/move/swap, source-aware targeting, grab cursor, swap visual, RPC,
> docs + showcase all shipped. Grip-widen + swap cue shipped on `feature/f4.5-grip-widen`.
> Remaining: **workspace drag-to-reorder — ⏸ DEFERRED (2026-06-18)**; Onto-third drop semantics (tiny,
> still opportunistic). Original plan kept below for reference.
- Framework ready (DnD Phase 1+2 merged): `drag::source_at`/`resolve_at` over the retained tree
  (kills `sidebar_hit_test` for DnD); paint `PaintCx::drag_ghost`/`drop_indicator`. Restore the
  drag-start block disabled in `45143b5`. Design: `dnd-framework-refactor-plan.md`.
- **Granularity (decided 2026-06-16): panes AND columns are draggable.** Mark each pane `Row`
  `.draggable(Pane)` and each `MarkerGroup` `.draggable(Column)`. Hit-testing is innermost-first,
  so a press on a card → pane drag; a press in the `MarkerGroup` **grip gutter** (its only own
  surface) → column drag. The gutter is the column's drag handle for free — no special-casing.
- **Drag cursor (decided 2026-06-16; belongs HERE, not earlier): app-side winit policy.** heca
  sets **no** OS cursor today (`grep set_cursor` = none); heca-grid-ui must stay cursor-free
  (emits a Scene). Add a general **cursor-policy** helper in the cursor-moved path (`mouse.rs`):
  each move compute `CursorIcon` from state — `is_dragging()`→`Grabbing`; pointer over a grabbable
  grip/item (via `MarkerGroup::hovered()` + the same `resolve_at`/`source_at` "is draggable" test)
  →`Grab`; else default. Build it general so it extends to text I-beam / resize cursors later.
  Deferred to here because `Grab`-on-hover would be a lie until items are actually `.draggable`.
- **Hover dispatch (decided 2026-06-16; belongs HERE): the app must feed `PointerMoved` into the
  retained chrome tree.** Today only `PointerPressed` is dispatched (`chrome_dispatch_click`), so
  `MarkerGroup`/`Row` `hovered` never updates in the app — the grip hover/grab affordance is inert
  in the real sidebar (works in the showcase, which runs a full event loop). Wire move-dispatch +
  repaint into the cursor-moved path alongside the cursor policy + drag, so the grab affordance and
  drag light up together (a hover "grab me" cue is meaningless until drag works). The widget is
  already correct — this is app interaction wiring only.

### P3 — Pane numbering feature — ⏸ DEFERRED (2026-06-18) (agreed, spec'd — memory `heca-pane-numbering-spec`)
- `prefix+<ws 1-9>+<pane 1-9>` → focus that pane (deterministic cross-ws chord; ws-switch is
  `prefix+w`+digit; `prefix+q` peek stays). Per-workspace pane numbers on cards. Touches card
  display (`chrome.rs`), new `WmAction` (input.rs/actions.rs), default keybindings, handler.
  Follow AGENTS.md "Adding New Actions" 11-step checklist.

### P4 — Appearance & sizing — ⏸ DEFERRED (2026-06-18) (user-facing; memory `appearance-blur-zoom-font`)
> **Do NOT keep deferring these** — blur was postponed and got forgotten; that violates
> never-defer / no-half-measures (memory `build-future-proof-no-half-measures`).
- **In-app blur (F3)** — finish the separable-Gaussian in-app blur (currently wired but inert).
  Real frosted chrome/overlays, host-owned at the compositor/shell layer.
- **App-wide zoom (size awareness like the showcase)** — whole-UI zoom **increase/decrease**
  (the showcase's zoom mode / `nudge_zoom`), as actions (mouse/keyboard/RPC parity).
- **Font-size increase/decrease — two distinct controls**: app/chrome font (`Theme.font_size`)
  and terminal font (`Theme.terminal_font_size`). Keep configurable in `Theme`
  (memory `grid-ui-font-configurable`); never hardcode. Bindings via prefix
  (memory `heca-tmux-prefix-keybindings`).

---

## Foundation gaps (open, lower urgency)
- **Collapsed sidebar rail** still legacy hand-drawn + `sidebar_hit_test` (only EXPANDED is grid-ui).
- **Sidebar buttons** (`+w/+c/+p`, workspace/column clicks) not wired (button_hitboxes unused).
- **NSWindow vibrancy console warning** — benign (memory `heca-nswindow-vibrancy-warning`); address.
- **Split `heca/src/app/render.rs`** into a `render/` folder — *do at the end, its own PR, not
  mid-feature.* **Partly done by #121** (the terminal pass is already extracted to
  `app/terminal_render.rs` — `PaneRenderState`, `paint_terminal_pane_shell`, `render_terminal_mount`,
  `selection_overlay_for_pane`); re-scope the remainder against that before splitting further.
  `render_frame` shrinks toward ~250. Remaining target files:
  `mod.rs` (frame orchestrator + `update_session_viewport`), `geometry.rs` (pane/scissor/textbox
  math), `terminal.rs` (`TerminalRenderPassContext` + `render_terminal_mount` — the terminal pass),
  `panes.rs` (tiled+floating passes: blur stamps, border scenes, content), `selection.rs`
  (`build_selection_overlay`/`selection_overlay_for_pane`/`status_mode_parts` + tests),
  `overlays.rs` (collapsed rails, drag ghost, pane-select labels, `render_chrome` flush).
  Easy: geometry/terminal/selection/`render_chrome` already take explicit params (mechanical move).
  Risk: `panes.rs` + rail/ghost live INSIDE `render_frame` and touch many `AppState` fields while
  `scene_view` borrows `state.compositor` → extract via a granular-field context struct (the
  `TerminalRenderPassContext` pattern), never `&mut AppState`. Verify by running the app (GPU
  ordering isn't unit-tested).

---

## niri parity — audit + improvements (from the 2026-06-17/18 compat re-check)

> Context: `niri-compatibility-review.md` was re-verified against current heca. The keybinding
> complaints are mostly fixed (registry rewrite). These three remain. **Lesson learned:** one claim
> ("focus-up jumps workspaces") was tested and proved false — so verify behavior by *running it*, not
> just by reading a code path.

- **Catalog every animation (for a future animation overhaul).** Write down the full set of motions niri
  animates and how (open, close, move, resize, workspace-switch, view-scroll, overview — niri gives each
  its own spring/easing config and an off switch), then map each to what heca does today. Right now heca
  appears to use one simple easing for everything, with no spring physics, no per-motion config, and no way
  to turn animations off. Goal of this task = the catalog/gap-list; the actual improvement is a later effort.
- **Confirm whether adding/removing a column actually resizes the other columns.** heca recomputes all
  column widths on every layout change (`update_all_column_widths()` runs on every mutation in
  `heca-core/src/layout/scrolling.rs`). niri never resizes existing columns when you open a new one. The
  open question is whether heca's recompute *actually* changes existing widths at runtime or just re-derives
  the same numbers. **Test it live** (resize a column, then add/remove another, then resize the window —
  does the first column keep its size?). If it reflows, switch to storing each column's width and only
  recomputing the changed one.
- **Re-audit the compat items not yet checked deeply.** Several rows in the compat review are still marked
  "unverified": prefix-mode timeout, whether modifiers are read from the event vs a cache, sending a literal
  prefix key through to the terminal, the viewport/chrome coordinate math, the single-animation model, and
  the column-width double-caching. Go through each against current code (and run it where behavior matters)
  and update the review's status table.

---

## Done — verified in code (don't re-plan these)
- **Appearance/transparency**: `[appearance]` config (`heca-config/src/appearance.rs`); render via
  `Compositor`; window/surface transparency; macOS vibrancy. *(grid-ui-integration F1, F2a–c)*
- **grid-ui vocabulary widgets all exist**: `Grid`, `Icon`, `ItemGroup`, `DockFrame`, `ChromeRegion`,
  `RailCell`, `KeyHint`, `Tag`, `Badge`, … *(grid-ui-plan G1–G5, G8)*
- **Renderer clip**: `PushClip`/`PopClip` **implemented** in `heca-renderer/src/scene.rs`
  (clip_stack + scissor) — **G7's blocker is closed**; the editor-viewport clip is authorized
  (memory `grid-ui-renderer-clip-authorized`).
- **Chrome integration**: `build_chrome_scene` + `render_chrome` (status bar through grid-ui);
  sidebar SHELL + WorkspacesContainer (F5); retained chrome tree (F4.1); click-select (F4.2);
  workspace collapse (F4.3).
- **Generic DnD framework** (DnD Phase 1+2): `DragContext<P>`, universal `DragExt`,
  `resolve_at`/`source_at`, `drag_ghost`/`drop_indicator`, generic theme tokens.
- **Terminal backend** live.
- **Pane shell (Phase 13 visual blocker 1)**: borders/radius now render through the `heca-grid-ui` `Pane` container (PR #121 — `GridRenderer` `begin_frame`-once-per-frame contract fix); terminal content clipped to the rounded border via a stencil-write pass; snug padding + chrome value clamps + theme-driven `pane_padding`. Blocker 2 (`terminal_blur`) still open — see P4.
- Pluggable-chrome-plugin-plan **Precondition** (the 10-phase refactor) complete.

---

## Backlog — grid-ui maturity & app integration (prioritized; detail in archived `grid-ui-plan.md`)
1. **G7 scroll/list primitive** — now **unblocked** (clip landed); build the embeddable scroll region.
2. **C7 `Pane` shell**: HUD header (title+status) + tab bar + focused brackets; expose inner rect
   (today `Pane` is a bracket container without header/tabs). Plus **C4** `CornerBrackets`/`Reticle`,
   **C5** `StatusBar` component.
3. **D-series app integration**: replace remaining hand-drawn chrome (tab bar, sidebar render) with
   grid-ui components wired to WM-state signals (D2–D5); `grid_tron` default theme (D1);
   `CycleTronIntensity` through the full action pipeline (D6); **D6.5 `ActionSink` keybinding
   integration**; `cargo clippy --workspace` clean + README/AGENTS refresh (D7).
4. **C8** extend showcase to exercise every component + snapshot/visual tests.
5. **E-series** effects: offscreen bloom (bright-pass → Gaussian → additive); `Custom` DrawCommand.
6. **Widgets**: `Item` DnD reorder (framework ready via `DragExt`)/description/custom-bg;
   multi-select `Select`; HUD Frame, Metric Row, Search Input, Accordion.
7. **Docs**: theme-token reference + config.toml configurability; demote `docs/the-grid-ui.md` to
   reference-only; end-user docs (after everything ships).
8. **B-series renderer**: physical-pixel 1px alignment on fractional scale; dedicated scanline shader.
9. **`NfIcon` — Nerd-Font icon widget** (planned 2026-06-20; enhances pane title + sidebar program
   icons + the Phase 5 process catalog). Phosphor has almost no brand/app/language logos (no rust,
   docker, nvim, node, python marks); Nerd Fonts (devicons/seti/font-awesome) do — exactly what a
   per-program title wants. **Resolved design (do it this way):**
   - **Embed the symbols-only font** ("Symbols Nerd Font Mono"), *not* a full patched font — just the
     glyph ranges (~1–2 MB), so it won't fight the text/mono fonts. **Verify + record the license**
     before embedding (NF symbols are redistributable; confirm OFL/MIT and note it).
   - **Separate flat widget, shared pipeline.** Keep `Icon` as the Phosphor **duotone** widget; add
     `NfIcon` as a **single-layer (flat)** widget (NF glyphs are monochrome). Reuse the existing
     glyph-atlas/text path — add an NF icon **font family/role** (the renderer already carries a
     per-command font family + an icon role; extend, don't fork).
   - **Reference by codepoint + a small curated enum.** Mirror `Icon::from_codepoint` (NF glyphs live
     at fixed PUA codepoints) plus a curated `NfGlyph` for the ones actually used.
   - **Wire into the catalog.** Let `[program.<id>].icon` optionally name an NF glyph so
     `nvim`/`lazygit`/`docker`/language panes get real logos; the pane title + sidebar card pick it up
     through the shared `pane_info_view` path. Showcase + `docs/widgets.md` updated (grid-ui rule).
   - Scope: its own task/PR (font asset + renderer family + widget + catalog resolver + showcase/docs);
     not bundled with the pane-title work (which already ships with Phosphor icons).
10. **`heca-grid-ui` crate-review debt** (from the two crate reviews, both ~8/10; docs removed):
   `badge.rs` `unreachable!()` in a reachable match arm; add `[workspace.lints]`/package lints;
   `#[allow]`→`#[expect]` in `component.rs`; `#![deny(missing_docs)]`; `#[non_exhaustive]` on public
   enums; hot-path allocs (`Input::chars_vec`, `CommandPalette::results`, scene `to_vec`/`clone`);
   widget test coverage (~3% — widgets largely untested); shared hover/flash/anim helper to cut
   ~200 lines duplicated across Button/Toggle/Checkbox/IconButton/Item/Row/RailCell.

---

## Pluggable-chrome architecture (the long arc — `pluggable-chrome-plugin-plan.md`)
Not started beyond the precondition. Order: Phase 1 contract → **Phase 2 = SharedChromeState (P0
above)** → Phase 3 ChromeHost/region hosts → Phase 4 built-in providers → Phase 5
WorkspacesContainer migration → Phase 6 dynamic ActionRegistry → Phase 7 ≈ grid-ui widget expansion
(largely done) → Phase 7.5 shell compositing (largely done) → Phase 8 overlay host APIs → Phase 9
WASM runtime → Phase 10 multi-region proof → Phase 11 config/keybinding/palette integration.
(Plus 8.1 placeholder tokens, 8.2 simple config.toml plugins — "to be analyzed later".)
**After this arc is complete**, the next initiative is **AI Agent Integration**
(`agent-integration/agent-integration-plan.md` + board `agent-integration/agent-integration-tasks.md`):
per-pane structured `AgentStatus` (Working/WaitingForInput/WaitingForPermission/Finished/Error/
Compacting/SubagentRunning) sourced from each agent's lifecycle hooks (Claude Code `terminalSequence`,
Codex native OSC 9, pi extension), carried over a per-driver transport (in-band OSC / AF_UNIX
side-channel) into `PaneRuntime.agent`, emitted as `pane.agent.changed` on the existing event bus
(plugin-visible), displayed alongside program/git, plus transition sounds (rodio). Generic +
pluggable: an `AgentDriver` trait + registry is the seam — built-in Rust drivers now, the same
contract becomes the WASM plugin host contract for third-party agents later. Research complete;
design locked; pre-implementation (parked until the plugin arc above is done).

---

## Locked rules / constraints (ignore → redo)
- **New UI = generic, theme-driven `heca-grid-ui` widget** — embed `Base`, read ALL styling from
  `Theme`; domain-neutral; never ad-hoc inline `Flex`/`Surface` in the app. (AGENTS.md; memory
  `heca-widgets-in-grid-ui`.) **Covers every new element/component/widget from the terminal backlog**
  (e.g. the Phase 11 context menu, any image-preview chrome): build it as a proper `heca-grid-ui`
  widget following the existing design rules (embed `Base`, read ALL styling from `Theme`,
  domain-neutral name/semantics, no hardcoded sizes/colors/alphas), route behavior through
  `ActionRegistry`/`KeymapRegistry` (mouse + keybinding + RPC parity). Low-level image *texture*
  rendering (Phase 12) is `heca-renderer` (wgpu) — a different altitude, not a grid-ui widget; any
  UI *around* it still is.
- **Foundation before style** (`foundation-state-before-style`); **best architecture up front, no
  half-measures** (`build-future-proof-no-half-measures`).
- **DnD framework stays domain-neutral** — payload in the app's `AppDragPayload`, never in `drag/`.
- **Never merge/push/force-push without an explicit, action-specific OK** (memory
  `never-merge-or-push-without-explicit-ok`).
- `KeyHint` universal; **no `cargo fmt`**; fix all warnings; **prefer one PR**; the showcase is the
  reference; heca is a tmux-like host (all bindings via prefix).

## Design references (kept in root — rationale, not task lists)
- `pane-runtime-state-plan.md` — **active initiative**: pane runtime state + reactive chrome store +
  plugin event bus (detailed phased plan); board in `pane-runtime-tasks.md`.
- `pluggable-chrome-plugin-plan.md` — chrome-plugin architecture north star.
- `agent-integration/agent-integration-plan.md` — **post-plugin-plan initiative**: AI agent status
  tracking + sounds (board in `agent-integration/agent-integration-tasks.md`). Research complete;
  parked until the pluggable-chrome arc is done.
- `grid-ui-chrome-plan.md` — chrome widget vocabulary + shared-state strategy (§4).
- `dnd-framework-refactor-plan.md` — generic DnD framework design (Phase 1+2 shipped).
- `niri-compatibility-review.md` + `docs/niri-wiki/` — **important** niri layout/keybinding reference;
  **needs updating** to reflect current heca (flagged 2026-06-17). Keep — do **not** delete.
  *(The `F4-chrome-state-design.md` doc was outdated and removed; its live SharedChromeState design is
  now in the **P0** section above.)*

## Archived (history → `.planning/archive/`)
- `grid-ui-plan.md` — the big grid-ui task board (full backlog detail).
- `grid-ui-integration-and-appearance-plan.md` — F1–F5 appearance/integration plan.
- `RESUME-ws-a.md` — WS-A session handoff (superseded by this file).
- `sidebar-gap.md` — Sidebar-shell vs WorkspacesContainer gap analysis (mostly addressed: shell +
  ChromeRegion + DockFrame exist; remaining behavior = the WorkspacesContainer migration, Phase 5).
