# heca — PLAN (single source of truth)

> **The** prioritized plan for active work. Rewritten 2026-06-15 to end plan-doc sprawl.
> Done/not-done below is **verified against the live codebase** (not the stale `.md` boards).
> Supersedes the grid-ui/chrome/WS-A task boards (archived → `.planning/archive/`).
> Product-milestone tracking still lives in GSD `.planning/` (`ROADMAP.md`, `STATE.md`).
>
> **Rule:** outstanding *tasks* live here; when one ships, **delete it** (git keeps history).
> Deep *design rationale* lives in the design docs linked at the bottom.

---

## On resume (every session)
**First `git fetch origin` and sync `origin/main` into the working branch** before any new work
(memory `resume-pull-main-and-sync`). Pushing/merging still needs an explicit, action-specific OK
(memory `never-merge-or-push-without-explicit-ok`).

## Status snapshot (2026-06-15)

- **main** has the full **WS-A** workstream merged (**PR #102**): `[appearance]` config +
  transparency/vibrancy + render-through-`Compositor`; grid-ui **chrome shell**; **retained
  chrome tree** (click-select + workspace collapse); **generic DnD framework** (Phase 1+2).
- Active branch: `grid-ui-chrome-integration` (in sync with main).
- Build + full test suite green; warning-clean (bar the transitive `block v0.1.6` note).

---

## Priority sequence (locked)

1. **Consolidate planning docs** ← *this file* (in progress)
2. **SharedChromeState** (global AppState store) — *discuss first*; **gates F4.4/F4.5**
3. **F4.4** — generic marker/rail widget + targeting
4. **F4.5 ≡ DnD Phase 3** — re-enable sidebar DnD on the framework
5. **Pane numbering** feature
6. **Appearance & sizing** — finish in-app blur, app zoom, app/terminal font sizing
7. grid-ui maturity backlog (scroll, Pane shell, app-integration, bloom) — see bottom

---

## Now / Next (detailed)

### P0 — SharedChromeState (global AppState store) — *discuss before building* — NOT BUILT
Verified absent: no `heca/src/chrome/state.rs`, no `SharedChromeState`. Today chrome uses a
`ChromeSinks` `Rc<Cell>` stopgap + scattered state (`SidebarTree`, `AppState.sidebar`,
`mouse.drag_ctx`, `input_mode`).
- New `heca/src/chrome/state.rs` `SharedChromeState`: region visibility/mode/widths, selection
  (active/hovered pane), per-workspace collapse, drag state, targeting candidates, scroll.
  Session stays canonical; this holds **derived UI state + ids**. Endgame = a **namespaced
  signal store** (one namespace per feature/dock/plugin).
- **Design:** `F4-chrome-state-design.md` §2.2, `grid-ui-chrome-plan.md` §4,
  `pluggable-chrome-plugin-plan.md` Phase 2. Foundation-first (memory `chrome-integration-realignment`)
  puts this *before* more chrome content.

### P1 — F4.4 — generic marker/rail widget + targeting (built drag-aware)
- Build a **generic** `MarkerGroup`/`RailGroup` widget in `heca-grid-ui` (rows + left marker/rail
  bar + `KeyHint` target slot + drag-handle seam), theme-driven, `Base` + `Component` + builders.
  Note: it's drag-ready already — the universal `DragExt` (`.draggable`/`.drop_target`) shipped.
- Migrate `chrome.rs`'s inline `column_view` / `pane_card` alphas/padding / the `ba8049b`
  active-ws wash onto theme-driven widgets — after this `chrome.rs` only *composes* + projects.
- Wire move/swap/take targeting: pick mode lights `KeyHint` letters; app feeds candidates from
  `collect_all_pane_candidates`. (KeyHint stays universal — memory `grid-ui-keyhint-universal`.)

### P2 — F4.5 ≡ DnD Phase 3 — re-enable sidebar DnD on the new framework
- Framework ready (DnD Phase 1+2 merged): use `drag::source_at`/`resolve_at` over the retained
  tree (kills `sidebar_hit_test` for DnD); F4.4 marker bar = drag handle / drop target; paint
  `PaintCx::drag_ghost`/`drop_indicator`. Restore the drag-start block disabled in `45143b5`.
  **One task** (F4.5 and "DnD Phase 3" are the same). Design: `dnd-framework-refactor-plan.md`.

### P3 — Pane numbering feature (agreed, spec'd — memory `heca-pane-numbering-spec`)
- `prefix+<ws 1-9>+<pane 1-9>` → focus that pane (deterministic cross-ws chord; ws-switch is
  `prefix+w`+digit; `prefix+q` peek stays). Per-workspace pane numbers on cards. Touches card
  display (`chrome.rs`), new `WmAction` (input.rs/actions.rs), default keybindings, handler.
  Follow AGENTS.md "Adding New Actions" 11-step checklist.

### P4 — Appearance & sizing (user-facing; memory `appearance-blur-zoom-font`)
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
- **Terminal backend** live (GSD Phase 3 "The Content").
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

---

## Pluggable-chrome architecture (the long arc — `pluggable-chrome-plugin-plan.md`)
Not started beyond the precondition. Order: Phase 1 contract → **Phase 2 = SharedChromeState (P0
above)** → Phase 3 ChromeHost/region hosts → Phase 4 built-in providers → Phase 5
WorkspacesContainer migration → Phase 6 dynamic ActionRegistry → Phase 7 ≈ grid-ui widget expansion
(largely done) → Phase 7.5 shell compositing (largely done) → Phase 8 overlay host APIs → Phase 9
WASM runtime → Phase 10 multi-region proof → Phase 11 config/keybinding/palette integration.
(Plus 8.1 placeholder tokens, 8.2 simple config.toml plugins — "to be analyzed later".)

---

## Locked rules / constraints (ignore → redo)
- **New UI = generic, theme-driven `heca-grid-ui` widget** — embed `Base`, read ALL styling from
  `Theme`; domain-neutral; never ad-hoc inline `Flex`/`Surface` in the app. (AGENTS.md; memory
  `heca-widgets-in-grid-ui`.)
- **Foundation before style** (`foundation-state-before-style`); **best architecture up front, no
  half-measures** (`build-future-proof-no-half-measures`).
- **DnD framework stays domain-neutral** — payload in the app's `AppDragPayload`, never in `drag/`.
- **Never merge/push/force-push without an explicit, action-specific OK** (memory
  `never-merge-or-push-without-explicit-ok`).
- `KeyHint` universal; **no `cargo fmt`**; fix all warnings; **prefer one PR**; the showcase is the
  reference; heca is a tmux-like host (all bindings via prefix).

## Design references (kept in root — rationale, not task lists)
- `pluggable-chrome-plugin-plan.md` — chrome-plugin architecture north star.
- `grid-ui-chrome-plan.md` — chrome widget vocabulary + shared-state strategy (§4).
- `F4-chrome-state-design.md` — SharedChromeState design.
- `dnd-framework-refactor-plan.md` — generic DnD framework design (Phase 1+2 shipped).

## Archived (history → `.planning/archive/`)
- `grid-ui-plan.md` — the big grid-ui task board (full backlog detail).
- `grid-ui-integration-and-appearance-plan.md` — F1–F5 appearance/integration plan.
- `RESUME-ws-a.md` — WS-A session handoff (superseded by this file).
- `sidebar-gap.md` — Sidebar-shell vs WorkspacesContainer gap analysis (mostly addressed: shell +
  ChromeRegion + DockFrame exist; remaining behavior = the WorkspacesContainer migration, Phase 5).
