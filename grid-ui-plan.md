# heca-grid-ui — Grid UI Component Library: Plan

> A reusable, signal-driven, composable GPU component library that gives heca a *Tron/GridCN* visual identity.
> Status: **A done · B mostly done · C catalog done, shells pending · D not started** · Last updated: 2026-06-08

**Doc roles (single source of truth for each):**
- **`grid-ui-plan.md`** (this file) — the **activity tracker**: what's done, what remains, phases/tasks. *Update this as work lands.*
- **`docs/widgets.md`** — canonical **per-widget API reference** (props/methods/events + examples). *Currently stale (Jun 4) — predates the theme-token/font pass; refresh is the [Documentation pass](#-remaining-work) task.*
- **`docs/the-grid-ui.md`** — **reference & vision only** (external GridCN analysis + original design vision + GridCN reference links). Describes an *older, never-implemented* architecture; do **not** track activity here.

---

## ✅ Task Board — Shipped

**Workflow** (agreed 2026-06-04): one **feature branch per task** off up-to-date `main`; **PR after each task**; mark a task **IN PROGRESS** before starting and **DONE** when finished; on stopping mid-task add a **Resume note** and delete it once resumed-and-completed. **Never run `cargo fmt`** (rustfmt 1.9 churns unrelated files). `heca-renderer/src` + `heca`/`heca-core`/`heca-config` are owned by other devs — only `heca-renderer/examples/showcase.rs` is ours.

| Milestone | PR |
|-----------|----|
| **Phase A** — crate skeleton + core model (reactive facade, taffy layout, Scene/DrawCommand, Component+Base, Style/Theme, Flex/Container/Label) | merged |
| **Phase B** — renderer (SDF rounded-rect + additive glow, brackets, embedded Geist Mono, text centering) + showcase | merged |
| **Phase C catalog** — Button (6×3), Toggle, Checkbox, Input (full keyboard model), Tabs, Badge, StatusDot, Separator, Spinner, Alert, ProgressBar, Gauge; `disabled`, `tab_index`, focus-visible | [#21](https://github.com/ovidius72/heca/pull/21) |
| Docs — `docs/widgets.md` API reference + `AGENTS.md` agent guide | [#22](https://github.com/ovidius72/heca/pull/22) |
| Overlay/popover layer + `Select` dropdown (scrollable long list, `select-change`) | [#23](https://github.com/ovidius72/heca/pull/23) |
| `Item` row (leading·label·trailing slots) + `ActiveMarker` + `Pane` container + dropdown-hover/Tabs-resize bug fixes | [#24](https://github.com/ovidius72/heca/pull/24) |
| **Theme-token pass** — `GlowLevel{None,Thin,Medium,Large}`; intensity↔glow disentangle (intensity = CRT scanlines only); radius+border width across all box/pill widgets (`control_radius()`, `border:0`); **central font inheritance** (`Style.font_size` sentinel + `font_scale` multiplier + `Base.font` + `Component::remeasure()` + `LayoutEngine.base_font`); Pane rounded corner brackets; single-select sidebar; whole-page scroll; Select flip-up + viewport-capped scroll + font-derived row height + content-adaptive width; showcase live GLOW/RADIUS/BORDER/FONT/INTENSITY controls + edge resize cursor | [#30](https://github.com/ovidius72/heca/pull/30) ✅ merged 2026-06-08 |

---

## 📍 Remaining Work

The authoritative checklist of what's left, by phase. (Supersedes the old flat task board.)

### Phase C — finish the catalog & shells

**Chrome vocabulary — see [`grid-ui-chrome-plan.md`](./grid-ui-chrome-plan.md).** Decisions locked 2026-06-09. The DnD framework already shipped (`heca-grid-ui/src/drag/`); build the rest of the vocabulary now; scroll waits on the renderer.

> **What G1–G8 supersede / don't.** These are the **grid-ui (presentation) layer** for the chrome/sidebar. They **replace** the old sidebar widget plan (old `C6 Sidebar = tree-nav`) and **absorb** old `C4` (CornerBrackets/Reticle) + `C5` (StatusBar) + `C7` (Pane HUD header). They **do not** touch the rest of this plan (shipped catalog, docs pass, end-user docs, Phase D). They also **do not** include the sidebar's *behavior* — the workspace tree / panes / git / docker live in **app-side Docks** (`WorkspacesDock`, …) tracked in `pluggable-chrome-plugin-plan.md` (Phase 5+), not here. grid-ui stays domain-neutral.

- [x] **G1 `Grid`** layout widget (taffy grid; `Track{Px,Fr,Auto,MinContent,MaxContent}` + named areas + explicit `.cell()`; `Style.grid_cell` + `Component::taffy_style()` hook). DONE — branch `grid-ui-grid-widget`.
- [x] **G2 `Icon`** — duotone glyph widget over an embedded, host-registered icon font (**Phosphor Duotone**, MIT, in `assets/`). Renderer loads it as a 2nd family; `FontRole::Icon` text runs select it. Duotone = two stacked layers (secondary `:before` dimmed + primary `secondary+1`); colors are theme-driven (`Theme.icon_secondary_alpha` + per-icon overrides), never baked. Curated `Glyph` enum + `from_codepoint`. DONE — branch `grid-ui-icon`.
- [x] **G3 `ItemGroup`** — collapsible group (header Item + chevron) over `Item` rows; collapse folds rows out of layout via new `Style.hidden` (`display:none`); `expanded` signal + `on_toggle`. DONE — branch `grid-ui-itemgroup`.
- [x] **G4 `DockFrame`** — titled/collapsible frame (drag-handle grip + chevron + title + interactive header-controls slot) over a foldable body; collapse hides the body via `Style.hidden`; `expanded` signal + `on_toggle`. Reuses `Pane` brackets via the new shared `PaintCx::bracket_frame` (extracted from `Pane`; `Pane` stays the plain container). DONE — branch `grid-ui-dockframe`.
- [x] **G5 `ChromeRegion` shell** — generic across all 4 regions (vertical sidebars + horizontal bars), mode-aware (`RegionMode::{Expanded,CollapsedRail,Hidden}` via a signal), collapse sizing (rail width/height), hosts `DockFrame`s; `toggle()` intent + `mode_signal()` binding point (P2: write-via-action, read-via-signal). **No** tree/workspace/drag semantics. DONE — core on branch `grid-ui-region-shell`; **collapsed icon-rail rendering** on branch `grid-ui-icon-rail` (`DockFrame::rail(mode_signal, Glyph)` folds header+body → centered `Icon` while `RegionMode::CollapsedRail`; `ChromeRegion::with_mode_signal` lets a host own the mode signal; showcase `[` toggles the sidebar rail). (Scroll overflow still waits on renderer clip / G7; Dock drop targets are G6.)
- [x] **G5.5 enumerate rail (`RailCell`) + generic pick overlay (`KeyHint`)** — a *list* dock (workspaces/columns/panes) must keep **one cell per pane** when collapsed, not fold to a single icon. `RailCell` = focusable square icon cell (state tint + same-hue border + glow, press flash, focus ring, `on_activate`). `KeyHint` = a **generic, reusable** transparent wrapper that overlays a glowing keycap letter over **any** actionable child while a host-owned `Signal<Option<String>>` is `Some` — the move/swap/select pick prefix (and reusable for content-area panes, command palettes, expanded rows). Signal-driven so mouse/keyboard/RPC all reach it (input parity). Showcase: a workspaces rail of icon cells; `p` opens the pick (letters appear), a letter selects, Esc cancels. **Mirrors `heca`'s existing collapsed-sidebar `candidates` flow** (`heca/src/sidebar/render.rs` + `app/selection.rs`). DONE — branch `grid-ui-rail-panes`.
- [ ] **G6 DnD hooks** — region `DragSurfaceId` + Dock `DragItem`; `DockFrame` drag handle drives `SurfaceDragState`, `ChromeRegion` drop targets set `hover_item`. Build on the **shipped** `src/drag/` framework; extend additively, never fork.
- [ ] **G7 scroll/list primitive** — **gated on renderer `PushClip`/`PopClip`** (request it).
- [x] **G2.5 `Row`** — focusable, clickable, single-selectable container for **arbitrary composed content** (`Item`'s interactive chrome — hover/active pill + `ActiveMarker` + press flash + focus ring + `on_activate`/Enter — generalized to wrap any children, e.g. a multi-line `Grid` of `Label`/`Icon`/`Badge`). Optional persistent background under the selection overlay. Unblocks clickable rich Dock rows. DONE — branch `grid-ui-row`.
- [x] **G8 rich status-item recipe** + `Tag`/`Chip` + showcase rich rows. Added `Tag` (bordered metadata chip with an optional leading icon — git branch/path/filter; hue-configurable, neutral). The PANES dock is now the composed recipe (state-colored leading `Icon` + program name + git-branch `Tag` + status `Badge`, in a `Grid` inside a selectable `Row`). grid-ui stays domain-neutral; the demo maps state→style. DONE — branch `grid-ui-status-rows`.
- [x] **Pane "needs attention" cue** (user-requested) — `effects::Attention` (N-pulse repeating flash) + `Row.attention(Signal<bool>)` / `.attention_color()`. Host sets the request signal → row flashes 4× + signal consumed; **sound stays host-side** (grid-ui audio-free; the app beeps when it sets the signal). Showcase `n` fires it + a terminal bell. DONE — branch `grid-ui-attention`.

**Catalog gaps:**
- [~] `IconButton` + **icon support** — **icon support DONE** (G2 `Icon`, embedded Phosphor font, `FontRole` text path). `IconButton` (a `Button` with an icon slot) still pending.
- [~] `Tag`/`Chip` — **DONE** as G8 `Tag` (multi-segment, leading icon, theme-driven). The **dismissible**/**selectable-filter** variants are not built yet.
- [ ] `EnergyMeter`, `SignalIndicator`.
- [ ] `DataCard`/`Panel`/`Hud`.
- [ ] `Tooltip`, `Modal`/`Dialog`, `CommandPalette` (all consume the overlay layer).
- [ ] Wire `Item` into `Select` options (label + `value: SignalData` + slots) — unify the two row models.
- [ ] **C8** extend showcase to exercise every component; visual/snapshot tests.

### Phase D — app adoption (not started)
- [ ] D1 `grid_tron` default theme; keep `mocha`/`latte` selectable.
- [ ] D2 replace `heca/src/sidebar.rs` rendering with the `Sidebar` component, wired to WM-state signals.
- [ ] D3 wrap each pane in the `Pane` shell (frame/brackets/header); backend renders into inner rect. **Layout engine untouched.**
- [ ] D4 rebuild tab bar + status bar as `heca-grid-ui` components.
- [ ] D5 bridge WM state → signals (focused pane, active workspace, titles).
- [ ] D6 `CycleTronIntensity` action through the full action pipeline.
- [ ] **D6.5 `ActionSink` keybinding integration** (design locked — see §12): real config.toml-driven shortcuts on all actionable widgets. Implement *here*, when widgets meet the app.
- [ ] D7 `cargo clippy --workspace` clean; update README + AGENTS.md.

### Documentation pass — **developer-facing** (do before/with Phase D)
- [ ] Refresh **`docs/widgets.md`** to the current API — `font_scale`/`GlowLevel`/`control_radius`/`remeasure`/`PaintCx::with_viewport`, adaptive `Select`, per-widget theme-token behavior.
- [ ] Add a **theme-token reference** (`glow_size`, `intensity`, `radius`, `border_width`, `font_size`/`font_scale`) + config.toml configurability.
- [ ] Formally demote **`docs/the-grid-ui.md`** to reference-only (or fold its still-useful catalog/links into widgets.md) and delete the stale architecture section.

### Infra / renderer dependency (other dev owns `heca-renderer/src`)
- [ ] **`PushClip`/`PopClip`** — currently a no-op in `heca-renderer/src/scene.rs`. Needed for an **embeddable scroll region** (today only whole-page scroll works).
- [ ] B5 physical-pixel alignment for crisp 1px borders on fractional scale.
- [ ] B2 dedicated full-region scanline shader (basic version exists).

### PLANNED enhancements (not scheduled)
- [ ] Multi-select (`Select` multi mode or `SelectMulti`): `Vec<usize>`, `Check` markers, toggle-on-click.
- [ ] `Item`: drag-and-drop reordering · description (2nd line) · custom fg/bg per row · inline progress strip · arbitrary child composition.
- [ ] New components (§11): HUD Frame, Metric Row, Modal, Notification, Search Input, Workspaces Container, SidebarItem variants, Tags, Toast, Accordion, Command Menu.

### End-user documentation — **the very last task**
- [ ] Author **end-user docs** (separate from the developer `docs/widgets.md`): for people *running* heca, not coding against it — config.toml, theme/intensity switching, keybindings/shortcuts (ties into §12 `ActionSink`/keymap). Do this **after everything else ships**.

**Resume notes (active):** *none.*

---

## ▶ Resume Here

### 🤝 Handoff — last updated 2026-06-10 (after the **pane "needs attention" cue** — `Attention` effect + `Row.attention`)

**Read this first to resume.** It's the single place that says where we are, what's
next, and how to start.

#### Git / PR state (verify before you start with `gh pr list` + `git fetch`)

- **Default branch:** `main` (in another worktree — `git fetch` + branch off `origin/main`).
  **This branch:** `grid-ui-attention` (attention cue, off fresh `origin/main`).
- **Merged into `main`** before this task: …**#57** G5 icon-rail (`DockFrame::rail`), **#60**
  enumerate rail (`RailCell` + `KeyHint`). So `main` has the full chrome vocabulary **G1–G5 (both
  rail flavors), G2, Row, G8, RailCell, KeyHint**.
- **This task (attention cue, user-requested):** **`effects::Attention`** (an N-pulse repeating
  sawtooth flash) + **`Row.attention(Signal<bool>)`** / `.attention_color()`. The host sets the
  request signal → the row flashes `ATTENTION_PULSES` (4)× and the signal is consumed back to
  `false`. **Sound stays the host's job** (grid-ui is audio-free): the app plays its beep when it
  sets the signal — showcase `n` does both (sets the signal + terminal bell `\x07`). +2 tests.
- **⚠️ G6 (dock DnD) is partly coordination-blocked — important.** `heca/src/mouse.rs:136` matches
  `DragSurfaceId` **exhaustively** (no wildcard), and `heca` owns those dispatch sites. So adding
  a region/dock `DragSurfaceId` variant (needed for cross-region dock move) **breaks `heca`'s
  build** until that dev adds the match arms — can't be done unilaterally. A *reorder-within-one-
  region* slice is still doable heca-safe (region-local state + `on_reorder` callback, reuse
  `drag::DEFAULT_DRAG_THRESHOLD_SQ`, no closed-enum changes). Coordinate with the WM dev before
  the cross-region/`SurfaceDragState` integration.
- **⚠️ Stranding gotcha (bit us 3×):** PRs here are merged by the user between turns. If you
  push commits to a branch **after** its PR is merged, they get stranded (not in `main`).
  Recovery: branch off fresh `origin/main`, `git cherry-pick` the stranded commits, new PR.
  **Rule:** after the user says "merged", `git fetch` and start the next task from
  `origin/main`; don't keep pushing to the old branch.
- Build is green on `main` (+ this branch): `cargo test -p heca-grid-ui` = **10 unit +
  86 integration + doctests**; clippy clean across `heca-grid-ui` + `heca-renderer`.
- **⚠️ Stranding gotcha (bit us 3×):** PRs here are merged by the user between turns. If you
  push commits to a branch **after** its PR is merged, they get stranded (not in `main`).
  Recovery: branch off fresh `origin/main`, `git cherry-pick` the stranded commits, new PR.
  This handoff PR already recovers the last stranded commit (`e228071`, header-badge align).
  **Rule:** after the user says "merged", `git fetch` and start the next task from
  `origin/main`; don't keep pushing to the old branch.
- Build is green on `main` (+ this branch): `cargo test -p heca-grid-ui` = **10 unit +
  86 integration + doctests**; clippy clean across `heca-grid-ui` + `heca-renderer`.

#### Hard rules (do not violate)

- **Never run `cargo fmt`** — rustfmt 1.9 churns unrelated files. (A linter may still
  touch files between your Read and Edit; just re-Read and retry.)
- **Mostly our files:** `heca-grid-ui/**` and `heca-renderer/examples/showcase.rs`.
  `heca`, `heca-core`, `heca-config` are owned by other devs — don't touch.
  **Exception already taken:** G2 (Icon) required `heca-renderer/src/{text.rs,scene.rs}`
  to load the icon font as a 2nd family + select it per `FontRole`. That's merged, but
  `heca-renderer/src/**` is the renderer dev's area — **coordinate before further edits**
  there (e.g. the still-blocked `PushClip`/`PopClip` for scroll).
- **Workflow:** one feature branch per task off **up-to-date `origin/main`**; PR per task;
  mark `[x]` DONE on the board (with branch name); leave a Resume note if you stop mid-task.
- **No hard-coded theme values** — derive font, **border width (`theme.border_width`)**,
  **radius (`theme.radius` / `theme.control_radius()`)**, and colors from the `Theme`,
  never literals. (Tag initially violated this and was fixed — check new widgets.)

#### Where we are

- **Phases A, B, C-catalog: shipped.** Full widget catalog + renderer; overlay layer;
  `Select`; `Item`/`ActiveMarker`; `Pane`; theme-token + central-font pass; dropdown flip/scroll.
- **Chrome vocabulary (this session): DONE — G1–G8 + Row.** Each widget lives in
  `heca-grid-ui/src/widgets/<file>.rs`, is exported from `widgets/mod.rs` + **both** lists in
  `lib.rs` (top-level `pub use` and `prelude`), has integration tests in `tests/phase_a.rs`,
  and is demoed in `examples/showcase.rs`:
  - **G1 `Grid`** (`grid.rs`) — taffy grid: `.columns/.rows([Track::{Px,Fr,Auto,…}])`,
    `.areas([...])` named areas, `.area(c,"name")`, explicit `.cell()`. `Style.grid_cell` +
    `Component::taffy_style()` hook.
  - **G2 `Icon`** (`icon.rs`) — **duotone** glyph over embedded **Phosphor Duotone**
    (`assets/Phosphor-Duotone.ttf`, MIT). Renders two stacked layers (secondary `:before`
    codepoint dimmed + primary `secondary+1`). `Icon::new(Glyph::…)` curated set or
    `from_codepoint`; `.size`, `.color`, `.secondary_color`. Colors theme-driven
    (`theme.icon_secondary_alpha`, default 0.45). Square layout.
  - **G3 `ItemGroup`** (`item_group.rs`) — collapsible group: header Item + chevron over
    rows; collapse folds rows via `Style.hidden` (`display:none`); `expanded` signal +
    `on_toggle("group-toggle")`.
  - **G4 `DockFrame`** (`dock_frame.rs`) — titled/collapsible frame: title bar (grip +
    chevron + title + interactive header-controls slot via `.header()`) over a foldable body
    (`.child()`); reuses `Pane`'s brackets via shared `PaintCx::bracket_frame`. Content inset
    from the frame (`CONTENT_PAD`). `expanded` signal + `on_toggle("dock-toggle")`. **Rail mode:**
    `.rail(region.mode_signal(), Glyph)` folds header+body → a centered `Icon` while the hosting
    region is `RegionMode::CollapsedRail` (tighter `RAIL_PAD` inset; brackets stay).
  - **G5 `ChromeRegion`** (`chrome_region.rs`) — oriented (`vertical()`/`horizontal()`),
    mode-aware shell hosting Docks. `RegionMode::{Expanded,CollapsedRail,Hidden}` via a signal
    (`mode_signal()` / host-owned via `with_mode_signal()`); collapse sizing; `toggle()` intent;
    `.dock()`. **No** tree/drag semantics. **Icon-rail DONE:** a rail-aware `DockFrame` (see
    `DockFrame::rail`) folds to a centered `Icon` in `RegionMode::CollapsedRail`.
  - **`Row`** (`row.rs`) — `Item`'s interactive chrome (hover/active selection pill +
    `ActiveMarker` + press `Flash` + focus ring + `on_activate`/Enter) generalized to wrap
    **any** children. Highlight **derives from the row's own background** (stronger same-hue
    fill + same-hue border) or `.highlight(c)`. This is what makes composed multi-line rows
    clickable/selectable.
  - **G8 `Tag`** (`tag.rs`) — bordered metadata chip; **multi-segment** (`.segment()` /
    `.segment_text()` with thin dividers, e.g. `path │ ⎇ main │ 5 +152 -12`), optional
    leading `Icon`, hue via `.color()`. Theme-driven radius (`theme.radius`) + border
    (`theme.border_width`); generous x-padding (`Style.padding_x/y`).
  - **`RailCell`** (`rail_cell.rs`) — focusable **square icon cell** for the *enumerate* rail:
    centers one `Icon`; active = accent tint + same-hue border + glow; hover/press flash + focus
    ring; `on_activate`; `.cell_size()`. The per-pane unit a collapsed *list* dock shows (vs a
    tool dock folding to one icon). Domain-neutral: the icon's color carries pane status.
  - **`KeyHint`** (`key_hint.rs`) — **generic** transparent wrapper that overlays a glowing
    accent **keycap letter** on any actionable child while a host-owned `Signal<Option<String>>`
    is `Some`. Reusable everywhere a keyboard pick/jump lights up targets (rail cells, content
    panes, command palettes). Transparent to focus + events (default container routing); only
    adds paint. `.hint(signal)`, `.placement(TopCenter|Center)`, `.size(px)`. **Signal-driven =
    input parity** (mouse/keyboard/RPC write the same signal).
- **The showcase PANES dock is the realized G8 recipe**: state-colored `Icon` + program name
  + multi-segment git `Tag` + status `Badge`, in a `Grid` inside a selectable `Row`. grid-ui
  stays **domain-neutral**; the *demo* maps state→style.
- **Docs:** `grid-ui-chrome-plan.md` holds the locked chrome decisions; `docs/widgets.md` is
  the API reference (may need a refresh for the new widgets — not yet done).

#### Shared infra added this session (reuse it; don't re-invent)

- **`PaintCx::bracket_frame(rect, fill)`** (`component.rs`) — the corner-bracket frame, shared
  by `Pane` + `DockFrame`.
- **`PaintCx::icon(rect, glyph, color, size)`** + **`FontRole::{Text,Icon}`** on `TextCmd`
  (`scene.rs`) — icon text path; renderer selects the icon family for `FontRole::Icon`.
- **`paint_child(c, cx)`** (`component.rs`) — skips `style.hidden` (`display:none`) children;
  used by default `Component::paint`, `Pane`, `DockFrame`. **Why:** layout collapses a hidden
  subtree to the top-left, so painting it stamps overlapping text there. Focus traversal
  (`focus.rs::for_each_focusable`) **also** skips hidden subtrees (no Tab into collapsed content).
- **`route_event(children, ev)`** (`component.rs`) — the default reverse event loop, reused by
  `ItemGroup`/`DockFrame` that wrap event handling.
- **`Style.padding_x/padding_y`** (`Option<f32>`, fall back to uniform `padding`) +
  **`LayoutExt::padding_xy(x,y)`** — per-axis padding (Tag uses it).
- **`Theme.icon_secondary_alpha`** — duotone dim factor, config-tunable.
- **`effects::Attention`** + **`Row.attention(Signal<bool>)`** / `.attention_color()` — a host-
  driven N-pulse "needs attention" flash (host sets the signal → row flashes 4× + consumes it;
  the **host** plays any sound, grid-ui is audio-free). Reuse the effect for other widgets.

#### What's next (in priority order)

1. **Wire the enumerate rail into the real `heca` Workspaces dock** (app-side, `heca/src/**` —
   coordinate; not ours to edit unilaterally). grid-ui now ships the primitives (`RailCell` +
   `KeyHint`); the app maps ws/cols/panes → cells (icons), feeds per-cell `KeyHint` signals from
   its existing `collect_all_pane_candidates()` during `PaneSelect`/`PaneSwap`/`PaneTake`, and
   dispatches the focus/swap/move **action** on cell activate. **icons not letters** by default;
   letters only during a pick. This *replaces* `heca/src/sidebar/render.rs::render_sidebar_collapsed`
   eventually. Likely the cleanest path: prototype the mapping in the showcase first (done — the
   `p`-pick demo), then port behind the chrome-plugin work.
2. **G6 — DnD hooks** — make Docks movable/reorderable by wiring `DockFrame`'s drag handle +
   region drop targets onto the **already-shipped** `heca-grid-ui/src/drag/` framework
   (`DragSurfaceId`/`DragItem`/`SurfaceDragState`/`DragContext`). Extend **additively**; never
   fork. Region = a `DragSurfaceId`, a Dock = a `DragItem`. Every drop = a dispatched **action**.
   **⚠️ Partly blocked:** `heca/src/mouse.rs:136` matches `DragSurfaceId` exhaustively, so a new
   region/dock surface variant breaks `heca`'s build until that dev adds arms — **coordinate**.
   The heca-safe slice you *can* do solo: **reorder within one region** (region-local drag state
   + `on_reorder(from,to)` callback + drop-indicator paint; reuse `drag::DEFAULT_DRAG_THRESHOLD_SQ`;
   no closed-enum changes). The grip is currently inside `DockFrame`'s toggle `Item` — making it a
   non-toggling drag handle likely means splitting it into a dedicated header child.
3. **`Pane`/`Item` adopt `Row.attention`** (optional) — the attention cue is built on `Row`
   (`effects::Attention` + `Row.attention(signal)`); extend to `Pane`/`Item` if the app needs it
   there. Sound stays host-side (the app beeps when it sets the signal).
4. **Color/readability pass** (user keeps flagging) — see the memory note + the levers below.
5. **G7 scroll** — **BLOCKED** on the renderer's `PushClip`/`PopClip` (no-op in
   `heca-renderer/src/scene.rs`, renderer dev's area). Only whole-page scroll works.
6. **Catalog gaps / docs** — `IconButton`, `Tooltip`/`Modal`/`CommandPalette`; refresh
   `docs/widgets.md` for the new widgets; **Phase D** app adoption + the §12 `ActionSink`
   keybinding integration; end-user docs last.

#### Open polish / readability levers (user-flagged, not yet "right")

- `Row`: `ACTIVE_TINT_ALPHA` (90), `ACTIVE_BORDER_ALPHA` (180), `ACTIVE_BORDER_W` (1.3).
- `theme.muted` reads dim on tinted backgrounds — pane subtitles were switched to
  `theme.foreground`; consider a brightened-muted midpoint token.
- `theme.icon_secondary_alpha` (0.45) — duotone visibility on dark; lower=subtler.
- `Tag` `RADIUS_MUL` (1.0) — chip roundness.

#### How the icon-rail was built (G5 seam, now closed)

- Chose option (b)+signal-sharing: **`DockFrame` learns a rail mode** by observing the region's
  `Signal<RegionMode>` (read-only — `Signal` is `Copy`, so obtained via `mode_signal()` before
  the region is moved into `.dock()`). No downcasting in the region; no rebuild — `sync()` runs
  each `remeasure` and flips `style.hidden` on the [HEADER, BODY, RAIL] children.
- `DockFrame::rail(mode_signal, Glyph)` appends a centered `Icon` child at index `RAIL=2`. In
  `RegionMode::CollapsedRail`, header+body fold (`display:none`), only the icon paints, and the
  frame inset tightens to `RAIL_PAD` so the icon fits a thin rail. Brackets still draw → each
  rail icon reads as a small framed dock button.
- `ChromeRegion::with_mode_signal(Signal<RegionMode>)` lets a host **own** the mode signal (a
  central layout store) and share it with the docks — the showcase uses this + binds `[` to
  toggle the sidebar (only when nothing is focused, so it still types into an Input).
- New widget wiring reminder: file in `widgets/`, export in `mod.rs` + **both** `lib.rs`
  lists, tests in `tests/phase_a.rs` (tail; relative-center asserts), demo in `showcase.rs`.

#### For G6 (DnD) — pointers

- The framework is **already in the crate**: `heca-grid-ui/src/drag/` (`DragContext`,
  `DragSurfaceId`, `DragItem*`, `SurfaceDragState`, `SurfaceDragPhase`) — exported in `prelude`.
- `DockFrame` already has a **drag-handle grip** (visual) + a `.child()` seam comment marking
  where dragging wires in. Add variants additively; never retype/remove existing drag types.

#### Gotchas

- **Stranded commits** — see the Git/PR state box. After "merged", `git fetch` and branch
  from `origin/main`.
- **Duotone = two glyphs.** An `Icon` emits two `cx.icon` runs (secondary dimmed + primary).
  Primary codepoint = secondary + 1 (Phosphor pairs them consecutively).
- **Hidden subtrees** — never paint or focus-traverse `style.hidden` nodes directly; go through
  `paint_child` / `for_each_focusable` which skip them.
- **Tag in a Grid cell** stretches to fill the cell — wrap it in `Flex::row().child(tag)` to
  hug-left (see the PANES branch tag).
- `Base.font` is the **resolved** font; widgets read it (not `style.font_size`) in `remeasure()`.
- Tests use **relative** centers (`b.size.h/2`) so they tolerate font-resolution changes; keep that.
- `PaintCx::new` defaults to an *infinite* viewport; pass `.with_viewport(size)` in a real host.

**Run / verify:**

```bash
cargo run -p heca-renderer --example showcase                 # live demo (needs a display)
cargo test -p heca-grid-ui                                    # 10 unit + 86 integration + doctests
cargo clippy -p heca-grid-ui -p heca-renderer --all-targets   # must be clean (ignore the `block v0.1.6` transitive note)
```

**Done (committed):**

- **Phase A** — `heca-grid-ui` crate: `reactive` facade (`floem_reactive`), `taffy` layout, `Scene`/`DrawCommand`, `Component` trait + `Base` (composition), `Style`/`Theme` (dark `grid_tron` default), `Flex`/`Container`/`Label`.
- **Phase B** — `heca-renderer`: SDF rounded-rect pipeline `grid.rs`/`grid.wgsl` with **premultiplied additive glow** (real translucent halo), `scene.rs` bridge (`enqueue_scene`), embedded **Geist Mono Regular+Bold**, bold + **metric-based text centering** in `text.rs`, `examples/showcase.rs`.
- **Phase C (partial)** — builder traits `LayoutExt`/`StyleExt`/`Parent`; `Flex` is layout-only; `Surface`, `Card`, **`Button`** (6 GridCN variants × 3 sizes; animated per-variant hover; tuned glow; per-variant **press `Flash`**; focus-visible ring), **`Toggle`** (sliding switch: accent-filled-on track, light knob + glow; `on_change(Action)`; `disabled`), **`Checkbox`** (box + pop-in glowing accent indicator; `checkbox-change`), **`Input`** (single-line editable buffer, blinking caret, placeholder; `input-change`); display widgets **`Separator`**, **`Badge`** (6 variants), **`StatusDot`**; **`Tabs`** (segmented selector, sliding underline, arrow-key nav; `tab-change`), **`Spinner`** (ring-of-dots brightness sweep via `tick`), **`Alert`** (4 variants, left accent bar + title/body), **`ProgressBar`** (eased signal-driven fill), **`Gauge`** (12-segment energy meter, success→warning→danger).
- **Accessibility** — `GridKey`, `Event::Key`, `FocusManager` (Tab/Shift+Tab + `focus_at` click-focus + wrap), `on_focus`/`on_blur`, Space/Enter activation, **focus-visible** (ring on keyboard focus only), `Theme.show_focus_border`. **`disabled`** widgets are skipped by traversal (`focusable()` returns `!disabled`).
- **Foundations** — `Action`/`SignalData` (`action.rs`) **wired** via `on_change(Fn(Action))` callbacks on change widgets; `Flash` (`effects.rs`, reusable press effect); **`Base.disabled`** common property + `LayoutExt::disabled` builder. `PaintCx::flash`/`PaintCx::dim` both take a **corner `radius`** so overlays follow rounded shapes (no square corners poking past a pill).

**Key files (`heca-grid-ui/src/`):** `component.rs` (Component/Base/PaintCx/Event/GridKey/on_focus/dim), `focus.rs` (FocusManager), `effects.rs` (Flash), `action.rs`, `builders.rs`, `theme.rs`, `style.rs`, `scene.rs`, `layout.rs`, `widgets/{flex,label,surface,card,button,toggle}.rs`. **Renderer:** `heca-renderer/src/{grid.rs,grid.wgsl,scene.rs,text.rs}` + `examples/showcase.rs`.

**Established pattern (Toggle, done):** change widgets flip a `Signal<T>` and fire `on_change(Fn(Action))` carrying `Action::value("…-change", SignalData::…)` with the new value. **Decision:** the `Action` model is wired via a callback (consistent with `Button.on_click`), *not* by changing `event() -> Handled` (which `focus.rs` depends on). All interactive widgets honor `Base.disabled` (dimmed via `PaintCx::dim`, inert, unfocusable) and reuse `Flash` + `focusable()`.

**Change widgets done:** `Toggle`, `Checkbox`, `Input` — all three emit `…-change` Actions via `on_change(Fn(Action))`, honor `disabled`, and are keyboard-operable. `Input` has an editable buffer, a click-to-place + blinking caret (monospace advance), a placeholder, a **multi-click selection cycle** (double=word, triple=all, fourth=clear; typing/backspace replace the selection), **granularity delete** (Ctrl/Alt + Backspace/Delete = word; Cmd/meta + Backspace/Delete = to start/end), and **keyboard selection** (anchor-based): Shift+Arrow = char, Shift+Ctrl/Cmd+Arrow = to start/end, Shift+Alt+Arrow = by word; plain Ctrl/Alt+Arrow move the caret by word/boundary. **Home/End** (+Shift) move/select to start/end; **Cmd/Ctrl+A** selects all (other modified chars are swallowed, not typed). Caret movement + delete share a `Granularity` (Char/Word/Line). Modifiers ride a renderer-agnostic `Modifiers` broadcast via `Event::ModifiersChanged` (host maps platform keys; word-mod = Ctrl **or** Alt for cross-platform Win/Linux + macOS). `Checkbox` has an optional **clickable label** with configurable side (`LabelSide`). Focus traversal honors an optional **`Base.tab_index`** (HTML-like), else tree position.

**Display widgets done:** `Separator` (cross-axis stretch + `.length()`), `Badge` (6 theme-mapped variants, neon chip), `StatusDot` (semantic glowing dot).

> The blocks above (Done / Key files / Established pattern / Change & Display widgets) are
> **historical reference** for the shipped catalog. For the live plan use the
> [🤝 Handoff](#-resume-here) and [📍 Remaining Work](#-remaining-work) sections at the top.

---
>
> Locked deps: `floem_reactive 0.2.0`, `taffy 0.7.7`. Q1 resolved (local `Color`), Q3 resolved (standalone example).

---

## 1. Goal & Non-Goals

### Goal

Build a new crate, **`heca-grid-ui`**, that is a **component framework** — not a theme — with these properties:

- **Composable**: small components nest to build big ones (`Flex` of `Card`s of `Gauge`s).
- **Extensible**: every component extends one shared base (`Base` struct + `Component` trait); adding a new widget is embedding `Base` + implementing the trait.
- **Signal-driven**: fine-grained reactivity — when a `Signal<T>` changes, only the dependent components repaint.
- **Easy to style**: builder-style API + token-based `Theme`; colors and effects fully configurable.
- **Tron look, natively**: glow, corner brackets, scanlines, HUD typography — realized in `wgpu`, not a webview.

The existing chrome (sidebar, status bar, pane frames) becomes a **consumer** of this library, and panes render as Grid-styled components over the unchanged niri layout engine.

### Non-Goals

- **Not** a new window-management layout engine. `heca-core`'s scrolling-column engine stays canonical for arranging panes/columns. `heca-grid-ui` lays out *widgets inside* components.
- **Not** a port of GridCN's web stack. No `oklch`, no Tailwind, no Radix — we pick our own palette and tokens.
- **Not** terminal/Neovim content rendering. The `Pane` component renders the *frame* (border, brackets, header); the `PaneBackend` still renders its cell grid into the inner content rect.

---

## 2. Locked Architecture Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Crate | New `heca-grid-ui` crate in the workspace | Reusable, isolated, testable. |
| D2 | GPU boundary | `heca-grid-ui` is **GPU-free**; emits a `Scene` (display list). `heca-renderer` rasterizes it. | Matches the project's `heca-core` rule: headless, unit-testable, fast compiles, swappable renderer. Same model as Zed/GPUI, Flutter, WebRender. |
| D3 | Reactivity | `floem_reactive` behind a thin `heca_grid_ui::reactive` facade | Fine-grained-reactivity correctness (effect tracking, cleanup, diamond problem) is hard; reuse a proven, renderer-agnostic runtime. Facade keeps the dependency un-named in component code so it can be swapped later. |
| D4 | Layout | `taffy` (Flexbox + Grid + Block) | The proven pure-Rust layout engine (Floem, GPUI, Bevy, Dioxus). Operates at component-internal altitude, orthogonal to the niri WM layout. |
| D5 | "Extends base" | Composition: embed `Base` + implement `Component` trait (idiomatic Rust, not inheritance) | Rust has no inheritance; embedding + trait gives shared behavior with `Deref`-ergonomic access. |
| D6 | Default theme | Dark Grid theme (`grid_tron`) becomes the default; background set via `Theme.background` | Already the mechanism (`catppuccin_mocha` is the current dark default). |
| D7 | Glow strategy | Per-primitive **SDF glow** (no extra render targets); full-frame bloom deferred | Cheaper, no render-loop restructuring, looks great for neon borders/brackets/HUD. |
| D8 | Coordinate space | `Scene` reuses `heca-core`'s **f64 logical** geometry (`Rectangle`/`Point`/`Size`); taffy's f32 output is cast to f64 at write-back; renderer converts f64→f32 at the GPU boundary | *Revised during Phase A.* Reusing `heca-core` geometry is cleaner than a parallel f32 type and matches the project's established "f64 logical, f32 at GPU" rule. HiDPI crispness preserved. |

---

## 3. Crate Layering

```
heca-grid-ui  (NEW, no wgpu)
  ├── reactive/   ← Signal/Memo/Effect facade over floem_reactive
  ├── scene.rs    ← Scene + DrawCommand display list (the visual vocabulary)
  ├── style.rs    ← Style (taffy::Style wrapper) + Theme tokens + builders
  ├── component.rs← Component trait + Base struct + layout/paint/event contexts
  ├── layout.rs   ← taffy tree sync + compute → Base.bounds
  └── widgets/    ← Flex, Grid, Stack, Label, Button, Card, Hud, Gauge,
                    CornerBrackets, StatusBar, Sidebar, Pane, ...
        │
        │ emits Scene
        ▼
heca-renderer  ← render_scene(&Scene): SDF-glow + scanline + text pipelines
heca-core      ← geometry (Point/Size/Rectangle), reused by heca-grid-ui
heca           ← builds component trees for chrome; feeds Scene to renderer
```

**Dependency rule:** `heca-grid-ui` → `heca-core` (+ `floem_reactive`, `taffy`). `heca-grid-ui` must **not** depend on `wgpu`, `winit`, or `heca-renderer`.

---

## 4. Core Abstractions

### 4.1 `Component` trait + `Base` struct

```rust
/// Shared state every component owns. Concrete widgets embed this.
pub struct Base {
    pub style: Style,            // wraps taffy::Style + Tron visual tokens
    pub node: taffy::NodeId,     // this component's node in the layout tree
    pub bounds: Rectangle,       // filled in AFTER taffy computes layout
    pub visible: Signal<bool>,
    pub children: Vec<Box<dyn Component>>,
}

pub trait Component {
    fn base(&self) -> &Base;
    fn base_mut(&mut self) -> &mut Base;

    /// Register taffy nodes/styles. Default: container of children.
    fn register(&mut self, cx: &mut LayoutCx);

    /// Emit draw commands into the scene. Default: bg + border + children.
    fn paint(&self, cx: &mut PaintCx);

    /// Handle input; default routes to children, then self.
    fn event(&mut self, _ev: &Event) -> Handled { Handled::No }
}
```

A concrete widget "extends" `Base` by embedding it and adding its own signals:

```rust
pub struct Button { base: Base, label: Signal<String>, on_click: Callback }

impl Button {
    pub fn new(label: impl Into<String>) -> Self { /* default Style */ }
    pub fn glow(mut self, c: Color) -> Self { self.base.style.glow = Some(c.into()); self }
    pub fn on_click(mut self, f: impl Fn() + 'static) -> Self { /* ... */ self }
}

impl Component for Button {
    fn base(&self) -> &Base { &self.base }
    fn base_mut(&mut self) -> &mut Base { &mut self.base }
    fn paint(&self, cx: &mut PaintCx) {
        let s = &self.base.style;
        cx.rect(self.base.bounds, s.fill, s.border, s.glow);   // SDF + glow
        cx.corner_brackets(self.base.bounds, s.accent);        // shared decorator (DRY)
        cx.text_centered(self.base.bounds, &self.label.get(), s.fg); // .get() subscribes
    }
}
```

### 4.2 Reactive facade (`reactive/`)

```rust
// Public API never names floem_reactive — swappable.
pub use facade::{Signal, RwSignal, Memo, Effect, batch};
```

- `Signal<T>` is a `Copy` handle. `.get()` subscribes the active node; `.set()`/`.update()` marks subscribers dirty.
- A dirty signal flips the app's existing `needs_redraw`; repaint is coalesced to one frame.

### 4.3 Layout (`layout.rs`, via taffy)

Per frame, only when the tree is dirty:

1. Sync each `Base.style` → `taffy::Style` on its node.
2. `taffy.compute_layout(root, available_space)`.
3. Walk the tree, copy each node's computed `(x, y, w, h)` → `Base.bounds`.
4. `paint` reads `bounds`.

`Flex` API:

```rust
Flex::row()                          // Flex::column() for vertical
    .gap(8.0)
    .justify(Justify::SpaceBetween)  // main axis
    .align(Align::Center)            // cross axis
    .padding(12.0)
    .child(Label::new("STATUS"))
    .child(Gauge::new(power).flex_grow(1.0))
    .child(Button::new("DEREZ").glow(theme.danger))
```

---

## 5. Visual Vocabulary (`scene.rs`)

The richness here is what makes the Tron look possible without `heca-grid-ui` touching the GPU.

```rust
pub struct Scene { commands: Vec<DrawCommand> }   // built fresh each repaint

pub enum DrawCommand {
    Rect(RectCmd),          // solid/SDF rounded rect, optional border + outer glow
    Brackets(BracketCmd),   // L-shaped corner reticle framing a rect
    Text(TextCmd),          // a positioned text run (color, size, weight, align)
    Scanline(ScanlineCmd),  // region scanline overlay (spacing, color, opacity)
    Gradient(GradientCmd),  // linear gradient fill
    PushClip(Rectangle),    // clip subsequent commands
    PopClip,
    Custom(CustomCmd),      // escape hatch: named shader effect + params (Phase E+)
}

pub struct RectCmd { pub rect: Rectangle, pub fill: Color, pub border: Option<Border>,
                     pub radius: f32, pub glow: Option<Glow> }
pub struct Glow { pub color: Color, pub radius: f32, pub intensity: f32 }
pub struct BracketCmd { pub rect: Rectangle, pub color: Color, pub len: f32,
                        pub thickness: f32, pub glow: Option<Glow> }
```

`heca-renderer::render_scene(&Scene)` maps each command to a pipeline (SDF-glow, scanline, text-atlas). Adding an effect = one variant + one shader branch; component code is untouched.

---

## 6. Component Catalog

Prioritized by what heca actually needs. Each extends `Base`.

| Tier | Components |
|------|-----------|
| **Primitives** | `Flex`/`Container` (layout-only), `Surface` (base styled box), `Grid`, `Stack`, `Spacer`, `Label` |
| **Interactive** | `Button` (6 variants: primary/secondary/destructive/outline/ghost/link × sm/md/lg — ref <https://thegridcn.com/components#button-example>), `IconButton`, `Toggle`, `Tabs` |
| **Display** | `Card`/`DataCard`, `Panel`/`Hud`, `Separator`, `Badge`, `StatusBar` |
| **Tron-flavor** | `CornerBrackets` (decorator), `Reticle`, `Gauge`, `EnergyMeter`, `SignalIndicator`, `ScanlineOverlay`, `CoordinateDisplay` |
| **heca-specific** | `Sidebar` (tree nav), `Pane` (frame + HUD header) |

---

## 7. Phases & Tasks

Build order: **A → B → C standalone; D adopts into the app** (lowest risk; "library + showcase first").

### Phase A — Crate skeleton & core model (no GPU)

**Goal:** `heca-grid-ui` compiles and is unit-tested with zero GPU; the reactive + layout + component model is proven by a trivial tree.

- [x] A1. Add `heca-grid-ui` to workspace members; `Cargo.toml` with `heca-core`, `floem_reactive`, `taffy` deps (pin versions; verify against rust 1.85 / edition 2024).
- [x] A2. `reactive/` facade: re-export `Signal`/`RwSignal`/`Memo`/`Effect`/`batch`; document the swap-ability contract in `//!`.
- [x] A3. **Resolved (Q1):** `heca-grid-ui` owns its own `Color` (richer helpers: `lerp`, `with_alpha`, hex `FromStr`). Keeps Phase A additive (zero churn in existing crates); `From`/`Into` conversions to `heca-config::Color` deferred to Phase D.
- [x] A4. `scene.rs`: `Scene` + `DrawCommand` enum + sub-structs (`RectCmd`, `Glow`, `BracketCmd`, `TextCmd`, `ScanlineCmd`). Derive `Debug`, `Clone`.
- [x] A5. `style.rs`: `Style` wrapping `taffy::Style` + Tron tokens (`fill`, `border`, `glow`, `accent`, `radius`, `intensity`); builder methods; `Theme` token struct with `grid_tron()` preset.
- [x] A6. `component.rs`: `Component` trait, `Base` struct, `LayoutCx`/`PaintCx`/`Event`/`Handled`; default `register`/`paint`/`event`.
- [x] A7. `layout.rs`: taffy tree build + `compute_layout` + write-back to `Base.bounds`.
- [x] A8. `widgets/`: `Container`, `Flex` (row/column/gap/justify/align/padding), `Label`.
- [x] A9. Tests: flex distribution (grow/justify/align/gap), nested layout rects, signal `set` → dirty propagation, `paint` emits expected `DrawCommand`s. **All headless.**

**Exit:** `cargo test -p heca-grid-ui` green; `Flex::row().child(..).child(..)` produces correct `bounds` and a `Scene` snapshot.

### Phase B — Renderer backend & showcase

**Goal:** First pixels. The Grid look is visible.

- [x] B1. `heca-renderer`: SDF rounded-rect + glow pipeline (`grid.wgsl`) — instanced quads carrying rect params + glow params; rounded-rect signed distance; interior fill + additive exponential outer falloff. Keep the existing flat pipeline intact.
- [~] B2. Scanlines — **basic** version done (thin glowing rects via the grid pipeline, intensity-gated through theme); dedicated full-region scanline shader still TODO.
- [x] B3. Corner-bracket rendering — 8 glowing arm-rects via the SDF pipeline (`scene::draw_brackets`).
- [x] B4. `heca-renderer::scene::enqueue_scene(grid, text, &Scene)`: maps every `DrawCommand` → grid/text renderer; basic text alignment. (Clip commands stubbed — see B-clip TODO.)
- [ ] B5. Physical-pixel alignment for crisp 1px borders on fractional scale (`docs/niri-wiki/04-development/fractional-layout.md`).
- [x] B7. **Embed default mono font** — Regular + Bold vendored (OFL 1.1), `DEFAULT_MONO_BYTES`/`DEFAULT_MONO_BOLD_BYTES`, loaded into `TextRenderer`'s `cosmic-text` font system; family stays theme-configurable. Italic = synthesized oblique (`OBLIQUE_SKEW`); Geist Mono has no italic face and no coding ligatures (`calt`/`dlig` absent).
- [x] B6. **Showcase** example — `cargo run -p heca-renderer --example showcase`: a row of glowing HUD cards (border + glow + corner brackets) with a scanline overlay on the dark `grid_tron` theme. *(Placed in `heca-renderer/examples` to avoid a dep cycle; needs a display to view.)* (`heca-grid-ui/examples/showcase.rs` driving a winit+wgpu window, or a `--showcase` flag in `heca`): a `Flex` of glowing `Card`s with brackets + scanline over a dark background.

**Exit:** `cargo run` shows the showcase: glowing rounded borders, corner brackets, scanline overlay, crisp text on retina.

### Phase C — Component library

**Goal:** The reusable Tron component set heca will consume.

- [x] C0. Builder traits `LayoutExt`/`StyleExt`/`Parent` enforcing layout-vs-surface separation; `Flex` made layout-only; showcase migrated to `Card`/`Button` with live hover/click.
- [x] C1. `Button` (6 variants × 3 sizes), `Toggle`, `Checkbox`, `Input` (full keyboard model) — all done. `IconButton` still pending (see Remaining Work).
- [~] C2. `Surface` + `Card` + `Separator` + `Badge` (6 variants) + `StatusDot` **done**; `DataCard`/`Panel`/`Hud` pending.
- [~] C3. `Gauge` + `ProgressBar` **done**; `EnergyMeter`/`SignalIndicator` pending.
- [x] C3.5. **Theme-token pass** (PR #30): `GlowLevel`, intensity↔glow split, theme-driven radius/border across all widgets, central font inheritance (`font_scale`/`Base.font`/`remeasure`), Pane rounded brackets, single-select sidebar, page scroll, Select flip/scroll/adaptive-width. *(Not in the original phase plan — added during build.)*
- [ ] C4. `CornerBrackets` decorator + `Reticle` (shared, used by Pane/focus chrome).
- [ ] C5. `StatusBar` (segmented `Flex`, signal-bound segments).
- [ ] C6. `Sidebar` component (tree model, expand/collapse, cursor, drag affordance) — Grid-styled, mirrors current `heca/src/sidebar.rs` behavior.
- [ ] C7. `Pane` shell: HUD header (title + status), tab bar, focused-state brackets; exposes an inner content `Rectangle` for the backend. *(Today `Pane` is a bracket-framed container without the header/tabs.)*
- [ ] C8. Extend showcase to exercise every component; visual + snapshot tests.

**Exit:** All components render in the showcase; each is composable in a `Flex`/`Grid` and stylable via builders + theme. **Remaining for full exit: C4–C8 (shells + decorators + tests).**

### Phase D — App adoption

**Goal:** heca itself wears the Grid skin.

- [ ] D1. `grid_tron` becomes the default theme; dark background via `Theme.background`. Keep `mocha`/`latte` selectable.
- [ ] D2. Replace `heca/src/sidebar.rs` rendering with the `heca-grid-ui` `Sidebar` component, wired to WM-state signals (tree, focused item).
- [ ] D3. Wrap each laid-out pane in the `heca-grid-ui` `Pane` shell (frame/brackets/header); backend content renders into the inner rect. **Layout engine untouched.**
- [ ] D4. Rebuild tab bar + status bar as `heca-grid-ui` components (replaces scattered `draw_rect` calls in `main.rs` — DRY win).
- [ ] D5. Bridge WM state → signals: focused pane, active workspace, pane titles update reactively.
- [ ] D6. `CycleTronIntensity` action (Off→Low→Med→Heavy) through the **full action pipeline**: `WmAction` variant → `action_from_name()` → explicit `action_priority()` → handler → `build_registry()` → default binding → `ActionRegistry::ALL` descriptor.
- [ ] D7. `cargo clippy --workspace --all-targets --all-features` clean; update README + AGENTS.md.

**Exit:** heca launches dark with a Grid sidebar, Grid pane frames, and live intensity switching; niri layout/animations behave exactly as before.

### Phase E — (Optional, later) Full-frame bloom

- [ ] E1. Offscreen render target + resize handling.
- [ ] E2. Bright-pass threshold → separable Gaussian blur (downsampled) → additive composite.
- [ ] E3. `Custom` DrawCommand wiring for exotic effects.

**Exit:** Cinematic full-frame bloom toggle; only pursued if per-primitive glow (D7) isn't enough.

---

## 8. Cross-Cutting Conventions

Per `AGENTS.md` and the Rust skill:

- **Idiomatic Rust**: `enum` state machines over bool flags (`InputState`, `Handled`, `Justify`, `Align`); newtypes for ids; builder pattern for widget config; `impl From`/`Display`/`Debug` for conversions; `Result` over panic in fallible paths; `pub(crate)` by default, expose only the surface API.
- **Docs**: `//!` module headers explaining design; `///` on public types/methods; examples on key APIs.
- **DRY**: shared decorators (`corner_brackets`, `glow_border`, `scanline`) live once in `PaintCx`; never duplicate draw patterns.
- **HiDPI**: logical `f32` in the `Scene`; physical scaling + pixel alignment only in the renderer.
- **Hygiene**: `cargo fmt`, `cargo check` before/after, `cargo clippy --workspace --all-targets --all-features` clean. No `#[allow(dead_code)]` without a justifying comment.
- **No registry bypasses / no second WM layout engine** — taffy is component-internal only.

---

## 9. Dependencies to Add

| Crate | Where | Purpose | Notes |
|-------|-------|---------|-------|
| `floem_reactive` | `heca-grid-ui` | Signals/memos/effects | Behind `reactive` facade; renderer-agnostic. |
| `taffy` | `heca-grid-ui` | Flexbox/Grid/Block layout | Component-internal layout only. |
| (none new) | `heca-renderer` | SDF/scanline shaders | Built on existing `wgpu` + `cosmic-text`. |

Pin and verify both against **edition 2024 / rust 1.85**.

---

## 10. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| `floem_reactive` API churn / transitive deps | Facade isolates it; can swap to a custom runtime with no component-code change. |
| taffy mistaken for a rival WM engine | Documented distinction in AGENTS.md; taffy never positions panes/columns. |
| `DrawCommand` vocabulary too thin to express effects | Design it rich in Phase B; `Custom` escape hatch for exotic shaders. |
| Per-primitive glow looks weaker than cinematic bloom | Phase E bloom available as escape hatch. |
| Repaint cost if whole tree repaints each frame | Signal dirty-tracking + coalesced `needs_redraw`; only repaint on change. |
| `Color` location coupling | Resolve Q1 early (promote to `heca-core` vs local type). |

---

## 11. Open Questions

- **Q1**: `Color` — promote to `heca-core` and re-export from `heca-config` (cleanest, DRY), or define a local `heca_grid_ui::Color`? *Recommendation: promote to `heca-core`.*
- **Q2**: Reactivity engine confirmed as `floem_reactive`? *Recommended.*
- **Q3**: Showcase as a `heca-grid-ui` example binary, or a `--showcase` flag in `heca`? *Recommendation: standalone example to keep `heca-grid-ui` self-demonstrating.*
- **Glow level** configurabel (off, low, medium, hight)
- **NEW COMPONENTS** TO ADD:
  - `HUD Frame` [[https://thegridcn.com/components#hud-frame]]
  - Metric Row (SidebarItem ? [[https://thegridcn.com/components#metric-row]])
  - Modal ([[https://thegridcn.com/components#modal]])
  - Notification and icons ([[https://thegridcn.com/components#notification]])
  - Search Input (for ws filtering) [[https://thegridcn.com/components#search-input]]
  - Workspaces Container ([[https://thegridcn.com/components#command-example]]) seaarch matching ws, cols, panes
  - SidebarItem 1 ([[https://thegridcn.com/components#changelog]])
  - SidebarItem 2 ([[https://thegridcn.com/components#sidebar-nav]])
  - SidebarItem 3 ([[https://thegridcn.com/components#activity-feed]])
  - SidebarItem 4 ([[https://thegridcn.com/components#kanban-board]])
  - Status Dots ([[https://thegridcn.com/components#status-dot]])
  - Tags ([[https://thegridcn.com/components#tag]])
  - Toast ([[https://thegridcn.com/components#toast]]) (Can be used as sidebar item ?)
  - Accordion ([[https://thegridcn.com/components#tron-accordion]]) (worspace and columns container)
  - Command Menu (Palette) [[https://thegridcn.com/components#command-menu]]

---

## 12. Keybindings & the Action Registry (DEFERRED — design locked)

**Status:** design agreed, implementation **deferred** until the `heca` app actually
consumes the widgets (so the port is designed against a real adapter, not an
imaginary one). The kbd-hint chips in the showcase (`CMD P`, `CMD ,`) are
**decorative** until this lands.

**Scope:** this is **not an `Item`-only feature**. It must cover **every actionable
widget** — `Button`, `Checkbox`, `Toggle`, `Select`/dropdown, `Item` (menu/sidebar
rows), `Tabs`, and any future actionable component (Command Menu, Modal buttons,
Search Input submit, etc.).

**Goal:** keybindings are real and configurable through `heca`'s `config.toml`,
routed through the app's action registry, **without coupling `heca-grid-ui` to that
registry**.

### Locked decisions

- **Dependency direction stays one-way:** `heca` → `heca-renderer` → `heca-grid-ui`
  → `heca-core`. `WmAction`, `ActionRegistry`, `KeymapRegistry`, and config.toml
  parsing all live in the top `heca` crate. `heca-grid-ui` never reaches *up* to it.
- **Dependency inversion via an `ActionSink` port** — trait defined in
  `heca-grid-ui`, implemented in `heca`. Widgets speak in **opaque string action
  IDs**, never `WmAction`:

  ```rust
  // heca-grid-ui — generic, no WmAction, no config
  pub trait ActionSink {
      fn dispatch(&self, action: &str);                  // opaque id → effect
      fn shortcut_hint(&self, action: &str) -> Option<String>; // for the kbd chip
  }
  ```
  ```rust
  // heca — the adapter over the real registries
  impl ActionSink for HecaActions {
      fn dispatch(&self, id: &str) { /* id → WmAction → ActionRegistry::execute */ }
      fn shortcut_hint(&self, id: &str) -> Option<String> { /* keymap reverse-lookup */ }
  }
  ```
- **Every actionable widget gains an optional `action: &'static str`** (the opaque
  id) alongside its existing `on_*` closure. The closure stays the simple path; the
  `action` id is the registry path. A widget with an `action` set routes through the
  shared `ActionSink`.
- **grid-ui does NOT match accelerators.** Flow: raw key → `heca` resolves
  `KeyCombo → WmAction` via the keymap → `ActionRegistry::execute` → mutates a signal
  → grid-ui re-renders. The app intercepts e.g. Cmd+P **globally**, before focus
  routing. grid-ui's only shortcut responsibility is **displaying** the hint.
- **The kbd-hint chip text is reverse-looked-up from the keymap** (`shortcut_hint`),
  so `config.toml` is the single source of truth — rebind in config and the chip
  updates, no duplicated strings in widget code.

### When we implement it

1. Add the `ActionSink` trait + an `action(&str)` builder to each actionable widget
   in `heca-grid-ui` (Button, Checkbox, Toggle, Select, Item, Tabs, …).
2. Thread a shared `&dyn ActionSink` (likely `Rc<dyn ActionSink>`) to widgets, or hand
   it in at paint/event time via the host.
3. In `heca`, implement `ActionSink` over `KeymapRegistry` + `ActionRegistry`; wire
   global accelerator dispatch in the input loop.

> Note: `heca-ui` is deprecated and being removed — all of this lands in
> **`heca-grid-ui`**.

